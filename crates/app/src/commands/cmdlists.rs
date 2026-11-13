//! User copies of the command lists shown in the bottom bar.
//!
//! The built-in lists ship inside the frontend as JSON. A user copy lives in
//! `<data dir>/cmdlists/<platform>.json`; when it exists the app uses it
//! instead of the built-in list, and updates never touch it. The frontend
//! validates the content, so this module only moves files around.

use std::path::{Path, PathBuf};

use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

/// Folder name under the data dir.
const DIR: &str = "cmdlists";

/// Written into the folder the first time it's opened.
const README: &str = r##"# ZapTerm · command lists

Each `<platform>.json` here is YOUR copy of the command lists ZapTerm shows
under the terminal for that platform (`fortigate.json`, `cisco_ios.json`,
`linux.json`…). When a copy exists ZapTerm uses it instead of the built-in
list, and updates never change it. Delete it (or use "Use built-in" in
Settings → Command Lists) to go back to the built-in list.

Make a copy from Settings → Command Lists → "Make my copy", edit it with any
text editor, then press "Reload". A file with an error is ignored (the
built-in list is used) and Settings shows what's wrong.

## Format

    {
      "format": "zapterm-command-lists",
      "version": 1,
      "platform": "fortigate",            // must match the file name
      "label": "FortiGate · FortiOS 7.6", // shown next to the lists
      "notes": "…",                       // optional, free text
      "stopDebug": ["diagnose debug disable"], // sent by "Stop debug"
      "stopKeys": "",               // optional keys sent first (Ctrl+C)
      "lists": [
        {
          "id": "system", "name": "System", "icon": "system",
          "color": "#5B4FC4", "summary": "One line under the name",
          "debug": false,                 // optional: shows "Stop debug"
          "sections": [
            { "title": "Status", "items": [
              { "kind": "command", "cmd": "get system status", "desc": "Version, serial, uptime" },
              { "kind": "command", "cmd": "execute ping <ip>", "desc": "<ip> is filled in by you" },
              { "kind": "command", "cmd": "diagnose sys session clear", "desc": "…", "action": true },
              { "kind": "recipe", "name": "Debug flow of an IP", "desc": "…",
                "fields": [ { "key": "ip", "label": "IP address", "default": "" } ],
                "lines": [ "diagnose debug flow filter addr {{ip}}", "diagnose debug enable" ] }
            ] }
          ]
        }
      ]
    }

- `<name>` in a command is a part you fill in before it runs.
- `"action": true` asks before sending (commands that change things).
- A recipe sends its `lines` in order after a small form fills `{{key}}`.
- Icons: system, interfaces, routing, sdwan, bgp, ospf, vpn, ha, security,
  users, fortilink, logs, debug (anything else shows the system icon).
"##;

/// A user list file as read from disk (validated by the frontend).
#[derive(serde::Serialize)]
pub struct UserListFile {
    pub platform: String,
    pub json: String,
}

fn user_dir(state: &AppState) -> PathBuf {
    state.data_dir.dir.join(DIR)
}

/// Platform keys are file names: lowercase letters, digits and `_` only, so a
/// key can never point outside the folder.
fn valid_platform(platform: &str) -> bool {
    !platform.is_empty()
        && platform.len() <= 64
        && platform
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn file_for(dir: &Path, platform: &str) -> Result<PathBuf, AppError> {
    if !valid_platform(platform) {
        return Err(AppError::Internal(format!(
            "invalid platform name {platform:?}"
        )));
    }
    Ok(dir.join(format!("{platform}.json")))
}

/// Every `<platform>.json` in the user folder, sorted by platform.
pub(crate) fn read_dir_lists(dir: &Path) -> Vec<UserListFile> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<UserListFile> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let platform = path
                .file_name()?
                .to_str()?
                .strip_suffix(".json")?
                .to_string();
            if !valid_platform(&platform) {
                return None;
            }
            let json = std::fs::read_to_string(&path).ok()?;
            Some(UserListFile { platform, json })
        })
        .collect();
    out.sort_by(|a, b| a.platform.cmp(&b.platform));
    out
}

/// Write `json` as the user copy for `platform` (atomically: temp + rename).
pub(crate) fn write_list(dir: &Path, platform: &str, json: &str) -> Result<(), AppError> {
    let path = file_for(dir, platform)?;
    std::fs::create_dir_all(dir).map_err(|e| AppError::Internal(e.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| AppError::Internal(e.to_string()))?;
    std::fs::rename(&tmp, &path).map_err(|e| AppError::Internal(e.to_string()))
}

/// The user's list files (raw JSON; the frontend validates them).
#[tauri::command]
pub async fn read_user_cmdlists(state: State<'_, AppState>) -> Result<Vec<UserListFile>, AppError> {
    Ok(read_dir_lists(&user_dir(&state)))
}

/// Save a user copy of a platform's lists.
#[tauri::command]
pub async fn write_user_cmdlist(
    state: State<'_, AppState>,
    platform: String,
    json: String,
) -> Result<(), AppError> {
    write_list(&user_dir(&state), &platform, &json)
}

/// Remove the user copy, so the built-in list is used again.
#[tauri::command]
pub async fn delete_user_cmdlist(
    state: State<'_, AppState>,
    platform: String,
) -> Result<(), AppError> {
    let path = file_for(&user_dir(&state), &platform)?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(AppError::Internal(e.to_string())),
    }
}

/// Create the user folder if needed, reveal it in the file manager and
/// return its path.
#[tauri::command]
pub async fn open_cmdlists_dir(state: State<'_, AppState>) -> Result<String, AppError> {
    let dir = user_dir(&state);
    std::fs::create_dir_all(&dir).map_err(|e| AppError::Internal(e.to_string()))?;
    let readme = dir.join("README.md");
    if !readme.exists() {
        let _ = std::fs::write(&readme, README);
    }
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    if let Err(e) = std::process::Command::new(opener).arg(&dir).spawn() {
        tracing::warn!("open_cmdlists_dir spawn failed: {e}");
    }
    Ok(dir.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::{read_dir_lists, valid_platform, write_list};

    #[test]
    fn platform_names_cannot_escape_the_folder() {
        assert!(valid_platform("cisco_ios"));
        assert!(valid_platform("macos"));
        for bad in ["", "../x", "a/b", "A", "x.json", "a b", "..", "c:\\x"] {
            assert!(!valid_platform(bad), "{bad:?}");
        }
    }

    #[test]
    fn write_then_read_back() {
        let dir = std::env::temp_dir().join(format!("zapterm-cmdlists-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write_list(&dir, "linux", "{\"lists\":[]}").unwrap();
        write_list(&dir, "fortigate", "{}").unwrap();
        std::fs::write(dir.join("README.md"), "x").unwrap();
        std::fs::write(dir.join("Bad Name.json"), "{}").unwrap();
        let got = read_dir_lists(&dir);
        let names: Vec<&str> = got.iter().map(|f| f.platform.as_str()).collect();
        assert_eq!(names, ["fortigate", "linux"]);
        assert_eq!(got[1].json, "{\"lists\":[]}");
        assert!(write_list(&dir, "../evil", "{}").is_err());
        assert!(!dir.join("linux.json.tmp").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
