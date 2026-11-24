//! macOS: install a downloaded update without the user dragging anything.
//!
//! "Download and install" used to open the disk image and leave ZapTerm
//! running, so the app in Applications could not be replaced. Now ZapTerm
//! starts [`UPDATER_SCRIPT`] and quits; the script waits for it to be gone,
//! copies the new app out of the disk image next to the old one, swaps them,
//! and opens ZapTerm again. The old app is kept until the new one is in place
//! and put back if anything fails, in which case the disk image is opened so
//! the user can finish by hand.
//!
//! The user's data (sessions, vault, settings, scripts) lives in
//! `~/Library/Application Support/app.zapterm.client`, outside the app
//! bundle: none of this touches it.

use std::path::{Path, PathBuf};

/// `sh updater.sh <pid> <dmg> <app bundle> <log>`.
///
/// Tools are found through `PATH` (set to the system directories when the
/// script is started) so the tests can run it with stand-ins.
pub(crate) const UPDATER_SCRIPT: &str = r#"#!/bin/sh
# ZapTerm updater (macOS). Replaces the app with the one in a disk image
# ZapTerm downloaded and verified, then opens it again.
PID="$1"
DMG="$2"
APP="$3"
LOG="$4"
# This copy of the script is not needed again (sh keeps it open).
rm -f "$0"
exec >>"$LOG" 2>&1
echo "== $(date) updating $APP from $DMG"

MNT=""
OLD="$APP.old-$$"
NEW="$APP.new-$$"

# Leave things as they were, reopen ZapTerm and hand the disk image over.
fail() {
  echo "FAILED: $1"
  rm -rf "$NEW"
  if [ -d "$OLD" ] && [ ! -d "$APP" ]; then mv "$OLD" "$APP"; fi
  if [ -n "$MNT" ]; then hdiutil detach "$MNT" -quiet; rmdir "$MNT" 2>/dev/null; fi
  open "$APP"
  open "$DMG"
  exit 1
}

# 1. Wait for ZapTerm to quit (30 s at most).
i=0
while kill -0 "$PID" 2>/dev/null; do
  i=$((i + 1))
  if [ "$i" -gt 150 ]; then fail "ZapTerm did not quit"; fi
  sleep 0.2
done

# 2. Open the disk image where Finder does not show it.
MNT=$(mktemp -d "${TMPDIR:-/tmp}/zapterm-update.XXXXXX") || { MNT=""; fail "no temporary folder"; }
if ! hdiutil attach "$DMG" -nobrowse -noautoopen -readonly -mountpoint "$MNT" -quiet; then
  rmdir "$MNT" 2>/dev/null
  MNT=""
  fail "could not open the disk image"
fi
SRC=$(find "$MNT" -maxdepth 1 -name '*.app' -type d | head -n 1)
[ -d "$SRC" ] || fail "no app in the disk image"

# 3. Copy the new app next to the old one, then swap them. The old one stays
#    until the new one is in place.
rm -rf "$NEW"
ditto "$SRC" "$NEW" || fail "could not copy the new version"
# The download is ZapTerm's own, checked against the published SHA-256:
# clear the quarantine mark so macOS opens it without the terminal command.
xattr -cr "$NEW" 2>/dev/null
mv "$APP" "$OLD" || fail "could not move the current version (no permission?)"
mv "$NEW" "$APP" || fail "could not put the new version in place"
rm -rf "$OLD"
hdiutil detach "$MNT" -quiet
rmdir "$MNT" 2>/dev/null
MNT=""

# 4. Open the new version.
open "$APP"
echo "done"
"#;

/// The `.app` bundle `exe` runs from, when it can be replaced in place.
/// `Err` says, in words for the user, why it can't.
pub(crate) fn replaceable_bundle(exe: &Path) -> Result<PathBuf, String> {
    // <bundle>.app/Contents/MacOS/<executable>
    let bundle = exe
        .parent()
        .filter(|p| p.ends_with("Contents/MacOS"))
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .filter(|b| b.extension().is_some_and(|e| e == "app"))
        .ok_or("ZapTerm is not running from an app bundle")?;
    let text = bundle.to_string_lossy();
    if text.contains("/AppTranslocation/") {
        // Gatekeeper runs a quarantined app from a read-only copy elsewhere.
        return Err("macOS is running ZapTerm from a temporary copy".into());
    }
    if text.starts_with("/Volumes/") {
        return Err("ZapTerm is running from a disk image, not from Applications".into());
    }
    let folder = bundle.parent().ok_or("ZapTerm's folder was not found")?;
    let probe = folder.join(format!(".zapterm-update-{}", std::process::id()));
    match std::fs::write(&probe, b"") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            Ok(bundle.to_path_buf())
        }
        Err(_) => Err(format!(
            "ZapTerm can't write in {} (the app has to be replaced by hand there)",
            folder.display()
        )),
    }
}

/// Where the updater finds `hdiutil`, `ditto`, `xattr` and `open`.
pub(crate) const SYSTEM_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";

/// Start the updater for `dmg`, to run once the process `pid` has quit.
/// It is detached, so it outlives ZapTerm; its progress goes to `log`.
pub(crate) fn spawn_updater(
    dmg: &Path,
    bundle: &Path,
    pid: u32,
    log: &Path,
    path_env: &str,
) -> std::io::Result<std::process::Child> {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    let script = std::env::temp_dir().join(format!("zapterm-update-{pid}.sh"));
    std::fs::write(&script, UPDATER_SCRIPT)?;
    if let Some(dir) = log.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    Command::new("/bin/sh")
        .arg(&script)
        .arg(pid.to_string())
        .arg(dmg)
        .arg(bundle)
        .arg(log)
        .env("PATH", path_env)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // Its own process group, so nothing aimed at ZapTerm reaches it.
        .process_group(0)
        .spawn()
}

/// Replace the app `exe` runs from with the one in `dmg` as soon as the
/// process `pid` (ZapTerm) has quit, then open it again. `Err` says why that
/// can't be done here; nothing was started then.
pub(crate) fn install(exe: &Path, dmg: &Path, pid: u32, log: &Path) -> Result<(), String> {
    let bundle = replaceable_bundle(exe)?;
    spawn_updater(dmg, &bundle, pid, log, SYSTEM_PATH)
        .map(|_| ())
        .map_err(|e| format!("the updater did not start: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    /// A scratch folder with stand-ins for the macOS tools the script calls,
    /// an installed "old" app, and a disk image holding the "new" one.
    struct Bench {
        dir: PathBuf,
        bin: PathBuf,
        app: PathBuf,
        dmg: PathBuf,
        log: PathBuf,
    }

    fn tool(bin: &Path, name: &str, body: &str) {
        let path = bin.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn bench(name: &str) -> Bench {
        let dir =
            std::env::temp_dir().join(format!("zapterm-macupd-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let bin = dir.join("bin");
        let app = dir.join("Applications/ZapTerm.app");
        let image = dir.join("image/ZapTerm.app/Contents/MacOS");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        std::fs::create_dir_all(&image).unwrap();
        std::fs::write(app.join("Contents/MacOS/zapterm"), "old").unwrap();
        std::fs::write(image.join("zapterm"), "new").unwrap();
        let dmg = dir.join("ZapTerm.dmg");
        std::fs::write(&dmg, "").unwrap();
        // `hdiutil attach … -mountpoint <dir>`: show the image's content there.
        tool(
            &bin,
            "hdiutil",
            &format!(
                r#"if [ "$1" = attach ]; then
  while [ "$1" != "-mountpoint" ]; do shift; done
  cp -R "{}/." "$2/"
else
  rm -rf "$2"/*
fi"#,
                dir.join("image").display()
            ),
        );
        tool(&bin, "ditto", r#"cp -R "$1" "$2""#);
        tool(&bin, "xattr", "exit 0");
        tool(
            &bin,
            "open",
            &format!(r#"echo "$1" >> "{}""#, dir.join("opened").display()),
        );
        Bench {
            log: dir.join("update.log"),
            dir,
            bin,
            app,
            dmg,
        }
    }

    /// Run the updater to the end with the stand-ins first in PATH.
    fn run(b: &Bench) -> bool {
        // A process that has already quit stands for ZapTerm.
        let mut gone = Command::new("true").spawn().unwrap();
        let pid = gone.id();
        gone.wait().unwrap();
        let path = format!("{}:/usr/bin:/bin", b.bin.display());
        spawn_updater(&b.dmg, &b.app, pid, &b.log, &path)
            .unwrap()
            .wait()
            .unwrap()
            .success()
    }

    fn installed(b: &Bench) -> String {
        std::fs::read_to_string(b.app.join("Contents/MacOS/zapterm")).unwrap()
    }

    fn leftovers(b: &Bench) -> Vec<String> {
        std::fs::read_dir(b.app.parent().unwrap())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n != "ZapTerm.app")
            .collect()
    }

    #[test]
    fn replaces_the_app_and_reopens_it() {
        let b = bench("ok");
        assert!(
            run(&b),
            "{}",
            std::fs::read_to_string(&b.log).unwrap_or_default()
        );
        assert_eq!(installed(&b), "new");
        assert!(leftovers(&b).is_empty(), "{:?}", leftovers(&b));
        // Only ZapTerm was opened, not the disk image.
        let opened = std::fs::read_to_string(b.dir.join("opened")).unwrap();
        assert_eq!(opened.trim(), b.app.to_string_lossy());
        assert!(std::fs::read_to_string(&b.log).unwrap().contains("done"));
        let _ = std::fs::remove_dir_all(&b.dir);
    }

    #[test]
    fn nothing_is_touched_until_zapterm_has_quit() {
        let b = bench("wait");
        // Stands for ZapTerm, still running for a moment.
        let mut zapterm = Command::new("sleep").arg("1").spawn().unwrap();
        let path = format!("{}:/usr/bin:/bin", b.bin.display());
        let mut updater = spawn_updater(&b.dmg, &b.app, zapterm.id(), &b.log, &path).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(400));
        assert_eq!(installed(&b), "old");
        assert!(!b.dir.join("opened").exists());
        zapterm.wait().unwrap();
        assert!(updater.wait().unwrap().success());
        assert_eq!(installed(&b), "new");
        let _ = std::fs::remove_dir_all(&b.dir);
    }

    #[test]
    fn a_failed_copy_keeps_the_old_app_and_hands_over_the_disk_image() {
        let b = bench("copyfail");
        tool(&b.bin, "ditto", "exit 1");
        assert!(!run(&b));
        assert_eq!(installed(&b), "old");
        assert!(leftovers(&b).is_empty(), "{:?}", leftovers(&b));
        let opened = std::fs::read_to_string(b.dir.join("opened")).unwrap();
        let opened: Vec<&str> = opened.lines().collect();
        assert_eq!(
            opened,
            [&*b.app.to_string_lossy(), &*b.dmg.to_string_lossy()],
            "the old app is reopened, then the disk image shown"
        );
        let log = std::fs::read_to_string(&b.log).unwrap();
        assert!(
            log.contains("FAILED: could not copy the new version"),
            "{log}"
        );
        let _ = std::fs::remove_dir_all(&b.dir);
    }

    #[test]
    fn a_disk_image_that_does_not_open_changes_nothing() {
        let b = bench("nomount");
        tool(&b.bin, "hdiutil", "exit 1");
        assert!(!run(&b));
        assert_eq!(installed(&b), "old");
        assert!(leftovers(&b).is_empty());
        let _ = std::fs::remove_dir_all(&b.dir);
    }

    #[test]
    fn a_failed_swap_puts_the_old_app_back() {
        let b = bench("swapfail");
        // The copy "succeeds" without producing the new app, so the second
        // move has nothing to move.
        tool(&b.bin, "ditto", "exit 0");
        assert!(!run(&b));
        assert_eq!(installed(&b), "old");
        assert!(leftovers(&b).is_empty(), "{:?}", leftovers(&b));
        let _ = std::fs::remove_dir_all(&b.dir);
    }

    #[test]
    fn only_an_app_bundle_in_a_writable_folder_is_replaced() {
        let b = bench("bundle");
        let exe = b.app.join("Contents/MacOS/zapterm");
        assert_eq!(replaceable_bundle(&exe).unwrap(), b.app);
        // A plain binary, a translocated copy, the disk image itself.
        assert!(replaceable_bundle(Path::new("/usr/local/bin/zapterm")).is_err());
        let moved = Path::new(
            "/private/var/folders/x/T/AppTranslocation/ABC/d/ZapTerm.app/Contents/MacOS/zapterm",
        );
        assert!(replaceable_bundle(moved)
            .unwrap_err()
            .contains("temporary copy"));
        let image = Path::new("/Volumes/ZapTerm/ZapTerm.app/Contents/MacOS/zapterm");
        assert!(replaceable_bundle(image)
            .unwrap_err()
            .contains("disk image"));
        let _ = std::fs::remove_dir_all(&b.dir);
    }
}
