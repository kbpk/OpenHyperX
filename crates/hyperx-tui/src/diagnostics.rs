//! File-only diagnostics. Field locations are typed validation metadata from
//! the shared app layer, never guessed from English error strings.

use hyperx_app::ProfileReadiness;
use hyperx_core::{SoftwareProfileDiff, SoftwareProfileFieldChange};

pub(crate) fn field_tab(field: &str) -> usize {
    if field.starts_with("dpi") || field.starts_with("polling") || field == "primary_buttons" {
        0
    } else if field.starts_with("buttons") {
        1
    } else if field.starts_with("macros") {
        2
    } else if field.starts_with("lighting") {
        3
    } else {
        4
    }
}

const SECTIONS: [&str; 5] = ["Performance", "Buttons", "Macros", "Lighting", "Profiles"];

pub(crate) fn validation_view(readiness: &ProfileReadiness) -> (String, Option<usize>) {
    let mut text = String::from(
        "Offline supplied-field validation; not a device read or permission to apply.\n\n",
    );
    let jump = readiness.field.as_deref().map(field_tab);
    if let Some(error) = &readiness.error {
        text.push_str(&format!("NOT READY: {error}\n"));
        if let (Some(field), Some(tab)) = (&readiness.field, jump) {
            text.push_str(&format!(
                "\nOffending file field: {}\nSection: {}\nPress g to open that section without changing the draft.\n",
                field.escape_debug(), SECTIONS[tab]
            ));
        } else {
            text.push_str("\nNo exact file field is known; inspect the full profile without guessing a target.\n");
        }
    } else {
        text.push_str("Supplied fields pass offline encoding validation.\nThis does not check current mouse state; live apply remains suspended.\n");
    }
    if !readiness.warnings.is_empty() {
        text.push_str("\nWarnings:\n");
        for warning in &readiness.warnings {
            text.push_str(&format!("  • {}\n", warning.escape_debug()));
        }
    }
    (text, jump)
}

fn bucket(field: &str) -> usize {
    let tab = field_tab(field);
    if tab == 4 {
        4
    } else {
        tab
    }
}

fn change_line(text: &mut String, change: &SoftwareProfileFieldChange) {
    let before = change.before.as_deref().unwrap_or("<not present>");
    let after = change.after.as_deref().unwrap_or("<not present>");
    text.push_str(&format!(
        "  {}\n    before: {}\n    after:  {}\n",
        change.field.escape_debug(),
        before.escape_debug(),
        after.escape_debug()
    ));
}

pub(crate) fn diff_view(diff: &SoftwareProfileDiff) -> String {
    let settings = diff.settings.len();
    let metadata = diff.metadata.len();
    let mut text = format!(
        "FILE DIFF: {settings} setting change(s), {metadata} metadata/provenance change(s).\nCompared with the last opened/saved file, never with the mouse.\n<not present> means preserve an omitted setting, not reset/disabled.\n"
    );
    if settings + metadata == 0 {
        text.push_str("\nNo differences.\n");
        return text;
    }
    let mut groups: [Vec<&SoftwareProfileFieldChange>; 5] = std::array::from_fn(|_| Vec::new());
    for change in &diff.settings {
        groups[bucket(&change.field)].push(change);
    }
    for (index, group) in groups.iter().enumerate() {
        if group.is_empty() {
            continue;
        }
        text.push_str(&format!("\n{} ({}):\n", SECTIONS[index], group.len()));
        for change in group {
            change_line(&mut text, change);
        }
    }
    if !diff.metadata.is_empty() {
        text.push_str(&format!(
            "\nSource and metadata ({}):\n",
            diff.metadata.len()
        ));
        for change in &diff.metadata {
            change_line(&mut text, change);
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_validation_field_selects_section_without_parsing_message() {
        let result = ProfileReadiness {
            error: Some("a completely unrelated localized sentence".into()),
            field: Some("dpi.stages[1].x".into()),
            warnings: Vec::new(),
        };
        let (text, jump) = validation_view(&result);
        assert_eq!(jump, Some(0));
        assert!(text.contains("Offending file field: dpi.stages[1].x"));
        assert!(text.contains("Section: Performance"));
        for (field, expected) in [
            ("buttons.button4.key", 1),
            ("macros[0]", 2),
            ("lighting.zones", 3),
            ("unresolved_button_assignments", 4),
        ] {
            assert_eq!(field_tab(field), expected);
        }
    }

    #[test]
    fn diff_groups_settings_and_preserves_omission_distinction() {
        let diff = SoftwareProfileDiff {
            settings: vec![
                SoftwareProfileFieldChange {
                    field: "polling.hz".into(),
                    before: Some("1000".into()),
                    after: Some("500".into()),
                },
                SoftwareProfileFieldChange {
                    field: "buttons[\"button4\"]".into(),
                    before: Some("Mouse: Back".into()),
                    after: None,
                },
            ],
            metadata: vec![SoftwareProfileFieldChange {
                field: "source.format".into(),
                before: None,
                after: Some("ngenuity-legacy-hxp".into()),
            }],
        };
        let text = diff_view(&diff);
        assert!(text.contains("2 setting change(s), 1 metadata/provenance"));
        assert!(text.contains("Performance (1):\n  polling.hz\n    before: 1000\n    after:  500"));
        assert!(text.contains("Buttons (1):\n  buttons[\\\"button4\\\"]\n    before: Mouse: Back\n    after:  <not present>"));
        assert!(text.contains("Source and metadata (1):"));
    }
}
