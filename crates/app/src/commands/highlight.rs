use tauri::State;

use core_persistence::HighlightRule;

use crate::error::AppError;
use crate::state::AppState;

pub(crate) fn rebuild_highlighter(state: &AppState) {
    let rules = state.db.list_highlight_rules().unwrap_or_default();
    let converted: Vec<core_highlight::HighlightRule> = rules
        .into_iter()
        .map(|r| core_highlight::HighlightRule {
            id: r.id,
            name: r.name,
            pattern: r.pattern,
            is_regex: r.is_regex,
            fg_color: r.fg_color,
            bg_color: r.bg_color,
            bold: r.bold,
            underline: r.underline,
            enabled: r.enabled,
            sort_order: r.sort_order,
        })
        .collect();
    if let Ok(mut h) = state.highlighter.write() {
        *h = core_highlight::Highlighter::new(converted);
    }
}

#[tauri::command]
pub async fn list_highlight_rules(
    state: State<'_, AppState>,
) -> Result<Vec<HighlightRule>, AppError> {
    state
        .db
        .list_highlight_rules()
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn create_highlight_rule(
    state: State<'_, AppState>,
    name: String,
    pattern: String,
    is_regex: bool,
    fg_color: Option<String>,
    bg_color: Option<String>,
    bold: bool,
    underline: bool,
) -> Result<i64, AppError> {
    let id = state
        .db
        .create_highlight_rule(
            &name,
            &pattern,
            is_regex,
            fg_color.as_deref(),
            bg_color.as_deref(),
            bold,
            underline,
        )
        .map_err(|e| AppError::Internal(e.to_string()))?;
    rebuild_highlighter(&state);
    Ok(id)
}

#[tauri::command]
pub async fn toggle_highlight_rule(
    state: State<'_, AppState>,
    id: i64,
    enabled: bool,
) -> Result<(), AppError> {
    state
        .db
        .toggle_highlight_rule(id, enabled)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    rebuild_highlighter(&state);
    Ok(())
}

#[tauri::command]
pub async fn delete_highlight_rule(state: State<'_, AppState>, id: i64) -> Result<(), AppError> {
    state
        .db
        .delete_highlight_rule(id)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    rebuild_highlighter(&state);
    Ok(())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn update_highlight_rule(
    state: State<'_, AppState>,
    id: i64,
    name: String,
    fg_color: Option<String>,
    bg_color: Option<String>,
    bold: bool,
    underline: bool,
    enabled: bool,
) -> Result<(), AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Internal("Name is required".into()));
    }
    state
        .db
        .update_highlight_rule(
            id,
            name,
            fg_color.as_deref(),
            bg_color.as_deref(),
            bold,
            underline,
            enabled,
        )
        .map_err(|e| AppError::Internal(e.to_string()))?;
    rebuild_highlighter(&state);
    Ok(())
}

// ---------------------------------------------------------------------------
// Export / import of the rules alone (the full environment export also
// carries them, together with sessions and snippets).
// ---------------------------------------------------------------------------

/// Marks a file as a ZapTerm highlight-rules export.
const RULES_FORMAT: &str = "zapx-highlight-rules";
const RULES_VERSION: u32 = 1;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RuleEntry {
    pub name: String,
    pub pattern: String,
    #[serde(default)]
    pub is_regex: bool,
    #[serde(default)]
    pub fg_color: Option<String>,
    #[serde(default)]
    pub bg_color: Option<String>,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub underline: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct RulesFile {
    format: String,
    version: u32,
    rules: Vec<RuleEntry>,
}

#[derive(Debug, Default, serde::Serialize, PartialEq)]
pub struct RulesImportSummary {
    pub added: usize,
    /// Same pattern (and regex flag) as an existing rule.
    pub skipped: usize,
    /// Rules that were left out, with the reason.
    pub invalid: Vec<String>,
}

fn rules_to_json(rules: &[HighlightRule]) -> Result<String, serde_json::Error> {
    let file = RulesFile {
        format: RULES_FORMAT.into(),
        version: RULES_VERSION,
        rules: rules
            .iter()
            .map(|r| RuleEntry {
                name: r.name.clone(),
                pattern: r.pattern.clone(),
                is_regex: r.is_regex,
                fg_color: r.fg_color.clone(),
                bg_color: r.bg_color.clone(),
                bold: r.bold,
                underline: r.underline,
                enabled: r.enabled,
            })
            .collect(),
    };
    serde_json::to_string_pretty(&file)
}

fn valid_color(c: &Option<String>) -> bool {
    match c {
        None => true,
        Some(s) => {
            s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|ch| ch.is_ascii_hexdigit())
        }
    }
}

/// Parse an export and keep the rules worth adding: valid, and not already
/// present (same pattern + regex flag, the rule's identity) in `existing` or
/// earlier in the file.
fn plan_import(
    json: &str,
    existing: &[HighlightRule],
) -> Result<(Vec<RuleEntry>, RulesImportSummary), String> {
    let file: RulesFile =
        serde_json::from_str(json).map_err(|e| format!("Not a highlight rules file: {e}"))?;
    if file.format != RULES_FORMAT {
        return Err(format!(
            "Not a highlight rules file (format \"{}\")",
            file.format
        ));
    }
    if file.version > RULES_VERSION {
        return Err(format!(
            "This file was made by a newer version (format version {})",
            file.version
        ));
    }
    let mut seen: std::collections::HashSet<(String, bool)> = existing
        .iter()
        .map(|r| (r.pattern.clone(), r.is_regex))
        .collect();
    let mut summary = RulesImportSummary::default();
    let mut keep = Vec::new();
    for mut r in file.rules {
        r.name = r.name.trim().to_string();
        let label = if r.name.is_empty() {
            r.pattern.clone()
        } else {
            r.name.clone()
        };
        if r.name.is_empty() || r.pattern.trim().is_empty() {
            summary
                .invalid
                .push(format!("{label}: name and pattern are required"));
            continue;
        }
        if r.is_regex {
            if let Err(e) = regex::Regex::new(&r.pattern) {
                summary
                    .invalid
                    .push(format!("{label}: invalid regex ({e})"));
                continue;
            }
        }
        if !valid_color(&r.fg_color) || !valid_color(&r.bg_color) {
            summary
                .invalid
                .push(format!("{label}: colors must look like #rrggbb"));
            continue;
        }
        if !seen.insert((r.pattern.clone(), r.is_regex)) {
            summary.skipped += 1;
            continue;
        }
        keep.push(r);
    }
    Ok((keep, summary))
}

#[tauri::command]
pub async fn export_highlight_rules(
    state: State<'_, AppState>,
    path: String,
) -> Result<usize, AppError> {
    let rules = state
        .db
        .list_highlight_rules()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let json = rules_to_json(&rules).map_err(|e| AppError::Internal(e.to_string()))?;
    std::fs::write(&path, json).map_err(|e| AppError::Internal(format!("{path}: {e}")))?;
    Ok(rules.len())
}

#[tauri::command]
pub async fn import_highlight_rules(
    state: State<'_, AppState>,
    path: String,
) -> Result<RulesImportSummary, AppError> {
    let json =
        std::fs::read_to_string(&path).map_err(|e| AppError::Internal(format!("{path}: {e}")))?;
    let existing = state
        .db
        .list_highlight_rules()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let (rules, mut summary) = plan_import(&json, &existing).map_err(AppError::Internal)?;
    for r in rules {
        let id = state
            .db
            .create_highlight_rule(
                &r.name,
                &r.pattern,
                r.is_regex,
                r.fg_color.as_deref(),
                r.bg_color.as_deref(),
                r.bold,
                r.underline,
            )
            .map_err(|e| AppError::Internal(e.to_string()))?;
        if !r.enabled {
            state
                .db
                .toggle_highlight_rule(id, false)
                .map_err(|e| AppError::Internal(e.to_string()))?;
        }
        summary.added += 1;
    }
    rebuild_highlighter(&state);
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(id: i64, name: &str, pattern: &str, is_regex: bool) -> HighlightRule {
        HighlightRule {
            id,
            name: name.into(),
            pattern: pattern.into(),
            is_regex,
            fg_color: Some("#ff0000".into()),
            bg_color: None,
            bold: true,
            underline: false,
            enabled: true,
            sort_order: id as i32,
        }
    }

    #[test]
    fn export_round_trips_into_import() {
        let rules = vec![
            rule(1, "Err", "%ERROR", false),
            rule(2, "Down", r"\bdown\b", true),
        ];
        let json = rules_to_json(&rules).unwrap();
        assert!(json.contains("\"format\": \"zapx-highlight-rules\""));
        let (keep, summary) = plan_import(&json, &[]).unwrap();
        assert_eq!(keep.len(), 2);
        assert_eq!(keep[1].pattern, r"\bdown\b");
        assert!(keep[0].bold && keep[0].enabled);
        assert_eq!(summary, RulesImportSummary::default());
    }

    #[test]
    fn import_skips_rules_already_present() {
        let json = rules_to_json(&[rule(1, "Err", "%ERROR", false)]).unwrap();
        let existing = vec![rule(9, "Other name", "%ERROR", false)];
        let (keep, summary) = plan_import(&json, &existing).unwrap();
        assert!(keep.is_empty());
        assert_eq!(summary.skipped, 1);
        // Same text as a regex is a different rule.
        let existing = vec![rule(9, "Rx", "%ERROR", true)];
        assert_eq!(plan_import(&json, &existing).unwrap().0.len(), 1);
    }

    #[test]
    fn import_drops_duplicates_inside_the_file() {
        let json = rules_to_json(&[rule(1, "A", "x", false), rule(2, "B", "x", false)]).unwrap();
        let (keep, summary) = plan_import(&json, &[]).unwrap();
        assert_eq!(keep.len(), 1);
        assert_eq!(summary.skipped, 1);
    }

    #[test]
    fn import_reports_invalid_rules() {
        let json = r#"{"format":"zapx-highlight-rules","version":1,"rules":[
            {"name":"Bad rx","pattern":"(unclosed","is_regex":true},
            {"name":"","pattern":"x"},
            {"name":"Bad color","pattern":"y","fg_color":"red"},
            {"name":"Ok","pattern":"z"}
        ]}"#;
        let (keep, summary) = plan_import(json, &[]).unwrap();
        assert_eq!(keep.len(), 1);
        assert_eq!(keep[0].name, "Ok");
        assert!(keep[0].enabled, "enabled defaults to true");
        assert_eq!(summary.invalid.len(), 3);
        assert!(summary.invalid[0].starts_with("Bad rx: invalid regex"));
    }

    #[test]
    fn import_rejects_other_files() {
        assert!(plan_import(r#"{"sessions":[]}"#, &[]).is_err());
        assert!(plan_import(r#"{"format":"other","version":1,"rules":[]}"#, &[]).is_err());
        assert!(plan_import(
            r#"{"format":"zapx-highlight-rules","version":99,"rules":[]}"#,
            &[]
        )
        .is_err());
    }
}
