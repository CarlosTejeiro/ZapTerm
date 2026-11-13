//! Platform auto-detection from terminal output.
//!
//! Watches the byte stream of an SSH session, looks for distinctive prompts
//! and corroborating signature words (e.g. "FortiGate", "Cisco IOS Software"),
//! and returns a [`Platform`] guess the moment both lines line up. Once a
//! platform is detected it's **latched** — we never flap to another vendor
//! mid-session because a confusing line scrolled by.
//!
//! The detector is intentionally conservative: when in doubt we return
//! `None` rather than guess a wrong vendor and load the wrong catalog. False
//! positives are worse than no detection because they auto-write the wrong
//! platform to the saved session.

use std::sync::OnceLock;

use regex::Regex;

use crate::Platform;

/// How many recent bytes to keep around for signature-word matching. Big
/// enough to cover a typical login banner + a `show version` head; small
/// enough that the regex passes stay cheap.
const RING_CAP: usize = 8 * 1024;

/// Per-session detector. Cheap to construct; one per live SSH session.
pub struct PlatformDetector {
    recent: Vec<u8>,
    locked: Option<Platform>,
}

impl Default for PlatformDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformDetector {
    pub fn new() -> Self {
        Self {
            recent: Vec::with_capacity(RING_CAP),
            locked: None,
        }
    }

    /// Already-determined platform (or `None` if still searching).
    pub fn detected(&self) -> Option<Platform> {
        self.locked
    }

    /// Feed a chunk of output. Returns the detected platform if THIS call
    /// is the one that latched it (so the host can react exactly once);
    /// returns `None` on every subsequent call. Idempotent across chunks.
    pub fn feed(&mut self, bytes: &[u8]) -> Option<Platform> {
        if self.locked.is_some() {
            return None;
        }
        self.recent.extend_from_slice(bytes);
        if self.recent.len() > RING_CAP {
            let drop_n = self.recent.len() - RING_CAP;
            self.recent.drain(..drop_n);
        }
        let text = String::from_utf8_lossy(&self.recent);
        for rule in rules() {
            if rule.matches(&text) {
                self.locked = Some(rule.platform);
                return Some(rule.platform);
            }
        }
        None
    }
}

struct Rule {
    platform: Platform,
    /// Anchored at the END of the buffered text — we only match what looks
    /// like the most recent prompt line.
    prompt: &'static OnceLock<Regex>,
    /// Optional substrings that must appear somewhere in the recent buffer
    /// for the rule to fire. Any one of them is enough (`OR`). Empty means
    /// the prompt alone is sufficient (only for unambiguous prompts).
    signatures: &'static [&'static str],
}

impl Rule {
    fn matches(&self, text: &str) -> bool {
        let re = self
            .prompt
            .get_or_init(|| Regex::new(self.prompt_src()).expect("valid regex"));
        // Quick reject on the buffer-wide signature check before running the
        // (potentially expensive) regex over the tail. Case-insensitive: a
        // server's MOTD might say "ubuntu" or "Ubuntu", a banner "PAN-OS" or
        // "pan-os" — we want both forms to count.
        if !self.signatures.is_empty() {
            let haystack = text.to_ascii_lowercase();
            let any_hit = self
                .signatures
                .iter()
                .any(|sig| haystack.contains(&sig.to_ascii_lowercase()));
            if !any_hit {
                return false;
            }
        }
        // Take the last non-empty line and match the regex against it. We
        // also tolerate a trailing partial prompt (no newline yet) — the
        // tail of the buffer.
        let candidate = last_non_empty_line(text).unwrap_or(text);
        re.is_match(candidate.trim_end())
    }

    /// The regex source — duplicated next to the rule below so it's obvious
    /// what `prompt` is compiling. Kept as a function to avoid lazy_static
    /// gymnastics.
    fn prompt_src(&self) -> &'static str {
        // SAFETY: we tag each rule's prompt source via a const string. The
        // lookup is by Platform discriminant.
        prompt_src_for(self.platform)
    }
}

fn last_non_empty_line(text: &str) -> Option<&str> {
    text.lines().rev().find(|l| !l.trim().is_empty())
}

/// Compiled prompt regex per platform, cached for the life of the process.
/// Separate from [`Rule`]'s per-rule `OnceLock` so the stateless
/// [`ends_with_prompt`] check below doesn't depend on the detector's rule
/// table being walked.
fn prompt_regex(p: Platform) -> &'static Regex {
    static CACHE: OnceLock<std::collections::HashMap<Platform, Regex>> = OnceLock::new();
    let map = CACHE.get_or_init(|| {
        use Platform::*;
        [
            Linux,
            CiscoIos,
            CiscoIosXr,
            AristaEos,
            Windows,
            Juniper,
            Mikrotik,
            Aruba,
            Fortigate,
            Fortimanager,
            PaloAlto,
            F5Bigip,
            CheckpointGaia,
            HpComware,
            BrocadeFos,
        ]
        .into_iter()
        .map(|pl| (pl, Regex::new(prompt_src_for(pl)).expect("valid regex")))
        .collect()
    });
    map.get(&p).expect("prompt regex for non-generic platform")
}

/// Stateless check: does `text`'s last non-empty line look like an idle prompt
/// for `platform`? Unlike [`PlatformDetector::feed`] this never latches and
/// applies no signature gate — the caller already knows the platform and only
/// wants to know whether the device has returned to its prompt (used by the
/// multi-host command runner to bound captured output per host). Returns
/// `false` for [`Platform::Generic`], which has no recognisable prompt.
pub fn ends_with_prompt(platform: Platform, text: &str) -> bool {
    if platform == Platform::Generic {
        return false;
    }
    let candidate = last_non_empty_line(text).unwrap_or(text);
    prompt_regex(platform).is_match(candidate.trim_end())
}

// ---------------------------------------------------------------------------
// Rule table — ordered from most-specific to least. The first match wins.
// ---------------------------------------------------------------------------

fn rules() -> &'static [Rule] {
    static R: OnceLock<Vec<Rule>> = OnceLock::new();
    R.get_or_init(|| {
        // OnceLocks live in `static`; we leak a Box for each so the rule's
        // `prompt` field can be a `&'static OnceLock<Regex>`. The leak is
        // bounded — twelve cells, once per process.
        macro_rules! r {
            ($platform:expr, $sigs:expr) => {{
                Rule {
                    platform: $platform,
                    prompt: Box::leak(Box::new(OnceLock::new())),
                    signatures: $sigs,
                }
            }};
        }
        vec![
            // ── Highly distinctive prompts (no signature needed) ─────────────
            r!(Platform::F5Bigip, &[]),
            r!(Platform::Mikrotik, &[]),
            r!(Platform::BrocadeFos, &[]),
            r!(Platform::CheckpointGaia, &["Check Point", "Gaia"]),
            r!(Platform::HpComware, &[]),
            // IOS XR names the node in the prompt (RP/0/RSP0/CPU0:host#),
            // which no other vendor here does.
            r!(Platform::CiscoIosXr, &[]),
            // A drive path before `>` (`PS C:\Users\me>` or `C:\Users\me>`)
            // is a Windows shell; nothing else here prompts like that.
            r!(Platform::Windows, &[]),
            r!(Platform::Aruba, &["ArubaOS", "ProCurve", "Aruba"]),
            // ── Cisco-style `hostname#` prompt — needs a banner clue to
            //    pick the right vendor (Cisco vs Fortigate vs PAN-OS).
            r!(Platform::Fortimanager, &["FortiManager"]),
            r!(
                Platform::Fortigate,
                &["FortiGate", "FortiOS", "FGT", "Fortinet"]
            ),
            r!(Platform::PaloAlto, &["Palo Alto Networks", "PAN-OS"]),
            r!(Platform::Juniper, &["JUNOS", "Juniper"]),
            // EOS prompts look like IOS (`switch#`, `switch(config-if-Et1)#`);
            // `show version` starts with "Arista <model>".
            r!(Platform::AristaEos, &["Arista", "vEOS", "cEOS"]),
            r!(
                Platform::CiscoIos,
                &[
                    "Cisco IOS Software",
                    "IOS XE",
                    "Cisco Internetwork",
                    "IOS-XE"
                ]
            ),
            // ── Generic Unix shell — last resort. The prompt regex matches
            //    bare `host#` too, so we *insist* on a distro hint to avoid
            //    misfiring on a Cisco-style `router#` that lost its banner
            //    from the scrollback. Common distro markers from MOTD, SSH
            //    banner or `uname -a` output.
            r!(
                Platform::Linux,
                &[
                    "Ubuntu",
                    "Debian",
                    "CentOS",
                    "Red Hat",
                    "Rocky Linux",
                    "AlmaLinux",
                    "Alpine",
                    "Arch Linux",
                    "Fedora",
                    "openSUSE",
                    "GNU/Linux",
                    "Linux Mint",
                    "Last login:",
                    "/etc/motd",
                    "BusyBox",
                    "kernel",
                    "uname",
                ]
            ),
        ]
    })
}

/// The regex used to recognise the LAST prompt line for each platform.
/// Patterns are intentionally anchored at the end (`\z`-equivalent with
/// trailing-whitespace tolerance) — we only care about the active prompt
/// the user is sitting at, not echoes of historical prompts.
fn prompt_src_for(p: Platform) -> &'static str {
    match p {
        // (Active)(/Common)(tmos)# — the (tmos) marker is the giveaway.
        Platform::F5Bigip => r"\(tmos\)\s*[#>]\s*$",

        // [admin@MikroTik] >  — username@host bracketed, very distinctive.
        Platform::Mikrotik => r"^\[[^]@]+@[^]]+\]\s*>\s*$",

        // switch:admin>  or  switch:user>  — the `:role>` is unusual.
        Platform::BrocadeFos => r"^[A-Za-z][\w.-]*:(admin|user|root)>\s*$",

        // CLISH `hostname> ` — banner clue narrows from generic Junos prompt.
        Platform::CheckpointGaia => r"^[A-Za-z][\w.-]*>\s*$",

        // <hostname> (operator) or [hostname] (system-view) — bracket types
        // are Comware-specific.
        Platform::HpComware => r"^(?:<[\w.-]+>|\[[\w.-]+\])\s*$",

        // Aruba CX-style:  hostname#  — but always with the banner clue.
        Platform::Aruba => r"^[A-Za-z][\w.-]*#\s*$",

        // FortiManager top-level looks like `hostname #` (space before #)
        // and the welcome banner says FortiManager.
        Platform::Fortimanager => r"^[A-Za-z][\w.-]*\s?#\s*$",

        // FortiGate: `hostname # ` or `hostname (vdom) # ` — space before #.
        Platform::Fortigate => r"^[A-Za-z][\w.-]*\s?(?:\([\w-]+\)\s?)?#\s*$",

        // PAN-OS CLI:  user@hostname> (operational) or # (configure).
        Platform::PaloAlto => r"^[\w.-]+@[\w.-]+\s*[#>]\s*$",

        // Junos operational `user@host> ` or config `user@host# `.
        Platform::Juniper => r"^[\w.-]+@[\w.-]+\s*[#>]\s*$",

        // Cisco IOS / IOS-XE: `hostname#` (priv) or `hostname>` (user) or
        // `hostname(config)#`.
        Platform::CiscoIos => r"^[A-Za-z][\w.-]*(?:\([^)]+\))?\s*[#>]\s*$",

        // Cisco IOS XR: `RP/0/RSP0/CPU0:router#` (ASR 9000) or
        // `RP/0/RP0/CPU0:router(config-if)#` (Cisco 8000, NCS) — the node
        // path before the hostname is unique to XR.
        Platform::CiscoIosXr => r"^RP/\d+/[A-Za-z0-9]+/CPU\d+:[\w.-]+(?:\([^)]+\))?#\s*$",

        // Arista EOS: same shape as IOS — `switch>`, `switch#`,
        // `switch(config-router-bgp)#`. Told apart by the banner clue.
        Platform::AristaEos => r"^[A-Za-z][\w.-]*(?:\([^)]+\))?\s*[#>]\s*$",

        // Windows: PowerShell `PS C:\Users\me> ` or cmd `C:\Users\me>`.
        Platform::Windows => r"^(?:PS )?[A-Za-z]:\\[^>\r\n]*>\s*$",

        // Linux/bash variants — broadest, last. The distro-signature gate
        // (see the rule table) keeps this broad regex from misfiring on
        // any-old-`host#` line, so we can accept the messy reality:
        //   user@host:path$    BusyBox `/ #`    `(venv) $`    bare `# `
        Platform::Linux => {
            r"(?:^|\n)(?:[\w.-]+@[\w.-]+(?::[^\s]*)?\s*[$#]|\[[^\]]*\]\s*[$#]|[^\n$#]*[$#])\s*$"
        }

        Platform::Generic => r"\A\z", // never matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detect(text: &str) -> Option<Platform> {
        let mut d = PlatformDetector::new();
        d.feed(text.as_bytes())
    }

    #[test]
    fn cisco_with_banner() {
        let text = "Cisco IOS Software, ...\n\nrouter1#";
        assert_eq!(detect(text), Some(Platform::CiscoIos));
    }

    #[test]
    fn cisco_config_mode() {
        let text = "Cisco IOS Software, ...\n\nrouter1(config-if)#";
        assert_eq!(detect(text), Some(Platform::CiscoIos));
    }

    #[test]
    fn iosxr_prompt_alone() {
        assert_eq!(
            detect("RP/0/RSP0/CPU0:asr9k-pe1#"),
            Some(Platform::CiscoIosXr)
        );
        assert_eq!(detect("RP/0/RP0/CPU0:router#"), Some(Platform::CiscoIosXr));
    }

    #[test]
    fn iosxr_config_mode() {
        let text = "Cisco IOS XR Software, Version 7.11.2\nRP/0/RP0/CPU0:pe2(config-if)#";
        assert_eq!(detect(text), Some(Platform::CiscoIosXr));
    }

    #[test]
    fn iosxr_wins_over_linux_keywords() {
        // XR's `show version` mentions the kernel on some releases; the XR
        // prompt must still win over the Linux catch-all.
        let text = "Linux kernel ...\nRP/0/RP0/CPU0:pe2#";
        assert_eq!(detect(text), Some(Platform::CiscoIosXr));
    }

    #[test]
    fn iosxe_not_taken_for_iosxr() {
        assert_eq!(
            detect("Cisco IOS XE Software\nrouter1#"),
            Some(Platform::CiscoIos)
        );
    }

    #[test]
    fn arista_from_show_version() {
        let text =
            "switch#show version\nArista DCS-7150S-64-CL-F\nHardware version:    01.01\n\nswitch#";
        assert_eq!(detect(text), Some(Platform::AristaEos));
    }

    #[test]
    fn arista_config_mode() {
        let text = "Arista vEOS\nleaf1(config-if-Et24)#";
        assert_eq!(detect(text), Some(Platform::AristaEos));
    }

    #[test]
    fn arista_needs_its_banner() {
        // A bare `switch#` stays undetected, and an IOS banner stays IOS.
        assert_eq!(detect("switch#"), None);
        assert_eq!(
            detect("Cisco IOS Software, Version 15.7\nswitch#"),
            Some(Platform::CiscoIos)
        );
    }

    #[test]
    fn windows_powershell_and_cmd_prompts() {
        assert_eq!(detect("PS C:\\Users\\carlos> "), Some(Platform::Windows));
        assert_eq!(detect("PS C:\\> "), Some(Platform::Windows));
        assert_eq!(
            detect("Microsoft Windows [Version 10.0.26100.2314]\r\nC:\\Users\\carlos>"),
            Some(Platform::Windows)
        );
    }

    #[test]
    fn windows_prompt_not_taken_from_linux_paths() {
        assert_eq!(detect("Ubuntu 24.04\nuser@host:~$ "), Some(Platform::Linux));
        assert_eq!(detect("router1>"), None);
    }

    #[test]
    fn fortigate_with_banner() {
        let text = "FortiGate-100D login...\nfgt-edge # ";
        assert_eq!(detect(text), Some(Platform::Fortigate));
    }

    #[test]
    fn fortigate_with_vdom() {
        let text = "FortiOS v7.4\nfgt-edge (root) # ";
        assert_eq!(detect(text), Some(Platform::Fortigate));
    }

    #[test]
    fn fortimanager() {
        let text = "FortiManager 7.6\nfmg01 # ";
        assert_eq!(detect(text), Some(Platform::Fortimanager));
    }

    #[test]
    fn paloalto() {
        // Real PAN-OS login banner advertises PAN-OS by name.
        let text = "PAN-OS 11.2.3\nadmin@PA-3220> ";
        assert_eq!(detect(text), Some(Platform::PaloAlto));
    }

    #[test]
    fn juniper() {
        let text = "JUNOS 23.4R1\nroot@mx-edge> ";
        assert_eq!(detect(text), Some(Platform::Juniper));
    }

    #[test]
    fn mikrotik() {
        let text = "MikroTik RouterOS\n[admin@MikroTik] > ";
        assert_eq!(detect(text), Some(Platform::Mikrotik));
    }

    #[test]
    fn comware_operator() {
        let text = "Comware Software\n<HP-5130>";
        assert_eq!(detect(text), Some(Platform::HpComware));
    }

    #[test]
    fn comware_system_view() {
        let text = "Comware Software\n[HP-5130]";
        assert_eq!(detect(text), Some(Platform::HpComware));
    }

    #[test]
    fn brocade() {
        let text = "Fabric OS\nsanswitch:admin> ";
        assert_eq!(detect(text), Some(Platform::BrocadeFos));
    }

    #[test]
    fn f5_tmos() {
        let text = "Welcome to the BIG-IP\n(Active)(/Common)(tmos)# ";
        assert_eq!(detect(text), Some(Platform::F5Bigip));
    }

    #[test]
    fn linux_full_prompt() {
        let text = "Last login: ...\ncarlos@bastion:~/work$ ";
        assert_eq!(detect(text), Some(Platform::Linux));
    }

    #[test]
    fn linux_container_dollar() {
        let text = "alpine\n$ ";
        assert_eq!(detect(text), Some(Platform::Linux));
    }

    #[test]
    fn no_match_random_text() {
        let text = "nothing here\nlorem ipsum dolor sit\n";
        assert_eq!(detect(text), None);
    }

    #[test]
    fn latch_does_not_flap() {
        let mut d = PlatformDetector::new();
        let first = d.feed(b"Cisco IOS Software\nrouter1#");
        assert_eq!(first, Some(Platform::CiscoIos));
        // A subsequent Linux-looking prompt must not change the verdict
        let second = d.feed(b"\nuser@host:~$ ");
        assert_eq!(second, None);
        assert_eq!(d.detected(), Some(Platform::CiscoIos));
    }

    #[test]
    fn cisco_alone_no_banner_no_match() {
        // Without a banner clue, a bare `hostname#` is too generic to call
        // Cisco — we'd rather not detect than detect wrong.
        assert_eq!(detect("router1#"), None);
    }

    #[test]
    fn host_hash_no_distro_does_not_misfire_as_linux() {
        // A scrollback consisting of just `host#` (a Cisco-style prompt
        // without any banner left around) must NOT be auto-classified as
        // Linux. We added a distro signature requirement to Linux exactly
        // for this case — otherwise we'd auto-write the wrong platform.
        assert_eq!(detect("router1#"), None);
        assert_eq!(detect("\n\nsw1-core#"), None);
    }

    #[test]
    fn linux_case_insensitive_signature() {
        // MOTD says "ubuntu" lowercase; signature is "Ubuntu". Should still match.
        let text = "Welcome to ubuntu 22.04\nuser@web:~$ ";
        assert_eq!(detect(text), Some(Platform::Linux));
    }

    #[test]
    fn linux_alpine_busybox() {
        let text = "BusyBox v1.36 built-in shell\n/ # ";
        assert_eq!(detect(text), Some(Platform::Linux));
    }

    #[test]
    fn linux_centos_motd() {
        let text = "CentOS Linux 7\n[root@db ~]# ";
        assert_eq!(detect(text), Some(Platform::Linux));
    }

    #[test]
    fn cisco_banner_wins_even_with_linux_keyword_present() {
        // If a Linux-y word appears in a Cisco router's `show running-config`
        // (unusual but possible), the Cisco rule must still win because it's
        // checked first in the rule table.
        let text = "Cisco IOS Software, Version 15.7\n\
                    ! banner motd ^ ubuntu ^\n\
                    sw1#";
        assert_eq!(detect(text), Some(Platform::CiscoIos));
    }

    #[test]
    fn switching_streams_does_not_re_detect() {
        let mut d = PlatformDetector::new();
        // First detection.
        assert_eq!(
            d.feed(b"Ubuntu 22.04\nuser@host:~$ "),
            Some(Platform::Linux)
        );
        // Now a stronger signal arrives — must NOT re-fire.
        assert_eq!(
            d.feed(b"\nCisco IOS Software, Version 15.7\nrouter1#"),
            None
        );
        assert_eq!(d.detected(), Some(Platform::Linux));
    }

    #[test]
    fn ring_buffer_is_bounded() {
        // Push way more than RING_CAP bytes; the detector should still work
        // (drain not panic) and remain unlatched if nothing matches.
        let mut d = PlatformDetector::new();
        for _ in 0..50 {
            assert_eq!(d.feed(&[b'A'; 1024]), None);
        }
        assert_eq!(d.detected(), None);
        // Now feed a real banner + prompt; it should still latch from the
        // last RING_CAP bytes alone.
        let n = d.feed(b"FortiGate-100D\nfgt # ");
        assert_eq!(n, Some(Platform::Fortigate));
    }

    // ── ends_with_prompt — stateless completion check ────────────────────

    #[test]
    fn prompt_returns_after_command_output() {
        // The runner appends a command's echo + output + the returned prompt;
        // ends_with_prompt sees the prompt at the tail and reports done.
        let buf = "router1#show version\nCisco IOS XE, Version 17.9\n...\nrouter1#";
        assert!(ends_with_prompt(Platform::CiscoIos, buf));
    }

    #[test]
    fn prompt_not_returned_mid_output() {
        // Still streaming output — no prompt at the tail yet.
        let buf = "router1#show version\nCisco IOS XE, Version 17.9\nuptime is 4 weeks";
        assert!(!ends_with_prompt(Platform::CiscoIos, buf));
    }

    #[test]
    fn prompt_config_mode_counts_as_returned() {
        assert!(ends_with_prompt(Platform::CiscoIos, "sw1(config-if)#"));
    }

    #[test]
    fn prompt_per_vendor_tails() {
        assert!(ends_with_prompt(Platform::Juniper, "root@mx> "));
        assert!(ends_with_prompt(
            Platform::F5Bigip,
            "(Active)(/Common)(tmos)# "
        ));
        assert!(ends_with_prompt(Platform::Mikrotik, "[admin@MikroTik] > "));
        assert!(ends_with_prompt(Platform::HpComware, "<HP-5130>"));
        assert!(ends_with_prompt(Platform::Linux, "carlos@bastion:~/work$ "));
        assert!(ends_with_prompt(
            Platform::AristaEos,
            "leaf1(config-router-bgp)#"
        ));
        assert!(ends_with_prompt(
            Platform::Windows,
            "PS C:\\Windows\\System32> "
        ));
        assert!(ends_with_prompt(
            Platform::CiscoIosXr,
            "RP/0/RSP0/CPU0:pe1(config-bgp-nbr)#"
        ));
    }

    #[test]
    fn prompt_generic_never_matches() {
        // Generic has no recognisable prompt — the runner must rely on the
        // time-window fallback for unknown platforms.
        assert!(!ends_with_prompt(Platform::Generic, "anything$ "));
        assert!(!ends_with_prompt(Platform::Generic, "host# "));
    }

    #[test]
    fn prompt_no_signature_gate() {
        // Unlike feed(), ends_with_prompt does NOT require a banner: the caller
        // already knows the platform, so a bare `router1#` is a valid Cisco
        // prompt here (whereas detect("router1#") returns None).
        assert!(ends_with_prompt(Platform::CiscoIos, "router1#"));
        assert_eq!(detect("router1#"), None);
    }
}
