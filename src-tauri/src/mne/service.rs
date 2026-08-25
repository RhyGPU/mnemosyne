use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use state_engine::{
    setting::{SessionWorld, SettingSoul},
    soul::Soul,
};
use tauri::{AppHandle, Manager, State, Window};

use crate::{
    commands::session::create_safety_backup,
    commands::{
        emit_dev_log, resolve_export_path, safe_filename, scene_state_present, uuid_like_id,
    },
    db::{self, ChatMessage, ConversationSummary, LlmPayloadLog},
    mne::{
        archive::{read_stored_zip, validate_bundle_path, validate_mne_manifest, write_stored_zip},
        contracts::{
            MneBundleContents, MneBundleManifest, MneExportResult, MneImportResult,
            MneValidationReport,
        },
    },
    AppState,
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct MneSessionLedgerExport {
    pub(crate) branches: Vec<db::SessionBranch>,
    pub(crate) turns: Vec<db::TurnCommit>,
    pub(crate) patches: Vec<db::StatePatchRecord>,
    pub(crate) variants: Vec<db::AssistantMessageVariant>,
}

#[tauri::command]
pub fn export_character_soul_mne(
    app: AppHandle,
    state: State<'_, AppState>,
    soul_id: String,
    output_path: String,
) -> Result<MneExportResult, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let mut soul = db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())?;
    soul.soul_kind = "savepoint".into();
    let soul_path = format!("souls/{}.json", safe_bundle_name(&soul.character_id));
    let manifest = mne_manifest(
        "character_soul",
        &soul.character_name,
        "Mnemosyne character Soul bundle",
        vec![soul_path.clone()],
        Vec::new(),
        None,
    );
    let mut manifest = manifest;
    manifest.soul_id = Some(soul.character_id.clone());
    manifest.source_savepoint_id = soul.source_savepoint_id.clone();
    let mut files = Vec::new();
    files.push(json_bundle_file("manifest.json", &manifest)?);
    files.push(json_bundle_file(&soul_path, &soul)?);
    write_mne_bundle(&app, &output_path, &manifest, files)
}

#[tauri::command]
pub fn export_world_setting_mne(
    app: AppHandle,
    state: State<'_, AppState>,
    setting_id: String,
    output_path: String,
) -> Result<MneExportResult, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let setting = db::get_setting(&conn, &setting_id).map_err(|err| err.to_string())?;
    let world_path = format!("worlds/{}.json", safe_bundle_name(&setting.setting_id));
    let manifest = mne_manifest(
        "world_setting",
        &setting.setting_name,
        "Mnemosyne world/setting bundle",
        Vec::new(),
        vec![world_path.clone()],
        None,
    );
    let mut manifest = manifest;
    manifest.world_id = Some(setting.setting_id.clone());
    let files = vec![
        json_bundle_file("manifest.json", &manifest)?,
        json_bundle_file(&world_path, &setting)?,
    ];
    write_mne_bundle(&app, &output_path, &manifest, files)
}

#[tauri::command]
pub fn export_scenario_bundle_mne(
    app: AppHandle,
    state: State<'_, AppState>,
    soul_id: String,
    world_id: String,
    output_path: String,
) -> Result<MneExportResult, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let mut soul = db::get_soul(&conn, &soul_id).map_err(|err| err.to_string())?;
    soul.soul_kind = "savepoint".into();
    let setting = db::get_setting(&conn, &world_id).map_err(|err| err.to_string())?;
    let soul_path = format!("souls/{}.json", safe_bundle_name(&soul.character_id));
    let world_path = format!("worlds/{}.json", safe_bundle_name(&setting.setting_id));
    let title = format!("{} + {}", soul.character_name, setting.setting_name);
    let manifest = mne_manifest(
        "scenario_bundle",
        &title,
        "Mnemosyne scenario bundle",
        vec![soul_path.clone()],
        vec![world_path.clone()],
        None,
    );
    let mut manifest = manifest;
    manifest.soul_id = Some(soul.character_id.clone());
    manifest.world_id = Some(setting.setting_id.clone());
    manifest.source_savepoint_id = soul.source_savepoint_id.clone();
    let files = vec![
        json_bundle_file("manifest.json", &manifest)?,
        json_bundle_file(&soul_path, &soul)?,
        json_bundle_file(&world_path, &setting)?,
    ];
    write_mne_bundle(&app, &output_path, &manifest, files)
}

#[tauri::command]
pub fn export_current_session_checkpoint_mne(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    conversation_id: String,
    output_path: String,
) -> Result<MneExportResult, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    export_current_session_checkpoint_mne_inner(
        &app,
        &window,
        &conn,
        &conversation_id,
        &output_path,
    )
}

pub(crate) fn export_current_session_checkpoint_mne_inner(
    app: &AppHandle,
    window: &Window,
    conn: &Connection,
    conversation_id: &str,
    output_path: &str,
) -> Result<MneExportResult, String> {
    let conversation =
        db::get_conversation_summary(conn, conversation_id).map_err(|err| err.to_string())?;
    let (soul, session_world, rebuilt_state_used) =
        if let Ok(branch) = db::get_active_session_branch(conn, conversation_id) {
            let rebuilt = db::rebuild_session_state(conn, conversation_id, &branch.branch_id)
                .map_err(|err| err.to_string())?;
            emit_dev_log(
                window,
                "success",
                "ledger",
                "mne_export_rebuilt_state_used",
                Some(serde_json::json!({
                    "conversation_id": conversation_id,
                    "branch_id": rebuilt.debug.branch_id,
                    "active_turn_id": rebuilt.debug.active_turn_id,
                    "rebuild_generation": rebuilt.debug.rebuild_generation
                })),
            );
            (rebuilt.soul, rebuilt.session_world, true)
        } else {
            let soul = db::get_soul(conn, &conversation.soul_id).map_err(|err| err.to_string())?;
            let session_world = db::get_conversation_session_world(conn, conversation_id)
                .map_err(|err| err.to_string())?
                .ok_or_else(|| "No SessionWorld linked to this conversation".to_string())?;
            (soul, session_world, false)
        };
    if !rebuilt_state_used {
        emit_dev_log(
            window,
            "info",
            "ledger",
            "mne_export_rebuilt_state_used",
            Some(serde_json::json!({
                "conversation_id": conversation_id,
                "rebuilt": false,
                "reason": "no_active_session_branch"
            })),
        );
    }
    let messages =
        db::list_messages(conn, conversation_id, 10_000).map_err(|err| err.to_string())?;
    let session_ledger = collect_mne_session_ledger(conn, conversation_id, &messages)
        .map_err(|err| err.to_string())?;
    let soul_path = format!("souls/{}.json", safe_bundle_name(&soul.character_id));
    let world_path = format!("worlds/{}.json", safe_bundle_name(&session_world.world_id));
    let conversation_path = "conversation/conversation.json".to_string();
    let manifest = mne_manifest(
        "session_checkpoint",
        &conversation.title,
        "Mnemosyne session checkpoint bundle",
        vec![soul_path.clone()],
        vec![world_path.clone()],
        Some(conversation_path.clone()),
    );
    let mut manifest = manifest;
    manifest.conversation_id = Some(conversation.conversation_id.clone());
    manifest.soul_id = Some(soul.character_id.clone());
    manifest.world_id = Some(session_world.world_id.clone());
    manifest.source_savepoint_id = soul.source_savepoint_id.clone();
    manifest.source_setting_id = session_world.source_setting_id.clone();
    let payload_logs = db::list_llm_payload_logs(conn, conversation_id).unwrap_or_default();
    let mut files = vec![
        json_bundle_file("manifest.json", &manifest)?,
        json_bundle_file(&soul_path, &soul)?,
        json_bundle_file(&world_path, &session_world)?,
        json_bundle_file(&conversation_path, &conversation)?,
        json_bundle_file("conversation/messages.json", &messages)?,
    ];
    if !payload_logs.is_empty() {
        files.push(json_bundle_file(
            "conversation/payload_logs.json",
            &payload_logs,
        )?);
    }
    if !session_ledger.branches.is_empty() {
        files.push(json_bundle_file(
            "conversation/branches.json",
            &session_ledger.branches,
        )?);
        files.push(json_bundle_file(
            "conversation/turns.json",
            &session_ledger.turns,
        )?);
        files.push(json_bundle_file(
            "conversation/patches.json",
            &session_ledger.patches,
        )?);
        if !session_ledger.variants.is_empty() {
            files.push(json_bundle_file(
                "conversation/variants.json",
                &session_ledger.variants,
            )?);
        }
    }
    let result = write_mne_bundle(app, output_path, &manifest, files)?;
    let export_trace = mne_export_state_trace_json(
        &manifest,
        conversation_id,
        &soul,
        &session_world,
        rebuilt_state_used,
        &result.path,
    );
    emit_dev_log(
        window,
        "success",
        "export",
        "mne_export_state_trace",
        Some(export_trace.clone()),
    );
    let _ = db::insert_llm_payload_log(
        conn,
        &LlmPayloadLog {
            conversation_id: conversation_id.to_string(),
            provider: "mne_export_trace".into(),
            mode: "mne_export".into(),
            context_mode: "export".into(),
            model: "local".into(),
            base_url: "local".into(),
            system_message: "MNE export state trace".into(),
            user_message: format!("export_current_session_checkpoint_mne({conversation_id})"),
            context_text: String::new(),
            created_at: db::now_ts(),
            pipeline_trace_json: Some(
                serde_json::to_string_pretty(&serde_json::json!({
                    "export_trace": export_trace
                }))
                .unwrap_or_default(),
            ),
            ..Default::default()
        },
    );
    Ok(result)
}

pub fn validate_mne_bundle_bytes(bytes: &[u8]) -> MneValidationReport {
    let mut report = MneValidationReport::default();

    let entries = match read_stored_zip(bytes) {
        Ok(e) => e,
        Err(err) => {
            report.valid = false;
            report
                .errors
                .push(format!("Invalid zip structure: {}", err));
            return report;
        }
    };

    let manifest_bytes = match entries.get("manifest.json") {
        Some(b) => b,
        None => {
            report.valid = false;
            report.errors.push("Missing manifest.json".to_string());
            return report;
        }
    };

    if manifest_bytes.is_empty() {
        report.valid = false;
        report.errors.push("manifest.json is empty".to_string());
        return report;
    }

    let manifest: MneBundleManifest = match serde_json::from_slice(manifest_bytes) {
        Ok(m) => m,
        Err(err) => {
            report.valid = false;
            report
                .errors
                .push(format!("Invalid manifest JSON: {}", err));
            return report;
        }
    };

    if manifest.mne_version != 1 {
        report
            .errors
            .push(format!("Unsupported .mne version {}", manifest.mne_version));
    }
    if !matches!(
        manifest.bundle_type.as_str(),
        "character_soul" | "world_setting" | "scenario_bundle" | "session_checkpoint"
    ) {
        report.errors.push(format!(
            "Unsupported .mne bundle_type: {}",
            manifest.bundle_type
        ));
    }

    let mut expected_files = HashSet::new();
    expected_files.insert("manifest.json".to_string());

    let mut parsed_souls = Vec::new();
    for soul_path in &manifest.contents.souls {
        expected_files.insert(soul_path.clone());
        match entries.get(soul_path) {
            Some(soul_bytes) => {
                if soul_bytes.is_empty() {
                    report
                        .errors
                        .push(format!("Required file {} is empty", soul_path));
                } else {
                    match serde_json::from_slice::<Soul>(soul_bytes) {
                        Ok(soul) => {
                            parsed_souls.push(soul);
                        }
                        Err(err) => {
                            report.errors.push(format!(
                                "Failed to parse Soul JSON at {}: {}",
                                soul_path, err
                            ));
                        }
                    }
                }
            }
            None => {
                report
                    .errors
                    .push(format!("Missing required file: {}", soul_path));
            }
        }
    }

    let mut parsed_worlds = Vec::new();
    let mut parsed_settings = Vec::new();
    for world_path in &manifest.contents.worlds {
        expected_files.insert(world_path.clone());
        match entries.get(world_path) {
            Some(world_bytes) => {
                if world_bytes.is_empty() {
                    report
                        .errors
                        .push(format!("Required file {} is empty", world_path));
                } else {
                    if manifest.bundle_type == "session_checkpoint" {
                        match serde_json::from_slice::<SessionWorld>(world_bytes) {
                            Ok(w) => parsed_worlds.push(w),
                            Err(_) => match setting_from_mne_world_bytes(world_bytes) {
                                Ok(s) => parsed_settings.push(s),
                                Err(err) => {
                                    report.errors.push(format!("Failed to parse SessionWorld JSON or Setting JSON at {}: {}", world_path, err));
                                }
                            },
                        }
                    } else {
                        match setting_from_mne_world_bytes(world_bytes) {
                            Ok(s) => parsed_settings.push(s),
                            Err(err) => {
                                report.errors.push(format!(
                                    "Failed to parse Setting JSON at {}: {}",
                                    world_path, err
                                ));
                            }
                        }
                    }
                }
            }
            None => {
                report
                    .errors
                    .push(format!("Missing required file: {}", world_path));
            }
        }
    }

    for img_path in &manifest.contents.images {
        expected_files.insert(img_path.clone());
        if !entries.contains_key(img_path) {
            report
                .errors
                .push(format!("Missing image asset: {}", img_path));
        }
    }

    let mut parsed_conversation = None;
    if let Some(conv_path) = &manifest.contents.conversation {
        expected_files.insert(conv_path.clone());
        match entries.get(conv_path) {
            Some(conv_bytes) => {
                if conv_bytes.is_empty() {
                    report
                        .errors
                        .push(format!("Required file {} is empty", conv_path));
                } else {
                    match serde_json::from_slice::<ConversationSummary>(conv_bytes) {
                        Ok(conv) => {
                            parsed_conversation = Some(conv);
                        }
                        Err(err) => {
                            report.errors.push(format!(
                                "Failed to parse Conversation JSON at {}: {}",
                                conv_path, err
                            ));
                        }
                    }
                }
            }
            None => {
                report
                    .errors
                    .push(format!("Missing required file: {}", conv_path));
            }
        }
    } else if manifest.bundle_type == "session_checkpoint" {
        report.errors.push(
            "Missing conversation path in manifest contents for session_checkpoint".to_string(),
        );
    }

    let mut parsed_messages = Vec::new();
    if manifest.bundle_type == "session_checkpoint" {
        let msg_path = "conversation/messages.json";
        expected_files.insert(msg_path.to_string());
        match entries.get(msg_path) {
            Some(msg_bytes) => {
                if msg_bytes.is_empty() {
                    report
                        .errors
                        .push(format!("Required file {} is empty", msg_path));
                } else {
                    match serde_json::from_slice::<Vec<ChatMessage>>(msg_bytes) {
                        Ok(msgs) => {
                            parsed_messages = msgs;
                        }
                        Err(err) => {
                            report.errors.push(format!(
                                "Failed to parse messages JSON at {}: {}",
                                msg_path, err
                            ));
                        }
                    }
                }
            }
            None => {
                report
                    .errors
                    .push(format!("Missing required file: {}", msg_path));
            }
        }
    }

    let mut parsed_payloads = Vec::new();
    let payload_path = "conversation/payload_logs.json";
    if entries.contains_key(payload_path) {
        expected_files.insert(payload_path.to_string());
        if let Some(payload_bytes) = entries.get(payload_path) {
            if !payload_bytes.is_empty() {
                match serde_json::from_slice::<Vec<LlmPayloadLog>>(payload_bytes) {
                    Ok(logs) => {
                        parsed_payloads = logs;
                    }
                    Err(err) => {
                        report.warnings.push(format!(
                            "Failed to parse payload logs JSON at {}: {}",
                            payload_path, err
                        ));
                    }
                }
            }
        }
    }

    for key in entries.keys() {
        if !expected_files.contains(key) {
            report
                .warnings
                .push(format!("Unknown extra file in bundle: {}", key));
        }
    }

    if let (Some(soul), Some(manifest_soul_id)) = (parsed_souls.first(), &manifest.soul_id) {
        if soul.character_id != *manifest_soul_id {
            report.errors.push(format!(
                "Soul ID mismatch: manifest soul_id '{}' does not match Soul character_id '{}'",
                manifest_soul_id, soul.character_id
            ));
        }
    }

    if manifest.bundle_type == "session_checkpoint" {
        if let (Some(world), Some(manifest_world_id)) = (parsed_worlds.first(), &manifest.world_id)
        {
            if world.world_id != *manifest_world_id {
                report.errors.push(format!("World ID mismatch: manifest world_id '{}' does not match SessionWorld world_id '{}'", manifest_world_id, world.world_id));
            }
        }

        if let (Some(conv), Some(manifest_conv_id)) =
            (&parsed_conversation, &manifest.conversation_id)
        {
            if conv.conversation_id != *manifest_conv_id {
                report.errors.push(format!("Conversation ID mismatch: manifest conversation_id '{}' does not match Conversation conversation_id '{}'", manifest_conv_id, conv.conversation_id));
            }

            for (idx, msg) in parsed_messages.iter().enumerate() {
                if msg.conversation_id != *manifest_conv_id {
                    report.errors.push(format!("Message conversation_id mismatch at index {}: message conversation_id '{}' does not match expected '{}'", idx, msg.conversation_id, manifest_conv_id));
                }
            }

            for (idx, log) in parsed_payloads.iter().enumerate() {
                if log.conversation_id != *manifest_conv_id {
                    report.errors.push(format!("Payload log conversation_id mismatch at index {}: log conversation_id '{}' does not match expected '{}'", idx, log.conversation_id, manifest_conv_id));
                }
            }
        }
    } else {
        if let (Some(setting), Some(manifest_world_id)) =
            (parsed_settings.first(), &manifest.world_id)
        {
            if setting.setting_id != *manifest_world_id {
                report.errors.push(format!("Setting ID mismatch: manifest world_id '{}' does not match Setting setting_id '{}'", manifest_world_id, setting.setting_id));
            }
        }
    }

    report.valid = report.errors.is_empty();

    if let Some(soul) = parsed_souls.first() {
        report.summary.soul_name = Some(soul.character_name.clone());
        report.summary.soul_id = Some(soul.character_id.clone());
        report.summary.memory_count =
            soul.memory.core.len() + soul.memory.recent.len() + soul.memory.schemas.len();
        report.summary.relationship_count = soul.relationships.len();
        report.summary.recent_event_count = soul.world.recent_events.len();
        report.summary.object_state_count = soul.world.object_states.len();
    }

    if manifest.bundle_type == "session_checkpoint" {
        if let Some(world) = parsed_worlds.first() {
            report.summary.world_name = Some(world.setting_name.clone());
            report.summary.world_id = Some(world.world_id.clone());
            report.summary.recent_event_count = world.recent_events.len();
            report.summary.object_state_count = world.object_states.len();
        }
        if let Some(conv) = &parsed_conversation {
            report.summary.conversation_title = Some(conv.title.clone());
            report.summary.conversation_id = Some(conv.conversation_id.clone());
        }
        report.summary.message_count = parsed_messages.len();
    } else {
        if let Some(setting) = parsed_settings.first() {
            report.summary.world_name = Some(setting.setting_name.clone());
            report.summary.world_id = Some(setting.setting_id.clone());
            report.summary.recent_event_count = setting.world.recent_events.len();
            report.summary.object_state_count = setting.world.object_states.len();
        }
    }

    report.summary.payload_log_count = parsed_payloads.len();

    report
}

#[tauri::command]
pub fn validate_mne_bundle(file_path: String) -> Result<MneValidationReport, String> {
    let path = PathBuf::from(&file_path);
    if path.extension().and_then(|ext| ext.to_str()) != Some("mne") {
        let mut report = MneValidationReport::default();
        report.valid = false;
        report
            .errors
            .push("Mnemosyne bundle import requires a .mne file".into());
        return Ok(report);
    }
    let bytes = fs::read(&path).map_err(|err| err.to_string())?;
    Ok(validate_mne_bundle_bytes(&bytes))
}

#[tauri::command]
pub fn preview_mne_import(file_path: String) -> Result<MneValidationReport, String> {
    let path = PathBuf::from(&file_path);
    if path.extension().and_then(|ext| ext.to_str()) != Some("mne") {
        let mut report = MneValidationReport::default();
        report.valid = false;
        report
            .errors
            .push("Mnemosyne bundle import requires a .mne file".into());
        return Ok(report);
    }
    let bytes = fs::read(&path).map_err(|err| err.to_string())?;
    Ok(validate_mne_bundle_bytes(&bytes))
}

pub fn import_mne_as_new_inner(conn: &Connection, bytes: &[u8]) -> Result<MneImportResult, String> {
    let report = validate_mne_bundle_bytes(&bytes);
    if !report.valid {
        return Err(format!(
            "Validation failed:\n- {}",
            report.errors.join("\n- ")
        ));
    }

    let entries = read_stored_zip(&bytes)?;
    let manifest_bytes = entries.get("manifest.json").unwrap();
    let manifest: MneBundleManifest = serde_json::from_slice(manifest_bytes).unwrap();

    let mut result = MneImportResult {
        bundle_id: manifest.bundle_id.clone(),
        bundle_type: manifest.bundle_type.clone(),
        ..MneImportResult::default()
    };

    let mut id_map: HashMap<String, String> = HashMap::new();
    let mut msg_map: HashMap<i64, i64> = HashMap::new();

    // A. Souls import
    for soul_path in &manifest.contents.souls {
        let soul_bytes = entries.get(soul_path).unwrap();
        let mut soul: Soul = serde_json::from_slice(soul_bytes).unwrap();
        let old_soul_id = soul.character_id.clone();

        let must_remap = db::get_soul(conn, &old_soul_id).is_ok();
        let target_soul_id = if must_remap {
            let remapped = uuid_like_id();
            id_map.insert(old_soul_id.clone(), remapped.clone());
            result
                .remapped_ids
                .insert(old_soul_id.clone(), remapped.clone());
            remapped
        } else {
            old_soul_id.clone()
        };

        soul.character_id = target_soul_id.clone();
        if must_remap {
            soul.source_soul_id = Some(old_soul_id.clone());
        }

        for mem in &mut soul.memory.recent {
            if let Some(owner) = &mem.owner_soul_id {
                if let Some(new_owner) = id_map.get(owner) {
                    mem.owner_soul_id = Some(new_owner.clone());
                }
            }
        }
        for schema in &mut soul.memory.schemas {
            if let Some(owner) = &schema.owner_soul_id {
                if let Some(new_owner) = id_map.get(owner) {
                    schema.owner_soul_id = Some(new_owner.clone());
                }
            }
        }

        if manifest.bundle_type != "session_checkpoint" {
            soul.soul_kind = "savepoint".into();
            soul.source_savepoint_id = None;
        }
        soul.last_updated = db::now_ts();

        db::upsert_soul(conn, &soul).map_err(|err| err.to_string())?;
        result.imported_soul_ids.push(soul.character_id);
    }

    // B. Worlds import
    let mut imported_session_world_id: Option<String> = None;
    if manifest.bundle_type == "session_checkpoint" {
        for world_path in &manifest.contents.worlds {
            let world_bytes = entries.get(world_path).unwrap();
            let mut session_world: SessionWorld =
                serde_json::from_slice(world_bytes).or_else(|_| {
                    let setting = setting_from_mne_world_bytes(world_bytes)?;
                    Ok::<SessionWorld, String>(state_engine::setting::session_world_from_setting(
                        &setting,
                    ))
                })?;

            let old_world_id = session_world.world_id.clone();
            let must_remap = db::get_session_world(conn, &old_world_id).is_ok();
            let target_world_id = if must_remap {
                let remapped = uuid_like_id();
                id_map.insert(old_world_id.clone(), remapped.clone());
                result
                    .remapped_ids
                    .insert(old_world_id.clone(), remapped.clone());
                remapped
            } else {
                old_world_id.clone()
            };

            session_world.world_id = target_world_id.clone();
            session_world.last_updated = db::now_ts();

            if let Some(ref setting_id) = session_world.source_setting_id {
                if db::get_setting(conn, setting_id).is_err() {
                    session_world.source_setting_id = None;
                }
            }

            db::upsert_session_world(conn, &session_world).map_err(|err| err.to_string())?;
            imported_session_world_id = Some(session_world.world_id);
        }
    } else {
        for world_path in &manifest.contents.worlds {
            let world_bytes = entries.get(world_path).unwrap();
            let mut setting = setting_from_mne_world_bytes(world_bytes)?;

            let old_setting_id = setting.setting_id.clone();
            let must_remap = db::get_setting(conn, &old_setting_id).is_ok();
            let target_setting_id = if must_remap {
                let remapped = uuid_like_id();
                id_map.insert(old_setting_id.clone(), remapped.clone());
                result
                    .remapped_ids
                    .insert(old_setting_id.clone(), remapped.clone());
                remapped
            } else {
                old_setting_id.clone()
            };

            setting.setting_id = target_setting_id.clone();
            setting.last_updated = db::now_ts();

            db::upsert_setting(conn, &setting).map_err(|err| err.to_string())?;
            result.imported_setting_ids.push(setting.setting_id);
        }
    }

    // C. Conversation/Messages import
    if manifest.bundle_type == "session_checkpoint" {
        let conversation_path = manifest.contents.conversation.as_ref().unwrap();
        let conversation_bytes = entries.get(conversation_path).unwrap();
        let conversation: ConversationSummary = serde_json::from_slice(conversation_bytes).unwrap();

        let old_conv_id = manifest
            .conversation_id
            .clone()
            .unwrap_or_else(|| conversation.conversation_id.clone());
        let must_remap = db::get_conversation_summary(conn, &old_conv_id).is_ok();
        let target_conv_id = if must_remap {
            let remapped = uuid_like_id();
            id_map.insert(old_conv_id.clone(), remapped.clone());
            result
                .remapped_ids
                .insert(old_conv_id.clone(), remapped.clone());
            remapped
        } else {
            old_conv_id.clone()
        };

        let soul_id = result
            .imported_soul_ids
            .first()
            .cloned()
            .or_else(|| manifest.soul_id.clone())
            .unwrap();

        let title = unique_imported_session_title(conn, &conversation.title)
            .map_err(|err| err.to_string())?;

        let safe_source_setting_id = manifest.source_setting_id.as_deref().and_then(|sid| {
            if db::get_setting(conn, sid).is_ok() {
                Some(sid)
            } else {
                None
            }
        });

        db::ensure_conversation_with_title_and_world(
            conn,
            &target_conv_id,
            &soul_id,
            imported_session_world_id.as_deref(),
            safe_source_setting_id,
            Some(&title),
        )
        .map_err(|err| err.to_string())?;
        let _ = db::set_active_player_persona(
            conn,
            &target_conv_id,
            &conversation.active_player_persona_id,
        );

        let messages_path = "conversation/messages.json";
        if let Some(message_bytes) = entries.get(messages_path) {
            let messages: Vec<ChatMessage> = serde_json::from_slice(message_bytes).unwrap();
            for message in messages {
                if message.role == "user" || message.role == "assistant" {
                    let new_msg_id = db::insert_message_and_get_id(
                        conn,
                        &target_conv_id,
                        &message.role,
                        &message.content,
                    )
                    .map_err(|err| err.to_string())?;
                    msg_map.insert(message.id, new_msg_id);
                }
            }
        }

        let mut variant_map: HashMap<i64, i64> = HashMap::new();
        if let Some(variant_bytes) = entries.get("conversation/variants.json") {
            if let Ok(variants) =
                serde_json::from_slice::<Vec<db::AssistantMessageVariant>>(variant_bytes)
            {
                for variant in variants {
                    let Some(new_message_id) = msg_map.get(&variant.message_id).copied() else {
                        continue;
                    };
                    let old_variant_id = variant.id;
                    let mut imported_variant = variant;
                    imported_variant.id = None;
                    imported_variant.message_id = new_message_id;
                    imported_variant.conversation_id = target_conv_id.clone();
                    let inserted =
                        db::insert_imported_assistant_message_variant(conn, &imported_variant)
                            .map_err(|err| err.to_string())?;
                    if let (Some(old_id), Some(new_id)) = (old_variant_id, inserted.id) {
                        variant_map.insert(old_id, new_id);
                    }
                }
            }
        }

        if let Some(imported_soul_id) = result.imported_soul_ids.first() {
            let mut soul = db::get_soul(conn, imported_soul_id).map_err(|err| err.to_string())?;
            let mut modified = false;
            for mem in &mut soul.memory.recent {
                if let Some(c_id) = &mem.source_conversation_id {
                    if c_id == &old_conv_id {
                        mem.source_conversation_id = Some(target_conv_id.clone());
                        modified = true;
                    }
                }
                if let Some(m_id) = mem.source_message_id {
                    if let Some(new_m_id) = msg_map.get(&m_id) {
                        mem.source_message_id = Some(*new_m_id);
                        modified = true;
                    }
                }
            }
            if modified {
                db::upsert_soul(conn, &soul).map_err(|err| err.to_string())?;
            }
        }

        let payload_path = "conversation/payload_logs.json";
        if let Some(payload_bytes) = entries.get(payload_path) {
            if let Ok(payload_logs) = serde_json::from_slice::<Vec<LlmPayloadLog>>(payload_bytes) {
                for mut log in payload_logs {
                    log.conversation_id = target_conv_id.clone();
                    if let Some(old_msg_id) = log.message_id {
                        if let Some(new_msg_id) = msg_map.get(&old_msg_id) {
                            log.message_id = Some(*new_msg_id);
                        }
                    }
                    db::insert_llm_payload_log(conn, &log).map_err(|err| err.to_string())?;
                }
            }
        }

        let ledger_restored = restore_imported_session_ledger(
            conn,
            &entries,
            &target_conv_id,
            &id_map,
            &msg_map,
            &variant_map,
        )?;

        if !ledger_restored {
            if let (Ok(soul), Some(world_id)) = (
                db::get_soul(conn, &soul_id),
                imported_session_world_id.as_deref(),
            ) {
                let world = db::get_session_world(conn, world_id).map_err(|err| err.to_string())?;
                let _ = db::create_session_branch(conn, &target_conv_id, &soul, &world);
            }
        }
    }

    let warning_part = if entries.contains_key("conversation/branches.json")
        || entries.contains_key("conversation/turns.json")
    {
        " (branch/variant structure restored for imported copy)"
    } else {
        ""
    };

    result.summary = format!(
        "Imported {} Soul(s), {} World/Setting savepoint(s) from {}{}",
        result.imported_soul_ids.len(),
        result.imported_setting_ids.len(),
        manifest.title,
        warning_part
    );

    Ok(result)
}

pub(crate) fn restore_imported_session_ledger(
    conn: &Connection,
    entries: &HashMap<String, Vec<u8>>,
    target_conv_id: &str,
    id_map: &HashMap<String, String>,
    msg_map: &HashMap<i64, i64>,
    variant_map: &HashMap<i64, i64>,
) -> Result<bool, String> {
    let Some(branch_bytes) = entries.get("conversation/branches.json") else {
        return Ok(false);
    };
    let Some(turn_bytes) = entries.get("conversation/turns.json") else {
        return Ok(false);
    };
    let Some(patch_bytes) = entries.get("conversation/patches.json") else {
        return Ok(false);
    };
    let branches: Vec<db::SessionBranch> =
        serde_json::from_slice(branch_bytes).map_err(|err| err.to_string())?;
    let turns: Vec<db::TurnCommit> =
        serde_json::from_slice(turn_bytes).map_err(|err| err.to_string())?;
    let patches: Vec<db::StatePatchRecord> =
        serde_json::from_slice(patch_bytes).map_err(|err| err.to_string())?;
    if branches.is_empty() || turns.is_empty() || patches.is_empty() {
        return Ok(false);
    }

    let mut branch_map = HashMap::new();
    for branch in branches.iter().filter(|branch| branch.is_active) {
        branch_map.insert(branch.branch_id.clone(), uuid_like_id());
    }
    if branch_map.is_empty() {
        return Ok(false);
    }
    let mut turn_map = HashMap::new();
    for turn in &turns {
        if branch_map.contains_key(&turn.branch_id) {
            turn_map.insert(turn.turn_id.clone(), uuid_like_id());
        }
    }
    let mut patch_map = HashMap::new();
    for patch in &patches {
        if turn_map.contains_key(&patch.turn_id) {
            patch_map.insert(patch.patch_id.clone(), uuid_like_id());
        }
    }

    for branch in branches.into_iter().filter(|branch| branch.is_active) {
        let Some(new_branch_id) = branch_map.get(&branch.branch_id).cloned() else {
            continue;
        };
        let mut imported = branch;
        imported.branch_id = new_branch_id;
        imported.conversation_id = target_conv_id.to_string();
        imported.active_turn_id = imported
            .active_turn_id
            .as_deref()
            .and_then(|turn_id| turn_map.get(turn_id).cloned());
        imported.base_soul_json = remap_json_string_ids(&imported.base_soul_json, id_map)?;
        imported.base_session_world_json =
            remap_json_string_ids(&imported.base_session_world_json, id_map)?;
        db::insert_imported_session_branch(conn, &imported).map_err(|err| err.to_string())?;
    }

    for turn in turns {
        if !branch_map.contains_key(&turn.branch_id) {
            continue;
        }
        let mut imported = turn;
        let Some(new_turn_id) = turn_map.get(&imported.turn_id).cloned() else {
            continue;
        };
        imported.turn_id = new_turn_id;
        imported.conversation_id = target_conv_id.to_string();
        imported.branch_id = branch_map
            .get(&imported.branch_id)
            .cloned()
            .unwrap_or(imported.branch_id);
        imported.parent_turn_id = imported
            .parent_turn_id
            .as_deref()
            .and_then(|turn_id| turn_map.get(turn_id).cloned());
        imported.user_message_id = imported
            .user_message_id
            .and_then(|message_id| msg_map.get(&message_id).copied());
        imported.assistant_message_id = imported
            .assistant_message_id
            .and_then(|message_id| msg_map.get(&message_id).copied());
        imported.state_patch_id = imported
            .state_patch_id
            .as_deref()
            .and_then(|patch_id| patch_map.get(patch_id).cloned());
        imported.selected_variant_id = imported
            .selected_variant_id
            .and_then(|variant_id| variant_map.get(&variant_id).copied());
        db::insert_imported_turn_commit(conn, &imported).map_err(|err| err.to_string())?;
    }

    for patch in patches {
        if !turn_map.contains_key(&patch.turn_id) {
            continue;
        }
        let mut imported = patch;
        imported.patch_id = patch_map
            .get(&imported.patch_id)
            .cloned()
            .unwrap_or(imported.patch_id);
        imported.turn_id = turn_map
            .get(&imported.turn_id)
            .cloned()
            .unwrap_or(imported.turn_id);
        imported.parent_baseline_patch_id = imported
            .parent_baseline_patch_id
            .as_deref()
            .and_then(|patch_id| patch_map.get(patch_id).cloned());
        imported.source_turn_id = imported
            .source_turn_id
            .as_deref()
            .and_then(|turn_id| turn_map.get(turn_id).cloned());
        imported.source_assistant_message_id = imported
            .source_assistant_message_id
            .and_then(|message_id| msg_map.get(&message_id).copied());
        imported.source_assistant_variant_id = imported
            .source_assistant_variant_id
            .and_then(|variant_id| variant_map.get(&variant_id).copied());
        imported.invalidated_by_patch_id = imported
            .invalidated_by_patch_id
            .as_deref()
            .and_then(|patch_id| patch_map.get(patch_id).cloned());
        imported.supersedes_patch_id = imported
            .supersedes_patch_id
            .as_deref()
            .and_then(|patch_id| patch_map.get(patch_id).cloned());
        imported.patch_json = remap_patch_json_ids(&imported.patch_json, id_map, msg_map)?;
        if let Some(inverse) = imported.inverse_patch_json.as_deref() {
            imported.inverse_patch_json = Some(remap_patch_json_ids(inverse, id_map, msg_map)?);
        }
        db::insert_imported_state_patch(conn, &imported).map_err(|err| err.to_string())?;
    }

    let active_branch =
        db::get_active_session_branch(conn, target_conv_id).map_err(|err| err.to_string())?;
    db::rebuild_session_state(conn, target_conv_id, &active_branch.branch_id)
        .map_err(|err| err.to_string())?;
    Ok(true)
}

pub(crate) fn remap_json_string_ids(
    value: &str,
    id_map: &HashMap<String, String>,
) -> Result<String, String> {
    let mut json: serde_json::Value = serde_json::from_str(value).map_err(|err| err.to_string())?;
    remap_json_value_ids(&mut json, id_map);
    serde_json::to_string(&json).map_err(|err| err.to_string())
}

pub(crate) fn remap_patch_json_ids(
    value: &str,
    id_map: &HashMap<String, String>,
    msg_map: &HashMap<i64, i64>,
) -> Result<String, String> {
    let mut json: serde_json::Value = serde_json::from_str(value).map_err(|err| err.to_string())?;
    remap_json_value_ids(&mut json, id_map);
    remap_json_message_ids(&mut json, msg_map);
    serde_json::to_string(&json).map_err(|err| err.to_string())
}

pub(crate) fn remap_json_value_ids(
    value: &mut serde_json::Value,
    id_map: &HashMap<String, String>,
) {
    match value {
        serde_json::Value::String(text) => {
            if let Some(mapped) = id_map.get(text) {
                *text = mapped.clone();
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                remap_json_value_ids(value, id_map);
            }
        }
        serde_json::Value::Object(map) => {
            for value in map.values_mut() {
                remap_json_value_ids(value, id_map);
            }
        }
        _ => {}
    }
}

pub(crate) fn remap_json_message_ids(value: &mut serde_json::Value, msg_map: &HashMap<i64, i64>) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                remap_json_message_ids(value, msg_map);
            }
        }
        serde_json::Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                if is_message_id_json_key(key) {
                    if let Some(old_id) = value.as_i64() {
                        if let Some(new_id) = msg_map.get(&old_id) {
                            *value = serde_json::Value::Number(serde_json::Number::from(*new_id));
                            continue;
                        }
                    }
                }
                remap_json_message_ids(value, msg_map);
            }
        }
        _ => {}
    }
}

pub(crate) fn is_message_id_json_key(key: &str) -> bool {
    matches!(
        key,
        "message_id" | "source_message_id" | "source_assistant_message_id"
    )
}

#[tauri::command]
pub fn import_mne_as_new(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    file_path: String,
) -> Result<MneImportResult, String> {
    let path = PathBuf::from(&file_path);
    if path.extension().and_then(|ext| ext.to_str()) != Some("mne") {
        return Err("Mnemosyne bundle import requires a .mne file".into());
    }
    create_safety_backup(&app, &window, "import_mne_as_new")?;
    let bytes = fs::read(&path).map_err(|err| err.to_string())?;
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    import_mne_as_new_inner(&conn, &bytes)
}

#[tauri::command]
pub fn import_mne_bundle(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    file_path: String,
) -> Result<MneImportResult, String> {
    let path = PathBuf::from(&file_path);
    if path.extension().and_then(|ext| ext.to_str()) != Some("mne") {
        return Err("Mnemosyne bundle import requires a .mne file".into());
    }
    create_safety_backup(&app, &window, "import_mne_bundle")?;
    let bytes = fs::read(&path).map_err(|err| err.to_string())?;
    let entries = read_stored_zip(&bytes)?;
    let manifest_bytes = entries
        .get("manifest.json")
        .ok_or_else(|| "Missing manifest.json".to_string())?;
    let manifest: MneBundleManifest = serde_json::from_slice(manifest_bytes)
        .map_err(|err| format!("Invalid manifest JSON: {err}"))?;
    validate_mne_manifest(&manifest)?;
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    import_mne_entries(&conn, &entries, &manifest)
}

pub(crate) fn import_mne_entries(
    conn: &Connection,
    entries: &HashMap<String, Vec<u8>>,
    manifest: &MneBundleManifest,
) -> Result<MneImportResult, String> {
    let mut result = MneImportResult {
        bundle_id: manifest.bundle_id.clone(),
        bundle_type: manifest.bundle_type.clone(),
        ..MneImportResult::default()
    };

    for soul_path in &manifest.contents.souls {
        validate_bundle_path(soul_path)?;
        let soul_bytes = entries
            .get(soul_path)
            .ok_or_else(|| format!("Missing Soul file {soul_path}"))?;
        let mut soul: Soul = serde_json::from_slice(soul_bytes)
            .map_err(|err| format!("Invalid Soul JSON: {err}"))?;
        let original_id = soul.character_id.clone();
        if db::get_soul(&conn, &soul.character_id).is_ok() {
            soul.character_id = uuid_like_id();
            soul.source_soul_id = Some(original_id.clone());
            result
                .remapped_ids
                .insert(original_id.clone(), soul.character_id.clone());
        }
        if manifest.bundle_type != "session_checkpoint" {
            soul.soul_kind = "savepoint".into();
            soul.source_savepoint_id = None;
        }
        soul.last_updated = db::now_ts();
        db::upsert_soul(&conn, &soul).map_err(|err| err.to_string())?;
        result.imported_soul_ids.push(soul.character_id);
    }

    let mut imported_session_world_id: Option<String> = None;
    if manifest.bundle_type == "session_checkpoint" {
        for world_path in &manifest.contents.worlds {
            validate_bundle_path(world_path)?;
            let world_bytes = entries
                .get(world_path)
                .ok_or_else(|| format!("Missing World file {world_path}"))?;
            let mut session_world: SessionWorld =
                serde_json::from_slice(world_bytes).or_else(|_| {
                    let setting = setting_from_mne_world_bytes(world_bytes)?;
                    Ok::<SessionWorld, String>(state_engine::setting::session_world_from_setting(
                        &setting,
                    ))
                })?;
            let original_id = session_world.world_id.clone();
            if db::get_session_world(&conn, &session_world.world_id).is_ok() {
                session_world.world_id = uuid_like_id();
                result
                    .remapped_ids
                    .insert(original_id.clone(), session_world.world_id.clone());
            }
            session_world.last_updated = db::now_ts();
            db::upsert_session_world(&conn, &session_world).map_err(|err| err.to_string())?;
            imported_session_world_id = Some(session_world.world_id);
        }
    } else {
        for world_path in &manifest.contents.worlds {
            validate_bundle_path(world_path)?;
            let world_bytes = entries
                .get(world_path)
                .ok_or_else(|| format!("Missing World file {world_path}"))?;
            let mut setting = setting_from_mne_world_bytes(world_bytes)?;
            let original_id = setting.setting_id.clone();
            if db::get_setting(&conn, &setting.setting_id).is_ok() {
                setting.setting_id = uuid_like_id();
                result
                    .remapped_ids
                    .insert(original_id.clone(), setting.setting_id.clone());
            }
            setting.last_updated = db::now_ts();
            db::upsert_setting(&conn, &setting).map_err(|err| err.to_string())?;
            result.imported_setting_ids.push(setting.setting_id);
        }
    }

    if manifest.bundle_type == "session_checkpoint" {
        let conversation_path = manifest
            .contents
            .conversation
            .as_deref()
            .ok_or_else(|| "Session checkpoint is missing conversation path".to_string())?;
        validate_bundle_path(conversation_path)?;
        let conversation_bytes = entries
            .get(conversation_path)
            .ok_or_else(|| format!("Missing conversation file {conversation_path}"))?;
        let conversation: ConversationSummary = serde_json::from_slice(conversation_bytes)
            .map_err(|err| format!("Invalid conversation JSON: {err}"))?;
        let original_conversation_id = manifest
            .conversation_id
            .clone()
            .unwrap_or_else(|| conversation.conversation_id.clone());
        let conversation_id =
            if db::get_conversation_summary(&conn, &original_conversation_id).is_ok() {
                let remapped = uuid_like_id();
                result
                    .remapped_ids
                    .insert(original_conversation_id.clone(), remapped.clone());
                remapped
            } else {
                original_conversation_id
            };
        let soul_id = result
            .imported_soul_ids
            .first()
            .cloned()
            .or_else(|| manifest.soul_id.clone())
            .ok_or_else(|| "Session checkpoint is missing Soul identity".to_string())?;
        let title = unique_imported_session_title(&conn, &conversation.title)
            .map_err(|err| err.to_string())?;
        db::ensure_conversation_with_title_and_world(
            &conn,
            &conversation_id,
            &soul_id,
            imported_session_world_id.as_deref(),
            manifest.source_setting_id.as_deref(),
            Some(&title),
        )
        .map_err(|err| err.to_string())?;
        let messages_path = "conversation/messages.json";
        if let Some(message_bytes) = entries.get(messages_path) {
            let messages: Vec<ChatMessage> = serde_json::from_slice(message_bytes)
                .map_err(|err| format!("Invalid messages JSON: {err}"))?;
            for message in messages
                .iter()
                .filter(|message| message.role == "user" || message.role == "assistant")
            {
                db::insert_message_and_get_id(
                    &conn,
                    &conversation_id,
                    &message.role,
                    &message.content,
                )
                .map_err(|err| err.to_string())?;
            }
        }
        if let (Ok(soul), Some(world_id)) = (
            db::get_soul(&conn, &soul_id),
            imported_session_world_id.as_deref(),
        ) {
            let world = db::get_session_world(&conn, world_id).map_err(|err| err.to_string())?;
            let _ = db::create_session_branch(&conn, &conversation_id, &soul, &world);
        }
    }

    result.summary = format!(
        "Imported {} Soul(s), {} World/Setting savepoint(s) from {}",
        result.imported_soul_ids.len(),
        result.imported_setting_ids.len(),
        manifest.title
    );
    Ok(result)
}

pub(crate) fn unique_imported_session_title(
    conn: &Connection,
    title: &str,
) -> rusqlite::Result<String> {
    let base = title.trim();
    let base = if base.is_empty() {
        "Imported Session"
    } else {
        base
    };
    let existing = db::list_conversations(conn)?
        .into_iter()
        .map(|conversation| conversation.title)
        .collect::<HashSet<_>>();
    if !existing.contains(base) {
        return Ok(base.to_string());
    }
    for index in 2..10_000 {
        let candidate = format!("{base} ({index})");
        if !existing.contains(&candidate) {
            return Ok(candidate);
        }
    }
    Ok(format!("{base} ({})", db::now_ts()))
}

pub(crate) fn resolve_mne_export_path(
    app: &AppHandle,
    requested: &str,
    manifest: &MneBundleManifest,
) -> Result<PathBuf, String> {
    let path = if requested.trim().is_empty() {
        let mut dir = app
            .path()
            .download_dir()
            .or_else(|_| app.path().app_data_dir())
            .map_err(|err| err.to_string())?;
        dir.push("mnemosyne-exports");
        dir.push(default_mne_filename(manifest));
        dir
    } else {
        resolve_export_path(app, requested, "mne")?
    };
    unique_export_path(path)
}

pub(crate) fn unique_export_path(path: PathBuf) -> Result<PathBuf, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    if !path.exists() {
        return Ok(path);
    }
    let parent = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(PathBuf::new);
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("mnemosyne_export");
    let extension = path.extension().and_then(|extension| extension.to_str());
    for index in 2..10_000 {
        let file_name = match extension {
            Some(extension) if !extension.is_empty() => format!("{stem}_{index}.{extension}"),
            _ => format!("{stem}_{index}"),
        };
        let candidate = parent.join(file_name);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err("Could not find an available export filename".into())
}

pub(crate) fn default_mne_filename(manifest: &MneBundleManifest) -> String {
    let title = safe_bundle_name(&manifest.title).replace('-', "_");
    let title = if title.is_empty() {
        "mnemosyne".to_string()
    } else {
        title
    };
    let identity = manifest
        .conversation_id
        .as_deref()
        .or(manifest.soul_id.as_deref())
        .or(manifest.world_id.as_deref())
        .unwrap_or(&manifest.bundle_id);
    format!(
        "{}_{}_{}_{}.mne",
        title,
        safe_bundle_name(&manifest.bundle_type),
        manifest.created_at,
        short_id_suffix(identity)
    )
}

pub(crate) fn short_id_suffix(value: &str) -> String {
    let segments: Vec<&str> = value.split('-').collect();
    for seg in &segments {
        let cleaned: String = seg.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        if cleaned.len() == 8 && cleaned.chars().all(|c| c.is_ascii_alphanumeric()) {
            return cleaned.to_ascii_lowercase();
        }
    }
    let cleaned: String = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect();
    if cleaned.is_empty() {
        "0000".into()
    } else {
        let start = cleaned.len().saturating_sub(4);
        cleaned[start..].to_ascii_lowercase()
    }
}

pub(crate) fn mne_manifest(
    bundle_type: &str,
    title: &str,
    description: &str,
    souls: Vec<String>,
    worlds: Vec<String>,
    conversation: Option<String>,
) -> MneBundleManifest {
    MneBundleManifest {
        mne_version: 1,
        bundle_id: uuid_like_id(),
        bundle_type: bundle_type.into(),
        title: title.trim().to_string(),
        description: description.into(),
        author: None,
        created_at: db::now_ts(),
        app: "Mnemosyne".into(),
        schema_version: 1,
        conversation_id: None,
        soul_id: None,
        world_id: None,
        source_savepoint_id: None,
        source_setting_id: None,
        contents: MneBundleContents {
            souls,
            worlds,
            images: Vec::new(),
            conversation,
        },
    }
}

pub(crate) fn json_bundle_file<T: Serialize>(
    path: &str,
    value: &T,
) -> Result<(String, Vec<u8>), String> {
    validate_bundle_path(path)?;
    let bytes = serde_json::to_vec_pretty(value).map_err(|err| err.to_string())?;
    Ok((path.replace('\\', "/"), bytes))
}

pub(crate) fn write_mne_bundle(
    app: &AppHandle,
    output_path: &str,
    manifest: &MneBundleManifest,
    files: Vec<(String, Vec<u8>)>,
) -> Result<MneExportResult, String> {
    let path = resolve_mne_export_path(app, output_path, manifest)?;
    write_stored_zip(&path, &files)?;
    Ok(MneExportResult {
        path: path.to_string_lossy().to_string(),
        manifest: manifest.clone(),
    })
}

pub(crate) fn setting_from_mne_world_bytes(bytes: &[u8]) -> Result<SettingSoul, String> {
    if let Ok(setting) = serde_json::from_slice::<SettingSoul>(bytes) {
        return Ok(setting);
    }
    let session_world: SessionWorld =
        serde_json::from_slice(bytes).map_err(|err| format!("Invalid World JSON: {err}"))?;
    Ok(SettingSoul {
        schema_version: session_world.schema_version,
        setting_id: session_world
            .source_setting_id
            .clone()
            .unwrap_or_else(|| session_world.world_id.clone()),
        setting_name: session_world.setting_name.clone(),
        scenario: session_world.scenario.clone(),
        last_updated: db::now_ts(),
        turn_counter: 0,
        world: session_world.world_log(),
    })
}

pub(crate) fn safe_bundle_name(value: &str) -> String {
    safe_filename(value).trim_matches('.').to_string()
}

pub(crate) fn mne_export_state_trace_json(
    manifest: &MneBundleManifest,
    conversation_id: &str,
    soul: &Soul,
    session_world: &SessionWorld,
    rebuilt_state_used: bool,
    export_path: &str,
) -> serde_json::Value {
    serde_json::json!({
        "export_bundle_id": manifest.bundle_id,
        "export_conversation_id": conversation_id,
        "export_source": if rebuilt_state_used { "rebuilt_ledger_state" } else { "materialized_cache" },
        "rebuilt_before_export": rebuilt_state_used,
        "exported_recent_event_count": session_world.recent_events.len(),
        "exported_memory_recent_count": soul.memory.recent.len(),
        "exported_object_state_count": session_world.object_states.len(),
        "exported_scene_state_present": scene_state_present(session_world),
        "export_filename": Path::new(export_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(export_path),
        "bundle_type": manifest.bundle_type,
        "conversation_id": manifest.conversation_id,
        "soul_id": manifest.soul_id,
        "world_id": manifest.world_id,
    })
}

pub(crate) fn collect_mne_session_ledger(
    conn: &Connection,
    conversation_id: &str,
    messages: &[ChatMessage],
) -> rusqlite::Result<MneSessionLedgerExport> {
    let branches = db::list_session_branches_for_conversation(conn, conversation_id)?;
    let mut turns = Vec::new();
    let mut patches = Vec::new();
    for branch in &branches {
        turns.extend(db::list_turn_commits_for_branch(conn, &branch.branch_id)?);
        patches.extend(db::list_state_patches_for_branch(conn, &branch.branch_id)?);
    }
    let mut variants = Vec::new();
    for message in messages
        .iter()
        .filter(|message| message.role == "assistant")
    {
        variants.extend(db::list_assistant_message_variants(
            conn,
            conversation_id,
            message.id,
        )?);
    }
    Ok(MneSessionLedgerExport {
        branches,
        turns,
        patches,
        variants,
    })
}
