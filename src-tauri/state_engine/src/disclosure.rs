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

use crate::context_compiler::ContextMessage;

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
