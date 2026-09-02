//! One pass over a character sheet at session start, to find what anyone in the
//! room could see.
//!
//! Of a sheet, only appearance is public: hair, build, clothing, whatever is
//! being carried. Occupation, family, history, and the rest all have to be told.
//! The engine cannot make that split itself, because a sheet is prose — "dark
//! hair over one eye, a faint scar bisecting her left eyebrow, oversized flannel
//! over a tank top" is one paragraph containing several separate observations.
//!
//! So a model splits it, once, at session start rather than every turn. The
//! result is a set of observations that seed as already-known, while everything
//! else in the sheet stays unknown until the story discloses it. The model only
//! ever *extracts* here — it never decides who knows what, which is why this can
//! run without the compiler's authority machinery.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::soul::{KnowledgeEntry, KnowledgeStatus};

pub const SHEET_PASS_SCHEMA_NAME: &str = "mnemosyne_observable_sheet_v1";

/// What a model may return for one sheet.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ObservableSheetDraft {
    pub schema_version: u32,
    /// Each entry is one thing a stranger could notice on sight.
    pub observations: Vec<ObservableDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ObservableDraft {
    pub detail: String,
    pub category: ObservableCategory,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObservableCategory {
    /// Body and face: build, hair, scars, apparent age.
    Appearance,
    /// What they are wearing.
    Clothing,
    /// Carried or worn objects a stranger would notice.
    Equipment,
}

impl ObservableCategory {
    pub fn as_label(self) -> &'static str {
        match self {
            ObservableCategory::Appearance => "appearance",
            ObservableCategory::Clothing => "clothing",
            ObservableCategory::Equipment => "equipment",
        }
    }
}

pub fn observable_sheet_json_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "observations"],
        "properties": {
            "schema_version": { "type": "integer", "enum": [1] },
            "observations": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["detail", "category"],
                    "properties": {
                        "detail": { "type": "string", "minLength": 1 },
                        "category": {
                            "type": "string",
                            "enum": ["appearance", "clothing", "equipment"]
                        }
                    }
                }
            }
        }
    })
}

pub fn observable_sheet_prompt() -> &'static str {
    r#"Split a character sheet into things a stranger could notice on sight.

Include only what is visible in a first look: build, face, hair, scars, apparent age, clothing, and objects being carried or worn.

Exclude anything that has to be told or inferred: name, exact age, job, address, family, history, feelings, motives, relationships, skills, and anything the character is hiding.

Split compound sentences into separate observations. Keep each one short and concrete. Copy the sheet's own wording; invent nothing. If the sheet describes nothing visible, return an empty list."#
}

/// Turn extracted observations into knowledge rows — unknown until seen.
///
/// The catalogue of what *is* visible about a character is not the same as what
/// anyone has actually looked at. A session can open on a phone call, a radio,
/// or two people in different rooms, so these seed `Unaware` like everything
/// else; `disclosure::grant_sight_facts` opens them when the story says the
/// observer has laid eyes on the subject.
pub fn observations_to_knowledge(
    draft: &ObservableSheetDraft,
    observers: &[String],
    subject_label: &str,
    turn: u64,
) -> Vec<KnowledgeEntry> {
    let mut entries = Vec::new();
    for observation in &draft.observations {
        let detail = observation.detail.trim();
        if detail.is_empty() {
            continue;
        }
        let proposition = format!(
            "{}'s {}: {detail}",
            subject_label.trim(),
            observation.category.as_label()
        );
        for observer in observers {
            let observer = observer.trim();
            if observer.is_empty() {
                continue;
            }
            entries.push(KnowledgeEntry {
                knowledge_id: format!(
                    "knowledge:{}:{}",
                    observer.to_ascii_lowercase(),
                    proposition.to_ascii_lowercase()
                ),
                holder_entity_id: observer.to_string(),
                proposition: proposition.clone(),
                status: KnowledgeStatus::Unaware,
                counterpart_entity_id: None,
                actual_truth: None,
                established_turn: turn,
                // The sheet is the evidence. There is no turn to quote.
                evidence_quote: None,
                is_active: true,
                superseded_by_knowledge_id: None,
            });
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> ObservableSheetDraft {
        ObservableSheetDraft {
            schema_version: 1,
            observations: vec![
                ObservableDraft {
                    detail: "faint scar bisecting her left eyebrow".into(),
                    category: ObservableCategory::Appearance,
                },
                ObservableDraft {
                    detail: "oversized flannel over a tank top".into(),
                    category: ObservableCategory::Clothing,
                },
            ],
        }
    }

    #[test]
    fn observations_are_catalogued_but_not_yet_known() {
        // What is visible about someone is not what anyone has looked at. The
        // scene may open on a call, a radio, or two separate rooms.
        let observers = vec!["aurora".to_string(), "player-1".to_string()];

        let entries = observations_to_knowledge(&draft(), &observers, "the visitor", 3);

        assert_eq!(entries.len(), 4);
        assert!(entries
            .iter()
            .all(|entry| entry.status == KnowledgeStatus::Unaware));
        assert!(entries
            .iter()
            .any(|entry| entry.holder_entity_id == "player-1"
                && entry.proposition.contains("faint scar")));
    }

    #[test]
    fn meeting_opens_the_catalogued_observations() {
        let observers = vec!["aurora".to_string()];
        let mut entries = observations_to_knowledge(&draft(), &observers, "the visitor", 3);

        let opened = crate::disclosure::grant_sight_facts(&mut entries, "aurora", "the visitor", 4);

        assert_eq!(opened, 2);
        assert!(entries
            .iter()
            .all(|entry| entry.status == KnowledgeStatus::Knows));
    }

    #[test]
    fn an_empty_detail_creates_nothing() {
        let empty = ObservableSheetDraft {
            schema_version: 1,
            observations: vec![ObservableDraft {
                detail: "   ".into(),
                category: ObservableCategory::Appearance,
            }],
        };

        assert!(observations_to_knowledge(&empty, &["aurora".into()], "x", 1).is_empty());
    }

    #[test]
    fn every_schema_category_is_one_the_meeting_event_can_open() {
        // A category the sheet pass can emit but `grant_sight_facts` does not
        // recognise would be catalogued and then never become knowable.
        for category in [
            ObservableCategory::Appearance,
            ObservableCategory::Clothing,
            ObservableCategory::Equipment,
        ] {
            assert!(
                crate::disclosure::SIGHT_LEARNED_LABELS.contains(&category.as_label()),
                "{} has no way to ever be learned",
                category.as_label()
            );
        }
    }

    #[test]
    fn the_schema_admits_only_the_three_visible_categories() {
        let schema = observable_sheet_json_schema();
        let categories =
            &schema["properties"]["observations"]["items"]["properties"]["category"]["enum"];

        assert_eq!(
            categories.as_array().map(Vec::len),
            Some(3),
            "widening this set is how private history starts leaking in as 'visible'"
        );
    }

    #[test]
    fn the_prompt_refuses_the_parts_of_a_sheet_that_must_be_told() {
        let prompt = observable_sheet_prompt();

        for private in ["name", "job", "address", "family", "history", "motives"] {
            assert!(
                prompt.contains(private),
                "{private} must be named as excluded, or a model will happily call it visible"
            );
        }
    }
}
