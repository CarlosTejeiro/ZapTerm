use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem as _};

use crate::error::Error;

/// A local pseudo-terminal connected to the system shell.
///
/// Created via [`LocalPty::spawn`]. Dropping it tears down the PTY and the child process.
pub struct LocalPty {
    master: Box<dyn portable_pty::MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send>,
    /// Shared writer so it can be held by the session while reading runs in a task.
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    cols: u16,
    rows: u16,
    /// The shell that was started (a path or a command name).
    shell: String,
    /// Shells tried before it that failed to start, as "shell: error".
    failed: Vec<String>,
}

impl LocalPty {
    /// Spawn a PTY with the system shell at the given initial dimensions.
    pub fn spawn(cols: u16, rows: u16) -> Result<Self, Error> {
        Self::spawn_with(cols, rows, None)
    }

    /// Spawn one program with arguments in a PTY (e.g. `wsl.exe
    /// --distribution Ubuntu`). No fallback: if it can't start, that's the
    /// error.
    pub fn spawn_program(
        cols: u16,
        rows: u16,
        program: &str,
        args: &[String],
    ) -> Result<Self, Error> {
        let pair = NativePtySystem::default()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| Error::SpawnFailed(e.to_string()))?;
        let mut cmd = shell_command(program);
        cmd.args(args);
        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| Error::SpawnFailed(format!("{program}: {e}")))?;
        let portable_pty::PtyPair { master, slave } = pair;
        drop(slave);
        let writer = master
            .take_writer()
            .map_err(|e| Error::SpawnFailed(e.to_string()))?;
        Ok(Self {
            master,
            child,
            writer: Arc::new(Mutex::new(writer)),
            cols,
            rows,
            shell: program.to_string(),
            failed: Vec::new(),
        })
    }

    /// Like [`Self::spawn`], trying `preferred` first: `"pwsh"`,
    /// `"powershell"`, `"cmd"` (Windows) or any shell path/command. When it
    /// can't start, the automatic choice is used instead of failing.
    pub fn spawn_with(cols: u16, rows: u16, preferred: Option<&str>) -> Result<Self, Error> {
        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| Error::SpawnFailed(e.to_string()))?;

        // Try each candidate shell in turn: a locked-down machine may refuse
        // to start one (e.g. a corporate policy blocking powershell.exe from
        // other programs: "Access is denied") while another works.
        let mut failures = Vec::new();
        let mut spawned = None;
        let mut candidates: Vec<String> = preferred
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(resolve_preferred)
            .into_iter()
            .collect();
        for auto in shell_candidates() {
            if !candidates.contains(&auto) {
                candidates.push(auto);
            }
        }
        for shell in candidates {
            match pair.slave.spawn_command(shell_command(&shell)) {
                Ok(child) => {
                    spawned = Some((shell, child));
                    break;
                }
                Err(e) => failures.push(format!("{shell}: {e}")),
            }
        }
        let Some((shell, child)) = spawned else {
            return Err(Error::SpawnFailed(failures.join("; ")));
        };
        // Close our copy of the slave end: only the child may hold it, so that
        // reads on the master end (EOF / EIO) when the shell exits. Keeping it
        // would leave the reader blocked forever after `exit`.
        let portable_pty::PtyPair { master, slave } = pair;
        drop(slave);

        let writer = master
            .take_writer()
            .map_err(|e| Error::SpawnFailed(e.to_string()))?;

        Ok(Self {
            master,
            child,
            writer: Arc::new(Mutex::new(writer)),
            cols,
            rows,
            shell,
            failed: failures,
        })
    }

    /// The shell that was started (a path or a command name).
    pub fn shell(&self) -> &str {
        &self.shell
    }

    /// Shells tried before [`Self::shell`] that failed to start, each as
    /// "shell: error". Empty when the first choice started.
    pub fn failed_shells(&self) -> &[String] {
        &self.failed
    }

    /// Clone the reader end of the PTY.
    ///
    /// The returned reader is blocking; run it inside `tokio::task::spawn_blocking`.
    pub fn take_reader(&self) -> Result<Box<dyn Read + Send>, Error> {
        self.master
            .try_clone_reader()
            .map_err(|e| Error::SpawnFailed(e.to_string()))
    }

    /// A cloneable handle for writing bytes into the PTY (keyboard input).
    pub fn writer(&self) -> Arc<Mutex<Box<dyn Write + Send>>> {
        Arc::clone(&self.writer)
    }

    /// Resize the PTY window and update stored dimensions.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), Error> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| Error::ResizeFailed(e.to_string()))?;
        self.cols = cols;
        self.rows = rows;
        Ok(())
    }

    /// Returns true if the child process has exited.
    pub fn is_done(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)))
    }

    /// A handle that can kill the child from another thread (the reader loop
    /// owns the `LocalPty` itself while it blocks on reads).
    pub fn killer(&self) -> Box<dyn portable_pty::ChildKiller + Send + Sync> {
        self.child.clone_killer()
    }

    /// Kill the child process. Idempotent.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
    }
}

/// The shells to try, best first. Windows: PowerShell 7 (pwsh), then Windows
/// PowerShell, then the command interpreter (COMSPEC, normally cmd.exe), so
/// a machine that blocks PowerShell still gets a terminal.
fn shell_candidates() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let mut shells: Vec<String> = ["pwsh.exe", "powershell.exe"]
            .iter()
            .filter_map(|name| first_on_path(&path, &[name]))
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        shells.push(std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into()));
        shells
    }
    #[cfg(not(target_os = "windows"))]
    {
        vec![std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into())]
    }
}

/// A preset name to the shell it means; anything else is used as given (a
/// path or a command on PATH).
fn resolve_preferred(preferred: &str) -> String {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let on_path = |exe: &str| {
        first_on_path(&path, &[exe])
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| exe.to_string())
    };
    match preferred {
        "pwsh" => on_path("pwsh.exe"),
        "powershell" => on_path("powershell.exe"),
        "cmd" => std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into()),
        other => other.to_string(),
    }
}

fn shell_command(shell: &str) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(shell);
    if let Some(home) = home::home_dir() {
        cmd.cwd(home);
    }
    // Force TERM so shells launched from the macOS .app bundle (which
    // inherits a sparse env from LaunchServices) still load the right
    // terminfo. Without this, zsh-syntax-highlighting + auto-suggestions
    // repaint the input line without proper cursor positioning and the
    // user sees doubled characters (typed `ls` shows as `lls`).
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    // Self-identification so user prompts / scripts can detect the host.
    cmd.env("TERM_PROGRAM", "zapterm");
    cmd.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
    cmd
}

/// The first of `names` found in a directory of `path` (a PATH-style list),
/// trying each name in order across every directory.
fn first_on_path(path: &std::ffi::OsStr, names: &[&str]) -> Option<std::path::PathBuf> {
    let dirs: Vec<std::path::PathBuf> = std::env::split_paths(path).collect();
    names.iter().find_map(|name| {
        dirs.iter()
            .map(|d| d.join(name))
            .find(|candidate| candidate.is_file())
    })
}

#[cfg(test)]
mod tests {
    use super::{first_on_path, resolve_preferred};

    #[test]
    fn custom_shells_are_used_as_given() {
        assert_eq!(resolve_preferred("/bin/zsh"), "/bin/zsh");
        assert_eq!(
            resolve_preferred("C:\\Program Files\\Git\\bin\\bash.exe"),
            "C:\\Program Files\\Git\\bin\\bash.exe"
        );
    }

    #[test]
    fn prefers_the_first_name_over_directory_order() {
        let base = std::env::temp_dir().join(format!("zapterm_path_{}", std::process::id()));
        let (a, b) = (base.join("a"), base.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(a.join("powershell.exe"), b"").unwrap();
        std::fs::write(b.join("pwsh.exe"), b"").unwrap();
        let path = std::env::join_paths([&a, &b]).unwrap();
        // pwsh wins although its directory comes later in PATH.
        assert_eq!(
            first_on_path(&path, &["pwsh.exe", "powershell.exe"]),
            Some(b.join("pwsh.exe"))
        );
        assert_eq!(first_on_path(&path, &["nothing.exe"]), None);
        let _ = std::fs::remove_dir_all(&base);
    }
}
