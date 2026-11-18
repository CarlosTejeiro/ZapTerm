//! Reading a device's OS version from its own output.
//!
//! The platform detector says *what* a device is; some command lists also
//! depend on *which release* it runs. FortiOS prints it in
//! `get system status`:
//!
//! ```text
//! Version: FortiGate-60F v7.4.3,build2573,240201 (GA.F)
//! ```
//!
//! [`OsVersionSniffer`] watches the output line by line for that and reports
//! each new version once. It never stops watching: a device upgraded during
//! the session, or a jump to another device, shows a different version.

use std::sync::OnceLock;

use regex::Regex;

use crate::Platform;

/// A version read from a device's output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsVersion {
    /// What the line proves the device is.
    pub platform: Platform,
    /// `major.minor.patch`, e.g. `7.4.3`.
    pub version: String,
}

impl OsVersion {
    /// `major.minor` — what command lists are written for.
    pub fn train(&self) -> String {
        self.version
            .splitn(3, '.')
            .take(2)
            .collect::<Vec<_>>()
            .join(".")
    }
}

/// Longest line worth looking at; anything longer is not a status line.
const MAX_LINE: usize = 256;

#[derive(Default)]
pub struct OsVersionSniffer {
    line: Vec<u8>,
    /// The current line outgrew [`MAX_LINE`]: skip to its end.
    overflow: bool,
    last: Option<OsVersion>,
}

impl OsVersionSniffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed raw output. `Some` when a complete line states a version that is
    /// not the one reported last.
    pub fn feed(&mut self, bytes: &[u8]) -> Option<OsVersion> {
        let mut found = None;
        for &b in bytes {
            if b == b'\n' {
                if !self.overflow {
                    if let Some(v) = parse_line(&String::from_utf8_lossy(&self.line)) {
                        if self.last.as_ref() != Some(&v) {
                            self.last = Some(v.clone());
                            found = Some(v);
                        }
                    }
                }
                self.line.clear();
                self.overflow = false;
            } else if self.overflow {
                // Still inside an over-long line.
            } else if self.line.len() >= MAX_LINE {
                self.line.clear();
                self.overflow = true;
            } else {
                self.line.push(b);
            }
        }
        found
    }
}

/// The version stated by one line of output, if it is a status line.
pub fn parse_line(line: &str) -> Option<OsVersion> {
    static FORTIOS: OnceLock<Regex> = OnceLock::new();
    let re = FORTIOS.get_or_init(|| {
        // The model follows the product name: FortiGate-60F, FortiWiFi-40F,
        // FortiGateRugged-60F, FortiGate-VM64…
        Regex::new(r"^Version:\s*Forti(?:Gate|WiFi)[\w-]*\s+v(\d+)\.(\d+)\.(\d+)\b")
            .expect("valid regex")
    });
    let caps = re.captures(line.trim_end_matches('\r').trim_start())?;
    Some(OsVersion {
        platform: Platform::Fortigate,
        version: format!("{}.{}.{}", &caps[1], &caps[2], &caps[3]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fgt(v: &str) -> Option<OsVersion> {
        Some(OsVersion {
            platform: Platform::Fortigate,
            version: v.into(),
        })
    }

    #[test]
    fn reads_the_fortios_status_line() {
        assert_eq!(
            parse_line("Version: FortiGate-60F v7.4.3,build2573,240201 (GA.F)"),
            fgt("7.4.3")
        );
        assert_eq!(
            parse_line("Version: FortiWiFi-40F v7.2.8,build1639,240313 (GA.M)\r"),
            fgt("7.2.8")
        );
        assert_eq!(
            parse_line("Version: FortiGate-VM64-KVM v7.6.0,build3401,240724 (GA.F)"),
            fgt("7.6.0")
        );
        assert_eq!(
            parse_line("Version: FortiGateRugged-60F v7.0.14,build0601,240206 (GA.M)"),
            fgt("7.0.14")
        );
        assert_eq!(fgt("7.4.3").unwrap().train(), "7.4");
    }

    #[test]
    fn other_lines_say_nothing() {
        for line in [
            "Version: FortiManager-VM64 v7.4.2-build2397 240131 (GA)",
            "Version: FortiAnalyzer-VM64 v7.4.2-build2397 240131 (GA)",
            "Firmware Signature: certified",
            "IPS-ETDB: 6.00741(2015-12-01 02:30)",
            "echo Version: FortiGate-60F v7.4.3",
            "Version: 7.4.3",
            "",
        ] {
            assert_eq!(parse_line(line), None, "{line:?}");
        }
    }

    #[test]
    fn the_sniffer_reports_a_version_once_across_chunks() {
        let mut s = OsVersionSniffer::new();
        assert_eq!(
            s.feed(b"FWF40F # get system status\r\nVersion: Forti"),
            None
        );
        assert_eq!(s.feed(b"WiFi-40F v7.2.8,build1639,240313 (GA.M)\r"), None);
        assert_eq!(s.feed(b"\nFirmware Signature: certified\r\n"), fgt("7.2.8"));
        // Asked again: same version, nothing new to say.
        assert_eq!(
            s.feed(b"Version: FortiWiFi-40F v7.2.8,build1639,240313 (GA.M)\r\n"),
            None
        );
        // After an upgrade the new version is reported.
        assert_eq!(
            s.feed(b"Version: FortiWiFi-40F v7.4.5,build2702,240917 (GA.M)\r\n"),
            fgt("7.4.5")
        );
    }

    #[test]
    fn an_over_long_line_is_skipped_whole() {
        let mut s = OsVersionSniffer::new();
        let mut long = vec![b'x'; MAX_LINE + 50];
        long.extend_from_slice(b"Version: FortiGate-60F v7.4.3,build1\n");
        assert_eq!(s.feed(&long), None);
        assert_eq!(
            s.feed(b"Version: FortiGate-60F v7.4.3,build1\n"),
            fgt("7.4.3")
        );
    }
}
