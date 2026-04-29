use std::{fs, path::PathBuf};

use tauri::State;

use state_engine::{
    consolidation::consolidate_soul,
    context_compiler::{compile_context_for_messages, ContextMessage, ContextPreview},
    hidden_state::{parse_hidden_state, HiddenState},
    soul::{new_default_soul, Soul},
};

use crate::{
    db::{self, ChatMessage, SoulSummary},
    providers::{
        api::{ApiProvider, ApiProviderSettings},
        mock::MockProvider,
    },
    AppState,
};

const CONSOLIDATION_INTERVAL_TURNS: u64 = 10;

#[derive(Debug, serde::Serialize)]
pub struct TurnResult {
    pub conversation_id: String,
    pub soul: Soul,
    pub visible_response: String,
    pub context_preview: ContextPreview,
    pub messages: Vec<ChatMessage>,
    pub consolidation_ran: bool,
}

#[tauri::command]
pub fn create_default_soul(character_name: String) -> Soul {
    new_default_soul(&character_name)
}

#[tauri::command]
pub fn load_soul_file(path: String) -> Result<Soul, String> {
    let content = fs::read_to_string(PathBuf::from(path)).map_err(|err| err.to_string())?;
    serde_json::from_str(&content).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn save_soul_file(path: String, soul: Soul) -> Result<(), String> {
    let content = serde_json::to_string_pretty(&soul).map_err(|err| err.to_string())?;
    fs::write(PathBuf::from(path), content).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_souls(state: State<'_, AppState>) -> Result<Vec<SoulSummary>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_souls(&conn).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn upsert_soul(state: State<'_, AppState>, soul: Soul) -> Result<SoulSummary, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::upsert_soul(&conn, &soul).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_soul(state: State<'_, AppState>, soul_id: String) -> Result<Soul, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_soul(state: State<'_, AppState>, soul_id: String) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::delete_soul(&conn, &soul_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_conversation_messages(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<ChatMessage>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_conversation(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::delete_conversation(&conn, &conversation_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn compile_context(
    state: State<'_, AppState>,
    soul_id: String,
    conversation_id: String,
) -> Result<ContextPreview, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let soul = db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())?;
    let messages = db::list_messages(&conn, &conversation_id, 5).map_err(|err| err.to_string())?;
    Ok(compile_context_for_messages(
        &soul,
        &messages_to_context(messages),
    ))
}

#[tauri::command]
pub fn run_consolidation(state: State<'_, AppState>, soul_id: String) -> Result<Soul, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let mut soul = db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())?;
    consolidate_soul(&mut soul);
    db::upsert_soul(&conn, &soul).map_err(|err| err.to_string())?;
    Ok(soul)
}

#[tauri::command]
pub fn send_mock_turn(
    state: State<'_, AppState>,
    conversation_id: String,
    soul_id: String,
    user_text: String,
    mode: String,
) -> Result<TurnResult, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let mut soul = db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())?;

    db::ensure_conversation(&conn, &conversation_id, &soul.character_id)
        .map_err(|err| err.to_string())?;
    db::insert_message(&conn, &conversation_id, "user", &user_text)
        .map_err(|err| err.to_string())?;

    let before_messages =
        db::list_messages(&conn, &conversation_id, 5).map_err(|err| err.to_string())?;
    let context_preview =
        compile_context_for_messages(&soul, &messages_to_context(before_messages));
    let provider = MockProvider::default();
    let raw_response = provider.complete(&soul, &context_preview.text, &user_text, &mode);
    let parsed = parse_hidden_state(&raw_response).map_err(|err| err.to_string())?;

    parsed.apply_to_soul(&mut soul);
    soul.turn_counter += 1;
    soul.turns_since_consolidation += 1;
    db::insert_message(&conn, &conversation_id, "assistant", &parsed.visible_text)
        .map_err(|err| err.to_string())?;

    let consolidation_ran = soul.turns_since_consolidation >= CONSOLIDATION_INTERVAL_TURNS;
    if consolidation_ran {
        consolidate_soul(&mut soul);
    }

    db::upsert_soul(&conn, &soul).map_err(|err| err.to_string())?;
    let messages =
        db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())?;
    let context_preview =
        compile_context_for_messages(&soul, &messages_to_context(messages.clone()));

    Ok(TurnResult {
        conversation_id,
        soul,
        visible_response: parsed.visible_text,
        context_preview,
        messages,
        consolidation_ran,
    })
}

#[tauri::command]
pub async fn send_api_turn(
    state: State<'_, AppState>,
    conversation_id: String,
    soul_id: String,
    user_text: String,
    mode: String,
    settings: ApiProviderSettings,
) -> Result<TurnResult, String> {
    let (mut soul, context_preview) = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        let soul = db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())?;
        db::ensure_conversation(&conn, &conversation_id, &soul.character_id)
            .map_err(|err| err.to_string())?;
        db::insert_message(&conn, &conversation_id, "user", &user_text)
            .map_err(|err| err.to_string())?;

        let before_messages =
            db::list_messages(&conn, &conversation_id, 5).map_err(|err| err.to_string())?;
        let context_preview =
            compile_context_for_messages(&soul, &messages_to_context(before_messages));
        (soul, context_preview)
    };

    let provider = ApiProvider::default();
    let raw_response = provider
        .complete(&settings, &soul, &context_preview.text, &user_text, &mode)
        .await?;
    let parsed = parse_hidden_state(&raw_response).map_err(|err| err.to_string())?;
    let hidden_state = if hidden_state_is_empty(&parsed.hidden_state) {
        generated_api_hidden_state(&soul, &user_text, &parsed.visible_text)
    } else {
        parsed.hidden_state.clone()
    };

    hidden_state.apply_to_soul(&mut soul);
    soul.turn_counter += 1;
    soul.turns_since_consolidation += 1;
    let visible_response = parsed.visible_text;

    let (messages, context_preview, consolidation_ran) = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        db::insert_message(&conn, &conversation_id, "assistant", &visible_response)
            .map_err(|err| err.to_string())?;

        let consolidation_ran = soul.turns_since_consolidation >= CONSOLIDATION_INTERVAL_TURNS;
        if consolidation_ran {
            consolidate_soul(&mut soul);
        }

        db::upsert_soul(&conn, &soul).map_err(|err| err.to_string())?;
        let messages =
            db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())?;
        let context_preview =
            compile_context_for_messages(&soul, &messages_to_context(messages.clone()));

        (messages, context_preview, consolidation_ran)
    };

    Ok(TurnResult {
        conversation_id,
        soul,
        visible_response,
        context_preview,
        messages,
        consolidation_ran,
    })
}

fn messages_to_context(messages: Vec<ChatMessage>) -> Vec<ContextMessage> {
    messages
        .into_iter()
        .map(|message| ContextMessage {
            role: message.role,
            content: message.content,
        })
        .collect()
}

fn hidden_state_is_empty(hidden_state: &HiddenState) -> bool {
    hidden_state.memory.is_none()
        && hidden_state.tag.is_none()
        && hidden_state.trust_delta.is_none()
        && hidden_state.affection_delta.is_none()
        && hidden_state.world_event.is_none()
}

fn generated_api_hidden_state(soul: &Soul, user_text: &str, visible_text: &str) -> HiddenState {
    let tag = classify_turn_tag(user_text);
    let assistant_excerpt = visible_text.chars().take(180).collect::<String>();
    HiddenState {
        memory: Some(format!(
            "{} responded through the API provider after the user said: {} Assistant cue: {}",
            soul.character_name,
            user_text.trim(),
            assistant_excerpt.trim()
        )),
        tag: Some(tag.into()),
        trust_delta: Some(if tag == "trust_building" { 3.0 } else { 1.0 }),
        affection_delta: Some(if tag == "bonding" { 3.0 } else { 1.0 }),
        world_event: Some(format!(
            "The API-driven exchange moved around: {}",
            user_text.trim()
        )),
    }
}

fn classify_turn_tag(text: &str) -> &'static str {
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

#[cfg(test)]
mod tests {
    use super::*;
    use state_engine::hidden_state::HiddenState;

    #[test]
    fn hidden_state_application_updates_soul() {
        let mut soul = new_default_soul("Aurora");
        let state = HiddenState {
            memory: Some("Aurora notices a safer rhythm in the exchange.".into()),
            tag: Some("trust_building".into()),
            trust_delta: Some(4.0),
            affection_delta: Some(2.0),
            world_event: Some("A small trust-building exchange changed the mood.".into()),
        };

        state.apply_to_soul(&mut soul);

        assert_eq!(soul.relationships["user"].trust, 14.0);
        assert_eq!(soul.memory.recent.len(), 1);
        assert_eq!(soul.world.recent_events.len(), 1);
    }
}
