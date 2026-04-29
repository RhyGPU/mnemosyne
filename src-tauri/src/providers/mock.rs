use state_engine::soul::Soul;

#[derive(Debug, Default)]
pub struct MockProvider;

impl MockProvider {
    pub fn complete(&self, soul: &Soul, _context: &str, user_text: &str) -> String {
        let trimmed = user_text.trim();
        let tag = classify_tag(trimmed);
        let response = format!(
            "{} listens closely, letting the moment settle before answering. \"{}\"",
            soul.character_name,
            reflective_line(trimmed)
        );
        let memory = format!(
            "{} responded to the user's turn: {}",
            soul.character_name, trimmed
        );
        let world_event = format!("The exchange shifted around: {}", trimmed);

        format!(
            "{response}\n\n[HIDDEN_STATE]\n{}",
            serde_json::json!({
                "memory": memory,
                "tag": tag,
                "trust_delta": if tag == "trust_building" { 3 } else { 1 },
                "affection_delta": if tag == "bonding" { 3 } else { 1 },
                "world_event": world_event
            })
        )
    }
}

fn classify_tag(text: &str) -> &'static str {
    let lower = text.to_lowercase();
    if lower.contains("trust") || lower.contains("promise") || lower.contains("safe") {
        "trust_building"
    } else if lower.contains("hurt") || lower.contains("blood") || lower.contains("danger") {
        "threat"
    } else if lower.contains("remember")
        || lower.contains("childhood")
        || lower.contains("together")
    {
        "bonding"
    } else if lower.contains("where") || lower.contains("look") || lower.contains("room") {
        "orientation"
    } else {
        "observation"
    }
}

fn reflective_line(text: &str) -> String {
    if text.is_empty() {
        "Say that again, slower.".into()
    } else if text.ends_with('?') {
        "I do not know yet, but I can feel which answer would change us.".into()
    } else {
        "I heard you. That matters more than I expected.".into()
    }
}
