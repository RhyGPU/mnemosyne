use super::*;

#[tauri::command]
pub fn create_default_soul(character_name: String) -> Soul {
    new_default_soul(&character_name)
}

#[tauri::command]
pub fn create_fresh_scenario_soul(
    state: State<'_, AppState>,
    soul_id: String,
    _setting_id: Option<String>,
) -> Result<Soul, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let base = db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())?;
    let fresh = session_soul_from_savepoint(&base);
    db::upsert_soul(&conn, &fresh).map_err(|err| err.to_string())?;
    Ok(fresh)
}

#[tauri::command]
pub fn create_session_soul_from_savepoint(
    state: State<'_, AppState>,
    source_soul_id: String,
    setting_id: Option<String>,
    title: Option<String>,
) -> Result<SessionStartResult, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let source = db::get_soul(&conn, &source_soul_id).map_err(|err| err.to_string())?;
    let session = session_soul_from_savepoint(&source);
    let session_world = if let Some(setting_id) = setting_id
        .as_deref()
        .map(str::trim)
        .filter(|setting_id| !setting_id.is_empty())
    {
        db::create_session_world_from_setting(&conn, setting_id).map_err(|err| err.to_string())?
    } else {
        db::create_legacy_session_world_from_soul(&conn, &source).map_err(|err| err.to_string())?
    };
    db::upsert_soul(&conn, &session).map_err(|err| err.to_string())?;
    let conversation_id =
        conversation_id_for_session(Some(&session_world.world_id), &session.character_id);
    let default_title = format!("{} Session", source.character_name.trim());
    let title = title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .unwrap_or(&default_title);
    db::ensure_conversation_with_title_and_world(
        &conn,
        &conversation_id,
        &session.character_id,
        Some(&session_world.world_id),
        session_world.source_setting_id.as_deref(),
        Some(title),
    )
    .map_err(|err| err.to_string())?;
    db::create_session_branch(&conn, &conversation_id, &session, &session_world)
        .map_err(|err| err.to_string())?;
    let opening = session.profile.opening_narrator_message.trim();
    seed_opening_narrator_message(&conn, &conversation_id, opening)
        .map_err(|err| err.to_string())?;
    let conversation =
        db::get_conversation_summary(&conn, &conversation_id).map_err(|err| err.to_string())?;
    let messages =
        db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())?;
    Ok(SessionStartResult {
        soul: session,
        conversation,
        messages,
    })
}

pub(crate) fn seed_opening_narrator_message(
    conn: &Connection,
    conversation_id: &str,
    opening: &str,
) -> rusqlite::Result<Option<i64>> {
    let opening = opening.trim();
    if opening.is_empty() || !db::list_messages(conn, conversation_id, 1)?.is_empty() {
        return Ok(None);
    }
    let message_id = db::insert_message_and_get_id(conn, conversation_id, "assistant", opening)?;
    db::seed_initial_assistant_message_variant(
        conn,
        conversation_id,
        message_id,
        opening,
        Some("opening_seed"),
        None,
        None,
    )?;
    Ok(Some(message_id))
}

fn conversation_id_for_session(setting_id: Option<&str>, session_soul_id: &str) -> String {
    match setting_id.map(str::trim).filter(|id| !id.is_empty()) {
        Some(setting_id) => format!("local-mock-{setting_id}-{session_soul_id}"),
        None => format!("local-mock-{session_soul_id}"),
    }
}

#[tauri::command]
pub fn save_session_as_new_soul(
    state: State<'_, AppState>,
    session_soul_id: String,
    name: String,
    soul_kind: Option<String>,
) -> Result<Soul, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let session = db::get_soul(&conn, &session_soul_id).map_err(|err| err.to_string())?;
    let kind = soul_kind.as_deref().unwrap_or("checkpoint");
    let name = name.trim();
    let name = if name.is_empty() {
        format!("{} Checkpoint", session.character_name)
    } else {
        name.to_string()
    };
    let savepoint = soul_savepoint_from_session(&session, &name, kind);
    db::upsert_soul(&conn, &savepoint).map_err(|err| err.to_string())?;
    Ok(savepoint)
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
pub fn save_soul_file(app: AppHandle, path: String, soul: Soul) -> Result<(), String> {
    let content = serde_json::to_string_pretty(&soul).map_err(|err| err.to_string())?;
    let path = resolve_export_path(&app, &path, "soul.json")?;
    fs::write(path, content).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn save_setting_file(app: AppHandle, path: String, setting: SettingSoul) -> Result<(), String> {
    let content = serde_json::to_string_pretty(&setting).map_err(|err| err.to_string())?;
    let path = resolve_export_path(&app, &path, "setting.json")?;
    fs::write(path, content).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_souls(state: State<'_, AppState>) -> Result<Vec<SoulSummary>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_souls(&conn).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_souls_debug(state: State<'_, AppState>) -> Result<Vec<SoulSummary>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_souls_including_session_clones(&conn).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_conversations(state: State<'_, AppState>) -> Result<Vec<ConversationSummary>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_conversations(&conn).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn touch_conversation_access(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<ConversationSummary, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::touch_conversation_access(&conn, &conversation_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_session_state_hub(
    state: State<'_, AppState>,
) -> Result<Vec<SessionStateHubItem>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let mut conversations = db::list_conversations(&conn).map_err(|err| err.to_string())?;
    conversations.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    conversations.truncate(STATE_MAP_RECENT_SESSION_LIMIT);
    let mut items = Vec::with_capacity(conversations.len());

    for conversation in conversations {
        let branch = match db::get_active_session_branch(&conn, &conversation.conversation_id) {
            Ok(branch) => branch,
            Err(rusqlite::Error::QueryReturnedNoRows) => continue,
            Err(err) => return Err(err.to_string()),
        };
        let rebuilt =
            db::rebuild_session_state(&conn, &conversation.conversation_id, &branch.branch_id)
                .map_err(|err| err.to_string())?;
        let soul = rebuilt.soul;
        let world = rebuilt.session_world;
        let positive_relationship_count = soul
            .relationships
            .values()
            .filter(|relationship| {
                relationship.trust > 35.0
                    || relationship.affection > 35.0
                    || relationship.intimacy > 35.0
                    || relationship.comfort > 35.0
                    || relationship.desire > 35.0
            })
            .count();
        let memory_count =
            soul.memory.core.len() + soul.memory.recent.len() + soul.memory.schemas.len();
        items.push(SessionStateHubItem {
            conversation,
            soul_name: soul.character_name,
            setting_name: world.setting_name,
            location: world.location,
            time_elapsed: world.time_elapsed,
            current_scene: world.scene_state.current_scene,
            focus: world.scene_state.focus,
            turn_counter: soul.turn_counter,
            memory_count,
            core_memory_count: soul.memory.core.len(),
            recent_memory_count: soul.memory.recent.len(),
            schema_count: soul.memory.schemas.len(),
            relationship_count: soul.relationships.len(),
            positive_relationship_count,
            object_count: world.object_states.len().max(world.key_objects.len()),
            event_count: world
                .recent_event_records
                .len()
                .max(world.recent_events.len()),
            active_plot_count: world.active_plots.len(),
        });
    }

    Ok(items)
}

#[tauri::command]
pub fn list_session_state_map(state: State<'_, AppState>) -> Result<SessionStateMap, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let mut conversations = db::list_conversations(&conn).map_err(|err| err.to_string())?;
    conversations.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    conversations.truncate(STATE_MAP_RECENT_SESSION_LIMIT);
    let mut sessions = Vec::with_capacity(conversations.len());
    let mut scenes = Vec::new();
    let mut characters = Vec::new();
    let mut relationships = Vec::new();
    let mut objects = Vec::new();
    let mut timeline = Vec::new();
    let mut memories = Vec::new();
    let mut memory_v2 = Vec::new();

    for conversation in conversations {
        let branch = match db::get_active_session_branch(&conn, &conversation.conversation_id) {
            Ok(branch) => branch,
            Err(rusqlite::Error::QueryReturnedNoRows) => continue,
            Err(err) => return Err(err.to_string()),
        };
        let rebuilt =
            db::rebuild_session_state(&conn, &conversation.conversation_id, &branch.branch_id)
                .map_err(|err| err.to_string())?;
        let soul = rebuilt.soul;
        let world = rebuilt.session_world;
        let session_id = conversation.conversation_id.clone();
        let session_title = if conversation.title.trim().is_empty() {
            "Untitled session".to_string()
        } else {
            conversation.title.clone()
        };
        let soul_name = soul.character_name.clone();
        for record in db::list_memory_v2_projection(
            &conn,
            &conversation.conversation_id,
            &branch.branch_id,
            true,
        )
        .map_err(|err| err.to_string())?
        {
            let source_memory_ids =
                serde_json::from_str::<Vec<String>>(&record.source_memory_ids_json)
                    .unwrap_or_default();
            let supporting_evidence_count =
                serde_json::from_str::<Vec<serde_json::Value>>(&record.supporting_evidence_json)
                    .map(|values| values.len())
                    .unwrap_or_default();
            let contradicting_evidence_count =
                serde_json::from_str::<Vec<serde_json::Value>>(&record.contradicting_evidence_json)
                    .map(|values| values.len())
                    .unwrap_or_default();
            memory_v2.push(StateMapMemoryV2Item {
                session_id: session_id.clone(),
                session_title: session_title.clone(),
                memory_id: record.memory_id,
                layer: record.layer,
                memory_kind: record.memory_kind,
                validity: record.validity,
                content: record.content,
                confidence: record.confidence,
                truth_status: record.truth_status,
                source_patch_id: record.source_patch_id,
                source_turn_id: record.source_turn_id,
                source_quote: record.source_quote,
                source_memory_ids,
                supporting_evidence_count,
                contradicting_evidence_count,
            });
        }
        let positive_relationship_count = soul
            .relationships
            .values()
            .filter(|relationship| {
                relationship.trust > 35.0
                    || relationship.affection > 35.0
                    || relationship.intimacy > 35.0
                    || relationship.comfort > 35.0
                    || relationship.desire > 35.0
            })
            .count();
        let memory_count =
            soul.memory.core.len() + soul.memory.recent.len() + soul.memory.schemas.len();

        scenes.push(StateMapSceneItem {
            session_id: session_id.clone(),
            session_title: session_title.clone(),
            soul_name: soul_name.clone(),
            setting_name: world.setting_name.clone(),
            turn_counter: soul.turn_counter,
            location: world.location.clone(),
            time_elapsed: world.time_elapsed.clone(),
            current_scene: world.scene_state.current_scene.clone(),
            focus: world.scene_state.focus.clone(),
            last_user_action: world.scene_state.last_user_action.clone(),
            pressure_point: world.scene_state.pressure_point.clone(),
        });

        characters.push(StateMapCharacterItem {
            session_id: session_id.clone(),
            session_title: session_title.clone(),
            name: soul_name.clone(),
            role: soul.soul_kind.clone(),
            detail: format!(
                "phase {} / resolve {} / openness {}",
                soul.trauma.phase, soul.global.resolve, soul.global.openness
            ),
        });

        for (target, relationship) in &soul.relationships {
            characters.push(StateMapCharacterItem {
                session_id: session_id.clone(),
                session_title: session_title.clone(),
                name: target.clone(),
                role: if relationship.love_type.trim().is_empty() {
                    "relationship".to_string()
                } else {
                    relationship.love_type.clone()
                },
                detail: format!(
                    "trust {} / fear {} / affection {}",
                    relationship.trust, relationship.fear, relationship.affection
                ),
            });
            relationships.push(StateMapRelationshipItem {
                session_id: session_id.clone(),
                session_title: session_title.clone(),
                soul_name: soul_name.clone(),
                target: target.clone(),
                love_type: relationship.love_type.clone(),
                trust: relationship.trust,
                affection: relationship.affection,
                intimacy: relationship.intimacy,
                fear: relationship.fear,
                desire: relationship.desire,
            });
        }

        for object in &world.object_states {
            let summary = object
                .contents_summary
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| object.last_observed_state.clone());
            objects.push(StateMapObjectItem {
                session_id: session_id.clone(),
                session_title: session_title.clone(),
                name: object.object_id.clone(),
                kind: object.object_kind.clone(),
                owner: object
                    .owner_entity_id
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                location: object.location.clone(),
                status: object.status.clone(),
                summary,
                confidence: object.confidence,
            });
        }
        for object in &world.key_objects {
            if !objects
                .iter()
                .any(|item| item.session_id == session_id && item.name == *object)
            {
                objects.push(StateMapObjectItem {
                    session_id: session_id.clone(),
                    session_title: session_title.clone(),
                    name: object.clone(),
                    kind: "key_object".to_string(),
                    owner: "unknown".to_string(),
                    location: world.location.clone(),
                    status: "tracked".to_string(),
                    summary: object.clone(),
                    confidence: 1.0,
                });
            }
        }

        if world.recent_event_records.is_empty() {
            for event in &world.recent_events {
                timeline.push(StateMapTimelineItem {
                    session_id: session_id.clone(),
                    session_title: session_title.clone(),
                    turn_counter: soul.turn_counter,
                    content: event.clone(),
                });
            }
        } else {
            for event in &world.recent_event_records {
                if event.is_active {
                    timeline.push(StateMapTimelineItem {
                        session_id: session_id.clone(),
                        session_title: session_title.clone(),
                        turn_counter: soul.turn_counter,
                        content: event.content.clone(),
                    });
                }
            }
        }

        for memory in &soul.memory.recent {
            memories.push(StateMapMemoryItem {
                session_id: session_id.clone(),
                session_title: session_title.clone(),
                soul_name: soul_name.clone(),
                content: memory.content.clone(),
                tag: memory.tag.clone(),
                source_turn: memory.source_message_id,
                confidence: memory.confidence,
                truth_status: memory.truth_status.as_label().to_string(),
                source_type: memory.source_type.as_label().to_string(),
                is_pinned: memory.is_pinned,
                is_active: memory.is_active,
            });
        }
        for memory in &soul.memory.core {
            memories.push(StateMapMemoryItem {
                session_id: session_id.clone(),
                session_title: session_title.clone(),
                soul_name: soul_name.clone(),
                content: memory.clone(),
                tag: "core".to_string(),
                source_turn: None,
                confidence: None,
                truth_status: "persistent_core".to_string(),
                source_type: "persistent_core".to_string(),
                is_pinned: false,
                is_active: true,
            });
        }

        sessions.push(SessionStateHubItem {
            conversation,
            soul_name,
            setting_name: world.setting_name,
            location: world.location,
            time_elapsed: world.time_elapsed,
            current_scene: world.scene_state.current_scene,
            focus: world.scene_state.focus,
            turn_counter: soul.turn_counter,
            memory_count,
            core_memory_count: soul.memory.core.len(),
            recent_memory_count: soul.memory.recent.len(),
            schema_count: soul.memory.schemas.len(),
            relationship_count: soul.relationships.len(),
            positive_relationship_count,
            object_count: world.object_states.len().max(world.key_objects.len()),
            event_count: world
                .recent_event_records
                .len()
                .max(world.recent_events.len()),
            active_plot_count: world.active_plots.len(),
        });
    }

    memories.sort_by_key(|memory| (!memory.is_pinned, !memory.is_active));

    Ok(SessionStateMap {
        sessions,
        scenes,
        characters,
        relationships,
        objects,
        timeline,
        memories,
        memory_v2,
    })
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlayerPersonaInput {
    pub persona_id: Option<String>,
    pub display_name: String,
    pub description: String,
    pub gender_code: String,
    pub pronouns: String,
    pub appearance: Option<String>,
    pub voice_style: Option<String>,
    pub boundaries: Option<String>,
    pub notes: Option<String>,
}

#[tauri::command]
pub fn list_player_personas(state: State<'_, AppState>) -> Result<Vec<PlayerPersona>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_player_personas(&conn).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_archived_player_personas(
    state: State<'_, AppState>,
) -> Result<Vec<PlayerPersona>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_archived_player_personas(&conn).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_active_player_persona(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<PlayerPersona, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::get_active_player_persona(&conn, &conversation_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn set_active_player_persona(
    state: State<'_, AppState>,
    conversation_id: String,
    persona_id: String,
) -> Result<PlayerPersona, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::set_active_player_persona(&conn, &conversation_id, &persona_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn upsert_player_persona(
    state: State<'_, AppState>,
    input: PlayerPersonaInput,
) -> Result<PlayerPersona, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let display_name = required_persona_field("display_name", &input.display_name)?;
    let description = required_persona_field("description", &input.description)?;
    let gender_code = required_persona_field("gender_code", &input.gender_code)?;
    let pronouns = required_persona_field("pronouns", &input.pronouns)?;
    let persona_id = input
        .persona_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("persona_{}", slugify_for_id(&display_name)));
    if db::built_in_player_personas()
        .iter()
        .any(|persona| persona.persona_id == persona_id)
    {
        return Err("Built-in personas cannot be edited.".into());
    }
    let now = db::now_ts();
    db::upsert_player_persona(
        &conn,
        &PlayerPersona {
            persona_id,
            display_name,
            description,
            gender_code,
            pronouns,
            is_builtin: false,
            is_archived: false,
            created_at: now,
            updated_at: now,
            appearance: clean_optional(input.appearance),
            voice_style: clean_optional(input.voice_style),
            boundaries: clean_optional(input.boundaries),
            notes: clean_optional(input.notes),
        },
    )
    .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn archive_player_persona(
    state: State<'_, AppState>,
    persona_id: String,
) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::archive_player_persona(&conn, &persona_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn restore_player_persona(
    state: State<'_, AppState>,
    persona_id: String,
) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::restore_player_persona(&conn, &persona_id).map_err(|err| err.to_string())
}

fn required_persona_field(name: &str, value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(format!("Persona {name} is required."))
    } else {
        Ok(trimmed.chars().take(500).collect())
    }
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().chars().take(1_000).collect::<String>())
        .filter(|value| !value.is_empty())
}

fn slugify_for_id(value: &str) -> String {
    let slug = value
        .trim()
        .to_ascii_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if slug.is_empty() {
        format!("custom_{}", uuid_like_id())
    } else {
        slug
    }
}

pub(super) fn player_persona_context(persona: &PlayerPersona) -> PlayerPersonaContext {
    PlayerPersonaContext {
        persona_id: persona.persona_id.clone(),
        display_name: persona.display_name.clone(),
        gender_code: persona.gender_code.clone(),
        pronouns: persona.pronouns.clone(),
        description: persona.description.clone(),
    }
}

#[tauri::command]
pub fn rename_conversation(
    state: State<'_, AppState>,
    conversation_id: String,
    title: String,
    soul_id: Option<String>,
) -> Result<ConversationSummary, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    match db::rename_conversation(&conn, &conversation_id, &title) {
        Ok(conversation) => Ok(conversation),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            let soul_id = soul_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "Conversation not found".to_string())?;
            db::ensure_conversation_with_title(&conn, &conversation_id, soul_id, Some(&title))
                .map_err(|err| err.to_string())
        }
        Err(err) => Err(err.to_string()),
    }
}

#[tauri::command]
pub fn import_image_asset(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    path: String,
    linked_soul_id: Option<String>,
    linked_conversation_id: Option<String>,
    linked_message_id: Option<i64>,
    source: Option<String>,
) -> Result<ImageAsset, String> {
    let source = normalize_image_source(source.as_deref())?;
    let source_path = PathBuf::from(path);
    let source_name = source_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("image")
        .to_string();
    emit_dev_log(
        &window,
        "info",
        "db",
        "image_import_started",
        Some(serde_json::json!({ "source": source, "file": source_name })),
    );

    let result = import_image_asset_inner(
        &app,
        &state,
        &source_path,
        linked_soul_id,
        linked_conversation_id,
        linked_message_id,
        &source,
    );
    match &result {
        Ok(asset) => emit_dev_log(
            &window,
            "success",
            "db",
            "image_import_success",
            Some(serde_json::json!({
                "image_asset_id": asset.id,
                "source": asset.source,
                "stored_file": Path::new(&asset.file_path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("image")
            })),
        ),
        Err(err) => emit_dev_log(
            &window,
            "error",
            "db",
            "image_import_failed",
            Some(serde_json::json!({ "file": source_name, "error": err })),
        ),
    }
    result
}

fn import_image_asset_inner(
    app: &AppHandle,
    state: &State<'_, AppState>,
    source_path: &Path,
    linked_soul_id: Option<String>,
    linked_conversation_id: Option<String>,
    linked_message_id: Option<i64>,
    source: &str,
) -> Result<ImageAsset, String> {
    let bytes = fs::read(source_path).map_err(|err| format!("Image read failed: {err}"))?;
    import_image_asset_bytes_inner(
        app,
        state,
        &bytes,
        linked_soul_id,
        linked_conversation_id,
        linked_message_id,
        source,
    )
}

fn import_image_asset_bytes_inner(
    app: &AppHandle,
    state: &State<'_, AppState>,
    bytes: &[u8],
    linked_soul_id: Option<String>,
    linked_conversation_id: Option<String>,
    linked_message_id: Option<i64>,
    source: &str,
) -> Result<ImageAsset, String> {
    let info = inspect_image_bytes(bytes)?;
    let mut images_dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    images_dir.push("images");
    fs::create_dir_all(images_dir.join("thumbnails")).map_err(|err| err.to_string())?;
    let id = uuid_like_id();
    let file_name = format!("{id}.{}", info.extension);
    let target_path = images_dir.join(file_name);
    fs::write(&target_path, bytes).map_err(|err| format!("Image copy failed: {err}"))?;

    let asset = ImageAsset {
        id,
        file_path: target_path.display().to_string(),
        thumbnail_path: None,
        source: source.to_string(),
        mime_type: Some(info.mime_type.to_string()),
        width: info.width,
        height: info.height,
        prompt: None,
        provider: None,
        model: None,
        linked_soul_id,
        linked_conversation_id,
        linked_message_id,
        created_at: db::now_ts(),
    };
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let saved = db::upsert_image_asset(&conn, &asset).map_err(|err| err.to_string())?;
    if let Some(message_id) = saved.linked_message_id {
        db::attach_image_to_message(&conn, message_id, &saved.id).map_err(|err| err.to_string())?;
    }
    Ok(saved)
}

#[tauri::command]
pub fn import_image_asset_bytes(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    file_name: String,
    data_base64: String,
    linked_soul_id: Option<String>,
    linked_conversation_id: Option<String>,
    linked_message_id: Option<i64>,
    source: Option<String>,
) -> Result<ImageAsset, String> {
    let source = normalize_image_source(source.as_deref())?;
    emit_dev_log(
        &window,
        "info",
        "db",
        "image_import_started",
        Some(serde_json::json!({ "source": source, "file": safe_image_log_name(&file_name) })),
    );
    let decoded = general_purpose::STANDARD
        .decode(data_base64)
        .map_err(|err| format!("Image decode failed: {err}"));
    let result = decoded.and_then(|bytes| {
        import_image_asset_bytes_inner(
            &app,
            &state,
            &bytes,
            linked_soul_id,
            linked_conversation_id,
            linked_message_id,
            &source,
        )
    });
    match &result {
        Ok(asset) => emit_dev_log(
            &window,
            "success",
            "db",
            "image_import_success",
            Some(serde_json::json!({
                "image_asset_id": asset.id,
                "source": asset.source,
                "stored_file": Path::new(&asset.file_path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("image")
            })),
        ),
        Err(err) => emit_dev_log(
            &window,
            "error",
            "db",
            "image_import_failed",
            Some(serde_json::json!({ "file": safe_image_log_name(&file_name), "error": err })),
        ),
    }
    result
}

#[tauri::command]
pub fn create_user_image_message(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    conversation_id: String,
    path: String,
    content: Option<String>,
) -> Result<Vec<ChatMessage>, String> {
    emit_dev_log(
        &window,
        "info",
        "db",
        "image_import_started",
        Some(serde_json::json!({ "conversation_id": conversation_id })),
    );
    let message_id = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        let content = content
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("[Image]");
        db::insert_message_and_get_id(&conn, &conversation_id, "user", content)
            .map_err(|err| err.to_string())?
    };
    let asset = import_image_asset_inner(
        &app,
        &state,
        &PathBuf::from(path),
        None,
        Some(conversation_id.clone()),
        Some(message_id),
        "uploaded",
    );
    match asset {
        Ok(asset) => {
            emit_dev_log(
                &window,
                "success",
                "db",
                "chat_image_attached",
                Some(serde_json::json!({
                    "conversation_id": conversation_id,
                    "message_id": message_id,
                    "image_asset_id": asset.id
                })),
            );
            let conn = state.conn.lock().map_err(|err| err.to_string())?;
            db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())
        }
        Err(err) => {
            let conn = state.conn.lock().map_err(|lock_err| lock_err.to_string())?;
            let _ = db::hard_delete_message_internal(&conn, &conversation_id, message_id);
            emit_dev_log(
                &window,
                "error",
                "db",
                "image_import_failed",
                Some(serde_json::json!({ "conversation_id": conversation_id, "error": err })),
            );
            Err(err)
        }
    }
}

#[tauri::command]
pub fn create_user_image_message_bytes(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    conversation_id: String,
    file_name: String,
    data_base64: String,
    content: Option<String>,
) -> Result<Vec<ChatMessage>, String> {
    emit_dev_log(
        &window,
        "info",
        "db",
        "image_import_started",
        Some(serde_json::json!({
            "conversation_id": conversation_id,
            "file": safe_image_log_name(&file_name)
        })),
    );
    let message_id = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        let content = content
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("[Image]");
        db::insert_message_and_get_id(&conn, &conversation_id, "user", content)
            .map_err(|err| err.to_string())?
    };
    let result = general_purpose::STANDARD
        .decode(data_base64)
        .map_err(|err| format!("Image decode failed: {err}"))
        .and_then(|bytes| {
            import_image_asset_bytes_inner(
                &app,
                &state,
                &bytes,
                None,
                Some(conversation_id.clone()),
                Some(message_id),
                "uploaded",
            )
        });
    match result {
        Ok(asset) => {
            emit_dev_log(
                &window,
                "success",
                "db",
                "chat_image_attached",
                Some(serde_json::json!({
                    "conversation_id": conversation_id,
                    "message_id": message_id,
                    "image_asset_id": asset.id
                })),
            );
            let conn = state.conn.lock().map_err(|err| err.to_string())?;
            db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())
        }
        Err(err) => {
            let conn = state.conn.lock().map_err(|lock_err| lock_err.to_string())?;
            let _ = db::hard_delete_message_internal(&conn, &conversation_id, message_id);
            emit_dev_log(
                &window,
                "error",
                "db",
                "image_import_failed",
                Some(serde_json::json!({
                    "conversation_id": conversation_id,
                    "file": safe_image_log_name(&file_name),
                    "error": err
                })),
            );
            Err(err)
        }
    }
}

#[tauri::command]
pub fn get_image_asset(
    state: State<'_, AppState>,
    image_asset_id: String,
) -> Result<ImageAsset, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::get_image_asset(&conn, &image_asset_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_image_asset_data_url(
    state: State<'_, AppState>,
    image_asset_id: String,
) -> Result<String, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let asset = db::get_image_asset(&conn, &image_asset_id).map_err(|err| err.to_string())?;
    let bytes = fs::read(&asset.file_path).map_err(|err| format!("Image read failed: {err}"))?;
    let info = inspect_image_bytes(&bytes)?;
    let mime_type = asset.mime_type.as_deref().unwrap_or(info.mime_type);
    Ok(format!(
        "data:{mime_type};base64,{}",
        general_purpose::STANDARD.encode(bytes)
    ))
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
pub fn clear_soul_world_state(
    window: Window,
    state: State<'_, AppState>,
    soul_id: String,
) -> Result<Soul, String> {
    repair_soul_section(window, state, soul_id, "world_state", |soul| {
        soul.world = Default::default();
    })
}

#[tauri::command]
pub fn clear_soul_profile_scenario(
    window: Window,
    state: State<'_, AppState>,
    soul_id: String,
) -> Result<Soul, String> {
    repair_soul_section(window, state, soul_id, "profile_scenario", |soul| {
        soul.profile.scenario.clear();
    })
}

#[tauri::command]
pub fn clear_soul_recent_events(
    window: Window,
    state: State<'_, AppState>,
    soul_id: String,
) -> Result<Soul, String> {
    repair_soul_section(window, state, soul_id, "recent_events", |soul| {
        soul.world.recent_events.clear();
    })
}

#[tauri::command]
pub fn clear_soul_memories(
    window: Window,
    state: State<'_, AppState>,
    soul_id: String,
) -> Result<Soul, String> {
    repair_soul_section(window, state, soul_id, "memories", |soul| {
        soul.memory.core.clear();
        soul.memory.recent.clear();
        soul.memory.schemas.clear();
    })
}

fn repair_soul_section<F>(
    window: Window,
    state: State<'_, AppState>,
    soul_id: String,
    section: &str,
    repair: F,
) -> Result<Soul, String>
where
    F: FnOnce(&mut Soul),
{
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let mut soul = db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())?;
    repair(&mut soul);
    soul.last_updated = db::now_ts();
    db::upsert_soul(&conn, &soul).map_err(|err| err.to_string())?;
    emit_dev_log(
        &window,
        "warn",
        "repair",
        "soul_debug_repair_applied",
        Some(serde_json::json!({
            "active_soul_id": soul.character_id.as_str(),
            "section": section
        })),
    );
    Ok(soul)
}

#[tauri::command]
pub fn get_setting(state: State<'_, AppState>, setting_id: String) -> Result<SettingSoul, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::get_setting(&conn, &setting_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_soul(_state: State<'_, AppState>, _soul_id: String) -> Result<bool, String> {
    Err("delete_soul is deprecated; use archive_soul with session safety guards.".into())
}

#[tauri::command]
pub fn archive_soul(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    soul_id: String,
) -> Result<bool, String> {
    create_safety_backup(&app, &window, "archive_soul")?;
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::archive_soul(&conn, &soul_id).map_err(|err| err.to_string())
}

/// Permanent hard delete of a character (Soul). Irreversible — takes a safety
/// backup first. Archive (archive_soul) is the recoverable default; this is the
/// explicit "purge" path.
#[tauri::command]
pub fn purge_soul(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    soul_id: String,
) -> Result<bool, String> {
    create_safety_backup(&app, &window, "purge_soul")?;
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::hard_delete_soul_internal(&conn, &soul_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn restore_soul(state: State<'_, AppState>, soul_id: String) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::restore_soul(&conn, &soul_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_archived_souls(state: State<'_, AppState>) -> Result<Vec<db::SoulSummary>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_archived_souls(&conn).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn archive_savepoint(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    soul_id: String,
) -> Result<bool, String> {
    create_safety_backup(&app, &window, "archive_savepoint")?;
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::archive_savepoint(&conn, &soul_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn restore_savepoint(state: State<'_, AppState>, soul_id: String) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::restore_savepoint(&conn, &soul_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_archived_savepoints(
    state: State<'_, AppState>,
) -> Result<Vec<db::SoulSummary>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_archived_savepoints(&conn).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_setting(_state: State<'_, AppState>, _setting_id: String) -> Result<bool, String> {
    Err(
        "delete_setting is deprecated; use archive_setting with active/default setting guard."
            .into(),
    )
}

#[tauri::command]
pub fn archive_setting(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    setting_id: String,
    active_or_default_ids: Vec<String>,
) -> Result<bool, String> {
    create_safety_backup(&app, &window, "archive_setting")?;
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let active_refs: Vec<&str> = active_or_default_ids.iter().map(|s| s.as_str()).collect();
    db::archive_setting(&conn, &setting_id, &active_refs)
}

/// Permanent hard delete of a world (Setting). Irreversible — safety backup
/// first, and refuses to purge the active/default setting.
#[tauri::command]
pub fn purge_setting(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    setting_id: String,
    active_or_default_ids: Vec<String>,
) -> Result<bool, String> {
    if active_or_default_ids.contains(&setting_id) {
        return Err("Cannot purge the active/default setting. Switch settings first.".into());
    }
    create_safety_backup(&app, &window, "purge_setting")?;
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::delete_setting_internal(&conn, &setting_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn restore_setting(state: State<'_, AppState>, setting_id: String) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::restore_setting(&conn, &setting_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_archived_settings(
    state: State<'_, AppState>,
) -> Result<Vec<db::SettingSummary>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_archived_settings(&conn).map_err(|err| err.to_string())
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
    _state: State<'_, AppState>,
    _conversation_id: String,
) -> Result<bool, String> {
    Err("delete_conversation is deprecated; use archive_session.".into())
}

#[tauri::command]
pub fn delete_message(
    state: State<'_, AppState>,
    conversation_id: String,
    message_id: i64,
) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::deactivate_downstream_from_message(&conn, &conversation_id, message_id)
        .map_err(|err| err.to_string())?;
    if let Ok(branch) = db::get_active_session_branch(&conn, &conversation_id) {
        db::rebuild_session_state(&conn, &conversation_id, &branch.branch_id)
            .map_err(|err| err.to_string())?;
    }
    Ok(true)
}

pub(super) struct PhoneContradictionGuard {
    pub(super) text: String,
    pub(super) repaired: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct RestoreTurnsResult {
    pub messages: Vec<ChatMessage>,
    pub preview: RestoreInactiveMessagesResult,
}

#[tauri::command]
pub fn restore_inactive_messages(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<RestoreTurnsResult, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let preview =
        db::restore_inactive_messages(&conn, &conversation_id).map_err(|err| err.to_string())?;
    if let Ok(branch) = db::get_active_session_branch(&conn, &conversation_id) {
        db::rebuild_session_state(&conn, &conversation_id, &branch.branch_id)
            .map_err(|err| err.to_string())?;
    }
    let messages =
        db::list_messages(&conn, &conversation_id, 500).map_err(|err| err.to_string())?;
    Ok(RestoreTurnsResult { messages, preview })
}

fn reveal_path_in_file_manager(path: &Path) -> Result<(), String> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn()
            .map_err(|err| format!("Failed to open Explorer: {}", err))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()
            .map_err(|err| format!("Failed to open Finder: {}", err))?;
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let open_path = path
            .parent()
            .filter(|parent| parent.exists())
            .unwrap_or(path);
        std::process::Command::new("xdg-open")
            .arg(open_path)
            .spawn()
            .map_err(|err| format!("Failed to open file manager: {}", err))?;
    }

    Ok(())
}

#[tauri::command]
pub fn open_session_data_location(app: AppHandle) -> Result<String, String> {
    let db_path = db::connection_path(&app).map_err(|err| err.to_string())?;
    reveal_path_in_file_manager(&db_path)?;
    Ok(db_path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn create_backup(app: AppHandle, _state: State<'_, AppState>) -> Result<String, String> {
    let db_path = db::connection_path(&app).map_err(|err| err.to_string())?;
    let mut backup_dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    backup_dir.push("backups");
    let backup_path = db::create_backup_file(&db_path, &backup_dir)?;
    Ok(backup_path.to_string_lossy().to_string())
}

pub(crate) fn create_safety_backup(
    app: &AppHandle,
    window: &Window,
    operation: &str,
) -> Result<String, String> {
    let db_path = db::connection_path(app).map_err(|err| err.to_string())?;
    let mut backup_dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    backup_dir.push("backups");
    let backup_path = db::create_backup_file(&db_path, &backup_dir)?;
    let backup_path = backup_path.to_string_lossy().to_string();
    emit_dev_log(
        window,
        "success",
        "backup",
        "automatic_safety_backup_created",
        Some(serde_json::json!({
            "operation": operation,
            "backup_path": backup_path
        })),
    );
    Ok(backup_path)
}

#[tauri::command]
pub fn archive_session(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<bool, String> {
    create_safety_backup(&app, &window, "archive_session")?;
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::archive_session(&conn, &conversation_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn restore_session(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::restore_session(&conn, &conversation_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_archived_sessions(
    state: State<'_, AppState>,
) -> Result<Vec<ConversationSummary>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_archived_sessions(&conn).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn hide_turn_range(
    state: State<'_, AppState>,
    conversation_id: String,
    start_message_id: i64,
    end_message_id: i64,
) -> Result<usize, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let count = db::hide_turn_range(&conn, &conversation_id, start_message_id, end_message_id)
        .map_err(|err| err.to_string())?;
    if let Ok(branch) = db::get_active_session_branch(&conn, &conversation_id) {
        db::rebuild_session_state(&conn, &conversation_id, &branch.branch_id)
            .map_err(|err| err.to_string())?;
    }
    Ok(count)
}

#[tauri::command]
pub fn hide_latest_benchmark_failed_user_message(
    state: State<'_, AppState>,
    conversation_id: String,
    user_text: String,
) -> Result<Option<i64>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let hidden_id = db::hide_latest_matching_active_user_tail(&conn, &conversation_id, &user_text)
        .map_err(|err| err.to_string())?;
    if hidden_id.is_some() {
        if let Ok(branch) = db::get_active_session_branch(&conn, &conversation_id) {
            db::rebuild_session_state(&conn, &conversation_id, &branch.branch_id)
                .map_err(|err| err.to_string())?;
        }
    }
    Ok(hidden_id)
}

#[tauri::command]
pub fn restore_turn_range(
    state: State<'_, AppState>,
    conversation_id: String,
    start_message_id: i64,
    end_message_id: i64,
) -> Result<usize, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let count = db::restore_turn_range(&conn, &conversation_id, start_message_id, end_message_id)
        .map_err(|err| err.to_string())?;
    if let Ok(branch) = db::get_active_session_branch(&conn, &conversation_id) {
        db::rebuild_session_state(&conn, &conversation_id, &branch.branch_id)
            .map_err(|err| err.to_string())?;
    }
    Ok(count)
}

#[tauri::command]
pub fn list_hidden_turns(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<ChatMessage>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_hidden_turns(&conn, &conversation_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn dedupe_active_adjacent_user_messages(
    window: Window,
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<db::DedupeAdjacentUserMessagesResult, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let result = db::dedupe_active_adjacent_user_messages(&conn, &conversation_id)
        .map_err(|err| err.to_string())?;
    if let Ok(branch) = db::get_active_session_branch(&conn, &conversation_id) {
        let rebuilt = db::rebuild_session_state(&conn, &conversation_id, &branch.branch_id)
            .map_err(|err| err.to_string())?;
        emit_dev_log(
            &window,
            "success",
            "ledger",
            "session_world_cache_refreshed_from_ledger",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "branch_id": rebuilt.debug.branch_id,
                "active_turn_id": rebuilt.debug.active_turn_id,
                "reason": "dedupe_active_adjacent_user_messages"
            })),
        );
    }
    emit_dev_log(
        &window,
        "success",
        "db",
        "dedupe_active_adjacent_user_messages",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str(),
            "hidden_duplicate_user_message_ids": result.hidden_duplicate_user_message_ids.as_slice(),
            "canonical_user_message_ids": result.canonical_user_message_ids.as_slice()
        })),
    );
    Ok(result)
}

#[tauri::command]
pub fn update_user_message(
    state: State<'_, AppState>,
    conversation_id: String,
    message_id: i64,
    content: String,
) -> Result<Vec<ChatMessage>, String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err("User message cannot be empty".into());
    }
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let updated = db::update_user_message_content(&conn, &conversation_id, message_id, trimmed)
        .map_err(|err| err.to_string())?;
    if !updated {
        return Err("User message not found".into());
    }
    if let Ok(branch) = db::get_active_session_branch(&conn, &conversation_id) {
        db::deactivate_downstream_from_message(&conn, &conversation_id, message_id)
            .map_err(|err| err.to_string())?;
        conn.execute(
            "UPDATE messages SET is_active = 1 WHERE conversation_id = ?1 AND id = ?2",
            rusqlite::params![conversation_id, message_id],
        )
        .map_err(|err| err.to_string())?;
        db::rebuild_session_state(&conn, &conversation_id, &branch.branch_id)
            .map_err(|err| err.to_string())?;
    }
    db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_assistant_message_variants(
    state: State<'_, AppState>,
    conversation_id: String,
    message_id: i64,
) -> Result<Vec<AssistantMessageVariant>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_assistant_message_variants(&conn, &conversation_id, message_id)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn select_assistant_message_variant(
    state: State<'_, AppState>,
    conversation_id: String,
    message_id: i64,
    variant_id: i64,
) -> Result<VariantSelectionResult, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::select_assistant_message_variant(&conn, &conversation_id, message_id, variant_id)
        .map_err(|err| err.to_string())?;
    if db::has_session_branch(&conn, &conversation_id).map_err(|err| err.to_string())? {
        if let Some(branch_id) = db::activate_variant_commit(&conn, &conversation_id, variant_id)
            .map_err(|err| err.to_string())?
        {
            db::rebuild_session_state(&conn, &conversation_id, &branch_id)
                .map_err(|err| err.to_string())?;
        }
    }
    Ok(VariantSelectionResult {
        variants: db::list_assistant_message_variants(&conn, &conversation_id, message_id)
            .map_err(|err| err.to_string())?,
        messages: db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())?,
    })
}

#[tauri::command]
pub fn delete_assistant_message_variant(
    state: State<'_, AppState>,
    conversation_id: String,
    message_id: i64,
    variant_id: i64,
) -> Result<VariantSelectionResult, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::delete_assistant_message_variant(&conn, &conversation_id, message_id, variant_id)
        .map_err(|err| err.to_string())?;
    if let Ok(branch) = db::get_active_session_branch(&conn, &conversation_id) {
        if let Some(commit) = db::get_turn_commit_by_assistant(&conn, &conversation_id, message_id)
            .map_err(|err| err.to_string())?
        {
            if commit.selected_variant_id == Some(variant_id) {
                db::discard_active_commits_for_assistant(&conn, &conversation_id, message_id)
                    .map_err(|err| err.to_string())?;
            }
        }
        db::rebuild_session_state(&conn, &conversation_id, &branch.branch_id)
            .map_err(|err| err.to_string())?;
    }
    Ok(VariantSelectionResult {
        variants: db::list_assistant_message_variants(&conn, &conversation_id, message_id)
            .map_err(|err| err.to_string())?,
        messages: db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())?,
    })
}

#[tauri::command]
pub fn inspect_turn_branch_integrity(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<serde_json::Value, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    inspect_turn_branch_integrity_with_conn(&conn, &conversation_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn repair_accidental_normal_send_variants(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<serde_json::Value, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    repair_accidental_normal_send_variants_with_conn(&conn, &conversation_id)
}

pub(crate) fn repair_accidental_normal_send_variants_with_conn(
    conn: &Connection,
    conversation_id: &str,
) -> Result<serde_json::Value, String> {
    let mut repaired = Vec::new();
    let mut stmt = conn
        .prepare(
            "
            SELECT message_id
            FROM assistant_message_variants
            WHERE conversation_id = ?1 AND COALESCE(is_discarded, 0) = 0
            GROUP BY message_id
            HAVING COUNT(*) > 1
            ",
        )
        .map_err(|err| err.to_string())?;
    let message_ids = stmt
        .query_map([conversation_id], |row| row.get::<_, i64>(0))
        .map_err(|err| err.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|err| err.to_string())?;
    drop(stmt);

    for message_id in message_ids {
        let variants = all_variant_rows_for_message(conn, conversation_id, message_id)
            .map_err(|err| err.to_string())?;
        let visible = variants
            .iter()
            .filter(|variant| !variant.is_discarded)
            .collect::<Vec<_>>();
        let has_user_requested_variant = visible.iter().any(|variant| {
            let source = variant.source.as_deref();
            source == Some(OP_REGENERATE)
                || source == Some(OP_FIX_RESPONSE)
                || source == Some("model_switch_retry")
        });
        if has_user_requested_variant || visible.len() <= 1 {
            continue;
        }
        let selected_id = visible
            .iter()
            .find(|variant| variant.is_selected)
            .or_else(|| visible.last())
            .and_then(|variant| variant.id);
        let Some(selected_id) = selected_id else {
            continue;
        };
        let selected_content = visible
            .iter()
            .find(|variant| variant.id == Some(selected_id))
            .map(|variant| variant.content.clone())
            .unwrap_or_default();
        for variant in visible {
            if variant.id == Some(selected_id) {
                continue;
            }
            let source = variant.source.as_deref();
            if variant.content == selected_content
                || source == Some("original")
                || source == Some("api_provider")
                || source == Some(OP_NORMAL_SEND)
                || source == Some(OP_BASELINE_PATCH)
                || source == Some(OP_ENRICHMENT_PATCH)
            {
                if let Some(id) = variant.id {
                    conn.execute(
                        "UPDATE assistant_message_variants SET is_discarded = 1, is_selected = 0 WHERE id = ?1",
                        [id],
                    )
                    .map_err(|err| err.to_string())?;
                    repaired.push(serde_json::json!({
                        "message_id": message_id,
                        "discarded_variant_id": id,
                        "kept_variant_id": selected_id,
                        "reason": "accidental_normal_send_duplicate"
                    }));
                }
            }
        }
        conn.execute(
            "UPDATE assistant_message_variants SET is_selected = 1, is_discarded = 0 WHERE id = ?1",
            [selected_id],
        )
        .map_err(|err| err.to_string())?;
    }

    let inspection = inspect_turn_branch_integrity_with_conn(conn, conversation_id)
        .map_err(|err| err.to_string())?;
    Ok(serde_json::json!({
        "conversation_id": conversation_id,
        "repaired": repaired,
        "inspection": inspection
    }))
}

#[derive(Debug, Clone)]
struct VariantIntegrityRow {
    id: Option<i64>,
    content: String,
    source: Option<String>,
    is_selected: bool,
    is_discarded: bool,
}

fn all_variant_rows_for_message(
    conn: &Connection,
    conversation_id: &str,
    message_id: i64,
) -> rusqlite::Result<Vec<VariantIntegrityRow>> {
    let mut stmt = conn.prepare(
        "
        SELECT id, content, source, is_selected, COALESCE(is_discarded, 0)
        FROM assistant_message_variants
        WHERE conversation_id = ?1 AND message_id = ?2
        ORDER BY id ASC
        ",
    )?;
    let rows = stmt.query_map(rusqlite::params![conversation_id, message_id], |row| {
        Ok(VariantIntegrityRow {
            id: Some(row.get(0)?),
            content: row.get(1)?,
            source: row.get(2)?,
            is_selected: row.get::<_, i64>(3)? != 0,
            is_discarded: row.get::<_, i64>(4)? != 0,
        })
    })?;
    rows.collect()
}

pub(crate) fn inspect_turn_branch_integrity_with_conn(
    conn: &Connection,
    conversation_id: &str,
) -> rusqlite::Result<serde_json::Value> {
    let messages = db::list_messages(conn, conversation_id, 100_000)?;
    let active_user_messages = messages
        .iter()
        .filter(|message| message.role == "user")
        .map(|message| {
            serde_json::json!({
                "message_id": message.id,
                "content": message.content,
                "created_at": message.created_at,
                "status": message.status,
                "origin": message.origin
            })
        })
        .collect::<Vec<_>>();
    let mut assistant_pairs = Vec::new();
    for (index, message) in messages.iter().enumerate() {
        if message.role != "user" {
            continue;
        }
        let assistant = messages
            .iter()
            .skip(index + 1)
            .find(|candidate| candidate.role == "assistant");
        assistant_pairs.push(serde_json::json!({
            "user_message_id": message.id,
            "assistant_message_id": assistant.map(|assistant| assistant.id),
            "assistant_preview": assistant.map(|assistant| head_tail_excerpt_chars(&assistant.content, 80, 0, 80))
        }));
    }

    let turn_commits = query_json_rows(
        conn,
        "
        SELECT turn_id, parent_turn_id, user_message_id, assistant_message_id, state_patch_id, selected_variant_id, branch_id, active_variant, is_active, is_discarded, is_regenerated_variant
        FROM turn_commits
        WHERE conversation_id = ?1
        ORDER BY created_at ASC
        ",
        conversation_id,
        |row| {
            Ok(serde_json::json!({
                "turn_id": row.get::<_, String>(0)?,
                "parent_turn_id": row.get::<_, Option<String>>(1)?,
                "user_message_id": row.get::<_, Option<i64>>(2)?,
                "assistant_message_id": row.get::<_, Option<i64>>(3)?,
                "state_patch_id": row.get::<_, Option<String>>(4)?,
                "selected_variant_id": row.get::<_, Option<i64>>(5)?,
                "branch_id": row.get::<_, String>(6)?,
                "active_variant": row.get::<_, i64>(7)? != 0,
                "is_active": row.get::<_, i64>(8)? != 0,
                "is_discarded": row.get::<_, i64>(9)? != 0,
                "is_regenerated_variant": row.get::<_, i64>(10)? != 0
            }))
        },
    )?;
    let assistant_variants = query_json_rows(
        conn,
        "
        SELECT id, message_id, content, source, is_selected, COALESCE(is_discarded, 0), turn_id, state_patch_id
        FROM assistant_message_variants
        WHERE conversation_id = ?1
        ORDER BY message_id ASC, id ASC
        ",
        conversation_id,
        |row| {
            Ok(serde_json::json!({
                "variant_id": row.get::<_, i64>(0)?,
                "message_id": row.get::<_, i64>(1)?,
                "content_hash": stable_debug_hash(row.get::<_, String>(2)?.as_str()),
                "source": row.get::<_, Option<String>>(3)?,
                "is_selected": row.get::<_, i64>(4)? != 0,
                "is_discarded": row.get::<_, i64>(5)? != 0,
                "turn_id": row.get::<_, Option<String>>(6)?,
                "state_patch_id": row.get::<_, Option<String>>(7)?
            }))
        },
    )?;
    let session_branches = query_json_rows(
        conn,
        "
        SELECT branch_id, active_turn_id, is_active, rebuild_generation
        FROM session_branches
        WHERE conversation_id = ?1
        ORDER BY created_at ASC
        ",
        conversation_id,
        |row| {
            Ok(serde_json::json!({
                "branch_id": row.get::<_, String>(0)?,
                "active_turn_id": row.get::<_, Option<String>>(1)?,
                "is_active": row.get::<_, i64>(2)? != 0,
                "rebuild_generation": row.get::<_, i64>(3)?
            }))
        },
    )?;
    let active_turn_id = db::get_active_session_branch(conn, conversation_id)
        .ok()
        .and_then(|branch| branch.active_turn_id);
    let mut visible_variant_counts = Vec::new();
    let mut suspected_duplicate_branch_causes = Vec::new();
    for message in messages
        .iter()
        .filter(|message| message.role == "assistant")
    {
        let variants = all_variant_rows_for_message(conn, conversation_id, message.id)?;
        let visible = variants
            .iter()
            .filter(|variant| !variant.is_discarded)
            .collect::<Vec<_>>();
        visible_variant_counts.push(serde_json::json!({
            "assistant_message_id": message.id,
            "visible_variant_count": visible.len(),
            "variant_ids": visible.iter().filter_map(|variant| variant.id).collect::<Vec<_>>(),
            "sources": visible.iter().map(|variant| variant.source.clone()).collect::<Vec<_>>()
        }));
        let only_normal_sources = visible.iter().all(|variant| {
            let source = variant.source.as_deref();
            source.is_none()
                || source == Some("original")
                || source == Some("api_provider")
                || source == Some(OP_NORMAL_SEND)
                || source == Some("opening_seed")
        });
        if visible.len() > 1 && only_normal_sources {
            suspected_duplicate_branch_causes.push(serde_json::json!({
                "assistant_message_id": message.id,
                "cause": "normal_send_created_multiple_visible_variants",
                "variant_ids": visible.iter().filter_map(|variant| variant.id).collect::<Vec<_>>(),
                "sources": visible.iter().map(|variant| variant.source.clone()).collect::<Vec<_>>()
            }));
        }
    }

    Ok(serde_json::json!({
        "conversation_id": conversation_id,
        "active_turn_id": active_turn_id,
        "active_user_messages": active_user_messages,
        "assistant_pairs": assistant_pairs,
        "turn_commits": turn_commits,
        "assistant_variants": assistant_variants,
        "session_branches": session_branches,
        "visible_variant_counts": visible_variant_counts,
        "suspected_duplicate_branch_causes": suspected_duplicate_branch_causes
    }))
}

fn query_json_rows<F>(
    conn: &Connection,
    sql: &str,
    conversation_id: &str,
    mut map_row: F,
) -> rusqlite::Result<Vec<serde_json::Value>>
where
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<serde_json::Value>,
{
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([conversation_id], |row| map_row(row))?;
    rows.collect()
}

fn stable_debug_hash(content: &str) -> String {
    let mut hash = 5381u32;
    for byte in content.trim().as_bytes() {
        hash = hash.wrapping_mul(33) ^ (*byte as u32);
    }
    format!("h{hash:08x}")
}

#[tauri::command]
pub fn list_llm_payload_logs(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<LlmPayloadLog>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_llm_payload_logs(&conn, &conversation_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_llm_payload_log(
    state: State<'_, AppState>,
    log_id: i64,
) -> Result<LlmPayloadLog, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::get_llm_payload_log(&conn, log_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_branch_patch_debug(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<db::BranchPatchDebug, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let branch =
        db::get_active_session_branch(&conn, &conversation_id).map_err(|err| err.to_string())?;
    let rebuilt = db::rebuild_session_state(&conn, &conversation_id, &branch.branch_id)
        .map_err(|err| err.to_string())?;
    Ok(rebuilt.debug)
}

#[tauri::command]
pub fn rebuild_session_from_ledger(
    window: Window,
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<db::BranchPatchDebug, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let branch =
        db::get_active_session_branch(&conn, &conversation_id).map_err(|err| err.to_string())?;
    let rebuilt = db::rebuild_session_state(&conn, &conversation_id, &branch.branch_id)
        .map_err(|err| err.to_string())?;
    emit_dev_log(
        &window,
        "success",
        "ledger",
        "session_world_cache_refreshed_from_ledger",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str(),
            "branch_id": rebuilt.debug.branch_id,
            "active_turn_id": rebuilt.debug.active_turn_id,
            "reason": "rebuild_session_from_ledger"
        })),
    );
    Ok(rebuilt.debug)
}

#[tauri::command]
pub fn export_visible_chat_log(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<ExportResult, String> {
    let messages = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        db::list_messages(&conn, &conversation_id, 10_000).map_err(|err| err.to_string())?
    };
    let markdown = render_visible_chat_log(&messages);
    let path = write_export_file(&app, &conversation_id, "visible-chat-log", &markdown)?;
    Ok(ExportResult {
        path: path.display().to_string(),
        message: "Visible chat log exported.".into(),
    })
}

#[tauri::command]
pub fn export_llm_payload_history(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<ExportResult, String> {
    let logs = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        db::list_llm_payload_logs(&conn, &conversation_id).map_err(|err| err.to_string())?
    };
    let markdown = render_llm_payload_history(&logs);
    let path = write_export_file(&app, &conversation_id, "llm-payload-history", &markdown)?;
    Ok(ExportResult {
        path: path.display().to_string(),
        message: if logs.is_empty() {
            NO_LLM_PAYLOAD_LOGS_MESSAGE.into()
        } else {
            format!("Exported {} LLM payload log(s).", logs.len())
        },
    })
}
