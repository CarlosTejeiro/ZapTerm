//! Help → Check for updates.
//!
//! Nothing here runs on its own: every request is started by the user from
//! the dialog. The check asks GitHub's public API for the list of releases;
//! downloading fetches one installer from the release's own download link,
//! checked against the SHA-256 digest GitHub publishes for it.
//!
//! Which releases count depends on the build: a stable build only looks at
//! stable releases, a beta build (built from a `vX.Y.Z-beta` tag, see
//! `release.yml`) also at betas. Which file to download depends on how this
//! copy was installed (setup EXE, MSI, Microsoft Store, portable EXE, DMG,
//! DEB, RPM, AppImage).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};

use crate::error::AppError;

const RELEASES_API: &str =
    "https://api.github.com/repos/CarlosTejeiro/ZapTerm/releases?per_page=30";
const RELEASES_PAGE: &str = "https://github.com/CarlosTejeiro/ZapTerm/releases";
/// Every download link must start with this: the project's own releases.
const DOWNLOAD_PREFIX: &str = "https://github.com/CarlosTejeiro/ZapTerm/releases/download/";

fn internal(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(e.to_string())
}

// ---------------------------------------------------------------------------
// Versions and releases
// ---------------------------------------------------------------------------

/// `v1.7.15`, `1.7.15` or `v1.8.3-beta` → (1, 7, 15) / (1, 8, 3).
pub(crate) fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim().trim_start_matches('v');
    let core = s.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let v = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    parts.next().is_none().then_some(v)
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GhAsset {
    pub name: String,
    pub size: u64,
    #[serde(default)]
    pub digest: Option<String>,
    pub browser_download_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GhRelease {
    pub tag_name: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    pub html_url: String,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub assets: Vec<GhAsset>,
}

/// The newest release this build should offer. Drafts never count; betas
/// only for a beta build. On equal versions a stable release wins.
pub(crate) fn pick_latest(releases: &[GhRelease], include_betas: bool) -> Option<&GhRelease> {
    releases
        .iter()
        .filter(|r| !r.draft && (include_betas || !r.prerelease))
        .filter_map(|r| parse_version(&r.tag_name).map(|v| (v, !r.prerelease, r)))
        .max_by_key(|(v, stable, _)| (*v, *stable))
        .map(|(_, _, r)| r)
}

/// A beta build is made from a `-beta` tag; `release.yml` sets
/// `ZAPTERM_CHANNEL=beta` for it.
fn is_beta_build() -> bool {
    option_env!("ZAPTERM_CHANNEL") == Some("beta")
}

// ---------------------------------------------------------------------------
// How this copy was installed, and which file updates it
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum InstallKind {
    /// Windows setup EXE (NSIS).
    Nsis,
    Msi,
    /// Microsoft Store (MSIX): the Store updates it.
    Store,
    /// Windows portable EXE.
    Portable,
    Dmg,
    Deb,
    Rpm,
    AppImage,
    /// Built from source or packaged by someone else.
    Other,
}

fn install_kind() -> InstallKind {
    use tauri::utils::config::BundleType;
    use tauri::utils::platform::bundle_type;
    if cfg!(target_os = "windows") {
        let in_store = std::env::current_exe()
            .map(|p| {
                p.to_string_lossy()
                    .to_ascii_lowercase()
                    .contains("\\windowsapps\\")
            })
            .unwrap_or(false);
        return match bundle_type() {
            _ if in_store => InstallKind::Store,
            Some(BundleType::Nsis) => InstallKind::Nsis,
            Some(BundleType::Msi) => InstallKind::Msi,
            _ => InstallKind::Portable,
        };
    }
    if cfg!(target_os = "macos") {
        return InstallKind::Dmg;
    }
    if std::env::var_os("APPIMAGE").is_some() {
        return InstallKind::AppImage;
    }
    match bundle_type() {
        Some(BundleType::Deb) => InstallKind::Deb,
        Some(BundleType::Rpm) => InstallKind::Rpm,
        Some(BundleType::AppImage) => InstallKind::AppImage,
        _ => InstallKind::Other,
    }
}

/// The release files that install `version` for this kind of install on
/// this CPU, named as `release.yml` names them. The portable EXE is named
/// after the tag, so a beta's is `ZapTerm_1.8.3-beta_x64_portable.exe`.
pub(crate) fn asset_names(kind: InstallKind, arch: &str, version: &str, tag: &str) -> Vec<String> {
    let mut names: Vec<String> = asset_name(kind, arch, version).into_iter().collect();
    if kind == InstallKind::Portable {
        let tag_version = tag.trim_start_matches('v');
        if tag_version != version {
            names.push(format!("ZapTerm_{tag_version}_x64_portable.exe"));
        }
    }
    names
}

fn asset_name(kind: InstallKind, arch: &str, version: &str) -> Option<String> {
    let arm = arch == "aarch64";
    Some(match kind {
        InstallKind::Nsis => format!("ZapTerm_{version}_x64-setup.exe"),
        InstallKind::Msi => format!("ZapTerm_{version}_x64_en-US.msi"),
        InstallKind::Portable => format!("ZapTerm_{version}_x64_portable.exe"),
        InstallKind::Dmg => format!(
            "ZapTerm_{version}_{}.dmg",
            if arm { "aarch64" } else { "x64" }
        ),
        InstallKind::Deb => format!(
            "ZapTerm_{version}_{}.deb",
            if arm { "arm64" } else { "amd64" }
        ),
        InstallKind::Rpm => format!(
            "ZapTerm-{version}-1.{}.rpm",
            if arm { "aarch64" } else { "x86_64" }
        ),
        InstallKind::AppImage => {
            format!(
                "ZapTerm_{version}_{}.AppImage",
                if arm { "aarch64" } else { "amd64" }
            )
        }
        InstallKind::Store | InstallKind::Other => return None,
    })
}

// ---------------------------------------------------------------------------
// The check
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct UpdateAsset {
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub current: String,
    /// `None` when GitHub lists no release this build can use.
    pub latest: Option<String>,
    pub newer: bool,
    pub beta_channel: bool,
    pub prerelease: bool,
    pub tag: Option<String>,
    pub release_url: String,
    pub published_at: Option<String>,
    /// The release notes (Markdown), shortened.
    pub notes: Option<String>,
    pub install_kind: InstallKind,
    /// The file "Download" would fetch; `None` when this install is updated
    /// some other way (Store, source build) or the release lacks the file.
    pub asset: Option<UpdateAsset>,
}

/// What a download may fetch: only what the last check offered.
#[derive(Debug, Clone)]
struct Offer {
    url: String,
    name: String,
    size: u64,
    sha256: Option<String>,
}

static OFFER: Mutex<Option<Offer>> = Mutex::new(None);
static DOWNLOADED: Mutex<Option<(PathBuf, InstallKind)>> = Mutex::new(None);

fn agent(timeout: Duration) -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig};
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        // A plain name: nothing about the user or the machine.
        .user_agent("ZapTerm")
        .tls_config(
            TlsConfig::builder()
                // The system's trust store, so a company proxy that
                // inspects TLS with its own CA keeps working.
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .into()
}

/// The release notes without the headings the dialog already shows
/// ("## Changelog", "## [1.7.15] - date"), shortened.
fn shorten_notes(body: &str) -> String {
    const MAX: usize = 1500;
    let kept: Vec<&str> = body
        .lines()
        .filter(|l| {
            let t = l.trim();
            !(t.eq_ignore_ascii_case("## changelog") || t.starts_with("## ["))
        })
        .collect();
    let joined = kept.join("\n");
    let body = joined.trim();
    if body.len() <= MAX {
        return body.to_string();
    }
    let mut cut = MAX;
    while !body.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}…", body[..cut].trim_end())
}

fn build_info(
    current: &str,
    releases: &[GhRelease],
    beta: bool,
    kind: InstallKind,
    arch: &str,
) -> (UpdateInfo, Option<Offer>) {
    let latest = pick_latest(releases, beta);
    let newer = match (
        latest.and_then(|r| parse_version(&r.tag_name)),
        parse_version(current),
    ) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    };
    let version = latest
        .and_then(|r| parse_version(&r.tag_name))
        .map(|(a, b, c)| format!("{a}.{b}.{c}"));
    let offer = match (latest, &version) {
        (Some(r), Some(v)) if newer => {
            let names = asset_names(kind, arch, v, &r.tag_name);
            r.assets
                .iter()
                .find(|a| {
                    names.contains(&a.name) && a.browser_download_url.starts_with(DOWNLOAD_PREFIX)
                })
                .map(|a| Offer {
                    url: a.browser_download_url.clone(),
                    name: a.name.clone(),
                    size: a.size,
                    sha256: a
                        .digest
                        .as_deref()
                        .and_then(|d| d.strip_prefix("sha256:"))
                        .map(|d| d.to_ascii_lowercase()),
                })
        }
        _ => None,
    };
    let info = UpdateInfo {
        current: current.to_string(),
        latest: version,
        newer,
        beta_channel: beta,
        prerelease: latest.map(|r| r.prerelease).unwrap_or(false),
        tag: latest.map(|r| r.tag_name.clone()),
        release_url: latest
            .map(|r| r.html_url.clone())
            .filter(|u| u.starts_with(RELEASES_PAGE))
            .unwrap_or_else(|| RELEASES_PAGE.to_string()),
        published_at: latest.and_then(|r| r.published_at.clone()),
        notes: latest.and_then(|r| r.body.as_deref()).map(shorten_notes),
        install_kind: kind,
        asset: offer.as_ref().map(|o| UpdateAsset {
            name: o.name.clone(),
            size: o.size,
        }),
    };
    (info, offer)
}

/// Ask GitHub which release is the newest one for this build.
#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> Result<UpdateInfo, AppError> {
    let current = app.package_info().version.to_string();
    let releases: Vec<GhRelease> = tokio::task::spawn_blocking(|| {
        let mut resp = agent(Duration::from_secs(20))
            .get(RELEASES_API)
            .header("Accept", "application/vnd.github+json")
            .call()
            .map_err(|e| internal(format!("Could not reach GitHub: {e}")))?;
        let text = resp
            .body_mut()
            .read_to_string()
            .map_err(|e| internal(format!("Could not read GitHub's answer: {e}")))?;
        serde_json::from_str::<Vec<GhRelease>>(&text)
            .map_err(|e| internal(format!("Unexpected answer from GitHub: {e}")))
    })
    .await
    .map_err(internal)??;
    let (info, offer) = build_info(
        &current,
        &releases,
        is_beta_build(),
        install_kind(),
        std::env::consts::ARCH,
    );
    *OFFER.lock().unwrap() = offer;
    *DOWNLOADED.lock().unwrap() = None;
    Ok(info)
}

// ---------------------------------------------------------------------------
// Download
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
struct Progress {
    downloaded: u64,
    total: u64,
}

fn download_to(
    agent: &ureq::Agent,
    offer: &Offer,
    dir: &Path,
    mut progress: impl FnMut(u64),
) -> Result<PathBuf, AppError> {
    let target = dir.join(&offer.name);
    let part = dir.join(format!("{}.part", offer.name));
    let mut resp = agent
        .get(&offer.url)
        .call()
        .map_err(|e| internal(format!("Download failed: {e}")))?;
    let mut reader = resp
        .body_mut()
        .with_config()
        .limit(offer.size.saturating_add(1))
        .reader();
    let mut file = std::fs::File::create(&part).map_err(internal)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut done = 0u64;
    let result = (|| {
        loop {
            let n = reader
                .read(&mut buf)
                .map_err(|e| internal(format!("Download failed: {e}")))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            file.write_all(&buf[..n]).map_err(internal)?;
            done += n as u64;
            progress(done);
        }
        file.flush().map_err(internal)?;
        if done != offer.size {
            return Err(internal(format!(
                "Download incomplete: got {done} of {} bytes",
                offer.size
            )));
        }
        if let Some(want) = &offer.sha256 {
            let got = super::sync::hex_encode(&hasher.finalize());
            if &got != want {
                return Err(internal(
                    "The downloaded file does not match the checksum GitHub publishes for it; it was deleted",
                ));
            }
        }
        Ok(())
    })();
    drop(file);
    if let Err(e) = result {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    std::fs::rename(&part, &target).map_err(internal)?;
    Ok(target)
}

/// Download the installer the last check offered into the Downloads folder.
/// Emits `update-download-progress` while it runs; returns the file's path.
#[tauri::command]
pub async fn download_update(app: AppHandle) -> Result<String, AppError> {
    let offer = OFFER
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| internal("Check for updates first"))?;
    // The Downloads folder; if the system doesn't name one, ~/Downloads
    // when it exists, else the temporary folder (the dialog shows the path).
    let dir = app
        .path()
        .download_dir()
        .ok()
        .or_else(|| {
            app.path()
                .home_dir()
                .ok()
                .map(|h| h.join("Downloads"))
                .filter(|d| d.is_dir())
        })
        .map(Ok)
        .unwrap_or_else(|| app.path().temp_dir())
        .map_err(internal)?;
    std::fs::create_dir_all(&dir).map_err(internal)?;
    let emitter = app.clone();
    let total = offer.size;
    let path = tokio::task::spawn_blocking(move || {
        let mut last = 0u64;
        download_to(&agent(Duration::from_secs(30 * 60)), &offer, &dir, |done| {
            // About every 1 % — plenty for a progress bar.
            if done == total || done - last > total / 100 {
                last = done;
                let _ = emitter.emit(
                    "update-download-progress",
                    Progress {
                        downloaded: done,
                        total,
                    },
                );
            }
        })
    })
    .await
    .map_err(internal)??;
    let kind = install_kind();
    #[cfg(unix)]
    if kind == InstallKind::AppImage {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
    }
    *DOWNLOADED.lock().unwrap() = Some((path.clone(), kind));
    Ok(path.to_string_lossy().into_owned())
}

// ---------------------------------------------------------------------------
// Install
// ---------------------------------------------------------------------------

fn spawn(cmd: &str, args: &[&std::ffi::OsStr]) -> Result<(), AppError> {
    std::process::Command::new(cmd)
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|e| internal(format!("Could not start {cmd}: {e}")))
}

/// Show `path` selected in the file manager.
fn reveal(path: &Path) -> Result<(), AppError> {
    #[cfg(target_os = "windows")]
    {
        let mut arg = std::ffi::OsString::from("/select,");
        arg.push(path.as_os_str());
        spawn("explorer.exe", &[&arg])
    }
    #[cfg(target_os = "macos")]
    {
        spawn("open", &[std::ffi::OsStr::new("-R"), path.as_os_str()])
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let dir = path.parent().unwrap_or(path);
        spawn("xdg-open", &[dir.as_os_str()])
    }
}

/// Start installing the downloaded file. Returns `true` when ZapTerm should
/// close so the installer can replace it (setup EXE, MSI); for the other
/// kinds it opens the file (DMG, DEB, RPM) or shows it in its folder
/// (portable EXE, AppImage).
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<bool, AppError> {
    let (path, kind) = DOWNLOADED
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| internal("Download the update first"))?;
    if !path.is_file() {
        return Err(internal("The downloaded file is gone; download it again"));
    }
    let p = path.as_os_str();
    match kind {
        InstallKind::Nsis => {
            spawn(&path.to_string_lossy(), &[])?;
            exit_soon(app);
            Ok(true)
        }
        InstallKind::Msi => {
            spawn("msiexec", &[std::ffi::OsStr::new("/i"), p])?;
            exit_soon(app);
            Ok(true)
        }
        #[cfg(target_os = "macos")]
        InstallKind::Dmg => spawn("open", &[p]).map(|_| false),
        InstallKind::Deb | InstallKind::Rpm => spawn("xdg-open", &[p]).map(|_| false),
        _ => reveal(&path).map(|_| false),
    }
}

/// Show the downloaded file in its folder.
#[tauri::command]
pub async fn reveal_update() -> Result<(), AppError> {
    let (path, _) = DOWNLOADED
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| internal("Download the update first"))?;
    reveal(&path)
}

/// Close ZapTerm shortly after the installer started, so it can replace the
/// program files.
fn exit_soon(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(800));
        app.exit(0);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(tag: &str, prerelease: bool, draft: bool) -> GhRelease {
        GhRelease {
            tag_name: tag.into(),
            draft,
            prerelease,
            html_url: format!("{RELEASES_PAGE}/tag/{tag}"),
            published_at: None,
            body: Some("notes".into()),
            assets: vec![],
        }
    }

    /// The files of the real v1.7.15 release.
    fn assets_1_7_15() -> Vec<GhAsset> {
        [
            "ZapTerm-1.7.15-1.aarch64.rpm",
            "ZapTerm-1.7.15-1.x86_64.rpm",
            "ZapTerm_1.7.15_aarch64.AppImage",
            "ZapTerm_1.7.15_aarch64.dmg",
            "ZapTerm_1.7.15_amd64.AppImage",
            "ZapTerm_1.7.15_amd64.deb",
            "ZapTerm_1.7.15_arm64.deb",
            "ZapTerm_1.7.15_x64-setup.exe",
            "ZapTerm_1.7.15_x64.dmg",
            "ZapTerm_1.7.15_x64.msix",
            "ZapTerm_1.7.15_x64_en-US.msi",
            "ZapTerm_1.7.15_x64_portable.exe",
            "ZapTerm_aarch64.app.tar.gz",
            "ZapTerm_x64.app.tar.gz",
        ]
        .iter()
        .map(|n| GhAsset {
            name: n.to_string(),
            size: 10,
            digest: Some("sha256:ABCDEF".into()),
            browser_download_url: format!("{DOWNLOAD_PREFIX}v1.7.15/{n}"),
        })
        .collect()
    }

    #[test]
    fn parses_tags_and_versions() {
        assert_eq!(parse_version("v1.7.15"), Some((1, 7, 15)));
        assert_eq!(parse_version("1.8.3"), Some((1, 8, 3)));
        assert_eq!(parse_version("v1.8.3-beta"), Some((1, 8, 3)));
        assert_eq!(parse_version("v1.10.0"), Some((1, 10, 0)));
        assert_eq!(parse_version("nightly"), None);
        assert_eq!(parse_version("v1.7"), None);
        assert_eq!(parse_version("v1.7.1.2"), None);
    }

    #[test]
    fn stable_builds_ignore_betas_and_drafts() {
        let rs = vec![
            rel("v1.8.3-beta", true, false),
            rel("v1.7.16", false, true), // draft
            rel("v1.7.15", false, false),
            rel("v1.7.9", false, false),
            rel("v1.7.10", false, false),
        ];
        assert_eq!(pick_latest(&rs, false).unwrap().tag_name, "v1.7.15");
        assert_eq!(pick_latest(&rs, true).unwrap().tag_name, "v1.8.3-beta");
    }

    #[test]
    fn beta_builds_take_a_newer_stable_and_prefer_stable_on_a_tie() {
        let rs = vec![rel("v1.8.3-beta", true, false), rel("v1.9.0", false, false)];
        assert_eq!(pick_latest(&rs, true).unwrap().tag_name, "v1.9.0");
        let tie = vec![rel("v1.9.0-beta", true, false), rel("v1.9.0", false, false)];
        assert_eq!(pick_latest(&tie, true).unwrap().tag_name, "v1.9.0");
    }

    #[test]
    fn picks_the_right_file_for_each_install() {
        let assets = assets_1_7_15();
        let find = |kind, arch| {
            asset_name(kind, arch, "1.7.15")
                .and_then(|n| assets.iter().find(|a| a.name == n).map(|a| a.name.clone()))
        };
        assert_eq!(
            find(InstallKind::Nsis, "x86_64").unwrap(),
            "ZapTerm_1.7.15_x64-setup.exe"
        );
        assert_eq!(
            find(InstallKind::Msi, "x86_64").unwrap(),
            "ZapTerm_1.7.15_x64_en-US.msi"
        );
        assert_eq!(
            find(InstallKind::Portable, "x86_64").unwrap(),
            "ZapTerm_1.7.15_x64_portable.exe"
        );
        assert_eq!(
            find(InstallKind::Dmg, "aarch64").unwrap(),
            "ZapTerm_1.7.15_aarch64.dmg"
        );
        assert_eq!(
            find(InstallKind::Dmg, "x86_64").unwrap(),
            "ZapTerm_1.7.15_x64.dmg"
        );
        assert_eq!(
            find(InstallKind::Deb, "x86_64").unwrap(),
            "ZapTerm_1.7.15_amd64.deb"
        );
        assert_eq!(
            find(InstallKind::Deb, "aarch64").unwrap(),
            "ZapTerm_1.7.15_arm64.deb"
        );
        assert_eq!(
            find(InstallKind::Rpm, "x86_64").unwrap(),
            "ZapTerm-1.7.15-1.x86_64.rpm"
        );
        assert_eq!(
            find(InstallKind::Rpm, "aarch64").unwrap(),
            "ZapTerm-1.7.15-1.aarch64.rpm"
        );
        assert_eq!(
            find(InstallKind::AppImage, "x86_64").unwrap(),
            "ZapTerm_1.7.15_amd64.AppImage"
        );
        assert_eq!(
            find(InstallKind::AppImage, "aarch64").unwrap(),
            "ZapTerm_1.7.15_aarch64.AppImage"
        );
        assert!(find(InstallKind::Store, "x86_64").is_none());
        assert!(find(InstallKind::Other, "x86_64").is_none());
    }

    #[test]
    fn build_info_offers_a_download_only_when_newer() {
        let mut r = rel("v1.7.15", false, false);
        r.assets = assets_1_7_15();
        let rs = vec![r];
        let (info, offer) = build_info("1.7.14", &rs, false, InstallKind::Nsis, "x86_64");
        assert!(info.newer);
        assert_eq!(info.latest.as_deref(), Some("1.7.15"));
        let offer = offer.unwrap();
        assert_eq!(offer.name, "ZapTerm_1.7.15_x64-setup.exe");
        assert_eq!(offer.sha256.as_deref(), Some("abcdef"));
        assert!(offer.url.starts_with(DOWNLOAD_PREFIX));

        let (info, offer) = build_info("1.7.15", &rs, false, InstallKind::Nsis, "x86_64");
        assert!(!info.newer);
        assert!(offer.is_none() && info.asset.is_none());

        // Store installs are told, not offered a file.
        let (info, offer) = build_info("1.7.14", &rs, false, InstallKind::Store, "x86_64");
        assert!(info.newer && offer.is_none());

        // A beta build ahead of the latest stable sees nothing newer.
        let (info, _) = build_info("1.8.3", &rs, true, InstallKind::Nsis, "x86_64");
        assert!(!info.newer);
    }

    #[test]
    fn beta_portable_exe_is_named_after_the_tag() {
        let mut r = rel("v1.8.3-beta", true, false);
        r.assets = [
            "ZapTerm_1.8.3-beta_x64_portable.exe",
            "ZapTerm_1.8.3_x64-setup.exe",
        ]
        .iter()
        .map(|n| GhAsset {
            name: n.to_string(),
            size: 1,
            digest: None,
            browser_download_url: format!("{DOWNLOAD_PREFIX}v1.8.3-beta/{n}"),
        })
        .collect();
        let rs = [r];
        let (_, offer) = build_info("1.8.2", &rs, true, InstallKind::Portable, "x86_64");
        assert_eq!(offer.unwrap().name, "ZapTerm_1.8.3-beta_x64_portable.exe");
        let (_, offer) = build_info("1.8.2", &rs, true, InstallKind::Nsis, "x86_64");
        assert_eq!(offer.unwrap().name, "ZapTerm_1.8.3_x64-setup.exe");
    }

    #[test]
    fn build_info_refuses_links_outside_the_project() {
        let mut r = rel("v1.7.15", false, false);
        r.assets = assets_1_7_15();
        for a in &mut r.assets {
            a.browser_download_url = format!("https://example.com/{}", a.name);
        }
        let (_, offer) = build_info("1.7.14", &[r], false, InstallKind::Nsis, "x86_64");
        assert!(offer.is_none());
    }

    /// Talks to the real GitHub API: `cargo test -p app live_github -- --ignored`.
    #[test]
    #[ignore]
    fn live_github_check() {
        let mut resp = agent(Duration::from_secs(20))
            .get(RELEASES_API)
            .header("Accept", "application/vnd.github+json")
            .call()
            .unwrap();
        let text = resp.body_mut().read_to_string().unwrap();
        let releases: Vec<GhRelease> = serde_json::from_str(&text).unwrap();
        let (info, offer) = build_info("1.0.0", &releases, false, InstallKind::Nsis, "x86_64");
        println!("{info:?}");
        assert!(info.newer && !info.prerelease);
        let offer = offer.expect("a setup EXE in the latest stable release");
        assert!(offer.url.starts_with(DOWNLOAD_PREFIX));
        assert_eq!(offer.sha256.as_ref().map(|s| s.len()), Some(64));
    }

    #[test]
    fn notes_drop_the_headings_the_dialog_shows() {
        let body = "## Changelog\n\n## [1.7.15] - 2026-11-22\n\n### Security\n- url 2.5.9\n";
        assert_eq!(shorten_notes(body), "### Security\n- url 2.5.9");
        let long = "x".repeat(2000);
        assert!(shorten_notes(&long).ends_with('…'));
        assert!(shorten_notes(&long).len() < 1600);
    }

    /// Serves `body` once over plain HTTP on a local port.
    fn serve_once(body: Vec<u8>) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                let mut req = [0u8; 2048];
                let _ = s.read(&mut req);
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = s.write_all(head.as_bytes());
                let _ = s.write_all(&body);
            }
        });
        format!("http://{addr}/file")
    }

    fn sha(data: &[u8]) -> String {
        super::super::sync::hex_encode(&Sha256::digest(data))
    }

    #[test]
    fn download_checks_size_and_checksum() {
        let dir = std::env::temp_dir().join(format!("zt-update-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let data = b"installer bytes".to_vec();
        let agent = ureq::Agent::new_with_defaults();

        let ok = Offer {
            url: serve_once(data.clone()),
            name: "ok.bin".into(),
            size: data.len() as u64,
            sha256: Some(sha(&data)),
        };
        let mut seen = 0;
        let path = download_to(&agent, &ok, &dir, |d| seen = d).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), data);
        assert_eq!(seen, data.len() as u64);

        let bad_sum = Offer {
            url: serve_once(data.clone()),
            name: "bad.bin".into(),
            size: data.len() as u64,
            sha256: Some(sha(b"something else")),
        };
        let e = download_to(&agent, &bad_sum, &dir, |_| {}).unwrap_err();
        assert!(format!("{e}").contains("checksum"), "{e}");
        assert!(!dir.join("bad.bin").exists() && !dir.join("bad.bin.part").exists());

        let short = Offer {
            url: serve_once(data.clone()),
            name: "short.bin".into(),
            size: data.len() as u64 + 5,
            sha256: None,
        };
        let e = download_to(&agent, &short, &dir, |_| {}).unwrap_err();
        assert!(format!("{e}").contains("incomplete"), "{e}");
        assert!(!dir.join("short.bin").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
