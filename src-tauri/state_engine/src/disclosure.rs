//! Whether a fact has been said in the story yet, and what to call someone
//! whose name has not.
//!
//! Two ideas, both deliberately free of model inference:
//!
//! *Detection is a string question, not a judgement.* Whether a name was spoken
//! is answerable by scanning the visible transcript. The engine has it, so no
//! evaluator call and no user checkbox is needed for the common case, and the
//! answer is the same every time.
//!
//! *Withholding beats instructing.* A prompt that carries an undisclosed name
//! and forbids its use relies on the model obeying; a prompt that never carries
//! it cannot leak it. So an entity whose name has not been given is rendered as
//! a descriptor instead.

use serde::{Deserialize, Serialize};

use crate::context_compiler::ContextMessage;
use crate::soul::{KnowledgeEntry, KnowledgeStatus, Relationship};

/// The standard things one character can know about another.
///
/// A fixed vocabulary is what makes a checkbox grid possible at all: free text
/// cannot be a column. It also gives the engine a baseline to seed, so "not told
/// yet" is the starting state rather than an absence nobody recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterFactKind {
    Name,
    Age,
    Residence,
    Occupation,
    Family,
    Background,
    Appearance,
}

impl CharacterFactKind {
    pub const ALL: [CharacterFactKind; 7] = [
        CharacterFactKind::Name,
        CharacterFactKind::Age,
        CharacterFactKind::Residence,
        CharacterFactKind::Occupation,
        CharacterFactKind::Family,
        CharacterFactKind::Background,
        CharacterFactKind::Appearance,
    ];

    pub fn as_label(self) -> &'static str {
        match self {
            CharacterFactKind::Name => "name",
            CharacterFactKind::Age => "age",
            CharacterFactKind::Residence => "where they live",
            CharacterFactKind::Occupation => "occupation",
            CharacterFactKind::Family => "family",
            CharacterFactKind::Background => "background",
            CharacterFactKind::Appearance => "appearance",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_label().eq_ignore_ascii_case(label.trim()))
    }

    /// Whether this is learned by looking rather than by being told.
    ///
    /// This is a route, not a default. A session can open with the parties on a
    /// phone call, on a radio, in separate rooms, in the dark, or masked — so
    /// "visible" never means "already known". It means this is what opens when
    /// they actually meet, in one step, without anyone having to say it.
    pub fn learned_by_sight(self) -> bool {
        matches!(self, CharacterFactKind::Appearance)
    }

    /// The canonical proposition text, so a fact written by seeding, by the
    /// checkbox grid, and by the evaluator all address the same row.
    pub fn proposition_about(self, subject: &str) -> String {
        format!("{}'s {}", subject.trim(), self.as_label())
    }
}

/// How far along a relationship starts, expressed as what each side already
/// knows about the other.
///
/// Stage and knowledge are the same thing said twice: "strangers" *means* having
/// none of the other's facts, and "we've met" *means* holding the ones you learn
/// by looking. Keeping them as one concept stops a session opening as strangers
/// who somehow know each other's names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipStage {
    /// Never met. Includes voices on a phone or a radio.
    Strangers,
    /// Have laid eyes on each other, nothing said.
    Seen,
    /// Names exchanged.
    NameKnown,
    /// Enough small talk to place each other: name, where they live, what they do.
    Acquainted,
    /// The grid decides; the stage grants nothing on its own.
    Custom,
}

impl RelationshipStage {
    pub const ALL: [RelationshipStage; 5] = [
        RelationshipStage::Strangers,
        RelationshipStage::Seen,
        RelationshipStage::NameKnown,
        RelationshipStage::Acquainted,
        RelationshipStage::Custom,
    ];

    pub fn as_label(self) -> &'static str {
        match self {
            RelationshipStage::Strangers => "strangers",
            RelationshipStage::Seen => "seen",
            RelationshipStage::NameKnown => "name_known",
            RelationshipStage::Acquainted => "acquainted",
            RelationshipStage::Custom => "custom",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|stage| stage.as_label().eq_ignore_ascii_case(label.trim()))
    }

    /// Which facts each side starts holding. Cumulative: every stage grants what
    /// the one before it did.
    pub fn granted_facts(self) -> Vec<CharacterFactKind> {
        match self {
            RelationshipStage::Strangers | RelationshipStage::Custom => Vec::new(),
            RelationshipStage::Seen => vec![CharacterFactKind::Appearance],
            RelationshipStage::NameKnown => {
                vec![CharacterFactKind::Appearance, CharacterFactKind::Name]
            }
            RelationshipStage::Acquainted => vec![
                CharacterFactKind::Appearance,
                CharacterFactKind::Name,
                CharacterFactKind::Residence,
                CharacterFactKind::Occupation,
            ],
        }
    }

    /// Whether the sight-learned catalogue opens at this stage — the same
    /// question [`grant_sight_facts`] answers mid-session, asked up front.
    pub fn has_met(self) -> bool {
        !matches!(
            self,
            RelationshipStage::Strangers | RelationshipStage::Custom
        )
    }
}

/// Baseline knowledge for one observer about one subject: nothing.
///
/// Every fact starts `Unaware`, appearance included. Seeding appearance as known
/// would assume the two are looking at each other when the session opens, which
/// is false for a phone call, a radio, separate rooms, darkness, or a mask. The
/// engine cannot tell those apart, so it must not guess — [`grant_sight_facts`]
/// opens them when the story says they have met.
pub fn seed_baseline_knowledge(
    observer: &str,
    subject_label: &str,
    turn: u64,
    stage: RelationshipStage,
) -> Vec<KnowledgeEntry> {
    let granted = stage.granted_facts();
    CharacterFactKind::ALL
        .into_iter()
        .map(|kind| {
            let proposition = kind.proposition_about(subject_label);
            KnowledgeEntry {
                knowledge_id: format!(
                    "knowledge:{}:{}",
                    observer.trim().to_ascii_lowercase(),
                    proposition.to_ascii_lowercase()
                ),
                holder_entity_id: observer.trim().to_string(),
                proposition,
                status: if granted.contains(&kind) {
                    KnowledgeStatus::Knows
                } else {
                    KnowledgeStatus::Unaware
                },
                counterpart_entity_id: None,
                actual_truth: None,
                established_turn: turn,
                evidence_quote: None,
                is_active: true,
                superseded_by_knowledge_id: None,
            }
        })
        .collect()
}

/// Everything about a person that is learned by looking rather than by being
/// told.
///
/// Wider than [`CharacterFactKind`] because the sheet pass splits what is
/// visible into finer categories than the checkbox grid needs. Both write
/// propositions of the form "<subject>'s <label>", so one list keeps the grid,
/// the sheet pass, and the meeting event addressing the same rows.
pub const SIGHT_LEARNED_LABELS: [&str; 3] = ["appearance", "clothing", "equipment"];

/// Starting numeric relationship for a stage.
///
/// The old default opened every relationship with desire, respect, curiosity and
/// comfort already above zero, which reads as mild fondness — wrong for two
/// people who have never met. Strangers start flat; later stages start with only
/// the mild familiarity the stage actually implies.
pub fn starting_relationship(stage: RelationshipStage) -> Relationship {
    let (curiosity, comfort, respect) = match stage {
        RelationshipStage::Strangers | RelationshipStage::Custom => (0.0, 0.0, 0.0),
        RelationshipStage::Seen => (8.0, 0.0, 0.0),
        RelationshipStage::NameKnown => (12.0, 5.0, 5.0),
        RelationshipStage::Acquainted => (15.0, 12.0, 10.0),
    };
    Relationship {
        curiosity,
        comfort,
        respect,
        ..Relationship::default()
    }
}

/// Open the sight-learned facts one observer holds about one subject.
///
/// The single deliberate "they have now met" event. Facts that must be told are
/// untouched: meeting someone tells you nothing about their job or their family.
/// Returns how many rows changed, so a caller can tell a real first meeting from
/// a repeat.
pub fn grant_sight_facts(
    knowledge: &mut [KnowledgeEntry],
    observer: &str,
    subject_label: &str,
    turn: u64,
) -> usize {
    let observer = observer.trim();
    let sight_propositions = SIGHT_LEARNED_LABELS
        .iter()
        .map(|label| format!("{}'s {label}", subject_label.trim()).to_ascii_lowercase())
        .collect::<Vec<_>>();

    let mut changed = 0usize;
    for entry in knowledge.iter_mut() {
        if !entry.is_active
            || entry.status == KnowledgeStatus::Knows
            || !entry.holder_entity_id.trim().eq_ignore_ascii_case(observer)
        {
            continue;
        }
        let proposition = entry.proposition.to_ascii_lowercase();
        // Matches both the bare fact row and the sheet-pass detail rows, which
        // are prefixed with the same "<subject>'s appearance" phrase.
        let is_sight_fact = sight_propositions
            .iter()
            .any(|sight| proposition == *sight || proposition.starts_with(&format!("{sight}:")));
        if is_sight_fact {
            entry.status = KnowledgeStatus::Knows;
            entry.established_turn = turn;
            changed += 1;
        }
    }
    changed
}

/// Shortest token worth matching. Two-letter fragments collide with ordinary
/// words far too often to be evidence that a name was spoken.
const MIN_TOKEN_LEN: usize = 3;

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '-' || ch == '\''
}

/// Case-insensitive whole-word search. `haystack` and `needle` must already be
/// lowercase.
fn contains_word(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let mut from = 0usize;
    while let Some(found) = haystack[from..].find(needle) {
        let start = from + found;
        let end = start + needle.len();
        let before_ok = haystack[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !is_word_char(c));
        let after_ok = haystack[end..]
            .chars()
            .next()
            .is_none_or(|c| !is_word_char(c));
        if before_ok && after_ok {
            return true;
        }
        from = start + needle.len().max(1);
        if from >= haystack.len() {
            break;
        }
    }
    false
}

/// Has `value` — or any distinctive part of it — appeared in the visible
/// transcript?
///
/// A multi-word name counts as disclosed when any single part was said, because
/// "I'm Aurora" discloses "Aurora Schwarz" for every practical purpose. The
/// whole string is also matched so hyphenated handles like `Echo-0` work.
pub fn appears_in_transcript(messages: &[ContextMessage], value: &str) -> bool {
    let value = value.trim().to_lowercase();
    if value.len() < MIN_TOKEN_LEN {
        return false;
    }
    let mut needles = vec![value.clone()];
    needles.extend(
        value
            .split_whitespace()
            .map(str::to_string)
            .filter(|token| token.len() >= MIN_TOKEN_LEN),
    );

    messages.iter().any(|message| {
        let content = message.content.to_lowercase();
        needles.iter().any(|needle| contains_word(&content, needle))
    })
}

/// A stand-in for someone whose name has not been given, built from whatever the
/// engine legitimately knows: anyone in the room can see roughly who they are
/// without being told their name.
pub fn descriptor_for(gender_code: &str, fallback: &str) -> String {
    match gender_code.trim().to_ascii_lowercase().as_str() {
        "male" | "m" | "man" => "the man".into(),
        "female" | "f" | "woman" => "the woman".into(),
        _ if fallback.trim().is_empty() => "the other person".into(),
        _ => fallback.trim().to_string(),
    }
}

/// What to call an entity in compiled context.
///
/// Returns the name once it has been said in the story, and the descriptor
/// until then — with a parenthetical so the narrator knows the name exists and
/// is simply not yet known, rather than assuming the character is anonymous.
pub fn entity_display(
    name: &str,
    descriptor: &str,
    messages: &[ContextMessage],
    always_known: bool,
) -> String {
    let name = name.trim();
    if name.is_empty() {
        return descriptor.to_string();
    }
    if always_known || appears_in_transcript(messages, name) {
        return name.to_string();
    }
    format!("{descriptor} (name not yet given)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strangers_know_nothing_including_what_is_visible() {
        // A session can open on a phone call. Granting appearance would assume
        // the two are looking at each other, which the engine cannot know.
        let seeded =
            seed_baseline_knowledge("aurora", "the visitor", 1, RelationshipStage::Strangers);

        assert_eq!(seeded.len(), CharacterFactKind::ALL.len());
        assert!(seeded
            .iter()
            .all(|entry| entry.status == KnowledgeStatus::Unaware));
    }

    #[test]
    fn each_stage_grants_what_the_one_before_it_did() {
        let mut previous: Vec<CharacterFactKind> = Vec::new();
        for stage in [
            RelationshipStage::Strangers,
            RelationshipStage::Seen,
            RelationshipStage::NameKnown,
            RelationshipStage::Acquainted,
        ] {
            let granted = stage.granted_facts();
            for earlier in &previous {
                assert!(
                    granted.contains(earlier),
                    "{} dropped {:?}, so advancing a relationship would forget something",
                    stage.as_label(),
                    earlier
                );
            }
            previous = granted;
        }
    }

    #[test]
    fn having_met_is_the_same_question_the_stage_answers() {
        assert!(!RelationshipStage::Strangers.has_met());
        assert!(RelationshipStage::Seen.has_met());
        // Custom grants nothing on its own; the grid decides.
        assert!(!RelationshipStage::Custom.has_met());
        assert!(RelationshipStage::Custom.granted_facts().is_empty());
    }

    #[test]
    fn a_name_known_relationship_still_does_not_know_the_family() {
        let seeded =
            seed_baseline_knowledge("aurora", "the visitor", 1, RelationshipStage::NameKnown);

        let status = |suffix: &str| {
            seeded
                .iter()
                .find(|entry| entry.proposition.ends_with(suffix))
                .map(|entry| entry.status)
        };
        assert_eq!(status("name"), Some(KnowledgeStatus::Knows));
        assert_eq!(status("appearance"), Some(KnowledgeStatus::Knows));
        assert_eq!(status("family"), Some(KnowledgeStatus::Unaware));
        assert_eq!(status("background"), Some(KnowledgeStatus::Unaware));
    }

    #[test]
    fn strangers_start_without_manufactured_fondness() {
        // The old default opened every relationship with desire and comfort
        // already positive, which reads as having taken a liking to someone
        // never met.
        let flat = starting_relationship(RelationshipStage::Strangers);

        assert_eq!(flat.curiosity, 0.0);
        assert_eq!(flat.comfort, 0.0);
        assert_eq!(flat.desire, 0.0);
        assert!(starting_relationship(RelationshipStage::Acquainted).comfort > flat.comfort);
    }

    #[test]
    fn meeting_opens_only_what_is_learned_by_looking() {
        let mut seeded =
            seed_baseline_knowledge("aurora", "the visitor", 1, RelationshipStage::Strangers);

        let opened = grant_sight_facts(&mut seeded, "aurora", "the visitor", 5);

        assert_eq!(opened, 1, "appearance is the only sight-learned fact");
        for entry in &seeded {
            let expected = if entry.proposition.ends_with("appearance") {
                KnowledgeStatus::Knows
            } else {
                // Meeting someone tells you nothing about their job or family.
                KnowledgeStatus::Unaware
            };
            assert_eq!(entry.status, expected, "{}", entry.proposition);
        }
    }

    #[test]
    fn meeting_a_second_time_changes_nothing() {
        let mut seeded =
            seed_baseline_knowledge("aurora", "the visitor", 1, RelationshipStage::Strangers);
        grant_sight_facts(&mut seeded, "aurora", "the visitor", 5);

        assert_eq!(
            grant_sight_facts(&mut seeded, "aurora", "the visitor", 6),
            0
        );
    }

    #[test]
    fn one_observer_meeting_does_not_open_anothers_knowledge() {
        let mut seeded =
            seed_baseline_knowledge("aurora", "the visitor", 1, RelationshipStage::Strangers);
        seeded.extend(seed_baseline_knowledge(
            "bystander",
            "the visitor",
            1,
            RelationshipStage::Strangers,
        ));

        grant_sight_facts(&mut seeded, "aurora", "the visitor", 5);

        assert!(seeded
            .iter()
            .filter(|entry| entry.holder_entity_id == "bystander")
            .all(|entry| entry.status == KnowledgeStatus::Unaware));
    }

    #[test]
    fn a_fact_kind_round_trips_through_its_label() {
        for kind in CharacterFactKind::ALL {
            assert_eq!(CharacterFactKind::from_label(kind.as_label()), Some(kind));
        }
    }

    #[test]
    fn seeded_and_hand_edited_rows_address_the_same_knowledge() {
        // Seeding, the checkbox grid, and the evaluator must all land on one row
        // per fact, or a correction creates a duplicate instead of a change.
        let seeded =
            seed_baseline_knowledge("aurora", "the visitor", 1, RelationshipStage::Strangers);
        let by_hand = CharacterFactKind::Name.proposition_about("the visitor");

        assert!(seeded.iter().any(|entry| entry.proposition == by_hand));
    }

    fn msg(role: &str, content: &str) -> ContextMessage {
        ContextMessage {
            role: role.into(),
            content: content.into(),
        }
    }

    #[test]
    fn a_name_never_said_is_not_disclosed() {
        let messages = [msg("user", "I knock twice and wait under the overhang.")];

        assert!(!appears_in_transcript(&messages, "Echo-0"));
    }

    #[test]
    fn a_name_said_once_stays_disclosed() {
        let messages = [
            msg("user", "I knock twice."),
            msg(
                "assistant",
                "\"Echo-0,\" he says. \"That's what they call me.\"",
            ),
        ];

        assert!(appears_in_transcript(&messages, "Echo-0"));
    }

    #[test]
    fn a_first_name_discloses_the_full_name() {
        // "I'm Aurora" is a disclosure of Aurora Schwarz for every purpose the
        // narrator cares about.
        let messages = [msg("assistant", "\"Aurora,\" she says, and nothing more.")];

        assert!(appears_in_transcript(&messages, "Aurora Schwarz"));
    }

    #[test]
    fn a_name_inside_a_longer_word_is_not_a_disclosure() {
        let messages = [msg("assistant", "The rhythm of the rain does not let up.")];

        assert!(!appears_in_transcript(&messages, "Rhy"));
    }

    #[test]
    fn an_undisclosed_entity_is_shown_as_a_descriptor() {
        let messages = [msg("user", "I knock twice.")];

        let shown = entity_display("Rhy", "the man", &messages, false);

        assert_eq!(shown, "the man (name not yet given)");
        assert!(!shown.contains("Rhy"));
    }

    #[test]
    fn the_active_soul_keeps_its_own_name() {
        // A character always knows what she is called, whatever the transcript
        // has managed to say so far.
        let messages = [msg("user", "I knock twice.")];

        assert_eq!(
            entity_display("Aurora Schwarz", "the woman", &messages, true),
            "Aurora Schwarz"
        );
    }

    #[test]
    fn descriptors_come_from_what_anyone_in_the_room_could_see() {
        assert_eq!(descriptor_for("male", ""), "the man");
        assert_eq!(descriptor_for("female", ""), "the woman");
        assert_eq!(descriptor_for("", ""), "the other person");
    }
}
