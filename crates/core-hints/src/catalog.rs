//! Command catalogs — bundled at compile time, optionally overridden at
//! runtime from a user directory.
//!
//! Drop a `<platform>.json` file (e.g. `cisco_ios.json`, `fortigate.json`)
//! into the directory passed to [`init`] and it replaces the bundled
//! catalog for that platform on the next [`reload`]. Unknown filenames are
//! silently ignored. Each file must be a JSON array of strings.

use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, OnceLock, RwLock};

use crate::{Error, Platform};

const LINUX_JSON: &str = include_str!("../data/linux.json");
const CISCO_JSON: &str = include_str!("../data/cisco_ios.json");
const CISCO_IOSXR_JSON: &str = include_str!("../data/cisco_iosxr.json");
const ARISTA_EOS_JSON: &str = include_str!("../data/arista_eos.json");
const WINDOWS_JSON: &str = include_str!("../data/windows.json");
const JUNIPER_JSON: &str = include_str!("../data/juniper.json");
const MIKROTIK_JSON: &str = include_str!("../data/mikrotik.json");
const ARUBA_JSON: &str = include_str!("../data/aruba.json");
const FORTIGATE_JSON: &str = include_str!("../data/fortigate.json");
const FORTIMANAGER_JSON: &str = include_str!("../data/fortimanager.json");
const FORTIANALYZER_JSON: &str = include_str!("../data/fortianalyzer.json");
const PALOALTO_JSON: &str = include_str!("../data/paloalto.json");
const F5_BIGIP_JSON: &str = include_str!("../data/f5_bigip.json");
const CHECKPOINT_GAIA_JSON: &str = include_str!("../data/checkpoint_gaia.json");
const HP_COMWARE_JSON: &str = include_str!("../data/hp_comware.json");
const BROCADE_FOS_JSON: &str = include_str!("../data/brocade_fos.json");

struct Registry {
    /// Directory the user can drop `<platform>.json` overrides into. `None`
    /// when [`init`] hasn't been called (e.g. tests).
    user_dir: Option<PathBuf>,
    /// Active catalog per platform. Built from bundled JSON, then mutated in
    /// place when [`reload`] sees a matching file in `user_dir`.
    catalogs: HashMap<Platform, Arc<Vec<String>>>,
}

fn registry() -> &'static RwLock<Registry> {
    static R: OnceLock<RwLock<Registry>> = OnceLock::new();
    R.get_or_init(|| RwLock::new(default_registry(None)))
}

fn default_registry(user_dir: Option<PathBuf>) -> Registry {
    let mut catalogs: HashMap<Platform, Arc<Vec<String>>> = HashMap::new();
    catalogs.insert(Platform::Linux, parse_bundled(LINUX_JSON));
    catalogs.insert(Platform::CiscoIos, parse_bundled(CISCO_JSON));
    catalogs.insert(Platform::CiscoIosXr, parse_bundled(CISCO_IOSXR_JSON));
    catalogs.insert(Platform::AristaEos, parse_bundled(ARISTA_EOS_JSON));
    catalogs.insert(Platform::Windows, parse_bundled(WINDOWS_JSON));
    catalogs.insert(Platform::Juniper, parse_bundled(JUNIPER_JSON));
    catalogs.insert(Platform::Mikrotik, parse_bundled(MIKROTIK_JSON));
    catalogs.insert(Platform::Aruba, parse_bundled(ARUBA_JSON));
    catalogs.insert(Platform::Fortigate, parse_bundled(FORTIGATE_JSON));
    catalogs.insert(Platform::Fortimanager, parse_bundled(FORTIMANAGER_JSON));
    catalogs.insert(Platform::Fortianalyzer, parse_bundled(FORTIANALYZER_JSON));
    catalogs.insert(Platform::PaloAlto, parse_bundled(PALOALTO_JSON));
    catalogs.insert(Platform::F5Bigip, parse_bundled(F5_BIGIP_JSON));
    catalogs.insert(
        Platform::CheckpointGaia,
        parse_bundled(CHECKPOINT_GAIA_JSON),
    );
    catalogs.insert(Platform::HpComware, parse_bundled(HP_COMWARE_JSON));
    catalogs.insert(Platform::BrocadeFos, parse_bundled(BROCADE_FOS_JSON));
    Registry { user_dir, catalogs }
}

fn parse_bundled(json: &'static str) -> Arc<Vec<String>> {
    Arc::new(serde_json::from_str(json).expect("bundled catalog must be valid JSON"))
}

/// Initialise the registry with a user-override directory. The directory
/// itself does not need to exist — it will simply be a no-op for [`reload`].
/// Safe to call multiple times; each call resets to the bundled defaults
/// and rescans.
pub fn init(user_dir: Option<PathBuf>) {
    let mut reg = registry().write().expect("catalog registry poisoned");
    *reg = default_registry(user_dir);
    // Best-effort: parse errors are not fatal — log if you wire one in. The
    // public [`reload`] surface returns them; init swallows them on purpose
    // so a single bad file doesn't prevent the app from booting.
    let _ = scan_locked(&mut reg);
}

/// Re-scan the user-override directory and apply matching JSON files.
/// Returns the number of platforms whose catalog was replaced.
pub fn reload() -> Result<usize, Error> {
    let mut reg = registry().write().expect("catalog registry poisoned");
    scan_locked(&mut reg)
}

fn scan_locked(reg: &mut Registry) -> Result<usize, Error> {
    let Some(dir) = reg.user_dir.clone() else {
        return Ok(0);
    };
    if !dir.exists() {
        return Ok(0);
    }
    let entries = std::fs::read_dir(&dir).map_err(|e| Error::Io(e.to_string()))?;
    let mut applied = 0;
    for entry in entries {
        let entry = entry.map_err(|e| Error::Io(e.to_string()))?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let platform = match Platform::from_str(&stem) {
            Ok(Platform::Generic) | Err(_) => continue,
            Ok(p) => p,
        };
        let raw = std::fs::read_to_string(&path).map_err(|e| Error::Io(e.to_string()))?;
        let items: Vec<String> =
            serde_json::from_str(&raw).map_err(|e| Error::InvalidUserCatalog {
                file: stem,
                reason: e.to_string(),
            })?;
        reg.catalogs.insert(platform, Arc::new(items));
        applied += 1;
    }
    Ok(applied)
}

/// Return the active catalog for `platform` (bundled or user-overridden).
/// `Generic` is intentionally empty.
pub fn for_platform(platform: Platform) -> Arc<Vec<String>> {
    if matches!(platform, Platform::Generic) {
        return Arc::new(Vec::new());
    }
    registry()
        .read()
        .expect("catalog registry poisoned")
        .catalogs
        .get(&platform)
        .cloned()
        .unwrap_or_else(|| Arc::new(Vec::new()))
}

/// Where the user can drop `<platform>.json` overrides, if [`init`] was
/// called with a directory.
pub fn user_dir() -> Option<PathBuf> {
    registry().read().ok().and_then(|r| r.user_dir.clone())
}

/// The exact path a user would write to override `platform`. Returns `None`
/// when no user dir is configured.
pub fn user_catalog_path(platform: Platform) -> Option<PathBuf> {
    user_dir().map(|d| d.join(format!("{}.json", platform.as_str())))
}

pub fn all_platforms() -> &'static [Platform] {
    &[
        Platform::Generic,
        Platform::Linux,
        Platform::CiscoIos,
        Platform::CiscoIosXr,
        Platform::AristaEos,
        Platform::Windows,
        Platform::Juniper,
        Platform::Mikrotik,
        Platform::Aruba,
        Platform::Fortigate,
        Platform::Fortimanager,
        Platform::Fortianalyzer,
        Platform::PaloAlto,
        Platform::F5Bigip,
        Platform::CheckpointGaia,
        Platform::HpComware,
        Platform::BrocadeFos,
    ]
}
