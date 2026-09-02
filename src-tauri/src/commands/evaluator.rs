use super::session::create_safety_backup;
use super::*;
use crate::mne::service::export_current_session_checkpoint_mne_inner;

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
    _state: State<'_, AppState>,
    _profile_id: String,
) -> Result<bool, String> {
    Err("delete_provider_profile is deprecated; use archive_provider_profile with active profile guard.".into())
}

#[tauri::command]
pub fn archive_provider_profile(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    profile_id: String,
    active_ids: Vec<String>,
) -> Result<bool, String> {
    if active_ids.is_empty() {
        return Err("active_ids is required and cannot be empty.".into());
    }
    create_safety_backup(&app, &window, "archive_provider_profile")?;
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let active_refs: Vec<&str> = active_ids.iter().map(|s| s.as_str()).collect();
    db::archive_provider_profile(&conn, &profile_id, &active_refs)
}

#[tauri::command]
pub fn restore_provider_profile(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::restore_provider_profile(&conn, &profile_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_archived_provider_profiles(
    state: State<'_, AppState>,
) -> Result<Vec<ProviderProfile>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::list_archived_provider_profiles(&conn).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_latest_evaluator_job(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Option<db::EvaluatorJob>, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::get_latest_evaluator_job(&conn, &conversation_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn cancel_evaluator_job(state: State<'_, AppState>, job_id: String) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let job = db::get_evaluator_job(&conn, &job_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "Evaluator job not found".to_string())?;
    if matches!(
        job.status.as_str(),
        "completed" | "failed" | "canceled" | "timed_out"
    ) {
        return Ok(());
    }
    db::update_evaluator_job_status(
        &conn,
        &job_id,
        "canceled",
        Some("Canceled by user"),
        Some(db::now_ts()),
        None,
        false,
    )
    .map_err(|err| err.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorContractTestReport {
    pub passed: bool,
    pub errors: Vec<String>,
    pub raw_response: String,
    /// Structured-output level achieved by the probe (see STRUCTURED_SUPPORT_*).
    #[serde(default)]
    pub structured_output_support: i32,
    #[serde(default)]
    pub evaluator_compatibility_status: i32,
    #[serde(default)]
    pub evaluator_compatibility_status_label: String,
}

/// Map the probe outcome to the persisted `structured_output_support` level.
/// The achieved enforcement only counts if the returned text actually parsed
/// into an EnginePatch — a provider that accepts `response_format` but returns
/// garbage is recorded as unsupported.
pub(crate) fn structured_support_level(enforcement: StructuredEnforcement, parsed_ok: bool) -> i32 {
    if !parsed_ok {
        return STRUCTURED_SUPPORT_UNTESTED;
    }
    match enforcement {
        StructuredEnforcement::JsonSchema => STRUCTURED_SUPPORT_JSON_SCHEMA,
        StructuredEnforcement::ToolCall | StructuredEnforcement::Grammar => {
            STRUCTURED_SUPPORT_JSON_SCHEMA
        }
        StructuredEnforcement::JsonObject => STRUCTURED_SUPPORT_JSON_OBJECT,
        StructuredEnforcement::None => STRUCTURED_SUPPORT_PROMPT_ONLY,
    }
}

fn evaluator_compatibility_status_for_structured_support(
    structured_output_support: i32,
    form_contract_passed: bool,
) -> i32 {
    if !form_contract_passed {
        return EVALUATOR_COMPAT_FAILED;
    }
    match structured_output_support {
        STRUCTURED_SUPPORT_JSON_SCHEMA => EVALUATOR_COMPAT_PASSED_SCHEMA_ENFORCED,
        STRUCTURED_SUPPORT_JSON_OBJECT => EVALUATOR_COMPAT_PASSED_JSON_OBJECT_ONLY,
        STRUCTURED_SUPPORT_UNTESTED => EVALUATOR_COMPAT_UNTESTED,
        _ => EVALUATOR_COMPAT_FAILED,
    }
}

fn evaluator_compatibility_status_label(status: i32) -> &'static str {
    match status {
        EVALUATOR_COMPAT_PASSED_SCHEMA_ENFORCED => "passed_schema_enforced",
        EVALUATOR_COMPAT_PASSED_JSON_OBJECT_ONLY => "passed_json_object_only",
        EVALUATOR_COMPAT_FAILED => "failed",
        EVALUATOR_COMPAT_STALE_PROMPT_VERSION => "stale_prompt_version",
        EVALUATOR_COMPAT_FAILED_SCHEMA_ENFORCED => "failed_schema_enforced",
        _ => "untested",
    }
}

#[tauri::command]
pub async fn run_evaluator_contract_test(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<EvaluatorContractTestReport, String> {
    let profile = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        db::get_provider_profile(&conn, &profile_id).map_err(|err| err.to_string())?
    };

    let settings = ApiProviderSettings {
        base_url: profile.base_url.clone(),
        api_key: profile.api_key.clone(),
        model: profile.model.clone(),
        system_prompt: profile.system_prompt.clone(),
        narrator_timeout_ms: profile.narrator_timeout_ms,
        narrator_temperature: None,
        narrator_max_tokens: None,
        context_max_tokens: None,
        narrator_top_p: None,
        narrator_frequency_penalty: None,
        narrator_presence_penalty: None,
        evaluator_timeout_ms: Some(
            profile
                .evaluator_timeout_ms
                .filter(|value| *value > 0)
                .unwrap_or(DEFAULT_EVALUATOR_TIMEOUT_MS)
                .min(DEFAULT_EVALUATOR_TIMEOUT_MS),
        ),
        structured_evaluator_timeout_ms: None,
        diagnostic_evaluator_timeout_ms: None,
        evaluator_timeout_mode: Some("finite".into()),
        // The contract test always exercises the FORM contract (form prompt +
        // form validation), so the call must not route through the structured
        // path even when the profile selects evaluator_structured_v1.
        evaluator_mode: Some(EVALUATOR_MODE_FORM_V1.into()),
        structured_evaluator_policy: Some("prefer".into()),
        structured_evaluator_transport: None,
        structured_evaluator_max_retries: Some(1),
        structured_require_ops: None,
        wait_for_evaluator_before_next_turn: profile.wait_for_evaluator_before_next_turn,
        allow_send_with_stale_state: profile.allow_send_with_stale_state,
        evaluator_background_enabled: profile.evaluator_background_enabled,
        anti_replay_forced_retry_enabled: profile.anti_replay_forced_retry_enabled,
        evaluator_execution_mode: None,
    };

    let test_user_text = "I promise to help you clean the laboratory tomorrow morning. I want you to feel comfortable trusting me.";
    let test_narrator_text = "Aurora smiles softly, her eyes warming. 'Thank you. That means a lot to me.' She takes your hand, showing a moment of rare vulnerability.";

    let mut soul = new_default_soul("Aurora");
    soul.relationships.insert(
        "default_player".to_string(),
        state_engine::soul::Relationship {
            trust: 10.0,
            affection: 10.0,
            intimacy: 10.0,
            respect: 10.0,
            comfort: 10.0,
            ..Default::default()
        },
    );

    let system_prompt = build_evaluator_form_prompt_compact_with_player_persona(
        &soul,
        None,
        test_user_text,
        test_narrator_text,
        "default_player",
        "User",
    );

    let user_message = build_evaluator_user_message(
        test_user_text,
        test_narrator_text,
        &format!("User: {}\nNarrator: {}", test_user_text, test_narrator_text),
        None,
        None,
        None,
    );

    let provider = ApiProvider::default();
    let raw_response =
        match complete_evaluator_with_config(&provider, &settings, &system_prompt, &user_message)
            .await
        {
            Ok(res) => res.raw_text,
            Err(err) => {
                let now = db::now_ts();
                let mut failed_profile = profile.clone();
                failed_profile.evaluator_compatibility_status = EVALUATOR_COMPAT_FAILED;
                failed_profile.evaluator_last_tested_at = Some(now);
                failed_profile.evaluator_last_failure_reason =
                    Some(format!("LLM Call Error: {}", err));
                failed_profile.evaluator_contract_version = CURRENT_EVALUATOR_CONTRACT_VERSION;
                failed_profile.evaluator_prompt_version = CURRENT_EVALUATOR_PROMPT_VERSION;

                let conn = state.conn.lock().map_err(|err| err.to_string())?;
                let _ = db::upsert_provider_profile(&conn, &failed_profile);

                return Ok(EvaluatorContractTestReport {
                    passed: false,
                    errors: vec![format!("LLM Call failed: {}", err)],
                    raw_response: String::new(),
                    structured_output_support: failed_profile.structured_output_support,
                    evaluator_compatibility_status: failed_profile.evaluator_compatibility_status,
                    evaluator_compatibility_status_label: evaluator_compatibility_status_label(
                        failed_profile.evaluator_compatibility_status,
                    )
                    .into(),
                });
            }
        };

    let validation_result = state_engine::evaluator_form::validate::validate_evaluator_contract(
        &raw_response,
        test_user_text,
        test_narrator_text,
    );

    // Probe how strictly this provider can enforce structured output, so
    // evaluator_structured_v1 eligibility is known per profile. Informational
    // only: the probe never flips the form-contract pass/fail.
    let structured_probe_prompt = build_structured_evaluator_prompt(&soul, None);
    let mut structured_schema_claim_failed = false;
    let structured_output_support = match provider
        .complete_structured_prompt(
            &settings,
            &structured_probe_prompt,
            &user_message,
            0.0,
            effective_evaluator_timeout_ms(&settings).map(Duration::from_millis),
            EVALUATOR_OPS_SCHEMA_NAME,
            &evaluator_ops_json_schema(),
        )
        .await
    {
        Ok(completion) => {
            let parsed_ok = match completion.enforcement {
                StructuredEnforcement::JsonSchema => {
                    let parsed = serde_json::from_str::<EvaluatorStructuredOutputV1>(
                        completion.raw_text.trim(),
                    )
                    .is_ok();
                    if !parsed {
                        structured_schema_claim_failed = true;
                    }
                    parsed
                }
                _ => {
                    serde_json::from_str::<EvaluatorStructuredOutputV1>(completion.raw_text.trim())
                        .is_ok()
                }
            };
            structured_support_level(completion.enforcement, parsed_ok)
        }
        Err(_) => STRUCTURED_SUPPORT_UNTESTED,
    };

    let now = db::now_ts();
    let mut updated_profile = profile.clone();
    updated_profile.evaluator_last_tested_at = Some(now);
    updated_profile.evaluator_contract_version = CURRENT_EVALUATOR_CONTRACT_VERSION;
    updated_profile.evaluator_prompt_version = CURRENT_EVALUATOR_PROMPT_VERSION;
    updated_profile.structured_output_support = structured_output_support;

    let report = match validation_result {
        Ok(_) => {
            if structured_schema_claim_failed {
                updated_profile.evaluator_compatibility_status =
                    EVALUATOR_COMPAT_FAILED_SCHEMA_ENFORCED;
                updated_profile.evaluator_last_failure_reason = Some(
                    "structured_schema_claim_failed: json_schema response did not parse as evaluator_structured_v1".into(),
                );
            } else {
                updated_profile.evaluator_compatibility_status =
                    evaluator_compatibility_status_for_structured_support(
                        structured_output_support,
                        true,
                    );
                updated_profile.evaluator_last_failure_reason = None;
            }
            EvaluatorContractTestReport {
                passed: !structured_schema_claim_failed,
                errors: if structured_schema_claim_failed {
                    vec![
                        "structured_schema_claim_failed: json_schema response did not parse as evaluator_structured_v1"
                            .into(),
                    ]
                } else {
                    Vec::new()
                },
                raw_response: raw_response.clone(),
                structured_output_support,
                evaluator_compatibility_status: updated_profile.evaluator_compatibility_status,
                evaluator_compatibility_status_label: evaluator_compatibility_status_label(
                    updated_profile.evaluator_compatibility_status,
                )
                .into(),
            }
        }
        Err(err) => {
            updated_profile.evaluator_compatibility_status = EVALUATOR_COMPAT_FAILED;
            updated_profile.evaluator_last_failure_reason = Some(err.clone());
            EvaluatorContractTestReport {
                passed: false,
                errors: vec![err],
                raw_response: raw_response.clone(),
                structured_output_support,
                evaluator_compatibility_status: updated_profile.evaluator_compatibility_status,
                evaluator_compatibility_status_label: evaluator_compatibility_status_label(
                    updated_profile.evaluator_compatibility_status,
                )
                .into(),
            }
        }
    };

    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    let _ = db::upsert_provider_profile(&conn, &updated_profile);

    Ok(report)
}

/// One turn's result in the session form-eval benchmark.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFormEvalTurn {
    pub turn_index: usize,
    pub user_excerpt: String,
    /// The FORM eval output passed the system's ingestion/validation contract.
    pub form_passed: bool,
    /// How many form rows the compiler accepted (durable state actually extracted).
    pub form_rows_accepted: usize,
    pub form_error: Option<String>,
    /// Repair was attempted (only when form validation failed).
    pub repair_attempted: bool,
    pub repair_ops: usize,
    /// Repair produced a non-empty engine patch (recovered state) on dry-run.
    pub repair_recovered: bool,
    pub repair_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFormEvalReport {
    pub conversation_id: String,
    pub model: String,
    /// The model the repair stage actually ran against (repair profile /
    /// embedded local model) — distinct from the eval `model` above.
    pub repair_model: String,
    pub turns_total: usize,
    pub form_passed: usize,
    pub form_failed: usize,
    pub repair_recovered: usize,
    pub per_turn: Vec<SessionFormEvalTurn>,
}

/// Dev-mode benchmark: replay the OPEN session's chat log through the non-tool-call
/// FORM evaluator and the repair path, validating each result the way the live
/// system does (parse + compile + `validate_evaluator_contract`) but WITHOUT ever
/// applying anything to the ledger. For form turns that fail validation, it runs
/// the repair (reextract) and reports whether it would have recovered the state.
/// The reusable core (per-turn form eval over a log) also backs a future
/// user-mode "re-eval selected chats". Nothing here is committed.
#[tauri::command]
pub async fn run_session_form_eval_benchmark(
    window: Window,
    state: State<'_, AppState>,
    conversation_id: String,
    profile_id: String,
    repair_settings: Option<ApiProviderSettings>,
) -> Result<SessionFormEvalReport, String> {
    let repair_settings_override = repair_settings;
    let profile = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        db::get_provider_profile(&conn, &profile_id).map_err(|err| err.to_string())?
    };

    // FORM contract settings (force the non-tool-call path, like the contract
    // test). Local endpoints get the generous timeout — CPU form-prompt eval is
    // slow and the default 25s would always time out.
    let mut form_settings = ApiProviderSettings {
        base_url: profile.base_url.clone(),
        api_key: profile.api_key.clone(),
        model: profile.model.clone(),
        system_prompt: profile.system_prompt.clone(),
        evaluator_mode: Some(EVALUATOR_MODE_FORM_V1.into()),
        evaluator_timeout_mode: Some("finite".into()),
        structured_evaluator_max_retries: Some(1),
        ..ApiProviderSettings::default()
    };
    if is_loopback_endpoint(&form_settings.base_url) {
        form_settings.evaluator_timeout_ms = Some(LOCAL_REPAIR_TIMEOUT_MS);
    }

    // Repair settings: the caller passes the CONFIGURED repair endpoint (repair
    // profile / embedded local model), because the whole architecture is
    // "weak eval generates failures, the dedicated repair model fixes them" —
    // testing repair against the same weak eval profile would be meaningless.
    // Fall back to the eval profile only when no repair endpoint is configured.
    let mut repair_settings = repair_settings_override.unwrap_or_else(|| form_settings.clone());
    repair_settings.evaluator_mode = Some(EVALUATOR_MODE_STRUCTURED_V1.into());
    repair_settings.structured_require_ops = Some(true);
    if is_loopback_endpoint(&repair_settings.base_url) {
        repair_settings.structured_evaluator_transport = Some("json_schema".into());
        repair_settings.structured_evaluator_policy = Some("allow_fallback".into());
        repair_settings.structured_evaluator_timeout_ms = Some(LOCAL_REPAIR_TIMEOUT_MS);
        repair_settings.evaluator_timeout_ms = Some(LOCAL_REPAIR_TIMEOUT_MS);
    }

    // Load the session's current soul/world and its user→narrator turns. We use
    // the current rebuilt state as context for every turn (a dev-benchmark
    // simplification — it tests JSON validity/recovery, not exact historical state).
    let (soul, session_world, turns) = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        let branch = db::get_active_session_branch(&conn, &conversation_id)
            .map_err(|err| err.to_string())?;
        let rebuilt = db::rebuild_session_state(&conn, &conversation_id, &branch.branch_id)
            .map_err(|err| err.to_string())?;
        let messages =
            db::list_messages(&conn, &conversation_id, 10_000).map_err(|err| err.to_string())?;
        let mut turns: Vec<(String, String)> = Vec::new();
        let mut pending_user: Option<String> = None;
        for message in messages.iter().filter(|message| message.status == "active") {
            match message.role.as_str() {
                "user" => pending_user = Some(message.content.clone()),
                "assistant" => {
                    if let Some(user_text) = pending_user.take() {
                        turns.push((user_text, strip_hidden_state_blocks(&message.content)));
                    }
                }
                _ => {}
            }
        }
        (rebuilt.soul, rebuilt.session_world, turns)
    };

    let provider = ApiProvider::default();
    let structured_system = build_structured_evaluator_prompt(&soul, Some(&session_world));
    // v1 uses default player aliases; the future selected-chats feature can resolve
    // the real persona per session.
    let player_id = "preset_male";
    let player_name = "Male Persona";

    let mut per_turn = Vec::new();
    let (mut form_passed, mut form_failed, mut repair_recovered) = (0usize, 0usize, 0usize);
    let job_id = format!("form_eval_{}", uuid_like_id());
    let started_at = db::now_ts();
    let started_clock = Instant::now();
    let mut job_history: Vec<BackgroundJobHistoryEntry> = Vec::new();

    emit_background_job_progress(
        &window,
        &BackgroundJobProgress {
            job_id: job_id.clone(),
            kind: "form_eval".into(),
            label: "Form Evaluator Dry Run".into(),
            status: "running".into(),
            phase: "preparing".into(),
            current: 0,
            total: turns.len(),
            succeeded: 0,
            failed: 0,
            recovered: 0,
            started_at,
            updated_at: db::now_ts(),
            elapsed_ms: 0,
            estimated_remaining_ms: None,
            detail: Some(format!("Replaying {} visible turn(s)", turns.len())),
            cancellable: false,
            history: Vec::new(),
        },
    );

    for (index, (user, narrator)) in turns.iter().enumerate() {
        let turn_started = Instant::now();
        emit_background_job_progress(
            &window,
            &BackgroundJobProgress {
                job_id: job_id.clone(),
                kind: "form_eval".into(),
                label: "Form Evaluator Dry Run".into(),
                status: "running".into(),
                phase: "evaluating".into(),
                current: index,
                total: turns.len(),
                succeeded: form_passed,
                failed: form_failed,
                recovered: repair_recovered,
                started_at,
                updated_at: db::now_ts(),
                elapsed_ms: started_clock.elapsed().as_millis() as u64,
                estimated_remaining_ms: None,
                detail: Some(format!("Evaluating turn {}/{}", index + 1, turns.len())),
                cancellable: false,
                history: job_history.clone(),
            },
        );
        let mut turn = SessionFormEvalTurn {
            turn_index: index,
            user_excerpt: user.chars().take(80).collect(),
            form_passed: false,
            form_rows_accepted: 0,
            form_error: None,
            repair_attempted: false,
            repair_ops: 0,
            repair_recovered: false,
            repair_error: None,
        };

        let system = build_evaluator_form_prompt_compact_with_player_persona(
            &soul,
            Some(&session_world),
            user,
            narrator,
            player_id,
            player_name,
        );
        let context = format!("User: {user}\nNarrator: {narrator}");
        let user_message = build_evaluator_user_message(
            user,
            narrator,
            &context,
            Some(&session_world),
            None,
            None,
        );

        let form_raw =
            match complete_evaluator_with_config(&provider, &form_settings, &system, &user_message)
                .await
            {
                Err(err) => {
                    turn.form_error = Some(format!("form call failed: {err}"));
                    None
                }
                Ok(completion) => Some(completion.raw_text),
            };

        // Mirror the LIVE ingestion (parse + raw_repair salvage + compile), NOT the
        // stricter contract validator. "Taken by the system" = it produced durable
        // state with no rejected rows; otherwise it's a form failure to repair.
        let needs_repair = if let Some(raw) = form_raw.as_deref() {
            let spec = build_eval_form_spec_with_player_persona(
                &soul,
                Some(&session_world),
                user,
                narrator,
                8,
                player_id,
                player_name,
            );
            match compile_evaluator_form_runtime(
                raw,
                spec,
                &soul,
                &session_world,
                user,
                narrator,
                None,
            ) {
                Err(err) => {
                    turn.form_error = Some(format!("form ingest failed: {err}"));
                    form_failed += 1;
                    true
                }
                Ok(form_outcome) => {
                    let rejected = form_outcome.form_rejected_rows.len();
                    turn.form_rows_accepted = form_outcome
                        .form_trace
                        .as_ref()
                        .map(|trace| trace.form_rows_accepted)
                        .unwrap_or(0);
                    // A real pass requires: non-empty patch, no rejected rows, and NOT the
                    // partial_success path — the minimal-scene fallback (used when parse
                    // fails outright, or when the form compiled to nothing) also yields a
                    // non-empty patch with zero rejections, and must not masquerade as OK.
                    if !form_outcome.conversion.patch.is_empty()
                        && rejected == 0
                        && !form_outcome.partial_success
                    {
                        turn.form_passed = true;
                        form_passed += 1;
                        false
                    } else {
                        turn.form_error = Some(
                            if let Some(reason) = form_outcome.partial_success_reason.as_deref() {
                                format!("form fell back: {reason}")
                            } else if rejected > 0 {
                                format!(
                                    "{rejected} form row(s) rejected: {}",
                                    form_outcome
                                        .form_rejected_rows
                                        .first()
                                        .map(|row| row.reason.clone())
                                        .unwrap_or_default()
                                )
                            } else {
                                "form produced no durable state (empty patch)".into()
                            },
                        );
                        form_failed += 1;
                        true
                    }
                }
            }
        } else {
            form_failed += 1;
            true
        };

        if needs_repair {
            emit_background_job_progress(
                &window,
                &BackgroundJobProgress {
                    job_id: job_id.clone(),
                    kind: "form_eval".into(),
                    label: "Form Evaluator Dry Run".into(),
                    status: "running".into(),
                    phase: "repairing".into(),
                    current: index,
                    total: turns.len(),
                    succeeded: form_passed,
                    failed: form_failed,
                    recovered: repair_recovered,
                    started_at,
                    updated_at: db::now_ts(),
                    elapsed_ms: started_clock.elapsed().as_millis() as u64,
                    estimated_remaining_ms: None,
                    detail: Some(format!("Repairing turn {}/{}", index + 1, turns.len())),
                    cancellable: false,
                    history: job_history.clone(),
                },
            );
            turn.repair_attempted = true;
            let repair_user = build_reextract_user_message(user, narrator);
            match provider
                .complete_structured_prompt(
                    &repair_settings,
                    &structured_system,
                    &repair_user,
                    0.3,
                    Some(Duration::from_millis(LOCAL_REPAIR_TIMEOUT_MS)),
                    EVALUATOR_OPS_REPAIR_SCHEMA_NAME,
                    &evaluator_ops_repair_json_schema(),
                )
                .await
            {
                Err(err) => turn.repair_error = Some(format!("repair call failed: {err}")),
                Ok(repair_completion) => match compile_evaluator_structured_runtime(
                    &repair_completion.raw_text,
                    Some(StructuredEnforcement::JsonSchema),
                    &soul,
                    &session_world,
                    user,
                    narrator,
                    None,
                    repair_settings.structured_require_ops == Some(true),
                ) {
                    Err(err) => turn.repair_error = Some(err),
                    Ok(outcome) => {
                        turn.repair_ops = outcome.structured_ops_count.unwrap_or(0);
                        turn.repair_recovered = !outcome.conversion.patch.is_empty();
                        if turn.repair_recovered {
                            repair_recovered += 1;
                        }
                    }
                },
            }
        }
        let turn_status = if turn.form_passed {
            "succeeded"
        } else if turn.repair_recovered {
            "recovered"
        } else {
            "failed"
        };
        job_history.push(BackgroundJobHistoryEntry {
            index: index + 1,
            label: format!("Turn {}", index + 1),
            status: turn_status.into(),
            detail: turn
                .form_error
                .clone()
                .or_else(|| turn.repair_error.clone())
                .or_else(|| Some(turn.user_excerpt.clone())),
            elapsed_ms: Some(turn_started.elapsed().as_millis() as u64),
        });
        per_turn.push(turn);
        let elapsed_ms = started_clock.elapsed().as_millis() as u64;
        let completed = index + 1;
        let average_ms = elapsed_ms / completed.max(1) as u64;
        emit_background_job_progress(
            &window,
            &BackgroundJobProgress {
                job_id: job_id.clone(),
                kind: "form_eval".into(),
                label: "Form Evaluator Dry Run".into(),
                status: "running".into(),
                phase: "turn_complete".into(),
                current: completed,
                total: turns.len(),
                succeeded: form_passed,
                failed: form_failed,
                recovered: repair_recovered,
                started_at,
                updated_at: db::now_ts(),
                elapsed_ms,
                estimated_remaining_ms: average_ms
                    .checked_mul(turns.len().saturating_sub(completed) as u64),
                detail: Some(format!("Completed turn {completed}/{}", turns.len())),
                cancellable: false,
                history: job_history.clone(),
            },
        );
    }

    let report = SessionFormEvalReport {
        conversation_id,
        model: form_settings.model.clone(),
        repair_model: repair_settings.model.clone(),
        turns_total: turns.len(),
        form_passed,
        form_failed,
        repair_recovered,
        per_turn,
    };
    emit_background_job_progress(
        &window,
        &BackgroundJobProgress {
            job_id,
            kind: "form_eval".into(),
            label: "Form Evaluator Dry Run".into(),
            status: if form_failed > repair_recovered {
                "failed".into()
            } else {
                "succeeded".into()
            },
            phase: "complete".into(),
            current: turns.len(),
            total: turns.len(),
            succeeded: form_passed,
            failed: form_failed,
            recovered: repair_recovered,
            started_at,
            updated_at: db::now_ts(),
            elapsed_ms: started_clock.elapsed().as_millis() as u64,
            estimated_remaining_ms: Some(0),
            detail: Some(format!(
                "{form_passed}/{} form-valid; {repair_recovered} recovered",
                turns.len()
            )),
            cancellable: false,
            history: job_history,
        },
    );
    Ok(report)
}

#[tauri::command]
pub async fn run_structured_evaluator_diagnostic(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    profile_id: Option<String>,
) -> Result<StructuredEvaluatorDiagnosticSummary, String> {
    let profile = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        if let Some(profile_id) = profile_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            db::get_provider_profile(&conn, profile_id).map_err(|err| err.to_string())?
        } else {
            db::list_provider_profiles(&conn)
                .map_err(|err| err.to_string())?
                .into_iter()
                .find(|profile| {
                    !profile.model.trim().is_empty()
                        && !profile.base_url.trim().is_empty()
                        && !profile.api_key.trim().is_empty()
                })
                .ok_or_else(|| {
                    "No configured provider profile with base_url, model, and API key found."
                        .to_string()
                })?
        }
    };
    let structured_policy = "required".to_string();
    let settings = diagnostic_structured_settings_from_profile(&profile, &structured_policy);
    let structured_mode_requested = EVALUATOR_MODE_STRUCTURED_V1.to_string();
    let provider = ApiProvider::default();
    let conversation_id = format!("structured-diagnostic-{}", uuid_like_id());
    let mut soul = new_default_soul("Aurora Diagnostic");
    soul.soul_kind = "session_clone".into();
    soul.created_from_name = Some("Structured Evaluator Diagnostic".into());
    let mut session_world = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        db::upsert_soul(&conn, &soul).map_err(|err| err.to_string())?;
        db::ensure_conversation_with_title_and_world(
            &conn,
            &conversation_id,
            &soul.character_id,
            None,
            None,
            Some("Structured Evaluator Diagnostic"),
        )
        .map_err(|err| err.to_string())?;
        let world = db::ensure_conversation_session_world(&conn, &conversation_id, &soul, None)
            .map_err(|err| err.to_string())?;
        db::set_active_player_persona(&conn, &conversation_id, "preset_male")
            .map_err(|err| err.to_string())?;
        db::set_active_evaluator_profile(&conn, &conversation_id, Some(&profile.id))
            .map_err(|err| err.to_string())?;
        db::create_session_branch(&conn, &conversation_id, &soul, &world)
            .map_err(|err| err.to_string())?;
        world
    };
    let (structured_mode_resolved, resolved_evaluator_source) = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        let resolved_setting = resolve_evaluator_mode_setting(&conn, &conversation_id, &settings)
            .or_else(|| settings.evaluator_mode.clone());
        let resolved_mode = evaluator_mode(&ApiProviderSettings {
            evaluator_mode: resolved_setting,
            ..settings.clone()
        });
        let resolved_source = selected_evaluator_source(&resolved_mode).to_string();
        if resolved_source != EVALUATOR_MODE_STRUCTURED_V1 {
            return Err(format!(
                "Structured diagnostic refused to run: requested {structured_mode_requested}, resolved mode {resolved_mode}, selected source {resolved_source}"
            ));
        }
        (resolved_mode, resolved_source)
    };

    emit_dev_log(
        &window,
        "info",
        "evaluator",
        "structured_evaluator_diagnostic_started",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str(),
            "profile_id": profile.id.as_str(),
            "model": profile.model.trim(),
            "base_url": redact_base_url(&profile.base_url),
            "structured_mode_requested": structured_mode_requested.as_str(),
            "structured_mode_resolved": structured_mode_resolved.as_str(),
            "resolved_evaluator_source": resolved_evaluator_source.as_str(),
            "structured_policy": structured_policy.as_str()
        })),
    );

    let turns = structured_diagnostic_turns();
    let diagnostic_job_id = format!("structured-diagnostic-{}", uuid_like_id());
    let diagnostic_started_at = db::now_ts();
    let diagnostic_started_clock = Instant::now();
    let mut diagnostic_succeeded = 0usize;
    let mut diagnostic_failed = 0usize;
    let mut diagnostic_recovered = 0usize;
    let mut diagnostic_job_history = Vec::<BackgroundJobHistoryEntry>::new();
    emit_background_job_progress(
        &window,
        &BackgroundJobProgress {
            job_id: diagnostic_job_id.clone(),
            kind: "structured_diagnostic".into(),
            label: "Structured evaluator diagnostic".into(),
            status: "running".into(),
            phase: "preparing".into(),
            current: 0,
            total: turns.len(),
            succeeded: 0,
            failed: 0,
            recovered: 0,
            started_at: diagnostic_started_at,
            updated_at: db::now_ts(),
            elapsed_ms: 0,
            estimated_remaining_ms: None,
            detail: Some(format!(
                "{} strict structured turns queued for {}",
                turns.len(),
                profile.model.trim()
            )),
            cancellable: false,
            history: Vec::new(),
        },
    );
    let mut runs = Vec::new();
    let mut total_memory_ops = 0usize;
    let mut total_relationship_ops = 0usize;
    let mut total_object_ops = 0usize;
    let mut total_scene_ops = 0usize;
    let mut syntactic_repair_used = false;
    let mut default_player_relationship_context_seen = false;

    for (index, (user_text, narrator_text)) in turns.iter().enumerate() {
        let current_turn = index + 1;
        emit_background_job_progress(
            &window,
            &BackgroundJobProgress {
                job_id: diagnostic_job_id.clone(),
                kind: "structured_diagnostic".into(),
                label: "Structured evaluator diagnostic".into(),
                status: "running".into(),
                phase: "evaluating".into(),
                current: index,
                total: turns.len(),
                succeeded: diagnostic_succeeded,
                failed: diagnostic_failed,
                recovered: diagnostic_recovered,
                started_at: diagnostic_started_at,
                updated_at: db::now_ts(),
                elapsed_ms: diagnostic_started_clock.elapsed().as_millis() as u64,
                estimated_remaining_ms: None,
                detail: Some(format!(
                    "Evaluating strict structured turn {current_turn}/{}",
                    turns.len()
                )),
                cancellable: false,
                history: diagnostic_job_history.clone(),
            },
        );
        let (user_message_id, assistant_message_id, branch_id, parent_turn_id, recent_excerpt) = {
            let conn = state.conn.lock().map_err(|err| err.to_string())?;
            let user_id = db::insert_message_and_get_id(&conn, &conversation_id, "user", user_text)
                .map_err(|err| err.to_string())?;
            let assistant_id =
                db::insert_message_and_get_id(&conn, &conversation_id, "assistant", narrator_text)
                    .map_err(|err| err.to_string())?;
            let branch = db::get_active_session_branch(&conn, &conversation_id)
                .map_err(|err| err.to_string())?;
            let messages =
                db::list_messages(&conn, &conversation_id, 50).map_err(|err| err.to_string())?;
            let excerpt = messages
                .iter()
                .map(|message| format!("{}: {}", message.role, message.content))
                .collect::<Vec<_>>()
                .join("\n");
            (
                user_id,
                assistant_id,
                branch.branch_id,
                branch.active_turn_id,
                excerpt,
            )
        };
        let diagnostic_turn_id = format!("structured_diag_turn_{}_{}", index + 1, uuid_like_id());

        let form_spec = build_eval_form_spec_with_player_persona(
            &soul,
            Some(&session_world),
            user_text,
            narrator_text,
            8,
            "preset_male",
            "Male Persona",
        );
        let updater_system_prompt = build_structured_evaluator_prompt_with_player_persona(
            &soul,
            Some(&session_world),
            "preset_male",
            "Male Persona",
        );
        let prompt_has_default_player =
            default_player_in_evaluator_relationship_context(&updater_system_prompt);
        default_player_relationship_context_seen |= prompt_has_default_player;
        let updater_user_message = build_evaluator_user_message(
            user_text,
            narrator_text,
            &recent_excerpt,
            Some(&session_world),
            Some("Latest normal RP speaker entity_id: preset_male\nActive player persona: preset_male"),
            None,
        );
        let perception_v2_prompt = build_perception_v2_prompt_with_player_persona(
            &soul,
            Some(&session_world),
            "preset_male",
            "Male Persona",
        );
        let perception_source = SourceEnvelope::new(
            SourceIdentity {
                conversation_id: conversation_id.clone(),
                branch_id: branch_id.clone(),
                turn_id: diagnostic_turn_id.clone(),
                parent_turn_id: parent_turn_id.clone(),
                user_message_id,
                assistant_message_id,
                assistant_variant_id: None,
            },
            vec![soul.character_id.clone()],
            *user_text,
            *narrator_text,
            None,
            db::now_ts().saturating_mul(1000),
        )
        .map_err(|error| format!("failed to create V2 shadow source: {error}"))?;
        let perception_catalog =
            compiler_entity_catalog(&soul, &session_world, "preset_male", "Male Persona");
        let perception_snapshot = SimulationSnapshot {
            state_hash: perception_source.parent_state_hash().map(str::to_string),
            existing_effect_ids: Vec::new(),
        };
        let mut perception_shadow = run_perception_v2_shadow(
            &provider,
            &settings,
            &perception_source,
            perception_catalog,
            &perception_snapshot,
            &perception_v2_prompt,
            &updater_user_message,
        )
        .await;
        let perception_run_id = format!("perception_shadow_{}_{}", index + 1, uuid_like_id());
        let perception_candidates = perception_shadow
            .batch
            .as_ref()
            .map(|batch| {
                batch
                    .candidates
                    .iter()
                    .enumerate()
                    .map(|(candidate_index, candidate)| db::CompilerCandidateRecord {
                        run_id: perception_run_id.clone(),
                        candidate_id: candidate.candidate_id.clone(),
                        candidate_index,
                        kind: serde_json::to_value(candidate.perception.kind)
                            .ok()
                            .and_then(|value| value.as_str().map(str::to_string))
                            .unwrap_or_else(|| "unknown".into()),
                        disposition: perception_shadow
                            .pipeline
                            .as_ref()
                            .and_then(|pipeline| {
                                pipeline.semantic.candidates.iter().find(|validated| {
                                    validated.candidate.candidate.candidate_id
                                        == candidate.candidate_id
                                })
                            })
                            .map(|validated| match validated.disposition {
                                state_engine::compiler::SemanticDisposition::Accepted => "accepted",
                                state_engine::compiler::SemanticDisposition::Rejected => "rejected",
                            })
                            .unwrap_or("shadow")
                            .into(),
                        candidate_json: serde_json::to_string(candidate)
                            .unwrap_or_else(|_| "{}".into()),
                        diagnostics_json: perception_shadow
                            .pipeline
                            .as_ref()
                            .map(|pipeline| {
                                let diagnostics = pipeline
                                    .binding
                                    .diagnostics
                                    .iter()
                                    .chain(pipeline.semantic.diagnostics.iter())
                                    .chain(pipeline.lowering.diagnostics.iter())
                                    .chain(pipeline.simulation.diagnostics.iter())
                                    .filter(|diagnostic| {
                                        diagnostic.candidate_id.as_deref()
                                            == Some(candidate.candidate_id.as_str())
                                    })
                                    .collect::<Vec<_>>();
                                serde_json::to_string(&diagnostics).unwrap_or_else(|_| "[]".into())
                            })
                            .unwrap_or_else(|| "[]".into()),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        {
            let conn = state.conn.lock().map_err(|err| err.to_string())?;
            db::record_compiler_run(
                &conn,
                &db::CompilerRunRecord {
                    run_id: perception_run_id,
                    conversation_id: conversation_id.clone(),
                    branch_id: branch_id.clone(),
                    turn_id: diagnostic_turn_id.clone(),
                    source_hash: perception_source.source_hash().into(),
                    mode: "perception_v2_shadow".into(),
                    schema_version: PERCEPTION_IR_SCHEMA_VERSION,
                    compiler_version: MEMORY_COMPILER_CONTRACT_VERSION,
                    provider: evaluator_provider_label(EVALUATOR_MODE_STRUCTURED_V1, false),
                    model: settings.model.trim().into(),
                    prompt_version: PERCEPTION_V2_PROMPT_VERSION.into(),
                    status: perception_shadow.trace.status.clone(),
                    enforcement_level: perception_shadow.trace.enforcement_level.clone(),
                    raw_response_json: perception_shadow.raw_response.clone(),
                    artifact_json: serde_json::to_string(&serde_json::json!({
                        "batch": perception_shadow.batch.as_ref(),
                        "pipeline": perception_shadow.pipeline.as_ref()
                    }))
                    .ok(),
                    error_message: perception_shadow.trace.error.clone(),
                    commit_allowed: false,
                    created_at: db::now_ts(),
                },
                &perception_candidates,
            )
            .map_err(|error| format!("failed to persist V2 shadow trace: {error}"))?;
        }
        emit_dev_log(
            &window,
            if perception_shadow.trace.schema_validated {
                "success"
            } else {
                "warn"
            },
            "evaluator",
            "perception_v2_shadow_completed",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "turn_index": index + 1,
                "source_hash": perception_source.source_hash(),
                "candidate_count": perception_shadow.trace.candidate_count,
                "schema_validated": perception_shadow.trace.schema_validated,
                "commit_allowed": false,
                "commit_count": 0,
                "error": perception_shadow.trace.error.as_deref()
            })),
        );
        let token_estimate =
            estimate_tokens(&updater_system_prompt) + estimate_tokens(&updater_user_message);
        let request_id = format!("structured_diag_{}_{}", index + 1, uuid_like_id());
        let updater_log_id = {
            let conn = state.conn.lock().map_err(|err| err.to_string())?;
            db::insert_llm_payload_log(
                &conn,
                &LlmPayloadLog {
                    conversation_id: conversation_id.clone(),
                    message_id: Some(assistant_message_id),
                    provider: evaluator_provider_label(EVALUATOR_MODE_STRUCTURED_V1, false),
                    mode: structured_mode_resolved.clone(),
                    context_mode: "structured_diagnostic".into(),
                    model: settings.model.trim().to_string(),
                    base_url: redact_base_url(&settings.base_url),
                    system_message: updater_system_prompt.clone(),
                    user_message: updater_user_message.clone(),
                    context_text: updater_system_prompt.clone(),
                    estimated_system_tokens: estimate_tokens(&updater_system_prompt),
                    estimated_user_tokens: estimate_tokens(&updater_user_message),
                    estimated_total_tokens: token_estimate,
                    truncated: false,
                    created_at: db::now_ts(),
                    branch_id: Some(branch_id.clone()),
                    active_turn_id: parent_turn_id.clone(),
                    parent_turn_id: parent_turn_id.clone(),
                    request_id: Some(request_id.clone()),
                    ..Default::default()
                },
            )
            .map_err(|err| err.to_string())?
        };

        emit_dev_log(
            &window,
            "info",
            "evaluator",
            "evaluator_called",
            Some(serde_json::json!({
                "conversation_id": conversation_id.as_str(),
                "assistant_message_id": assistant_message_id,
                "model": settings.model.trim(),
                "base_url": redact_base_url(&settings.base_url),
                "evaluator_mode": EVALUATOR_MODE_STRUCTURED_V1,
                "structured_mode_requested": structured_mode_requested.as_str(),
                "structured_mode_resolved": structured_mode_resolved.as_str(),
                "resolved_evaluator_source": resolved_evaluator_source.as_str(),
                "structured_policy": structured_policy.as_str(),
                "selected_evaluator_source": EVALUATOR_MODE_STRUCTURED_V1,
                "diagnostic_turn_index": index + 1
            })),
        );

        let completion = complete_evaluator_with_config(
            &provider,
            &settings,
            &updater_system_prompt,
            &updater_user_message,
        )
        .await;
        let (raw_response, structured_enforcement) = match completion.as_ref() {
            Ok(completion) => (
                Some(completion.raw_text.clone()),
                completion.structured_enforcement,
            ),
            Err(_) => (None, None),
        };
        let completion_trace = completion
            .as_ref()
            .map(|completion| completion.trace.clone())
            .unwrap_or_default();
        let structured_enforcement_requested = structured_enforcement
            .map(StructuredEnforcement::as_label)
            .unwrap_or("none")
            .to_string();
        let mut run_failure_reasons = Vec::<String>::new();
        if prompt_has_default_player {
            run_failure_reasons.push("default_player_in_structured_prompt".into());
        }
        if let Some(raw) = raw_response.as_ref() {
            let conn = state.conn.lock().map_err(|err| err.to_string())?;
            let _ = db::update_llm_payload_log_response(
                &conn,
                updater_log_id,
                &db::LlmPayloadResponseUpdate {
                    raw_provider_response: Some(raw.clone()),
                    normalized_response: Some(raw.clone()),
                    ..Default::default()
                },
            );
        }

        let structured_step = structured_fallback_step(structured_enforcement).to_string();
        let mut pending_retry_failure: Option<StructuredRetryFailure> = None;
        let outcome_result = match completion {
            Ok(completion) => match compile_selected_evaluator_runtime(
                EVALUATOR_MODE_STRUCTURED_V1,
                Some(form_spec.clone()),
                &completion.raw_text,
                completion.structured_enforcement,
                &soul,
                &session_world,
                user_text,
                narrator_text,
                None,
                false,
            ) {
                Ok(mut outcome) => {
                    apply_completion_retry_trace(&mut outcome, &completion.trace);
                    Ok(outcome)
                }
                Err(err) => {
                    if completion.structured_enforcement == Some(StructuredEnforcement::JsonSchema)
                    {
                        run_failure_reasons.push("schema_claim_not_validated".into());
                        emit_dev_log(
                            &window,
                            "warn",
                            "evaluator",
                            "structured_schema_claim_failed",
                            Some(serde_json::json!({
                                "conversation_id": conversation_id.as_str(),
                                "assistant_message_id": assistant_message_id,
                                "structured_enforcement_requested": StructuredEnforcement::JsonSchema.as_label(),
                                "structured_schema_validation_status": structured_validation_status_from_error(&err),
                                "structured_schema_validation_error": err.as_str()
                            })),
                        );
                    }
                    emit_dev_log(
                        &window,
                        "error",
                        "evaluator",
                        "structured_evaluator_failed",
                        Some(serde_json::json!({
                            "conversation_id": conversation_id.as_str(),
                            "assistant_message_id": assistant_message_id,
                            "error": err.as_str(),
                            "structured_enforcement": structured_enforcement.map(StructuredEnforcement::as_label)
                        })),
                    );
                    match retry_structured_tool_call_after_compile_failure(
                        &provider,
                        &settings,
                        &updater_system_prompt,
                        &updater_user_message,
                        &completion,
                        &err,
                        &soul,
                        &session_world,
                        user_text,
                        narrator_text,
                        None,
                    )
                    .await
                    {
                        Ok(outcome) => Ok(outcome),
                        Err(retry_failure) => {
                            if err.contains("malformed_schema_output") {
                                run_failure_reasons.push("malformed_schema_output".into());
                            }
                            if err.contains("zero_ops_on_durable_turn") {
                                run_failure_reasons.push("zero_ops_on_durable_turn".into());
                            }
                            run_failure_reasons.extend(retry_failure.retry_reasons.iter().cloned());
                            run_failure_reasons
                                .push("strict_tool_structured_validation_failed".into());
                            pending_retry_failure = Some(retry_failure.clone());
                            Err(retry_failure.final_error)
                        }
                    }
                }
            },
            Err(err) => {
                if structured_enforcement == Some(StructuredEnforcement::JsonSchema) {
                    run_failure_reasons.push("schema_claim_not_validated".into());
                    emit_dev_log(
                        &window,
                        "warn",
                        "evaluator",
                        "structured_schema_claim_failed",
                        Some(serde_json::json!({
                            "conversation_id": conversation_id.as_str(),
                            "assistant_message_id": assistant_message_id,
                            "structured_enforcement_requested": StructuredEnforcement::JsonSchema.as_label(),
                            "structured_schema_validation_status": structured_validation_status_from_error(&err),
                            "structured_schema_validation_error": err.as_str()
                        })),
                    );
                }
                emit_dev_log(
                    &window,
                    "error",
                    "evaluator",
                    "structured_evaluator_failed",
                    Some(serde_json::json!({
                        "conversation_id": conversation_id.as_str(),
                        "assistant_message_id": assistant_message_id,
                        "error": err.as_str(),
                        "structured_enforcement": structured_enforcement.map(StructuredEnforcement::as_label)
                    })),
                );
                run_failure_reasons.push("strict_tool_call_failed".into());
                Err(err)
            }
        };

        let mut error = None;
        let mut outcome = match outcome_result {
            Ok(outcome) => outcome,
            Err(err) => {
                error = Some(err.clone());
                let mut outcome =
                    strict_tool_diagnostic_failed_outcome(vec![structured_step.clone()], err);
                if let Some(retry_failure) = pending_retry_failure.as_ref() {
                    apply_structured_retry_failure(&mut outcome, retry_failure);
                }
                outcome
            }
        };
        let durable_kind = durable_change_required(user_text, narrator_text);
        let mut patch = outcome.conversion.patch.clone();
        let pre_guarantee_counts = diagnostic_patch_counts(&patch);
        if error.is_none()
            && durable_kind == Some(DurableChangeKind::Object)
            && pre_guarantee_counts.object_update_ops_count == 0
        {
            if outcome
                .fallback_path
                .iter()
                .any(|step| step == EVALUATOR_MODE_FORM_V1)
            {
                run_failure_reasons.push("fallback_form_empty".into());
            }
            merge_world_guarantee_patch(
                &mut patch,
                diagnostic_object_scene_guarantee_patch(&soul, user_text, narrator_text),
            );
            outcome.conversion.patch = patch.clone();
        }
        if error.is_none() && durable_kind.is_some() && diagnostic_total_patch_ops(&patch) == 0 {
            run_failure_reasons.push("zero_ops_on_durable_turn".into());
        }
        if outcome
            .fallback_path
            .iter()
            .any(|step| step == EVALUATOR_MODE_FORM_V1)
        {
            run_failure_reasons.push("strict_tool_forbids_evaluator_form_v1_fallback".into());
        }
        if !outcome.structured_enforcement_validated {
            run_failure_reasons.push("structured_enforcement_not_validated".into());
        }
        if structured_enforcement != Some(StructuredEnforcement::ToolCall) {
            run_failure_reasons.push("tool_call_not_validated".into());
        }
        run_failure_reasons.sort();
        run_failure_reasons.dedup();
        let patch_counts = diagnostic_patch_counts(&patch);
        perception_shadow.trace.v1_ops_count = Some(diagnostic_total_patch_ops(&patch));
        total_memory_ops += patch_counts.memory_ops_count;
        total_relationship_ops += patch_counts.relationship_event_ops_count;
        total_object_ops += patch_counts.object_update_ops_count;
        total_scene_ops += patch_counts.scene_update_ops_count;
        syntactic_repair_used |= outcome.syntactic_repair_used;

        let state_patch_id = {
            let conn = state.conn.lock().map_err(|err| err.to_string())?;
            let (_commit, patch_record) = db::record_turn_commit_with_patch_for_turn_id(
                &conn,
                &diagnostic_turn_id,
                &conversation_id,
                &branch_id,
                parent_turn_id.as_deref(),
                Some(user_message_id),
                assistant_message_id,
                None,
                &patch,
                false,
            )
            .map_err(|err| err.to_string())?;
            let rebuilt = db::rebuild_session_state(&conn, &conversation_id, &branch_id)
                .map_err(|err| err.to_string())?;
            soul = rebuilt.soul;
            session_world = rebuilt.session_world;
            let _ = db::set_llm_payload_log_ledger_metadata(
                &conn,
                updater_log_id,
                &rebuilt.debug,
                parent_turn_id.as_deref(),
                None,
            );
            let trace = serde_json::json!({
                "diagnostic": true,
                "evaluator_mode": EVALUATOR_MODE_STRUCTURED_V1,
                "structured_mode_requested": structured_mode_requested.as_str(),
                "structured_mode_resolved": structured_mode_resolved.as_str(),
                "resolved_evaluator_source": resolved_evaluator_source.as_str(),
                "structured_policy": structured_policy.as_str(),
                "structured_schema_version": state_engine::evaluator_structured::EVALUATOR_STRUCTURED_SCHEMA_VERSION,
                "structured_compiler_version": state_engine::evaluator_structured::EVALUATOR_STRUCTURED_COMPILER_VERSION,
                "structured_enforcement": structured_enforcement.map(StructuredEnforcement::as_label).unwrap_or("none"),
                "structured_enforcement_requested": structured_enforcement_requested.as_str(),
                "structured_enforcement_validated": outcome.structured_enforcement_validated,
                "structured_schema_validation_status": outcome.structured_schema_validation_status.as_str(),
                "structured_schema_validation_error": outcome.structured_schema_validation_error.as_deref(),
                "fallback_path": &outcome.fallback_path,
                "fallback_used": outcome.fallback_path.iter().any(|step| step == EVALUATOR_MODE_FORM_V1),
                "failure_reasons": &run_failure_reasons,
                "tool_calls_present": completion_trace.tool_calls_present,
                "tool_call_count": completion_trace.tool_call_count,
                "tool_call_names": &completion_trace.tool_call_names,
                "raw_content_present": completion_trace.raw_content_present,
                "raw_tool_calls_present": completion_trace.raw_tool_calls_present,
                "structured_retry_count": outcome.structured_retry_count,
                "structured_retry_reasons": &outcome.structured_retry_reasons,
                "structured_retry_succeeded": outcome.structured_retry_succeeded,
                "structured_retry_final_error": outcome.structured_retry_final_error.as_deref(),
                "ops_count": outcome.structured_ops_count.unwrap_or_else(|| diagnostic_total_patch_ops(&patch)),
                "compiled_patch_summary": engine_patch_summary(&patch),
                "syntactic_repair_used": outcome.syntactic_repair_used,
                "perception_v2_shadow": &perception_shadow.trace,
                "ledger_apply_trace": {
                    "state_patch_id": patch_record.patch_id,
                    "turn_commit_id": diagnostic_turn_id,
                    "branch_id": branch_id,
                    "patch_stored": true,
                    "patch_applied": !patch.is_empty(),
                    "branch_rebuilt": true,
                    "applied_patch_count": rebuilt.debug.applied_patches.len(),
                    "skipped_patch_count": rebuilt.debug.skipped_discarded_patches.len(),
                    "invalidated_patch_count": rebuilt.debug.invalidated_patches.len()
                },
                "before_after_state_summary": {
                    "after": compact_state_summary_json(&soul, &session_world)
                }
            });
            let _ = update_llm_payload_pipeline_trace(&conn, updater_log_id, &trace);
            patch_record.patch_id
        };

        runs.push(StructuredEvaluatorDiagnosticRun {
            turn_index: index + 1,
            user_message: (*user_text).to_string(),
            narrator_response: (*narrator_text).to_string(),
            evaluator_mode: structured_mode_resolved.clone(),
            enforcement_level: structured_enforcement
                .map(StructuredEnforcement::as_label)
                .unwrap_or("none")
                .to_string(),
            structured_enforcement_requested,
            structured_enforcement_validated: outcome.structured_enforcement_validated,
            structured_schema_validation_status: outcome
                .structured_schema_validation_status
                .clone(),
            structured_schema_validation_error: outcome.structured_schema_validation_error.clone(),
            fallback_path: outcome.fallback_path.clone(),
            failure_reasons: run_failure_reasons,
            ops_count: outcome
                .structured_ops_count
                .unwrap_or_else(|| diagnostic_total_patch_ops(&patch)),
            compiled_patch_summary: engine_patch_summary(&patch),
            syntactic_repair_used: outcome.syntactic_repair_used,
            memory_ops_count: patch_counts.memory_ops_count,
            relationship_event_ops_count: patch_counts.relationship_event_ops_count,
            object_update_ops_count: patch_counts.object_update_ops_count,
            scene_update_ops_count: patch_counts.scene_update_ops_count,
            state_patch_id: Some(state_patch_id),
            error,
            tool_calls_present: completion_trace.tool_calls_present,
            tool_call_count: completion_trace.tool_call_count,
            tool_call_names: completion_trace.tool_call_names,
            raw_content_present: completion_trace.raw_content_present,
            raw_tool_calls_present: completion_trace.raw_tool_calls_present,
            structured_retry_count: outcome.structured_retry_count,
            structured_retry_reasons: outcome.structured_retry_reasons.clone(),
            structured_retry_succeeded: outcome.structured_retry_succeeded,
            structured_retry_final_error: outcome.structured_retry_final_error.clone(),
            perception_v2_shadow: perception_shadow.trace,
        });
        let completed_run = runs
            .last()
            .expect("structured diagnostic run was just appended");
        let run_passed = completed_run.error.is_none()
            && completed_run.failure_reasons.is_empty()
            && completed_run.structured_enforcement_validated
            && completed_run.enforcement_level == StructuredEnforcement::ToolCall.as_label();
        let run_recovered = completed_run.structured_retry_succeeded.unwrap_or(false);
        if run_passed {
            diagnostic_succeeded += 1;
        } else {
            diagnostic_failed += 1;
        }
        if run_recovered {
            diagnostic_recovered += 1;
        }
        let completed_elapsed_ms = diagnostic_started_clock.elapsed().as_millis() as u64;
        let average_ms = completed_elapsed_ms / current_turn as u64;
        let estimated_remaining_ms =
            average_ms.checked_mul(turns.len().saturating_sub(current_turn) as u64);
        let run_detail = if run_passed {
            format!(
                "{} ops{}",
                completed_run.ops_count,
                if run_recovered {
                    " after structured retry"
                } else {
                    ""
                }
            )
        } else if let Some(error) = completed_run.error.as_deref() {
            error.to_string()
        } else {
            completed_run.failure_reasons.join(", ")
        };
        diagnostic_job_history.push(BackgroundJobHistoryEntry {
            index: current_turn,
            label: format!("Turn {current_turn}"),
            status: if run_passed {
                if run_recovered {
                    "recovered".into()
                } else {
                    "succeeded".into()
                }
            } else {
                "failed".into()
            },
            detail: Some(run_detail.clone()),
            elapsed_ms: Some(completed_elapsed_ms),
        });
        emit_background_job_progress(
            &window,
            &BackgroundJobProgress {
                job_id: diagnostic_job_id.clone(),
                kind: "structured_diagnostic".into(),
                label: "Structured evaluator diagnostic".into(),
                status: "running".into(),
                phase: "turn_complete".into(),
                current: current_turn,
                total: turns.len(),
                succeeded: diagnostic_succeeded,
                failed: diagnostic_failed,
                recovered: diagnostic_recovered,
                started_at: diagnostic_started_at,
                updated_at: db::now_ts(),
                elapsed_ms: completed_elapsed_ms,
                estimated_remaining_ms,
                detail: Some(format!(
                    "Turn {current_turn}/{} {}",
                    turns.len(),
                    if run_passed { "passed" } else { "failed" }
                )),
                cancellable: false,
                history: diagnostic_job_history.clone(),
            },
        );
    }

    emit_background_job_progress(
        &window,
        &BackgroundJobProgress {
            job_id: diagnostic_job_id.clone(),
            kind: "structured_diagnostic".into(),
            label: "Structured evaluator diagnostic".into(),
            status: "running".into(),
            phase: "exporting_artifacts".into(),
            current: turns.len(),
            total: turns.len(),
            succeeded: diagnostic_succeeded,
            failed: diagnostic_failed,
            recovered: diagnostic_recovered,
            started_at: diagnostic_started_at,
            updated_at: db::now_ts(),
            elapsed_ms: diagnostic_started_clock.elapsed().as_millis() as u64,
            estimated_remaining_ms: None,
            detail: Some("Writing diagnostic summary, payload history, and checkpoint".into()),
            cancellable: false,
            history: diagnostic_job_history.clone(),
        },
    );
    let payload_logs = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        db::list_llm_payload_logs(&conn, &conversation_id).map_err(|err| err.to_string())?
    };
    let payload_history = render_llm_payload_history(&payload_logs);
    let payload_history_path = write_export_file(
        &app,
        &conversation_id,
        "structured-diagnostic-payload-history",
        &payload_history,
    )?
    .display()
    .to_string();
    let mne_checkpoint = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        export_current_session_checkpoint_mne_inner(&app, &window, &conn, &conversation_id, "")?
    };
    let final_relationship_target_ids = {
        let mut ids = soul.relationships.keys().cloned().collect::<Vec<_>>();
        ids.sort();
        ids
    };
    let final_object_states = session_world
        .object_states
        .iter()
        .map(|object| serde_json::to_value(object).unwrap_or_default())
        .collect::<Vec<_>>();
    let final_scene_participants = session_world.scene_state.participants.clone();
    let default_player_leaked_into_normal_rp_state = serde_json::to_string(&serde_json::json!({
        "relationships": &soul.relationships,
        "memories": &soul.memory,
        "objects": &session_world.object_states,
        "scene_state": &session_world.scene_state
    }))
    .unwrap_or_default()
    .contains("default_player");
    let default_player_in_relationship_context = default_player_relationship_context_seen;
    let fallback_used = runs.iter().any(|run| {
        run.fallback_path
            .iter()
            .any(|step| step == EVALUATOR_MODE_FORM_V1)
    });
    let failure_turns = runs
        .iter()
        .filter(|run| {
            run.error.is_some()
                || !run.failure_reasons.is_empty()
                || !run.structured_enforcement_validated
                || run.enforcement_level != StructuredEnforcement::ToolCall.as_label()
                || run
                    .fallback_path
                    .iter()
                    .any(|step| step == EVALUATOR_MODE_FORM_V1)
        })
        .map(|run| run.turn_index)
        .collect::<Vec<_>>();
    let strict_tool_passed = !fallback_used
        && !default_player_in_relationship_context
        && failure_turns.is_empty()
        && runs.iter().all(|run| {
            run.enforcement_level == StructuredEnforcement::ToolCall.as_label()
                && run.structured_enforcement_requested
                    == StructuredEnforcement::ToolCall.as_label()
                && run.structured_enforcement_validated
                && run.tool_calls_present
                && run.tool_call_count > 0
                && !run.syntactic_repair_used
        });
    let perception_v2_shadow_attempted = runs
        .iter()
        .filter(|run| run.perception_v2_shadow.attempted)
        .count();
    let perception_v2_shadow_validated = runs
        .iter()
        .filter(|run| run.perception_v2_shadow.schema_validated)
        .count();
    let perception_v2_shadow_candidates = runs
        .iter()
        .map(|run| run.perception_v2_shadow.candidate_count)
        .sum();
    let perception_v2_shadow_commit_count = runs
        .iter()
        .map(|run| run.perception_v2_shadow.commit_count)
        .sum();
    let mut summary = StructuredEvaluatorDiagnosticSummary {
        conversation_id: conversation_id.clone(),
        provider_profile_id: profile.id.clone(),
        provider_model: profile.model.trim().to_string(),
        base_url_redacted: redact_base_url(&profile.base_url),
        structured_mode_requested,
        structured_mode_resolved: structured_mode_resolved.clone(),
        resolved_evaluator_source,
        structured_policy: structured_policy.clone(),
        structured_evaluator_policy: structured_policy,
        evaluator_mode: structured_mode_resolved,
        strict_tool_diagnostic: true,
        strict_tool_passed,
        fallback_used,
        failure_turns,
        structured_schema_version:
            state_engine::evaluator_structured::EVALUATOR_STRUCTURED_SCHEMA_VERSION,
        perception_v2_schema_version: PERCEPTION_IR_SCHEMA_VERSION,
        perception_v2_compiler_version: MEMORY_COMPILER_CONTRACT_VERSION,
        perception_v2_shadow_attempted,
        perception_v2_shadow_validated,
        perception_v2_shadow_candidates,
        perception_v2_shadow_commit_count,
        enforcement_levels: runs
            .iter()
            .map(|run| run.enforcement_level.clone())
            .collect(),
        evaluator_mode_per_run: runs.iter().map(|run| run.evaluator_mode.clone()).collect(),
        structured_enforcement_per_run: runs
            .iter()
            .map(|run| run.enforcement_level.clone())
            .collect(),
        structured_enforcement_requested_per_run: runs
            .iter()
            .map(|run| run.structured_enforcement_requested.clone())
            .collect(),
        structured_enforcement_validated_per_run: runs
            .iter()
            .map(|run| run.structured_enforcement_validated)
            .collect(),
        structured_schema_validation_status_per_run: runs
            .iter()
            .map(|run| run.structured_schema_validation_status.clone())
            .collect(),
        failure_reasons: {
            let mut reasons = runs
                .iter()
                .flat_map(|run| run.failure_reasons.iter().cloned())
                .collect::<Vec<_>>();
            reasons.sort();
            reasons.dedup();
            reasons
        },
        fallback_paths: runs.iter().map(|run| run.fallback_path.clone()).collect(),
        ops_counts: runs.iter().map(|run| run.ops_count).collect(),
        memory_ops_count: total_memory_ops,
        relationship_event_ops_count: total_relationship_ops,
        object_update_ops_count: total_object_ops,
        scene_update_ops_count: total_scene_ops,
        syntactic_repair_used,
        final_memory_count: soul.memory.core.len()
            + soul.memory.recent.len()
            + soul.memory.schemas.len(),
        final_relationship_target_ids,
        final_object_states,
        final_scene_participants,
        default_player_leaked_into_normal_rp_state,
        default_player_in_relationship_context,
        payload_history_path,
        mne_checkpoint_path: mne_checkpoint.path.clone(),
        summary_json_path: String::new(),
        runs,
    };
    if summary.fallback_used {
        summary
            .failure_reasons
            .push("strict_tool_forbids_evaluator_form_v1_fallback".into());
    }
    if summary.default_player_in_relationship_context {
        summary
            .failure_reasons
            .push("default_player_in_relationship_context".into());
    }
    if summary.perception_v2_shadow_validated < summary.perception_v2_shadow_attempted {
        summary
            .failure_reasons
            .push("perception_v2_shadow_validation_failed".into());
    }
    if summary.perception_v2_shadow_commit_count != 0 {
        summary.strict_tool_passed = false;
        summary
            .failure_reasons
            .push("perception_v2_shadow_mutated_state".into());
    }
    if !summary.strict_tool_passed {
        summary
            .failure_reasons
            .push("strict_tool_diagnostic_failed".into());
    }
    summary.failure_reasons.sort();
    summary.failure_reasons.dedup();
    summary.summary_json_path = write_diagnostic_json_file(
        &app,
        &conversation_id,
        "structured-evaluator-diagnostic-summary",
        &summary,
    )?
    .display()
    .to_string();

    emit_dev_log(
        &window,
        "success",
        "evaluator",
        "structured_evaluator_diagnostic_completed",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str(),
            "summary_json_path": summary.summary_json_path,
            "payload_history_path": summary.payload_history_path,
            "mne_checkpoint_path": summary.mne_checkpoint_path
        })),
    );
    emit_background_job_progress(
        &window,
        &BackgroundJobProgress {
            job_id: diagnostic_job_id,
            kind: "structured_diagnostic".into(),
            label: "Structured evaluator diagnostic".into(),
            status: if summary.strict_tool_passed {
                "succeeded".into()
            } else {
                "failed".into()
            },
            phase: "complete".into(),
            current: turns.len(),
            total: turns.len(),
            succeeded: diagnostic_succeeded,
            failed: diagnostic_failed,
            recovered: diagnostic_recovered,
            started_at: diagnostic_started_at,
            updated_at: db::now_ts(),
            elapsed_ms: diagnostic_started_clock.elapsed().as_millis() as u64,
            estimated_remaining_ms: Some(0),
            detail: Some(if summary.strict_tool_passed {
                format!(
                    "All {} strict structured turns passed; {} recovered",
                    turns.len(),
                    diagnostic_recovered
                )
            } else {
                format!(
                    "{} passed, {} failed, {} recovered",
                    diagnostic_succeeded, diagnostic_failed, diagnostic_recovered
                )
            }),
            cancellable: false,
            history: diagnostic_job_history,
        },
    );

    Ok(summary)
}

pub(crate) fn diagnostic_structured_settings_from_profile(
    profile: &ProviderProfile,
    structured_policy: &str,
) -> ApiProviderSettings {
    ApiProviderSettings {
        base_url: profile.base_url.clone(),
        api_key: profile.api_key.clone(),
        model: profile.model.clone(),
        system_prompt: profile.system_prompt.clone(),
        narrator_timeout_ms: profile.narrator_timeout_ms,
        narrator_temperature: None,
        narrator_max_tokens: None,
        context_max_tokens: None,
        narrator_top_p: None,
        narrator_frequency_penalty: None,
        narrator_presence_penalty: None,
        evaluator_timeout_ms: Some(
            profile
                .evaluator_timeout_ms
                .unwrap_or(DEFAULT_DIAGNOSTIC_EVALUATOR_TIMEOUT_MS),
        ),
        structured_evaluator_timeout_ms: Some(DEFAULT_DIAGNOSTIC_EVALUATOR_TIMEOUT_MS),
        diagnostic_evaluator_timeout_ms: Some(DEFAULT_DIAGNOSTIC_EVALUATOR_TIMEOUT_MS),
        evaluator_timeout_mode: Some("finite".into()),
        evaluator_mode: Some(EVALUATOR_MODE_STRUCTURED_V1.into()),
        structured_evaluator_policy: Some(structured_policy.to_string()),
        structured_evaluator_transport: Some("tool_call".into()),
        structured_evaluator_max_retries: Some(1),
        structured_require_ops: None,
        wait_for_evaluator_before_next_turn: profile.wait_for_evaluator_before_next_turn,
        allow_send_with_stale_state: profile.allow_send_with_stale_state,
        evaluator_background_enabled: Some(false),
        anti_replay_forced_retry_enabled: profile.anti_replay_forced_retry_enabled,
        evaluator_execution_mode: None,
    }
}

fn structured_diagnostic_turns() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "Set the scene: I stand outside Aurora's apartment during a cold rain.",
            "Aurora's apartment is warm beyond the door while preset_male waits in the hallway, rain ticking against the stairwell window.",
        ),
        (
            "I knock at the door.",
            "Aurora hears the knock and pauses near the entry, recognizing preset_male's familiar cadence through the door.",
        ),
        (
            "When Aurora opens it, I step inside and greet her.",
            "Aurora lets preset_male into the apartment, watching him shake rainwater from his hair as the hallway chill follows him in.",
        ),
        (
            "I slip off my wet jacket and drape it over the chair.",
            "preset_male removes his wet jacket and lays it over the wooden chair near the kitchen table, leaving dark damp marks on the fabric.",
        ),
        (
            "I move the jacket from the chair to a hook near the door.",
            "preset_male picks up the same wet jacket from the chair and hangs it on the hook beside the apartment door so it can drip onto the mat.",
        ),
    ]
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DiagnosticPatchCounts {
    pub(crate) memory_ops_count: usize,
    pub(crate) relationship_event_ops_count: usize,
    pub(crate) object_update_ops_count: usize,
    pub(crate) scene_update_ops_count: usize,
}

pub(crate) fn diagnostic_patch_counts(patch: &EnginePatch) -> DiagnosticPatchCounts {
    DiagnosticPatchCounts {
        memory_ops_count: patch
            .soul_patch
            .as_ref()
            .map(|soul| soul.new_memories.len() + soul.memory_operations.len())
            .unwrap_or(0),
        relationship_event_ops_count: patch
            .soul_patch
            .as_ref()
            .map(|soul| {
                soul.relationship_deltas.len()
                    + usize::from(soul.relationship_delta.as_ref().is_some())
            })
            .unwrap_or(0),
        object_update_ops_count: patch
            .world_patch
            .as_ref()
            .map(|world| {
                world.object_observation_operations.len() + world.corrected_object_states.len()
            })
            .unwrap_or(0),
        scene_update_ops_count: patch
            .world_patch
            .as_ref()
            .and_then(|world| world.scene_state.as_ref())
            .map(|scene| usize::from(!scene.is_empty()))
            .unwrap_or(0),
    }
}

pub(crate) fn diagnostic_total_patch_ops(patch: &EnginePatch) -> usize {
    let counts = diagnostic_patch_counts(patch);
    counts.memory_ops_count
        + counts.relationship_event_ops_count
        + counts.object_update_ops_count
        + counts.scene_update_ops_count
        + patch
            .world_patch
            .as_ref()
            .map(|world| world.event_operations.len())
            .unwrap_or(0)
}

fn redact_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let without_fragment = trimmed.split('#').next().unwrap_or(trimmed);
    let without_query = without_fragment
        .split('?')
        .next()
        .unwrap_or(without_fragment);
    let (scheme, authority_and_path) = without_query
        .split_once("://")
        .map(|(scheme, rest)| (format!("{scheme}://"), rest))
        .unwrap_or_else(|| (String::new(), without_query));
    let without_credentials = authority_and_path
        .rsplit('@')
        .next()
        .unwrap_or(authority_and_path);
    format!("{scheme}{without_credentials}")
}

pub(crate) fn write_diagnostic_json_file<T: Serialize>(
    app: &AppHandle,
    conversation_id: &str,
    label: &str,
    value: &T,
) -> Result<PathBuf, String> {
    let mut dir = app
        .path()
        .download_dir()
        .or_else(|_| std::env::current_dir())
        .map_err(|err| err.to_string())?;
    dir.push("mnemosyne-exports");
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let safe_conversation = safe_filename(conversation_id);
    let filename = format!(
        "mnemosyne-{safe_conversation}-{label}-{}.json",
        db::now_ts()
    );
    dir.push(filename);
    let json = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    fs::write(&dir, json).map_err(|err| err.to_string())?;
    Ok(dir)
}

#[tauri::command]
pub fn set_active_evaluator_profile(
    state: State<'_, AppState>,
    conversation_id: String,
    profile_id: Option<String>,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    db::set_active_evaluator_profile(&conn, &conversation_id, profile_id.as_deref())
        .map_err(|err| err.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCurationResult {
    pub patch_id: String,
    pub turn_id: String,
    pub branch_id: String,
    pub memory_id: String,
    pub operation: String,
    pub soul: Soul,
}

/// Memory curation (pin / unpin / restore_archived) for the Memory Inspector.
/// IMPORTANT: curation is committed as a patch through the ledger, never as a
/// direct soul mutation — the materialized soul is rebuilt by replaying the
/// patch ledger, so direct mutation would be silently lost on rebuild.
pub(crate) fn curate_memory_with_conn(
    conn: &Connection,
    conversation_id: &str,
    soul_id: &str,
    memory_id: &str,
    operation: &str,
) -> Result<MemoryCurationResult, String> {
    if !matches!(operation, "pin" | "unpin" | "restore_archived") {
        return Err(format!(
            "Unsupported memory curation operation '{operation}'; expected pin, unpin, or restore_archived"
        ));
    }
    let mut turn_state = load_command_turn_state(conn, conversation_id, soul_id)?;

    // Dry-run the engine helper on a clone so an ineffective operation fails
    // before anything is committed to the ledger.
    let mut probe = turn_state.soul.clone();
    let effective = match operation {
        "pin" | "unpin" => set_memory_pinned(&mut probe, memory_id, operation == "pin"),
        _ => restore_archived_memory(&mut probe, memory_id),
    };
    if !effective {
        return Err(format!(
            "Memory curation '{operation}' had no effect on memory '{memory_id}' (missing, invalidated, or not archived)"
        ));
    }

    let patch = EnginePatch {
        schema_version: Some(PATCH_PROTOCOL_VERSION),
        soul_patch: Some(SoulPatch {
            memory_operations: vec![MemoryPatch {
                operation: Some(operation.to_string()),
                target_memory_id: Some(memory_id.to_string()),
                ..MemoryPatch::default()
            }],
            ..SoulPatch::default()
        }),
        ..EnginePatch::default()
    };

    let summary_label = match operation {
        "pin" => "Pinned memory",
        "unpin" => "Unpinned memory",
        _ => "Restored archived memory",
    };
    let assistant_message_id = db::insert_message_with_channel_and_get_id(
        conn,
        conversation_id,
        "assistant",
        &format!("{summary_label} {memory_id}."),
        db::MESSAGE_CHANNEL_COMMAND_STATE,
    )
    .map_err(|err| err.to_string())?;
    let turn_id = format!("turn_{}", uuid_like_id());
    let outcome = apply_command_patch_to_ledger(
        conn,
        conversation_id,
        &turn_id,
        None,
        assistant_message_id,
        &mut turn_state,
        &patch,
    )?;
    Ok(MemoryCurationResult {
        patch_id: outcome.patch_id,
        turn_id: outcome.turn_id,
        branch_id: outcome.branch_id,
        memory_id: memory_id.to_string(),
        operation: operation.to_string(),
        soul: turn_state.soul,
    })
}

#[tauri::command]
pub fn curate_memory(
    window: Window,
    state: State<'_, AppState>,
    conversation_id: String,
    soul_id: String,
    memory_id: String,
    operation: String,
) -> Result<MemoryCurationResult, String> {
    let result = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        curate_memory_with_conn(&conn, &conversation_id, &soul_id, &memory_id, &operation)?
    };
    emit_dev_log(
        &window,
        "info",
        "memory",
        "memory_curation_applied",
        Some(serde_json::json!({
            "conversation_id": conversation_id.as_str(),
            "memory_id": result.memory_id.as_str(),
            "operation": result.operation.as_str(),
            "patch_id": result.patch_id.as_str(),
            "turn_id": result.turn_id.as_str(),
            "branch_id": result.branch_id.as_str()
        })),
    );
    Ok(result)
}

#[tauri::command]
pub async fn retry_evaluator_job(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    conversation_id: String,
    assistant_message_id: i64,
    state_updater_settings: ApiProviderSettings,
) -> Result<(), String> {
    let (
        soul,
        session_world,
        snapshot_user_text,
        visible_response,
        context_preview,
        entity_updater_context,
        branch_id,
        parent_turn_id,
        baseline_patch_id,
        user_message_id,
        selected_variant_id,
    ) = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        let assistant_message = db::get_message(&conn, &conversation_id, assistant_message_id)
            .map_err(|err| err.to_string())?;
        if assistant_message.role != "assistant" {
            return Err("Evaluator retry requires an assistant message".into());
        }
        let snapshot = db::get_turn_snapshot(&conn, &conversation_id, assistant_message_id)
            .map_err(|err| err.to_string())?
            .ok_or_else(|| "No turn snapshot found for evaluator retry".to_string())?;
        let fallback_soul: Soul =
            serde_json::from_str(&snapshot.soul_json).map_err(|err| err.to_string())?;
        let commit =
            db::get_turn_commit_by_assistant(&conn, &conversation_id, assistant_message_id)
                .map_err(|err| err.to_string())?;
        let branch = db::get_active_session_branch(&conn, &conversation_id).ok();
        let (soul, session_world, parent_turn_id) = if let Some(branch) = branch.as_ref() {
            let parent_turn_id = commit
                .as_ref()
                .and_then(|commit| commit.parent_turn_id.clone())
                .or_else(|| branch.active_turn_id.clone());
            let rebuilt = db::rebuild_session_state_until(
                &conn,
                &conversation_id,
                &branch.branch_id,
                parent_turn_id.as_deref(),
            )
            .map_err(|err| err.to_string())?;
            (rebuilt.soul, rebuilt.session_world, parent_turn_id)
        } else {
            let session_world =
                load_session_world_for_context(&window, &conn, &conversation_id, &fallback_soul)
                    .map_err(|err| err.to_string())?;
            (fallback_soul, session_world, None)
        };
        let messages =
            db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())?;
        let context_preview = compile_context_for_session(
            &soul,
            Some(&session_world),
            &messages_to_context(messages),
        );
        let entity_context =
            resolve_speaker_for_turn(&conn, &conversation_id, &soul, &snapshot.user_text)
                .map_err(|err| err.to_string())?;
        let entity_updater_context = build_entity_updater_context(&soul, &entity_context);
        let branch_id = branch.map(|branch| branch.branch_id);
        let (source_turn_id, baseline_patch_id, source_user_message_id, source_variant_id) =
            resolve_evaluator_source_turn(&conn, &conversation_id, assistant_message_id)?;
        (
            soul,
            session_world,
            snapshot.user_text,
            strip_hidden_state_blocks(&assistant_message.content),
            context_preview,
            entity_updater_context,
            branch_id,
            source_turn_id.or(parent_turn_id),
            baseline_patch_id,
            source_user_message_id
                .or_else(|| commit.as_ref().and_then(|commit| commit.user_message_id)),
            source_variant_id.or_else(|| {
                commit
                    .as_ref()
                    .and_then(|commit| commit.selected_variant_id)
            }),
        )
    };
    let request_id = uuid_like_id();
    let evaluator_request_id = format!("eval_retry_{request_id}");
    let before_state_summary = compact_state_summary_json(&soul, &session_world);
    let baseline_patch_id = baseline_patch_id.ok_or_else(|| {
        "Evaluator retry requires an existing baseline patch; refusing to create a turn row"
            .to_string()
    })?;
    run_evaluator_repair_attempt(
        app,
        window,
        conversation_id,
        assistant_message_id,
        selected_variant_id,
        request_id,
        evaluator_request_id,
        None,
        "brief".into(),
        soul,
        session_world,
        snapshot_user_text,
        visible_response,
        context_preview.text,
        state_updater_settings,
        entity_updater_context,
        format!("memory-debug-{}", uuid_like_id()),
        branch_id,
        parent_turn_id,
        user_message_id,
        before_state_summary,
        baseline_patch_id,
        None,
    )?;
    Ok(())
}

/// One op that failed validation in the main eval, plus why it failed. The op is
/// the raw JSON the model produced (it already carries its evidence_quote / source
/// line), so the repair model gets the exact thing to fix and the reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorOpRepairRequest {
    pub op_json: String,
    pub reason: String,
}

/// Build the list of failed ops for repair by snapshotting the model's raw ops
/// (`normalized_json`) and the SYSTEM's own validation verdict
/// (`rejected_candidates`, candidate_id `op:N` + reason) and pairing them. This is
/// the "snatch the json + compare" step: the failure verdict is ours, not the
/// tool-call's, so we can always reconstruct exactly which op failed and why.
pub(super) fn rejected_ops_for_repair(
    normalized_json: &str,
    rejected: &[state_engine::evaluator::EvaluatorCandidateRejection],
) -> Vec<EvaluatorOpRepairRequest> {
    if rejected.is_empty() {
        return Vec::new();
    }
    if let Ok(parsed) = serde_json::from_str::<EvaluatorStructuredOutputV1>(normalized_json.trim())
    {
        let mut out = Vec::new();
        for rejection in rejected {
            let Some(index_str) = rejection.candidate_id.strip_prefix("op:") else {
                continue;
            };
            let Ok(index) = index_str.trim().parse::<usize>() else {
                continue;
            };
            let Some(op) = parsed.ops.get(index) else {
                continue;
            };
            let Ok(op_json) = serde_json::to_string(op) else {
                continue;
            };
            out.push(EvaluatorOpRepairRequest {
                op_json,
                reason: rejection.reason.clone(),
            });
        }
        return out;
    }
    let Ok(parsed) =
        serde_json::from_str::<state_engine::compiler::PerceptionBatch>(normalized_json.trim())
    else {
        return Vec::new();
    };
    rejected
        .iter()
        .filter_map(|rejection| {
            let candidate = parsed
                .candidates
                .iter()
                .find(|candidate| candidate.candidate_id == rejection.candidate_id)?;
            Some(EvaluatorOpRepairRequest {
                op_json: serde_json::to_string(&candidate.perception).ok()?,
                reason: rejection.reason.clone(),
            })
        })
        .collect()
}

/// Focused repair user message: fix ONLY the failed ops, given each broken op and
/// its failure reason, anchored to the turn text. Not a full re-extraction.
pub(crate) fn build_op_repair_user_message(
    failed_ops: &[EvaluatorOpRepairRequest],
    user_text: &str,
    narrator_text: &str,
) -> String {
    let mut out = String::new();
    out.push_str(
        "REPAIR TASK. The state-extraction candidates below failed validation. Return the \
         corrected structured evaluator payload required by the system prompt, containing ONLY \
         fixed versions of these candidates — fix exactly the stated problem, keep everything else faithful to the \
         scene, invent nothing, and add no new ops.\n\n",
    );
    out.push_str("Scene this turn:\n");
    out.push_str("User: ");
    out.push_str(user_text.trim());
    out.push_str("\nNarrator: ");
    out.push_str(narrator_text.trim());
    out.push_str("\n\nFailed ops to fix:\n");
    for (index, failed) in failed_ops.iter().enumerate() {
        out.push_str(&format!(
            "\n[{}] failure reason: {}\noriginal op: {}\n",
            index + 1,
            failed.reason.trim(),
            failed.op_json.trim()
        ));
    }
    out.push_str("\nReturn the corrected ops payload now.");
    out
}

/// Build repair requests from a FORM evaluator's rejected rows. The structured
/// path (`rejected_ops_for_repair`) only understands `op:N` candidates, so form
/// failures never reached repair. Each rejected row trace already carries the raw
/// row JSON the model produced plus the validation reason, which is exactly what
/// the repair model needs to re-ground and fix.
pub(super) fn form_rejected_ops_for_repair(
    form_trace: &state_engine::evaluator_form::EvalFormTrace,
) -> Vec<EvaluatorOpRepairRequest> {
    form_trace
        .evaluator_row_traces
        .iter()
        .filter(|row| row.validation_status == "rejected")
        .filter_map(|row| {
            let op_json = serde_json::to_string(&row.raw_row).ok()?;
            let reason = row
                .rejection_reason
                .clone()
                .filter(|reason| !reason.trim().is_empty())
                .unwrap_or_else(|| {
                    format!("{} row rejected: {}", row.row_kind, row.compiler_result)
                });
            Some(EvaluatorOpRepairRequest { op_json, reason })
        })
        .collect()
}

/// Full re-extraction prompt used when the evaluator produced nothing usable —
/// an empty/no-op patch despite a real exchange, or a transport/parse failure
/// that returned no body at all. Unlike `build_op_repair_user_message` (which
/// fixes specific failed ops), this asks the repair model to extract all durable
/// state from the exchange from scratch, in the same structured ops schema.
pub(crate) fn build_reextract_user_message(user_text: &str, narrator_text: &str) -> String {
    let mut out = String::new();
    out.push_str(
        "RE-EXTRACTION TASK. The primary evaluator produced no state for this turn, so you are \
         the fallback — extract what it missed. Most substantive roleplay turns contain at least \
         one durable change. Actively look for ALL of these before concluding otherwise:\n\
         - relationship_event: any trust / comfort / respect / affection / fear / tension shift \
         implied by what a character did — respecting or pushing a boundary, opening up, testing, \
         reassuring, withdrawing. If a character acted toward another in a way that would move how \
         they feel, score it.\n\
         - add_memory: any newly revealed fact, stated intention, or emotionally significant beat \
         worth remembering later.\n\
         - object / scene state: anything moved, set down, opened, changed, or newly observed.\n\n\
         Rules: copy EXACT evidence_quote substrings verbatim from the exchange below; invent \
         nothing and add no change the text doesn't support; resolve \"I\" to the active player \
         persona; prefer entity aliases (active_soul, active_player). You MUST return at least \
         one op — empty ops are not accepted on this task.\n\n\
         Op shapes (copy these exactly, filling your own values):\n\
         {\"op\":\"relationship_event\",\"source_soul_id\":\"active_soul\",\"target_entity_id\":\"active_player\",\"actor_entity_id\":\"active_player\",\"perceived_by_entity_id\":\"active_soul\",\"evidence_quote\":\"<verbatim substring>\",\"axes\":{\"intent\":2,\"honesty\":1,\"reliability\":1,\"boundary_treatment\":3,\"responsiveness\":2,\"power_use\":0,\"evaluation_tone\":1,\"competence\":0,\"disclosure\":1,\"reciprocity\":1,\"repair\":0,\"predictability\":1},\"modifiers\":{\"salience\":70,\"certainty\":80,\"directness\":70,\"costliness\":20,\"stakes\":50,\"repetition\":0},\"event_flags_u64\":0}\n\
         {\"op\":\"add_memory\",\"owner_soul_id\":\"active_soul\",\"slot\":\"relationship_memory\",\"content\":\"<one-line durable fact>\",\"evidence_quote\":\"<verbatim substring>\",\"confidence\":0.8,\"salience\":70,\"source_message_id\":null,\"target_entity_ids\":[\"active_player\"],\"truth_status\":\"scene_event\"}\n\
         {\"op\":\"update_object_state\",\"object_label\":\"<object>\",\"object_type\":\"<kind>\",\"owner_entity_id\":\"active_soul\",\"status\":\"<status>\",\"location\":\"<where>\",\"last_observed_state\":\"<observed state>\",\"evidence_quote\":\"<verbatim substring>\"}\n\n",
    );
    out.push_str("Scene this turn:\n");
    out.push_str("User: ");
    out.push_str(user_text.trim());
    out.push_str("\nNarrator: ");
    out.push_str(narrator_text.trim());
    out.push_str("\n\nReturn the ops payload now.");
    out
}

/// Emit the background-repair signal the frontend listens for. `repair_kind` is
/// "fix_rejected" (carries the specific failed ops to correct) or "reextract"
/// (no ops — the model re-extracts the whole turn because the evaluator produced
/// nothing usable). Centralized so the structured, form, and parse-failure paths
/// all emit the same shape.
pub(super) fn emit_evaluator_repair_signal(
    window: &Window,
    conversation_id: &str,
    assistant_message_id: i64,
    evaluator_job_id: &str,
    repair_kind: &str,
    failed_ops: &[EvaluatorOpRepairRequest],
) {
    emit_dev_log(
        window,
        "warn",
        "evaluator",
        "evaluator_ops_rejected",
        Some(serde_json::json!({
            "conversation_id": conversation_id,
            "assistant_message_id": assistant_message_id,
            "evaluator_job_id": evaluator_job_id,
            "repair_kind": repair_kind,
            "failed_op_count": failed_ops.len(),
            "failed_ops": failed_ops,
        })),
    );
    let _ = window.emit(
        "evaluator-ops-rejected",
        serde_json::json!({
            "conversation_id": conversation_id,
            "assistant_message_id": assistant_message_id,
            "repair_kind": repair_kind,
            "failed_ops": failed_ops,
        }),
    );
}

/// True when a provider base_url points at the local machine (an embedded
/// llamafile / llama.cpp server). Used to pick a transport the local server
/// actually supports rather than OpenAI tool-calling.
pub(crate) fn is_loopback_endpoint(base_url: &str) -> bool {
    let lower = base_url.trim().to_ascii_lowercase();
    let host = lower
        .strip_prefix("http://")
        .or_else(|| lower.strip_prefix("https://"))
        .unwrap_or(&lower);
    host.starts_with("localhost")
        || host.starts_with("127.0.0.1")
        || host.starts_with("0.0.0.0")
        || host.starts_with("[::1]")
        || host.starts_with("::1")
}

fn resolve_evaluator_source_turn(
    conn: &Connection,
    conversation_id: &str,
    assistant_message_id: i64,
) -> Result<(Option<String>, Option<String>, Option<i64>, Option<i64>), String> {
    let commit = db::get_turn_commit_by_assistant(conn, conversation_id, assistant_message_id)
        .map_err(|err| err.to_string())?;
    let Some(commit) = commit else {
        return Ok((None, None, None, None));
    };
    Ok((
        Some(commit.turn_id),
        commit.state_patch_id,
        commit.user_message_id,
        commit.selected_variant_id,
    ))
}

#[allow(clippy::too_many_arguments)]
fn run_evaluator_repair_attempt(
    app: AppHandle,
    window: Window,
    conversation_id: String,
    assistant_message_id: i64,
    selected_variant_id: Option<i64>,
    parent_narrator_request_id: String,
    evaluator_request_id: String,
    turn_id: Option<String>,
    context_mode_label: String,
    soul: Soul,
    session_world: SessionWorld,
    snapshot_user_text: String,
    visible_response_for_updater: String,
    context_preview_text: String,
    state_updater_settings: ApiProviderSettings,
    entity_updater_context: String,
    memory_debug_nonce: String,
    ledger_branch_id: Option<String>,
    ledger_parent_turn_id: Option<String>,
    ledger_user_message_id: Option<i64>,
    before_state_summary: serde_json::Value,
    baseline_patch_id: String,
    repair_user_message_override: Option<String>,
) -> Result<db::EvaluatorJob, String> {
    start_background_evaluator_job(
        app,
        window,
        conversation_id,
        assistant_message_id,
        selected_variant_id,
        parent_narrator_request_id,
        evaluator_request_id,
        turn_id,
        context_mode_label,
        soul,
        session_world,
        snapshot_user_text,
        visible_response_for_updater,
        context_preview_text,
        state_updater_settings,
        entity_updater_context,
        memory_debug_nonce,
        ledger_branch_id,
        ledger_parent_turn_id,
        ledger_user_message_id,
        false,
        before_state_summary,
        Some(baseline_patch_id),
        repair_user_message_override,
    )
}

/// Background op-repair: re-runs ONLY the failed ops through a (configurable,
/// e.g. local) repair model, up to 5 structured attempts, applying any that now
/// validate via the same proven evaluator apply path. Fire-and-forget after a
/// turn — it does not block chat or the main eval, and failures stay dropped.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn repair_evaluator_ops(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    conversation_id: String,
    assistant_message_id: i64,
    failed_ops: Vec<EvaluatorOpRepairRequest>,
    repair_settings: ApiProviderSettings,
    repair_kind: Option<String>,
) -> Result<(), String> {
    // "reextract" re-runs the whole turn (no specific ops to fix); every other
    // kind is a focused fix and needs at least one failed op to act on.
    let reextract = repair_kind.as_deref() == Some("reextract");
    if !reextract && failed_ops.is_empty() {
        return Ok(());
    }
    let (
        soul,
        session_world,
        snapshot_user_text,
        visible_response,
        context_preview,
        entity_updater_context,
        branch_id,
        parent_turn_id,
        baseline_patch_id,
        user_message_id,
        selected_variant_id,
    ) = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        let assistant_message = db::get_message(&conn, &conversation_id, assistant_message_id)
            .map_err(|err| err.to_string())?;
        if assistant_message.role != "assistant" {
            return Err("Evaluator repair requires an assistant message".into());
        }
        let snapshot = db::get_turn_snapshot(&conn, &conversation_id, assistant_message_id)
            .map_err(|err| err.to_string())?
            .ok_or_else(|| "No turn snapshot found for evaluator repair".to_string())?;
        let fallback_soul: Soul =
            serde_json::from_str(&snapshot.soul_json).map_err(|err| err.to_string())?;
        let commit =
            db::get_turn_commit_by_assistant(&conn, &conversation_id, assistant_message_id)
                .map_err(|err| err.to_string())?;
        let branch = db::get_active_session_branch(&conn, &conversation_id).ok();
        let (soul, session_world, parent_turn_id) = if let Some(branch) = branch.as_ref() {
            let parent_turn_id = commit
                .as_ref()
                .and_then(|commit| commit.parent_turn_id.clone())
                .or_else(|| branch.active_turn_id.clone());
            let rebuilt = db::rebuild_session_state_until(
                &conn,
                &conversation_id,
                &branch.branch_id,
                parent_turn_id.as_deref(),
            )
            .map_err(|err| err.to_string())?;
            (rebuilt.soul, rebuilt.session_world, parent_turn_id)
        } else {
            let session_world =
                load_session_world_for_context(&window, &conn, &conversation_id, &fallback_soul)
                    .map_err(|err| err.to_string())?;
            (fallback_soul, session_world, None)
        };
        let messages =
            db::list_messages(&conn, &conversation_id, 100).map_err(|err| err.to_string())?;
        let context_preview = compile_context_for_session(
            &soul,
            Some(&session_world),
            &messages_to_context(messages),
        );
        let entity_context =
            resolve_speaker_for_turn(&conn, &conversation_id, &soul, &snapshot.user_text)
                .map_err(|err| err.to_string())?;
        let entity_updater_context = build_entity_updater_context(&soul, &entity_context);
        let branch_id = branch.map(|branch| branch.branch_id);
        let (source_turn_id, baseline_patch_id, source_user_message_id, source_variant_id) =
            resolve_evaluator_source_turn(&conn, &conversation_id, assistant_message_id)?;
        (
            soul,
            session_world,
            snapshot.user_text,
            strip_hidden_state_blocks(&assistant_message.content),
            context_preview,
            entity_updater_context,
            branch_id,
            source_turn_id.or(parent_turn_id),
            baseline_patch_id,
            source_user_message_id
                .or_else(|| commit.as_ref().and_then(|commit| commit.user_message_id)),
            source_variant_id.or_else(|| {
                commit
                    .as_ref()
                    .and_then(|commit| commit.selected_variant_id)
            }),
        )
    };

    let repair_user_message = if reextract {
        build_reextract_user_message(&snapshot_user_text, &visible_response)
    } else {
        build_op_repair_user_message(&failed_ops, &snapshot_user_text, &visible_response)
    };

    // Force the repair onto the structured path with a generous retry budget; it
    // runs against the caller-supplied (e.g. local) endpoint.
    let mut repair_settings = repair_settings;
    repair_settings.evaluator_mode = Some(EVALUATOR_MODE_STRUCTURED_V1.into());
    // Repair only fires on turns already known to contain durable change, so use
    // the strict repair schema: at least one real op, no `no_op` escape. Small
    // models otherwise reason correctly and then punt into no_op anyway.
    repair_settings.structured_require_ops = Some(true);
    if repair_settings
        .structured_evaluator_max_retries
        .unwrap_or(0)
        < 5
    {
        repair_settings.structured_evaluator_max_retries = Some(5);
    }
    // Local llama.cpp/llamafile servers don't speak OpenAI tool-calling unless
    // launched with --jinja, so the default `auto` transport (tool-call first)
    // gets a prose reply, treats it as a rejection, and drops the repair with 0
    // rows. They DO support response_format grammar natively, so pin the local
    // endpoint to the json_schema rung (allow_fallback lets it degrade to
    // json_object / prompt for older builds). Remote endpoints keep their config.
    if is_loopback_endpoint(&repair_settings.base_url) {
        repair_settings.structured_evaluator_transport = Some("json_schema".into());
        repair_settings.structured_evaluator_policy = Some("allow_fallback".into());
        // CPU inference of the ~2k-token repair prompt is slow on consumer
        // hardware (measured ~150s just for prompt eval on an i5-1335U). The
        // default ~25s evaluator timeout therefore ALWAYS fires before the local
        // model can answer — reqwest surfaces it as "error sending request",
        // which looks like a connection failure but is really a timeout. Give the
        // local endpoint a generous ceiling so it can actually finish. Repair is
        // background/fire-and-forget, so a long ceiling costs nothing on success.
        repair_settings.structured_evaluator_timeout_ms = Some(LOCAL_REPAIR_TIMEOUT_MS);
        repair_settings.evaluator_timeout_ms = Some(LOCAL_REPAIR_TIMEOUT_MS);
    }
    // Repair is the enrichment, never the baseline: run it in the background and
    // commit through the same proven evaluator apply path.
    repair_settings.evaluator_background_enabled = Some(true);

    let request_id = uuid_like_id();
    let evaluator_request_id = format!("eval_repair_{request_id}");
    let before_state_summary = compact_state_summary_json(&soul, &session_world);
    let baseline_patch_id = baseline_patch_id.ok_or_else(|| {
        "Evaluator repair requires an existing baseline patch; refusing to create a turn row"
            .to_string()
    })?;
    run_evaluator_repair_attempt(
        app,
        window,
        conversation_id,
        assistant_message_id,
        selected_variant_id,
        request_id,
        evaluator_request_id,
        None,
        "brief".into(),
        soul,
        session_world,
        snapshot_user_text,
        visible_response,
        context_preview.text,
        repair_settings,
        entity_updater_context,
        format!("memory-debug-{}", uuid_like_id()),
        branch_id,
        parent_turn_id,
        user_message_id,
        before_state_summary,
        baseline_patch_id,
        Some(repair_user_message),
    )?;
    Ok(())
}

#[cfg(test)]
pub(crate) use crate::embedded_model::{exe_sibling_for_llamafile, parse_listening_port};
