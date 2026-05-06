use std::{fs, path::PathBuf};

use rusqlite::Connection;
use tauri::{Emitter, State, Window};

use state_engine::{
    consolidation::consolidate_soul,
    context_compiler::{compile_context_for_messages, ContextMessage, ContextPreview},
    hidden_state::{apply_hidden_state, parse_hidden_state, HiddenState},
    setting::{new_default_setting, SettingSoul},
    soul::{new_default_soul, Soul},
};

use crate::{
    db::{self, ChatMessage, ProviderProfile, SettingSummary, SoulSummary},
    providers::{
        api::{build_system_prompt, ApiProvider, ApiProviderSettings},
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
    pub debug: TurnDebug,
}

#[derive(Debug, serde::Serialize)]
pub struct TurnDebug {
    pub provider: String,
    pub hidden_state_found: bool,
    pub fallback_hidden_state_generated: bool,
    pub tag: Option<String>,
    pub trust_delta: Option<f32>,
    pub affection_delta: Option<f32>,
    pub new_location: Option<String>,
    pub present_characters: Vec<String>,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct StreamChunk {
    pub conversation_id: String,
    pub chunk: String,
}

#[tauri::command]
pub fn create_default_soul(character_name: String) -> Soul {
    new_default_soul(&character_name)
}

#[tauri::command]
pub fn create_default_setting(setting_name: String) -> SettingSoul {
    new_default_setting(&setting_name)
}

#[tauri::command]
pub fn load_soul_file(path: String) -> Result<Soul, String> {
    let content = fs::read_to_string(PathBuf::from(path)).map_err(|err| err.to_string())?;
    serde_json::from_str(&content).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn load_setting_file(path: String) -> Result<SettingSoul, String> {
    let content = fs::read_to_string(PathBuf::from(path)).map_err(|err| err.to_string())?;
    serde_json::from_str(&content).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn save_soul_file(path: String, soul: Soul) -> Result<(), String> {
    let content = serde_json::to_string_pretty(&soul).map_err(|err| err.to_string())?;
    fs::write(PathBuf::from(path), content).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn save_setting_file(path: String, setting: SettingSoul) -> Result<(), String> {
    let content = serde_json::to_string_pretty(&setting).map_err(|err| err.to_string())?;
    fs::write(PathBuf::from(path), content).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_souls(state: State<'_, AppState>) -> Result<Vec<SoulSummary>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_souls(&conn).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_settings(state: State<'_, AppState>) -> Result<Vec<SettingSummary>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_settings(&conn).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn upsert_soul(state: State<'_, AppState>, soul: Soul) -> Result<SoulSummary, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::upsert_soul(&conn, &soul).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn upsert_setting(
    state: State<'_, AppState>,
    setting: SettingSoul,
) -> Result<SettingSummary, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::upsert_setting(&conn, &setting).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_soul(state: State<'_, AppState>, soul_id: String) -> Result<Soul, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_setting(state: State<'_, AppState>, setting_id: String) -> Result<SettingSoul, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::get_setting(&conn, &setting_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_soul(state: State<'_, AppState>, soul_id: String) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::delete_soul(&conn, &soul_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_setting(state: State<'_, AppState>, setting_id: String) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::delete_setting(&conn, &setting_id).map_err(|err| err.to_string())
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
pub fn delete_message(
    state: State<'_, AppState>,
    conversation_id: String,
    message_id: i64,
) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::delete_message(&conn, &conversation_id, message_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_provider_profiles(state: State<'_, AppState>) -> Result<Vec<ProviderProfile>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_provider_profiles(&conn).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_provider_profile(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<ProviderProfile, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::get_provider_profile(&conn, &profile_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn upsert_provider_profile(
    state: State<'_, AppState>,
    profile: ProviderProfile,
) -> Result<ProviderProfile, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::upsert_provider_profile(&conn, &profile).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_provider_profile(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::delete_provider_profile(&conn, &profile_id).map_err(|err| err.to_string())
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
    replacement_assistant_id: Option<i64>,
) -> Result<TurnResult, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    send_mock_turn_with_conn(
        &conn,
        conversation_id,
        soul_id,
        user_text,
        mode,
        replacement_assistant_id,
    )
}

fn send_mock_turn_with_conn(
    conn: &Connection,
    conversation_id: String,
    soul_id: String,
    user_text: String,
    mode: String,
    replacement_assistant_id: Option<i64>,
) -> Result<TurnResult, String> {
    let mut soul = db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())?;

    db::ensure_conversation(&conn, &conversation_id, &soul.character_id)
        .map_err(|err| err.to_string())?;
    if replacement_assistant_id.is_none() {
        db::insert_message(&conn, &conversation_id, "user", &user_text)
            .map_err(|err| err.to_string())?;
    }

    let before_messages = match replacement_assistant_id {
        Some(message_id) => db::list_messages_before_id(&conn, &conversation_id, message_id, 5),
        None => db::list_messages(&conn, &conversation_id, 5),
    }
    .map_err(|err| err.to_string())?;
    let context_preview =
        compile_context_for_messages(&soul, &messages_to_context(before_messages));
    let provider = MockProvider::default();
    let raw_response = provider.complete(&soul, &context_preview.text, &user_text, &mode);
    let parsed = parse_hidden_state(&raw_response).map_err(|err| err.to_string())?;
    let debug = debug_from_hidden_state("Mock", &parsed.hidden_state, true, false);

    parsed.apply_to_soul(&mut soul);
    soul.turn_counter += 1;
    soul.turns_since_consolidation += 1;
    if let Some(message_id) = replacement_assistant_id {
        let replaced =
            db::update_message_content(&conn, &conversation_id, message_id, &parsed.visible_text)
                .map_err(|err| err.to_string())?;
        if !replaced {
            return Err("Assistant message to regenerate was not found".into());
        }
    } else {
        db::insert_message(&conn, &conversation_id, "assistant", &parsed.visible_text)
            .map_err(|err| err.to_string())?;
    }

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
        debug,
    })
}

#[tauri::command]
pub async fn send_api_turn(
    window: Window,
    state: State<'_, AppState>,
    conversation_id: String,
    soul_id: String,
    user_text: String,
    mode: String,
    settings: ApiProviderSettings,
    replacement_assistant_id: Option<i64>,
) -> Result<TurnResult, String> {
    let (mut soul, context_preview) = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        let soul = db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())?;
        db::ensure_conversation(&conn, &conversation_id, &soul.character_id)
            .map_err(|err| err.to_string())?;
        if replacement_assistant_id.is_none() {
            db::insert_message(&conn, &conversation_id, "user", &user_text)
                .map_err(|err| err.to_string())?;
        }

        let before_messages = match replacement_assistant_id {
            Some(message_id) => db::list_messages_before_id(&conn, &conversation_id, message_id, 5),
            None => db::list_messages(&conn, &conversation_id, 5),
        }
        .map_err(|err| err.to_string())?;
        let context_preview =
            compile_context_for_messages(&soul, &messages_to_context(before_messages));
        (soul, context_preview)
    };

    let system_prompt = build_system_prompt(&settings, &soul, &context_preview.text, &mode);
    let provider = ApiProvider::default();
    let stream_conversation_id = conversation_id.clone();
    let raw_response = provider
        .complete_streaming(
            &settings,
            &system_prompt,
            &user_text,
            |chunk| {
                window
                    .emit(
                        "api-chunk",
                        StreamChunk {
                            conversation_id: stream_conversation_id.clone(),
                            chunk: chunk.to_string(),
                        },
                    )
                    .map_err(|err| err.to_string())
            },
        )
        .await?;
    let parsed = parse_hidden_state(&raw_response).map_err(|err| err.to_string())?;
    let hidden_state_found = !hidden_state_is_empty(&parsed.hidden_state);
    let fallback_hidden_state_generated = !hidden_state_found;
    let hidden_state = if fallback_hidden_state_generated {
        generated_api_hidden_state(&soul, &user_text, &parsed.visible_text)
    } else {
        parsed.hidden_state.clone()
    };
    let debug = debug_from_hidden_state(
        "API",
        &hidden_state,
        hidden_state_found,
        fallback_hidden_state_generated,
    );

    apply_hidden_state(&hidden_state, &mut soul);
    soul.turn_counter += 1;
    soul.turns_since_consolidation += 1;
    let visible_response = parsed.visible_text;

    let (messages, context_preview, consolidation_ran) = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        if let Some(message_id) = replacement_assistant_id {
            let replaced =
                db::update_message_content(&conn, &conversation_id, message_id, &visible_response)
                    .map_err(|err| err.to_string())?;
            if !replaced {
                return Err("Assistant message to regenerate was not found".into());
            }
        } else {
            db::insert_message(&conn, &conversation_id, "assistant", &visible_response)
                .map_err(|err| err.to_string())?;
        }

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
        debug,
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
        && hidden_state.new_location.is_none()
        && hidden_state.present_characters.is_none()
        && hidden_state.arousal_delta.is_none()
        && hidden_state.arousal_denied.is_none()
        && hidden_state.orgasm_allowed.is_none()
        && hidden_state.forced_orgasm.is_none()
}

fn debug_from_hidden_state(
    provider: &str,
    hidden_state: &HiddenState,
    hidden_state_found: bool,
    fallback_hidden_state_generated: bool,
) -> TurnDebug {
    TurnDebug {
        provider: provider.into(),
        hidden_state_found,
        fallback_hidden_state_generated,
        tag: hidden_state.tag.clone(),
        trust_delta: hidden_state.trust_delta,
        affection_delta: hidden_state.affection_delta,
        new_location: hidden_state.new_location.clone(),
        present_characters: hidden_state.present_characters.clone().unwrap_or_default(),
    }
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
        new_location: None,
        present_characters: Some(vec![soul.character_name.clone()]),
        arousal_delta: None,
        arousal_denied: None,
        orgasm_allowed: None,
        forced_orgasm: None,
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
    use state_engine::{context_compiler::estimate_tokens, hidden_state::HiddenState};

    #[test]
    fn hidden_state_application_updates_soul() {
        let mut soul = new_default_soul("Aurora");
        let state = HiddenState {
            memory: Some("Aurora notices a safer rhythm in the exchange.".into()),
            tag: Some("trust_building".into()),
            trust_delta: Some(4.0),
            affection_delta: Some(2.0),
            world_event: Some("A small trust-building exchange changed the mood.".into()),
            new_location: None,
            present_characters: None,
            arousal_delta: None,
            arousal_denied: None,
            orgasm_allowed: None,
            forced_orgasm: None,
        };

        state.apply_to_soul(&mut soul);

        assert_eq!(soul.relationships["user"].trust, 14.0);
        assert_eq!(soul.memory.recent.len(), 1);
        assert_eq!(soul.world.recent_events.len(), 1);
    }

    #[test]
    fn ten_mock_turns_trigger_consolidation_and_keep_context_lean() {
        let conn = db::init_memory_connection().expect("db");
        let soul = new_default_soul("Aurora");
        let soul_id = soul.character_id.clone();
        db::upsert_soul(&conn, &soul).expect("upsert soul");

        let turns = [
            "I promise this is safe.",
            "Look at the wall and the room.",
            "We remember childhood rain together.",
            "There is danger near the door.",
            "The light flickers without changing much.",
            "A neutral breath passes in the silence.",
            "Another quiet observation settles over the silence.",
            "One more observation keeps the scene grounded.",
            "Trust the route I found.",
            "Where are we now?",
        ];

        let mut final_result = None;
        for turn in turns {
            final_result = Some(
                send_mock_turn_with_conn(
                    &conn,
                    "acceptance".into(),
                    soul_id.clone(),
                    turn.into(),
                    "Reader".into(),
                    None,
                )
                .expect("mock turn"),
            );
        }

        let result = final_result.expect("result");
        assert!(result.consolidation_ran);
        assert_eq!(result.soul.turn_counter, 10);
        assert_eq!(result.soul.turns_since_consolidation, 0);
        assert!(result.soul.memory.recent.len() <= 4);
        assert!(result.soul.memory.core.len() > soul.memory.core.len());
        assert!(result
            .soul
            .memory
            .schemas
            .iter()
            .any(|schema| schema.schema_type == "observation"));
        assert!(!result
            .soul
            .memory
            .recent
            .iter()
            .any(|memory| memory.tag == "observation"));
        assert!(result.context_preview.estimated_tokens <= 2_000);
        assert!(estimate_tokens(&result.context_preview.text) <= 2_000);
    }
}
