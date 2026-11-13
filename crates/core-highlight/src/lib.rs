#![forbid(unsafe_code)]

//! `core-highlight` — keyword highlighting engine with regex support.
//!
//! Applies ANSI true-color sequences around pattern matches in the terminal
//! byte stream. Rules are ordered by `sort_order`; the first matching rule wins
//! for any given text position (overlapping rules are ignored after the first).

pub mod error;

use regex::{Regex, RegexBuilder};

/// Upper bound on a user-supplied pattern's source length. Anything longer is
/// rejected at compile time — guards against pathological patterns being fed
/// into the per-line, per-rule hot path.
const MAX_PATTERN_LEN: usize = 1024;

/// Compile-time memory ceiling for a single compiled rule (1 MiB). The `regex`
/// crate already guarantees linear-time matching (no catastrophic
/// backtracking), so this only bounds compilation memory for adversarial
/// patterns.
const REGEX_SIZE_LIMIT: usize = 1 << 20;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A highlight rule as stored in the database.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HighlightRule {
    pub id: i64,
    pub name: String,
    pub pattern: String,
    pub is_regex: bool,
    pub fg_color: Option<String>, // "#RRGGBB" or None
    pub bg_color: Option<String>,
    pub bold: bool,
    pub underline: bool,
    pub enabled: bool,
    pub sort_order: i32,
}

// ---------------------------------------------------------------------------
// Highlighter
// ---------------------------------------------------------------------------

struct CompiledRule {
    rule: HighlightRule,
    regex: Regex,
}

pub struct Highlighter {
    rules: Vec<CompiledRule>,
}

impl Highlighter {
    /// Build a highlighter from a list of rules (typically loaded from the DB).
    /// Invalid regex patterns are silently skipped.
    pub fn new(rules: Vec<HighlightRule>) -> Self {
        let mut compiled: Vec<CompiledRule> = rules
            .into_iter()
            .filter(|r| r.enabled)
            .filter_map(|r| {
                if r.pattern.is_empty() || r.pattern.len() > MAX_PATTERN_LEN {
                    return None;
                }
                let pattern = if r.is_regex {
                    r.pattern.clone()
                } else {
                    regex::escape(&r.pattern)
                };
                RegexBuilder::new(&pattern)
                    .size_limit(REGEX_SIZE_LIMIT)
                    .dfa_size_limit(REGEX_SIZE_LIMIT)
                    .build()
                    .ok()
                    .map(|regex| CompiledRule { rule: r, regex })
            })
            .collect();
        compiled.sort_by_key(|c| c.rule.sort_order);
        Self { rules: compiled }
    }

    /// Apply highlighting to a raw terminal data chunk.
    ///
    /// ANSI escape sequences already present in the input are stripped first so
    /// that our injected codes don't interfere with them. The returned bytes are
    /// valid UTF-8 / ANSI for xterm.js.
    pub fn apply(&self, input: &[u8]) -> Vec<u8> {
        if self.rules.is_empty() {
            return input.to_vec();
        }
        let text = String::from_utf8_lossy(input);
        let mut out = String::with_capacity(text.len() + 64);
        // Process line by line so \r\n is preserved correctly.
        for segment in text.split_inclusive('\n') {
            out.push_str(&self.highlight_segment(segment));
        }
        out.into_bytes()
    }

    fn highlight_segment(&self, segment: &str) -> String {
        // Strip existing ANSI sequences to get the clean text we match against,
        // keeping a byte-offset map back into the original segment so we can
        // copy through the server's own colouring outside of our matches.
        let (clean, map) = strip_ansi_mapped(segment);

        // Collect non-overlapping matches sorted by start position.
        // First rule in sort_order wins if ranges overlap.
        let mut spans: Vec<(usize, usize, &CompiledRule)> = vec![];
        for cr in &self.rules {
            for m in cr.regex.find_iter(&clean) {
                let (s, e) = (m.start(), m.end());
                let overlaps = spans.iter().any(|(ms, me, _)| s < *me && e > *ms);
                if !overlaps {
                    spans.push((s, e, cr));
                }
            }
        }
        if spans.is_empty() {
            return segment.to_string();
        }
        spans.sort_by_key(|(s, _, _)| *s);

        // Reconstruct the segment with ANSI highlight codes injected. Text
        // *between* matches is copied from the original `segment` (preserving
        // the server's ANSI styling); the matched text itself is copied from
        // `clean` so the highlight colour applies uniformly. All slices use
        // `.get(..)` so a future change to the index sources can never panic on
        // a char boundary.
        let mut out = String::with_capacity(segment.len() + spans.len() * 24);
        let mut cursor = 0usize;
        for (start, end, cr) in &spans {
            if cursor < *start {
                out.push_str(segment.get(map[cursor]..map[*start]).unwrap_or(""));
            }
            out.push_str(&build_open_code(&cr.rule));
            out.push_str(clean.get(*start..*end).unwrap_or(""));
            out.push_str("\x1b[0m");
            cursor = *end;
        }
        if cursor < clean.len() {
            out.push_str(segment.get(map[cursor]..).unwrap_or(""));
        }
        out
    }
}

// ---------------------------------------------------------------------------
// ANSI helpers
// ---------------------------------------------------------------------------

/// Remove ANSI escape sequences from a string (CSI + 2-char sequences),
/// returning the clean text together with a byte-offset map back into `s`.
///
/// `map` has `clean.len() + 1` entries: `map[i]` is the byte offset in `s`
/// that the i-th byte of the clean string originated from, and
/// `map[clean.len()]` is `s.len()`. Every entry is a valid char boundary in
/// `s`, so slicing `s` at any `map[..]` value is always safe.
fn strip_ansi_mapped(s: &str) -> (String, Vec<usize>) {
    let mut out = String::with_capacity(s.len());
    let mut map: Vec<usize> = Vec::with_capacity(s.len() + 1);
    let mut iter = s.char_indices().peekable();
    while let Some((off, c)) = iter.next() {
        if c == '\x1b' {
            // Consume the escape sequence.
            if matches!(iter.peek(), Some((_, '['))) {
                iter.next(); // consume '['
                             // Consume until a final byte (0x40–0x7E).
                for (_, ch) in iter.by_ref() {
                    if ('\x40'..='\x7e').contains(&ch) {
                        break;
                    }
                }
            } else {
                // Two-character escape sequence: consume one more char.
                iter.next();
            }
        } else {
            for _ in 0..c.len_utf8() {
                map.push(off);
            }
            out.push(c);
        }
    }
    map.push(s.len());
    (out, map)
}

/// Build the ANSI SGR open sequence for a rule's style.
fn build_open_code(rule: &HighlightRule) -> String {
    let mut params: Vec<String> = vec![];
    if rule.bold {
        params.push("1".into());
    }
    if rule.underline {
        params.push("4".into());
    }
    if let Some(fg) = &rule.fg_color {
        if let Some((r, g, b)) = parse_hex_color(fg) {
            params.push(format!("38;2;{r};{g};{b}"));
        }
    }
    if let Some(bg) = &rule.bg_color {
        if let Some((r, g, b)) = parse_hex_color(bg) {
            params.push(format!("48;2;{r};{g};{b}"));
        }
    }
    if params.is_empty() {
        return String::new();
    }
    format!("\x1b[{}m", params.join(";"))
}

fn parse_hex_color(hex: &str) -> Option<(u8, u8, u8)> {
    let h = hex.trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&h[0..2], 16).ok()?;
    let g = u8::from_str_radix(&h[2..4], 16).ok()?;
    let b = u8::from_str_radix(&h[4..6], 16).ok()?;
    Some((r, g, b))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(pattern: &str, fg: &str, id: i64) -> HighlightRule {
        HighlightRule {
            id,
            name: pattern.to_string(),
            pattern: pattern.to_string(),
            is_regex: true,
            fg_color: Some(fg.to_string()),
            bg_color: None,
            bold: false,
            underline: false,
            enabled: true,
            sort_order: id as i32,
        }
    }

    #[test]
    fn highlights_simple_match() {
        let h = Highlighter::new(vec![rule("error", "#ef4444", 1)]);
        let out = String::from_utf8(h.apply(b"this is an error here")).unwrap();
        assert!(out.contains("\x1b[38;2;239;68;68merror\x1b[0m"));
    }

    #[test]
    fn passthrough_when_no_rules() {
        let h = Highlighter::new(vec![]);
        let input = b"plain text";
        assert_eq!(h.apply(input), input);
    }

    #[test]
    fn strips_existing_ansi_before_matching() {
        let h = Highlighter::new(vec![rule("down", "#ef4444", 1)]);
        let out = String::from_utf8(h.apply(b"interface is \x1b[32mdown\x1b[0m")).unwrap();
        assert!(out.contains("down"));
    }

    #[test]
    fn preserves_server_ansi_outside_matches() {
        // The server colours "OK" green; our rule highlights "error". The green
        // run sits outside the match and must survive into the output.
        let h = Highlighter::new(vec![rule("error", "#ef4444", 1)]);
        let out = String::from_utf8(h.apply(b"status \x1b[32mOK\x1b[0m but error seen")).unwrap();
        assert!(
            out.contains("\x1b[32mOK\x1b[0m"),
            "server green lost: {out:?}"
        );
        assert!(out.contains("\x1b[38;2;239;68;68merror\x1b[0m"));
    }

    #[test]
    fn handles_multibyte_text_without_panicking() {
        // Accented / multibyte bytes around and inside matches must not break
        // byte-offset slicing.
        let h = Highlighter::new(vec![rule("café", "#ef4444", 1)]);
        let out = String::from_utf8(h.apply("añadir café ☕ aquí".as_bytes())).unwrap();
        assert!(out.contains("\x1b[38;2;239;68;68mcafé\x1b[0m"));
        assert!(out.contains("añadir"));
        assert!(out.contains("☕"));
    }

    #[test]
    fn rejects_oversized_pattern() {
        let mut r = rule("x", "#ffffff", 1);
        r.pattern = "a".repeat(MAX_PATTERN_LEN + 1);
        let h = Highlighter::new(vec![r]);
        // Oversized pattern is dropped → input passes through untouched.
        assert_eq!(h.apply(b"aaa"), b"aaa");
    }
}
