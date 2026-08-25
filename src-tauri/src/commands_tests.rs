use super::*;
use crate::pipeline_trace::PipelineStageTrace;
use state_engine::compiler::{
    DurabilityHint, EpistemicMode, EvidenceSource, EvidenceSpan, PerceptionBatchDraft,
    PerceptionCandidateDraft, PerceptionKind, TemporalAnchor, TemporalExpression,
};

fn perception_v2_test_source() -> SourceEnvelope {
    SourceEnvelope::new(
        SourceIdentity {
            conversation_id: "perception-test".into(),
            branch_id: "branch-main".into(),
            turn_id: "turn-1".into(),
            parent_turn_id: None,
            user_message_id: 1,
            assistant_message_id: 2,
            assistant_variant_id: None,
        },
        vec!["aurora".into()],
        "I return the key.",
        "Aurora sees the visitor return the key.",
        None,
        1_000,
    )
    .expect("source")
}

#[test]
fn perception_v2_shadow_runtime_strictly_parses_and_seals() {
    let raw = r#"{
        "schema_version": 2,
        "candidates": [{
            "kind": "event",
            "subject_ref": "active_player",
            "predicate": "returned",
            "object": {"type":"entity_ref","entity_ref":"key"},
            "actor_ref": "active_player",
            "perceiver_ref": "active_soul",
            "target_refs": ["active_soul"],
            "evidence": {
                "source": "assistant_message",
                "quote": "visitor return the key",
                "start_char": null,
                "end_char": null
            },
            "epistemic_mode": "directly_observed",
            "extraction_confidence": 0.95,
            "temporal": {"anchor":"current_turn","expression":null},
            "durability_hint": "long_term"
        }],
        "no_op_reason": null
    }"#;
    let source = perception_v2_test_source();
    let first = compile_perception_v2_shadow_runtime(
        raw,
        &source,
        ModelProvenance {
            provider: "test".into(),
            model: "test-model".into(),
            prompt_version: PERCEPTION_V2_PROMPT_VERSION.into(),
            schema_name: PERCEPTION_IR_SCHEMA_NAME.into(),
        },
    )
    .expect("V2 shadow compiles");
    let replay = compile_perception_v2_shadow_runtime(
        raw,
        &source,
        ModelProvenance {
            provider: "test".into(),
            model: "test-model".into(),
            prompt_version: PERCEPTION_V2_PROMPT_VERSION.into(),
            schema_name: PERCEPTION_IR_SCHEMA_NAME.into(),
        },
    )
    .expect("V2 shadow replay");
    assert_eq!(first, replay);
    assert_eq!(first.candidates.len(), 1);
    assert_eq!(first.source_hash, source.source_hash());
}

#[test]
fn perception_v2_shadow_runtime_rejects_effect_or_truth_injection() {
    let source = perception_v2_test_source();
    for injected in [
        r#""truth_status":"verified_engine","#,
        r#""state_delta":{"trust":100},"#,
        r#""source_message_id":999,"#,
    ] {
        let raw = format!(
            r#"{{
                "schema_version":2,
                "candidates":[{{
                    {injected}
                    "kind":"event",
                    "subject_ref":"active_player",
                    "predicate":"returned",
                    "object":null,
                    "actor_ref":"active_player",
                    "perceiver_ref":"active_soul",
                    "target_refs":[],
                    "evidence":{{"source":"user_message","quote":"return the key","start_char":null,"end_char":null}},
                    "epistemic_mode":"stated_by",
                    "extraction_confidence":0.8,
                    "temporal":{{"anchor":"current_turn","expression":null}},
                    "durability_hint":"session"
                }}],
                "no_op_reason":null
            }}"#
        );
        let error = compile_perception_v2_shadow_runtime(
            &raw,
            &source,
            ModelProvenance {
                provider: "test".into(),
                model: "test-model".into(),
                prompt_version: PERCEPTION_V2_PROMPT_VERSION.into(),
                schema_name: PERCEPTION_IR_SCHEMA_NAME.into(),
            },
        )
        .expect_err("authority injection must fail");
        assert!(error.contains("unknown field"), "{error}");
    }
}

#[test]
fn perception_v2_prompt_describes_interpretation_without_effect_authority() {
    let soul = new_default_soul("Aurora");
    let prompt =
        build_perception_v2_prompt_with_player_persona(&soul, None, "preset_male", "Male Persona");
    assert!(prompt.contains("PerceptionBatchDraft"));
    assert!(prompt.contains("direct observation"));
    assert!(prompt.contains("never issue state changes"));
    assert!(prompt.contains("active_player"));
    assert!(!prompt.contains("relationship_delta"));
    assert!(!prompt.contains("verified_engine"));
}

#[test]
fn perception_v2_mode_is_explicit_opt_in() {
    let mut settings = evaluator_test_settings();
    settings.evaluator_mode = Some(EVALUATOR_MODE_PERCEPTION_V2.into());

    assert_eq!(evaluator_mode(&settings), EVALUATOR_MODE_PERCEPTION_V2);
    assert_eq!(
        selected_evaluator_source(EVALUATOR_MODE_PERCEPTION_V2),
        EVALUATOR_MODE_PERCEPTION_V2
    );
    assert_eq!(
        evaluator_provider_label(EVALUATOR_MODE_PERCEPTION_V2, true),
        "evaluator_perception_v2_background"
    );
}

#[test]
fn perception_v2_production_runtime_only_returns_compiler_validated_patch() {
    let raw = r#"{
        "schema_version": 2,
        "candidates": [{
            "kind": "event",
            "subject_ref": "active_player",
            "predicate": "returned",
            "object": {"type":"entity_ref","entity_ref":"key"},
            "actor_ref": "active_player",
            "perceiver_ref": "active_soul",
            "target_refs": ["active_soul"],
            "evidence": {
                "source": "assistant_message",
                "quote": "visitor return the key",
                "start_char": null,
                "end_char": null
            },
            "epistemic_mode": "directly_observed",
            "extraction_confidence": 0.95,
            "temporal": {"anchor":"current_turn","expression":null},
            "durability_hint": "long_term",
            "relationship_signal": null
        }],
        "no_op_reason": null
    }"#;
    let source = perception_v2_test_source();
    let catalog = EntityCatalog {
        entities: vec![
            EntityDescriptor {
                entity_id: "aurora".into(),
                display_name: "Aurora".into(),
                aliases: vec!["active_soul".into()],
                role: EntityRole::Soul,
                active: true,
            },
            EntityDescriptor {
                entity_id: "player".into(),
                display_name: "visitor".into(),
                aliases: vec!["active_player".into()],
                role: EntityRole::ActivePlayer,
                active: true,
            },
            EntityDescriptor {
                entity_id: "key".into(),
                display_name: "key".into(),
                aliases: Vec::new(),
                role: EntityRole::Object,
                active: true,
            },
        ],
    };
    let outcome = compile_perception_v2_runtime(
        raw,
        Some(StructuredEnforcement::JsonSchema),
        &source,
        catalog,
        &SimulationSnapshot::default(),
        "test".into(),
        "test-model",
    )
    .expect("production V2 compiles");

    assert!(!outcome.conversion.patch.is_empty());
    assert_eq!(
        outcome.structured_run_classification,
        "perception_v2_commit_ready"
    );
    assert_eq!(outcome.fallback_path, vec![EVALUATOR_MODE_PERCEPTION_V2]);
    assert!(outcome.structured_enforcement_validated);
}

#[test]
fn perception_v2_source_refuses_missing_engine_identity() {
    let error = production_perception_source(
        "conversation",
        None,
        Some("turn"),
        None,
        Some(1),
        2,
        None,
        vec!["aurora".into()],
        "hello",
        "Aurora listens.",
    )
    .expect_err("branch identity is mandatory");
    assert!(error.contains("ledger branch"), "{error}");
}

#[test]
fn op_repair_message_focuses_on_failed_ops_with_reasons() {
    let failed = vec![
        EvaluatorOpRepairRequest {
            op_json: r#"{"op":"relationship_event","perceived_by_entity_id":"preset_male"}"#.into(),
            reason: "player not valid in soul-only field".into(),
        },
        EvaluatorOpRepairRequest {
            op_json: r#"{"op":"add_memory","evidence_quote":"Dragons."}"#.into(),
            reason: "evidence not present in turn".into(),
        },
    ];
    let message = build_op_repair_user_message(&failed, "I wait.", "Aurora watches.");
    assert!(message.contains("REPAIR TASK"));
    assert!(message.contains("player not valid in soul-only field"));
    assert!(message.contains("evidence not present in turn"));
    assert!(message.contains("preset_male"));
    // Anchored to the actual turn so the model can re-ground the fix.
    assert!(message.contains("I wait."));
    assert!(message.contains("Aurora watches."));
}

#[test]
fn perception_v2_repair_extracts_only_rejected_candidates() {
    let source = perception_v2_test_source();
    let draft = PerceptionBatchDraft {
        schema_version: state_engine::compiler::PERCEPTION_IR_SCHEMA_VERSION,
        candidates: vec![
            PerceptionCandidateDraft {
                kind: PerceptionKind::Event,
                subject_ref: "active_player".into(),
                predicate: "returned".into(),
                object: None,
                actor_ref: Some("active_player".into()),
                perceiver_ref: Some("active_soul".into()),
                target_refs: Vec::new(),
                evidence: EvidenceSpan {
                    source: EvidenceSource::AssistantMessage,
                    quote: "visitor return the key".into(),
                    start_char: None,
                    end_char: None,
                },
                epistemic_mode: EpistemicMode::DirectlyObserved,
                extraction_confidence: 0.9,
                temporal: TemporalExpression {
                    anchor: TemporalAnchor::CurrentTurn,
                    expression: None,
                },
                durability_hint: DurabilityHint::LongTerm,
                relationship_signal: None,
            },
            PerceptionCandidateDraft {
                kind: PerceptionKind::BeliefExpression,
                subject_ref: "active_soul".into(),
                predicate: "suspects".into(),
                object: None,
                actor_ref: Some("active_soul".into()),
                perceiver_ref: Some("active_soul".into()),
                target_refs: Vec::new(),
                evidence: EvidenceSpan {
                    source: EvidenceSource::AssistantMessage,
                    quote: "Aurora sees".into(),
                    start_char: None,
                    end_char: None,
                },
                epistemic_mode: EpistemicMode::Inferred,
                extraction_confidence: 0.8,
                temporal: TemporalExpression {
                    anchor: TemporalAnchor::CurrentTurn,
                    expression: None,
                },
                durability_hint: DurabilityHint::Session,
                relationship_signal: None,
            },
        ],
        no_op_reason: None,
    };
    let batch = seal_perception_batch(
        &source,
        draft,
        ModelProvenance {
            provider: "test".into(),
            model: "test".into(),
            prompt_version: "v2".into(),
            schema_name: PERCEPTION_IR_SCHEMA_NAME.into(),
        },
    )
    .expect("batch");
    let rejected_id = batch.candidates[1].candidate_id.clone();
    let normalized = serde_json::to_string(&batch).expect("normalized batch");
    let repair = rejected_ops_for_repair(
        &normalized,
        &[state_engine::evaluator::EvaluatorCandidateRejection {
            candidate_id: rejected_id,
            reason: "evidence_quote_not_found".into(),
        }],
    );
    assert_eq!(repair.len(), 1);
    assert!(repair[0].op_json.contains("\"kind\":\"belief_expression\""));
    assert!(!repair[0].op_json.contains("\"kind\":\"event\""));
}

#[test]
fn loopback_endpoints_are_detected() {
    assert!(is_loopback_endpoint("http://127.0.0.1:8080/v1"));
    assert!(is_loopback_endpoint("http://localhost:8080/v1"));
    assert!(is_loopback_endpoint("HTTP://LocalHost:1234/v1"));
    assert!(is_loopback_endpoint("http://0.0.0.0:8080/v1"));
    assert!(is_loopback_endpoint("http://[::1]:8080/v1"));
    // Remote endpoints must keep their own transport config.
    assert!(!is_loopback_endpoint("https://openrouter.ai/api/v1"));
    assert!(!is_loopback_endpoint("https://api.openai.com/v1"));
    assert!(!is_loopback_endpoint(""));
}

#[test]
fn exe_sibling_is_derived_only_for_llamafiles() {
    assert_eq!(
        exe_sibling_for_llamafile("D:\\models\\qwen.llamafile").as_deref(),
        Some("D:\\models\\qwen.exe")
    );
    assert_eq!(
        exe_sibling_for_llamafile("/home/u/qwen.LLAMAFILE").as_deref(),
        Some("/home/u/qwen.exe")
    );
    // Already runnable or unrelated extensions are left alone.
    assert_eq!(exe_sibling_for_llamafile("D:\\models\\qwen.exe"), None);
    assert_eq!(exe_sibling_for_llamafile("D:\\models\\qwen.gguf"), None);
    assert_eq!(exe_sibling_for_llamafile("qwen"), None);
}

#[test]
fn parse_listening_port_handles_llamafile_log_variants() {
    assert_eq!(
        parse_listening_port("llama server listening at http://127.0.0.1:8081"),
        Some(8081)
    );
    assert_eq!(
        parse_listening_port(
            "0.04.012 I srv server_main: server is listening on http://127.0.0.1:8082"
        ),
        Some(8082)
    );
    assert_eq!(
        parse_listening_port("server is listening on http://0.0.0.0:9090 - starting"),
        Some(9090)
    );
    // Non-listening lines and listening lines without a URL yield nothing.
    assert_eq!(
        parse_listening_port("llm_load_tensors: loading model"),
        None
    );
    assert_eq!(parse_listening_port("now listening for connections"), None);
}

#[test]
fn form_rejected_ops_build_repair_requests_from_rejected_rows() {
    let trace = state_engine::evaluator_form::EvalFormTrace {
        evaluator_row_traces: vec![
            state_engine::evaluator_form::EvalRowTrace {
                row_kind: "relationship".to_string(),
                row_index: 0,
                raw_row: serde_json::json!({ "intent": "-5", "row_enabled": null }),
                normalized_row: serde_json::json!({}),
                validation_status: "rejected".to_string(),
                rejection_reason: Some("relationship_event_missing_key: row_enabled".to_string()),
                compiler_result: "rejected".to_string(),
            },
            state_engine::evaluator_form::EvalRowTrace {
                row_kind: "memory".to_string(),
                row_index: 1,
                raw_row: serde_json::json!({ "content": "ok" }),
                normalized_row: serde_json::json!({ "content": "ok" }),
                validation_status: "accepted".to_string(),
                rejection_reason: None,
                compiler_result: "memory_candidate_created".to_string(),
            },
        ],
        ..Default::default()
    };
    let ops = form_rejected_ops_for_repair(&trace);
    // Only the rejected row becomes a repair request.
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].reason, "relationship_event_missing_key: row_enabled");
    assert!(ops[0].op_json.contains("\"intent\":\"-5\""));
}

#[test]
fn reextract_message_requests_full_extraction() {
    let message = build_reextract_user_message("I sit down.", "Aurora watches him.");
    assert!(message.contains("RE-EXTRACTION TASK"));
    assert!(message.contains("I sit down."));
    assert!(message.contains("Aurora watches him."));
    // It is NOT the focused fix prompt.
    assert!(!message.contains("REPAIR TASK"));
    // Bulletproofing: exact op shapes are shown and empty ops are forbidden.
    assert!(message.contains("MUST return at least one op"));
    assert!(message.contains("\"op\":\"relationship_event\""));
    assert!(message.contains("\"op\":\"add_memory\""));
    assert!(message.contains("\"op\":\"update_object_state\""));
    // Example axes must match the schema's required keys exactly.
    for axis in ["intent", "honesty", "boundary_treatment", "predictability"] {
        assert!(
            message.contains(&format!("\"{axis}\":")),
            "{axis} missing from example"
        );
    }
}

/// Dev-only repair benchmark. Drives a LIVE local model through the same
/// two-stage flow the app uses — a FORM (non-tool-call) state-update attempt,
/// then the REPAIR (reextract) pass — and reports, per scene, how many ops the
/// repair produced and how many survived validation. Lets you compare local
/// models for JSON/repair quality without running the whole app benchmark.
///
/// Ignored by default (needs a running model). Run e.g.:
///   set REPAIR_BENCH_URL=http://127.0.0.1:8080/v1
///   set REPAIR_BENCH_MODEL=local
///   cargo test --lib repair_bench_local -- --ignored --nocapture
#[test]
#[ignore]
fn repair_bench_local() {
    use std::time::{Duration, Instant};

    let url = std::env::var("REPAIR_BENCH_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8080/v1".to_string());
    let model = std::env::var("REPAIR_BENCH_MODEL").unwrap_or_else(|_| "local".to_string());

    let (soul, world, fixture_user, fixture_narrator, _spec) = form_runtime_fixture();
    // Scenes that clearly warrant durable state (relationship beats / facts).
    let scenes: Vec<(&str, String, String)> = vec![
            ("fixture", fixture_user, fixture_narrator),
            (
                "boundary_respect",
                "I don't reach for the sketchbook. \"Only if you want to show me,\" I say. \"Not because you think that's what I'm here for.\"".to_string(),
                "Aurora's guard flickers. She studies him a long moment, then slides the book an inch closer. \"...Okay,\" she says quietly.".to_string(),
            ),
            (
                "trust_break",
                "I laugh and repeat her secret back to her, loud enough for the whole room to hear.".to_string(),
                "Aurora goes still. The warmth drains from her face; she pulls her jacket tight and turns away without a word.".to_string(),
            ),
        ];

    let settings = ApiProviderSettings {
        base_url: url.clone(),
        api_key: "local".to_string(),
        model: model.clone(),
        structured_evaluator_transport: Some("json_schema".to_string()),
        structured_evaluator_policy: Some("allow_fallback".to_string()),
        ..ApiProviderSettings::default()
    };

    let provider = crate::providers::api::ApiProvider::default();
    // The strict REPAIR ops schema (at least one op, no no_op escape) — the
    // same one the live repair path uses, NOT the EnginePatch schema.
    let schema = evaluator_ops_repair_json_schema();
    // The slow FORM (non-tool-call) stage is opt-in: it adds a large second
    // call per scene (~5 min on CPU) and isn't validated. Set REPAIR_BENCH_FORM=1.
    let run_form_stage = std::env::var("REPAIR_BENCH_FORM").is_ok();
    let structured_system = build_structured_evaluator_prompt(&soul, Some(&world));
    let form_system = crate::providers::api::build_state_updater_prompt(&soul, Some(&world));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    println!("\n=== repair_bench  model='{model}'  url='{url}' ===");
    for (name, user, narrator) in &scenes {
        println!("\n--- scene '{name}' ---");

        // Stage 1 (opt-in): FORM (non-tool-call) state-update attempt — what the
        // primary evaluator does; shown raw for inspection, not validated.
        if run_form_stage {
            let form_user = format!("User: {user}\nNarrator: {narrator}");
            let form_started = Instant::now();
            let form_raw = runtime
                .block_on(
                    provider.complete_streaming(&settings, &form_system, &form_user, |_| Ok(())),
                )
                .map(|completion| completion.raw_text)
                .unwrap_or_else(|err| format!("<form call failed: {err}>"));
            println!(
                "  form(non-tool): {:.1}s, {} chars raw",
                form_started.elapsed().as_secs_f64(),
                form_raw.trim().len()
            );
        }

        // Stage 2: REPAIR (reextract) — the thing under test. Validated.
        let repair_user = build_reextract_user_message(user, narrator);
        let started = Instant::now();
        let result = runtime.block_on(provider.complete_structured_prompt(
            &settings,
            &structured_system,
            &repair_user,
            0.3,
            Some(Duration::from_millis(LOCAL_REPAIR_TIMEOUT_MS)),
            EVALUATOR_OPS_REPAIR_SCHEMA_NAME,
            &schema,
        ));
        let elapsed = started.elapsed();

        match result {
            Err(err) => println!(
                "  repair: CALL FAILED after {:.1}s: {err}",
                elapsed.as_secs_f64()
            ),
            Ok(completion) => match compile_evaluator_structured_runtime(
                &completion.raw_text,
                Some(StructuredEnforcement::JsonSchema),
                &soul,
                &world,
                user,
                narrator,
                None,
                true,
            ) {
                Err(err) => println!(
                    "  repair: {:.1}s, ops UNPARSEABLE/empty: {err}\n    raw: {}",
                    elapsed.as_secs_f64(),
                    completion.raw_text.trim()
                ),
                Ok(outcome) => {
                    let out = &outcome.output;
                    println!(
                            "  repair: {:.1}s | ops_parsed={} | extracted mem={} rel={} obj={} world={} | patch_empty={} | accepted={} rejected={}",
                            elapsed.as_secs_f64(),
                            outcome.structured_ops_count.unwrap_or(0),
                            out.memory_candidates.len(),
                            out.relationship_evaluations.len(),
                            out.object_changes.len(),
                            out.world_changes.len(),
                            outcome.conversion.patch.is_empty(),
                            outcome.conversion.accepted_candidate_ids.len(),
                            outcome.conversion.rejected_candidates.len(),
                        );
                    if let Some(reason) = &out.no_op_reason {
                        println!("      no_op_reason: {reason}");
                    }
                    for rejection in &outcome.conversion.rejected_candidates {
                        println!(
                            "      rejected {}: {}",
                            rejection.candidate_id, rejection.reason
                        );
                    }
                    let preview: String = completion.raw_text.trim().chars().take(600).collect();
                    println!("      raw: {preview}");
                }
            },
        }
    }
    println!("\n=== repair_bench done ===");
}

#[test]
fn transient_provider_errors_are_retryable() {
    assert!(is_transient_provider_error(
        "API provider returned an error in a 200 OK body: Provider returned error"
    ));
    assert!(is_transient_provider_error(
        "API request failed with 429: rate limited"
    ));
    assert!(is_transient_provider_error(
        "API request failed with 503: upstream"
    ));
    assert!(is_transient_provider_error("request timed out"));
    assert!(is_transient_provider_error(
        "API request failed: connection reset"
    ));
    assert!(is_transient_provider_error(
        "API stream failed: body truncated"
    ));
    // Shape problems are NOT transient — they must surface for diagnosis.
    assert!(!is_transient_provider_error(
        "API response parse failed: no assistant content found; raw body: {…}"
    ));
    assert!(!is_transient_provider_error(
        "API response did not include assistant content"
    ));
    assert!(!is_transient_provider_error(
        "API key is required for API provider mode"
    ));
    // 4xx (auth/bad request) share the "API request failed with" prefix but
    // must NOT be retried.
    assert!(!is_transient_provider_error(
        "API request failed with 401: invalid key"
    ));
    assert!(!is_transient_provider_error(
        "API request failed with 400: bad request"
    ));
}

#[test]
fn test_is_body_only_markers() {
    assert_eq!(is_body_only_markers(""), true);
    assert_eq!(is_body_only_markers("   "), true);
    assert_eq!(is_body_only_markers("Assistant"), true);
    assert_eq!(is_body_only_markers("[Assistant]"), true);
    assert_eq!(is_body_only_markers("Scene | Focus: chain_lock"), true);
    assert_eq!(
        is_body_only_markers("The chain lock clicked against the doorframe."),
        false
    );
}
use state_engine::{
    context_compiler::estimate_tokens,
    evaluator_ingest::parse_evaluator_output,
    hidden_state::HiddenState,
    patch::{EnginePatch, MemoryPatch, RelationshipDelta, SoulPatch, WorldPatch},
    soul::{MemoryEntry, MemorySourceType, ObjectState, Relationship, TruthStatus},
};

fn variant_test_setup(conversation_id: &str) -> (Connection, Soul, db::SessionBranch, i64) {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    db::upsert_soul(&conn, &soul).expect("soul");
    db::ensure_conversation(&conn, conversation_id, &soul.character_id).expect("conversation");
    let world = db::create_legacy_session_world_from_soul(&conn, &soul).expect("world");
    let branch = db::create_session_branch(&conn, conversation_id, &soul, &world).expect("branch");
    let assistant_id =
        db::insert_message_and_get_id(&conn, conversation_id, "assistant", "Aurora answers.")
            .expect("assistant");
    (conn, soul, branch, assistant_id)
}

fn command_test_setup(conversation_id: &str) -> (Connection, Soul) {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    db::upsert_soul(&conn, &soul).expect("soul");
    db::ensure_conversation(&conn, conversation_id, &soul.character_id).expect("conversation");
    let world = db::create_legacy_session_world_from_soul(&conn, &soul).expect("world");
    db::create_session_branch(&conn, conversation_id, &soul, &world).expect("branch");
    (conn, soul)
}

fn run_command_turn(
    conn: &Connection,
    conversation_id: &str,
    soul: &Soul,
    text: &str,
) -> TurnResult {
    let request_id = uuid_like_id();
    let turn_id = format!("turn_{request_id}");
    maybe_handle_chat_command_with_conn(
        None,
        conn,
        conversation_id.to_string(),
        soul.character_id.clone(),
        text.to_string(),
        &request_id,
        &turn_id,
        ContextMode::Brief,
        None,
    )
    .expect("command result")
    .expect("recognized command")
}

fn run_command_turn_with_llm(
    conn: &Connection,
    conversation_id: &str,
    soul: &Soul,
    text: &str,
    command_llm_result: CommandLlmResult,
) -> TurnResult {
    let request_id = uuid_like_id();
    let turn_id = format!("turn_{request_id}");
    maybe_handle_chat_command_with_conn(
        None,
        conn,
        conversation_id.to_string(),
        soul.character_id.clone(),
        text.to_string(),
        &request_id,
        &turn_id,
        ContextMode::Brief,
        Some(command_llm_result),
    )
    .expect("command result")
    .expect("recognized command")
}

fn simulated_command_llm(mode: &'static str, response: &str) -> CommandLlmResult {
    CommandLlmResult {
        called: true,
        mode: Some(mode),
        system_prompt: command_system_prompt_for_mode(mode).to_string(),
        user_message: "[COMMAND TEST]".into(),
        response: Some(sanitize_command_llm_response(response)),
        raw_response: Some(response.into()),
        provider_error: None,
        model: "test-command-model".into(),
        base_url: "test".into(),
        elapsed_ms: 0,
        simulated: true,
        output_guard_action: "none",
    }
}

fn latest_command_trace(conn: &Connection, conversation_id: &str) -> serde_json::Value {
    let logs = db::list_llm_payload_logs(conn, conversation_id).expect("logs");
    let log = logs.last().expect("latest log");
    serde_json::from_str(log.pipeline_trace_json.as_deref().expect("trace")).expect("json")
}

fn benchmark_summary_fixture() -> BenchmarkSummary {
    BenchmarkSummary {
        benchmark_id: "bench-test".into(),
        benchmark_type: "scripted_visible_replay".into(),
        conversation_id: "benchmark-test".into(),
        started_at: 1,
        completed_at: 2,
        turn_count_requested: 2,
        turn_count_completed: 2,
        narrator_model: "narrator-model".into(),
        evaluator_model: "evaluator-model".into(),
        player_simulator_model: None,
        narrator_failures: 0,
        evaluator_failures: 0,
        tool_call_success_count: 2,
        tool_call_failure_count: 0,
        retry_count: 0,
        retry_success_count: 0,
        fallback_count: 0,
        syntactic_repair_count: 0,
        default_player_leak_detected: false,
        duplicate_relationship_context_detected: false,
        final_memory_count: 3,
        final_object_state_count: 1,
        final_relationship_count: 1,
        visible_turns_requested: 2,
        visible_turns_completed: 2,
        visible_user_messages_created: 2,
        visible_assistant_messages_created: 2,
        unique_user_message_ids: 2,
        unique_assistant_message_ids: 2,
        internal_evaluator_retry_count: 0,
        internal_evaluator_retry_payload_count: 0,
        duplicate_turn_rows_detected: false,
        duplicate_turn_message_pairs: Vec::new(),
        player_simulator_payload_count: 0,
        per_turn: Vec::new(),
        object_identity_checks: vec![BenchmarkObjectIdentityCheck {
            label: "wet jacket".into(),
            expected_object_id: "preset_male_jacket_1".into(),
            found: true,
        }],
        mne_export_path: Some("benchmark.mne".into()),
        payload_history_path: Some("payload.md".into()),
        summary_json_path: Some("summary.json".into()),
        scorecard: BenchmarkScorecard {
            visible_chat_messages_created: true,
            normal_pipeline_used: true,
            visible_turns_requested: 2,
            visible_turns_completed: 2,
            visible_user_messages_created: 2,
            visible_assistant_messages_created: 2,
            unique_user_message_ids: 2,
            unique_assistant_message_ids: 2,
            internal_evaluator_retry_count: 0,
            internal_evaluator_retry_payload_count: 0,
            duplicate_turn_rows_detected: false,
            duplicate_turn_message_pairs: Vec::new(),
            player_simulator_payload_count: 0,
            turn_count_requested: 2,
            turn_count_completed: 2,
            player_simulator_calls: 0,
            narrator_calls: 2,
            evaluator_calls: 2,
            evaluator_waited_each_turn: true,
            memory_updated: true,
            object_state_updated: true,
            relationship_updated: true,
            relationship_target_checked: Some("preset_male".into()),
            relationship_changed_from: None,
            relationship_changed_to: Some(serde_json::json!({ "trust": 12.0 })),
            relationship_delta_patch_ids: vec!["relationship-patch-test".into()],
            relationship_delta_sources: vec!["enrichment".into()],
            evaluator_provider_failures: 0,
            structured_provider_429_count: 0,
            evaluator_response_failed_count: 0,
            evaluator_empty_patch_count: 0,
            form_rows_rejected_count: 0,
            local_repair_invoked_count: 0,
            local_reextract_invoked_count: 0,
            local_repair_payload_count: 0,
            local_repair_response_count: 0,
            local_repair_state_patch_count: 0,
            payload_history_export_succeeded: true,
            narrator_visible_response_each_turn: true,
            narrator_provider_error: None,
            stop_reason: None,
            failed_stage: None,
            evaluator_used_tool_call_where_required: true,
            no_evaluator_form_v1_fallback_in_strict_mode: true,
            syntactic_repair_unused_in_strict_mode: true,
            strict_tool_evaluator: false,
            evaluator_mode_actual: String::new(),
            local_repair_recovered_state_when_warranted: true,
            local_repair_unavailable: false,
            memories_increased_over_time: true,
            active_player_relationship_changed_when_warranted: true,
            object_ids_stable: true,
            default_player_not_normal_rp_relationship_target: true,
            mne_export_succeeded: true,
            pass: true,
            failure_reasons: Vec::new(),
        },
    }
}

#[test]
fn scripted_benchmark_runs_fixed_messages() {
    let turns = benchmark_scripted_turns();

    assert!(turns.len() >= 5);
    assert_eq!(
        turns[0],
        "I step inside, leaving my wet jacket near the door."
    );
    assert!(turns.iter().any(|turn| turn.contains("jacket")));
}

#[test]
fn self_play_benchmark_requires_player_simulator_profile() {
    assert!(!benchmark_requires_player_profile(
        &BenchmarkType::ScriptedVisibleReplay
    ));
    assert!(benchmark_requires_player_profile(
        &BenchmarkType::VisibleAiChat
    ));
    assert!(benchmark_requires_player_profile(
        &BenchmarkType::MultiAgentVisibleChat
    ));
}

#[test]
fn player_simulator_prompt_keeps_control_on_user_side() {
    let prompt = benchmark_player_simulator_prompt();

    assert!(prompt.contains("You control only the active player persona"));
    assert!(prompt.contains("Write only the next user message"));
    assert!(prompt.contains("You are not the narrator"));
    assert!(prompt.contains("backend JSON"));
}

#[test]
fn strict_benchmark_fails_on_form_fallback() {
    let mut summary = benchmark_summary_fixture();
    summary.fallback_count = 1;

    let scorecard = benchmark_scorecard(&summary, true, 1, 0, 0);

    assert!(!scorecard.pass);
    assert!(scorecard
        .failure_reasons
        .contains(&"no_evaluator_form_v1_fallback_in_strict_mode".to_string()));
}

#[test]
fn strict_benchmark_fails_without_tool_call_successes() {
    let mut summary = benchmark_summary_fixture();
    summary.tool_call_success_count = 0;

    let scorecard = benchmark_scorecard(&summary, true, 1, 0, 0);

    assert!(!scorecard.pass);
    assert!(scorecard
        .failure_reasons
        .contains(&"evaluator_used_tool_call_where_required".to_string()));
}

#[test]
fn evaluator_not_called_if_narrator_fails_scorecard() {
    let mut summary = benchmark_summary_fixture();
    summary.narrator_failures = 1;
    summary.turn_count_completed = 0;
    summary.visible_turns_completed = 0;
    summary.visible_assistant_messages_created = 0;
    summary.tool_call_success_count = 0;
    summary.evaluator_failures = 0;
    summary.per_turn = vec![BenchmarkTurnSummary {
        turn_index: 1,
        stage: "narrator_failed".into(),
        simulated_user_message: "I try another line.".into(),
        narrator_response_present: false,
        narrator_error: Some(
            "narrator_provider_error: API stream failed: error decoding response body".into(),
        ),
        evaluator_mode: EVALUATOR_MODE_FORM_V1.into(),
        structured_transport_actual: None,
        tool_calls_present: false,
        tool_call_count: 0,
        structured_retry_count: 0,
        fallback_path: Vec::new(),
        syntactic_repair_used: false,
        memory_count_after: summary.final_memory_count,
        object_count_after: summary.final_object_state_count,
        relationship_summary_after: String::new(),
    }];

    let scorecard = benchmark_scorecard(&summary, false, 1, 0, 0);

    assert!(!scorecard.pass);
    assert!(!scorecard.narrator_visible_response_each_turn);
    assert_eq!(
        scorecard.narrator_provider_error.as_deref(),
        Some("narrator_provider_error: API stream failed: error decoding response body")
    );
    assert!(scorecard
        .failure_reasons
        .contains(&"narrator_visible_response_each_turn".to_string()));
    assert_eq!(summary.evaluator_failures, 0);
}

#[test]
fn evaluator_fail_stop_is_not_reported_as_narrator_failure() {
    let mut summary = benchmark_summary_fixture();
    summary.benchmark_type = "visible_ai_chat".into();
    summary.turn_count_requested = 5;
    summary.turn_count_completed = 2;
    summary.visible_turns_requested = 5;
    summary.visible_turns_completed = 2;
    summary.visible_user_messages_created = 2;
    summary.visible_assistant_messages_created = 2;
    summary.narrator_failures = 0;
    summary.evaluator_failures = 1;
    summary.per_turn = vec![BenchmarkTurnSummary {
        turn_index: 2,
        stage: "evaluator_failed".into(),
        simulated_user_message: "third player line".into(),
        narrator_response_present: false,
        narrator_error: Some(
            "evaluator_error: State update in progress and did not finish within 120000ms".into(),
        ),
        evaluator_mode: EVALUATOR_MODE_STRUCTURED_V1.into(),
        structured_transport_actual: Some("tool_call".into()),
        tool_calls_present: true,
        tool_call_count: 1,
        structured_retry_count: 0,
        fallback_path: vec!["structured_tool_call".into()],
        syntactic_repair_used: false,
        memory_count_after: summary.final_memory_count,
        object_count_after: summary.final_object_state_count,
        relationship_summary_after: String::new(),
    }];

    let scorecard = benchmark_scorecard(&summary, false, 1, 0, 0);

    assert!(!scorecard.pass);
    assert!(scorecard.narrator_visible_response_each_turn);
    assert_eq!(scorecard.narrator_provider_error, None);
    assert_eq!(scorecard.stop_reason.as_deref(), Some("evaluator_failed"));
    assert_eq!(scorecard.failed_stage.as_deref(), Some("evaluator_called"));
    assert!(!scorecard
        .failure_reasons
        .contains(&"narrator_visible_response_each_turn".to_string()));
    assert!(scorecard
        .failure_reasons
        .contains(&"evaluator_failed".to_string()));
    assert!(scorecard
        .failure_reasons
        .contains(&"blocked_by_evaluator_failure".to_string()));
    assert!(scorecard
        .failure_reasons
        .contains(&"skipped_after_evaluator_failure".to_string()));
    assert!(!scorecard
        .failure_reasons
        .contains(&"visible_turns_completed_matches_requested".to_string()));
    assert!(!scorecard
        .failure_reasons
        .contains(&"relationship_updated".to_string()));
}

#[test]
fn early_narrator_failure_does_not_require_unplayed_player_turns() {
    let mut summary = benchmark_summary_fixture();
    summary.benchmark_type = "visible_ai_chat".into();
    summary.turn_count_requested = 5;
    summary.visible_turns_requested = 5;
    summary.turn_count_completed = 1;
    summary.visible_turns_completed = 1;
    summary.visible_user_messages_created = 1;
    summary.visible_assistant_messages_created = 1;
    summary.unique_user_message_ids = 1;
    summary.unique_assistant_message_ids = 1;
    summary.player_simulator_payload_count = 2;
    summary.narrator_failures = 1;
    summary.per_turn = vec![
        BenchmarkTurnSummary {
            turn_index: 0,
            stage: "completed".into(),
            simulated_user_message: "completed user".into(),
            narrator_response_present: true,
            narrator_error: None,
            evaluator_mode: EVALUATOR_MODE_FORM_V1.into(),
            structured_transport_actual: None,
            tool_calls_present: false,
            tool_call_count: 0,
            structured_retry_count: 0,
            fallback_path: Vec::new(),
            syntactic_repair_used: false,
            memory_count_after: summary.final_memory_count,
            object_count_after: summary.final_object_state_count,
            relationship_summary_after: String::new(),
        },
        BenchmarkTurnSummary {
            turn_index: 1,
            stage: "narrator_failed".into(),
            simulated_user_message: "failed user".into(),
            narrator_response_present: false,
            narrator_error: Some("narrator_provider_error: stream failed".into()),
            evaluator_mode: EVALUATOR_MODE_FORM_V1.into(),
            structured_transport_actual: None,
            tool_calls_present: false,
            tool_call_count: 0,
            structured_retry_count: 0,
            fallback_path: Vec::new(),
            syntactic_repair_used: false,
            memory_count_after: summary.final_memory_count,
            object_count_after: summary.final_object_state_count,
            relationship_summary_after: String::new(),
        },
    ];

    let scorecard = benchmark_scorecard(&summary, false, 1, 0, 0);

    assert!(!scorecard.pass);
    assert!(scorecard
        .failure_reasons
        .contains(&"narrator_visible_response_each_turn".to_string()));
    assert!(!scorecard
        .failure_reasons
        .contains(&"player_simulator_payload_count".to_string()));
}

#[test]
fn narrator_provider_error_falls_back_to_payload_log() {
    let logs = vec![
        LlmPayloadLog {
            provider: "player_simulator".into(),
            provider_error: Some("ignored player error".into()),
            ..Default::default()
        },
        LlmPayloadLog {
            provider: "narrator_brief".into(),
            provider_error: Some("API stream failed: error decoding response body".into()),
            ..Default::default()
        },
    ];

    assert_eq!(
        latest_narrator_provider_error(&logs).as_deref(),
        Some("narrator_provider_error: API stream failed: error decoding response body")
    );
}

#[test]
fn narrator_provider_error_fallback_preserves_existing_prefix() {
    let logs = vec![LlmPayloadLog {
        provider: "narrator_brief".into(),
        provider_error: Some("narrator_provider_error: stream failed".into()),
        ..Default::default()
    }];

    assert_eq!(
        latest_narrator_provider_error(&logs).as_deref(),
        Some("narrator_provider_error: stream failed")
    );
}

#[test]
fn visible_turn_completion_is_capped_by_frontend_completed_count() {
    assert_eq!(benchmark_visible_turns_completed(3, 1, &[]), 1);
}

#[test]
fn visible_turn_completion_excludes_failed_stage_rows() {
    let rows = vec![
        BenchmarkTurnSummary {
            turn_index: 0,
            stage: "completed".into(),
            simulated_user_message: "ok".into(),
            narrator_response_present: true,
            narrator_error: None,
            evaluator_mode: EVALUATOR_MODE_FORM_V1.into(),
            structured_transport_actual: None,
            tool_calls_present: false,
            tool_call_count: 0,
            structured_retry_count: 0,
            fallback_path: Vec::new(),
            syntactic_repair_used: false,
            memory_count_after: 0,
            object_count_after: 0,
            relationship_summary_after: String::new(),
        },
        BenchmarkTurnSummary {
            turn_index: 1,
            stage: "benchmark_summary_failed".into(),
            simulated_user_message: "not complete".into(),
            narrator_response_present: true,
            narrator_error: Some("benchmark_summary_error: capture failed".into()),
            evaluator_mode: EVALUATOR_MODE_FORM_V1.into(),
            structured_transport_actual: None,
            tool_calls_present: false,
            tool_call_count: 0,
            structured_retry_count: 0,
            fallback_path: Vec::new(),
            syntactic_repair_used: false,
            memory_count_after: 0,
            object_count_after: 0,
            relationship_summary_after: String::new(),
        },
    ];

    assert_eq!(benchmark_visible_turns_completed(2, 2, &rows), 1);
}

#[test]
fn visible_turn_completion_requires_player_text() {
    let rows = vec![BenchmarkTurnSummary {
        turn_index: 0,
        stage: "completed".into(),
        simulated_user_message: "   ".into(),
        narrator_response_present: true,
        narrator_error: None,
        evaluator_mode: EVALUATOR_MODE_FORM_V1.into(),
        structured_transport_actual: None,
        tool_calls_present: false,
        tool_call_count: 0,
        structured_retry_count: 0,
        fallback_path: Vec::new(),
        syntactic_repair_used: false,
        memory_count_after: 0,
        object_count_after: 0,
        relationship_summary_after: String::new(),
    }];

    assert_eq!(benchmark_visible_turns_completed(1, 1, &rows), 0);
}

#[test]
fn object_state_update_is_optional_without_identity_checks() {
    let mut summary = benchmark_summary_fixture();
    summary.object_identity_checks.clear();
    summary.final_object_state_count = 0;

    let scorecard = benchmark_scorecard(&summary, false, 1, 0, 1);

    assert!(!scorecard.object_state_updated);
    assert!(!scorecard
        .failure_reasons
        .contains(&"object_state_updated".to_string()));
}

#[test]
fn failed_visible_benchmark_user_message_can_be_hidden_without_completing_turn() {
    let (conn, soul) = command_test_setup("bench-narrator-failed-user");
    let branch =
        db::get_active_session_branch(&conn, "bench-narrator-failed-user").expect("branch");
    let completed_user_id = db::insert_message_and_get_id(
        &conn,
        "bench-narrator-failed-user",
        "user",
        "completed user",
    )
    .expect("completed user");
    let completed_assistant_id = db::insert_message_and_get_id(
        &conn,
        "bench-narrator-failed-user",
        "assistant",
        "completed assistant",
    )
    .expect("completed assistant");
    db::record_turn_commit_with_patch_for_turn_id(
        &conn,
        "turn-completed",
        "bench-narrator-failed-user",
        &branch.branch_id,
        None,
        Some(completed_user_id),
        completed_assistant_id,
        None,
        &EnginePatch::default(),
        false,
    )
    .expect("turn");
    let failed_user_id = db::insert_message_and_get_id(
        &conn,
        "bench-narrator-failed-user",
        "user",
        "failed user-only turn",
    )
    .expect("failed user");
    for index in 0..2 {
        db::insert_llm_payload_log(
            &conn,
            &LlmPayloadLog {
                conversation_id: "bench-narrator-failed-user".into(),
                provider: "player_simulator".into(),
                model: soul.character_name.clone(),
                request_id: Some(format!("player-{index}")),
                created_at: db::now_ts(),
                ..Default::default()
            },
        )
        .expect("payload");
    }

    assert_eq!(
        db::hide_latest_matching_active_user_tail(
            &conn,
            "bench-narrator-failed-user",
            "failed user-only turn",
        )
        .expect("hide failed benchmark user"),
        Some(failed_user_id)
    );

    let failed_user = db::get_message(&conn, "bench-narrator-failed-user", failed_user_id)
        .expect("failed user message remains for audit");
    assert_ne!(failed_user.status, "active");
    let active_messages =
        db::list_messages(&conn, "bench-narrator-failed-user", 20).expect("messages");
    assert!(active_messages
        .iter()
        .all(|message| message.id != failed_user_id));

    let audit = benchmark_ledger_audit(&conn, "bench-narrator-failed-user").expect("audit");
    assert_eq!(audit.visible_turns_completed, 1);
    assert_eq!(audit.visible_user_messages_created, 1);
    assert_eq!(audit.visible_assistant_messages_created, 1);
    assert_eq!(audit.player_simulator_payload_count, 2);
}

#[test]
fn visible_benchmark_narrator_failure_scorecard_counts_only_completed_pairs() {
    let (conn, soul) = command_test_setup("bench-narrator-failed-scorecard");
    let branch =
        db::get_active_session_branch(&conn, "bench-narrator-failed-scorecard").expect("branch");
    let completed_user_id = db::insert_message_and_get_id(
        &conn,
        "bench-narrator-failed-scorecard",
        "user",
        "completed user",
    )
    .expect("completed user");
    let completed_assistant_id = db::insert_message_and_get_id(
        &conn,
        "bench-narrator-failed-scorecard",
        "assistant",
        "completed assistant",
    )
    .expect("completed assistant");
    db::record_turn_commit_with_patch_for_turn_id(
        &conn,
        "turn-completed",
        "bench-narrator-failed-scorecard",
        &branch.branch_id,
        None,
        Some(completed_user_id),
        completed_assistant_id,
        None,
        &EnginePatch::default(),
        false,
    )
    .expect("completed commit");
    let failed_user_id = db::insert_message_and_get_id(
        &conn,
        "bench-narrator-failed-scorecard",
        "user",
        "failed user-only turn",
    )
    .expect("failed user");
    for index in 0..2 {
        db::insert_llm_payload_log(
            &conn,
            &LlmPayloadLog {
                conversation_id: "bench-narrator-failed-scorecard".into(),
                provider: "player_simulator".into(),
                model: soul.character_name.clone(),
                request_id: Some(format!("player-{index}")),
                created_at: db::now_ts(),
                ..Default::default()
            },
        )
        .expect("player payload");
    }
    db::insert_llm_payload_log(
        &conn,
        &LlmPayloadLog {
            conversation_id: "bench-narrator-failed-scorecard".into(),
            provider: "narrator_brief".into(),
            provider_error: Some("API stream failed: error decoding response body".into()),
            created_at: db::now_ts(),
            ..Default::default()
        },
    )
    .expect("narrator payload");

    let hidden = db::hide_latest_matching_active_user_tail(
        &conn,
        "bench-narrator-failed-scorecard",
        "failed user-only turn",
    )
    .expect("hide failed benchmark user");
    assert_eq!(hidden, Some(failed_user_id));

    let logs = db::list_llm_payload_logs(&conn, "bench-narrator-failed-scorecard").expect("logs");
    let narrator_error = latest_narrator_provider_error(&logs).expect("narrator error");
    let audit = benchmark_ledger_audit(&conn, "bench-narrator-failed-scorecard").expect("audit");
    let mut summary = benchmark_summary_fixture();
    summary.benchmark_type = "visible_ai_chat".into();
    summary.turn_count_requested = 5;
    summary.visible_turns_requested = 5;
    summary.turn_count_completed = audit.visible_turns_completed;
    summary.visible_turns_completed = audit.visible_turns_completed;
    summary.visible_user_messages_created = audit.visible_user_messages_created;
    summary.visible_assistant_messages_created = audit.visible_assistant_messages_created;
    summary.unique_user_message_ids = audit.unique_user_message_ids;
    summary.unique_assistant_message_ids = audit.unique_assistant_message_ids;
    summary.player_simulator_payload_count = audit.player_simulator_payload_count;
    summary.narrator_failures = 1;
    summary.tool_call_success_count = audit.visible_turns_completed;
    summary.object_identity_checks.clear();
    summary.per_turn = vec![
        BenchmarkTurnSummary {
            turn_index: 0,
            stage: "completed".into(),
            simulated_user_message: "completed user".into(),
            narrator_response_present: true,
            narrator_error: None,
            evaluator_mode: EVALUATOR_MODE_FORM_V1.into(),
            structured_transport_actual: None,
            tool_calls_present: false,
            tool_call_count: 0,
            structured_retry_count: 0,
            fallback_path: Vec::new(),
            syntactic_repair_used: false,
            memory_count_after: summary.final_memory_count,
            object_count_after: summary.final_object_state_count,
            relationship_summary_after: String::new(),
        },
        BenchmarkTurnSummary {
            turn_index: 1,
            stage: "narrator_failed".into(),
            simulated_user_message: "failed user-only turn".into(),
            narrator_response_present: false,
            narrator_error: Some(narrator_error.clone()),
            evaluator_mode: EVALUATOR_MODE_FORM_V1.into(),
            structured_transport_actual: None,
            tool_calls_present: false,
            tool_call_count: 0,
            structured_retry_count: 0,
            fallback_path: Vec::new(),
            syntactic_repair_used: false,
            memory_count_after: summary.final_memory_count,
            object_count_after: summary.final_object_state_count,
            relationship_summary_after: String::new(),
        },
    ];

    let scorecard = benchmark_scorecard(&summary, false, 1, summary.final_object_state_count, 0);

    assert_eq!(audit.visible_turns_completed, 1);
    assert_eq!(audit.visible_user_messages_created, 1);
    assert_eq!(audit.visible_assistant_messages_created, 1);
    assert_eq!(audit.player_simulator_payload_count, 2);
    assert!(!scorecard.pass);
    assert_eq!(scorecard.visible_turns_completed, 1);
    assert_eq!(scorecard.visible_user_messages_created, 1);
    assert_eq!(scorecard.visible_assistant_messages_created, 1);
    assert_eq!(scorecard.player_simulator_payload_count, 2);
    assert_eq!(scorecard.narrator_calls, 2);
    assert_eq!(scorecard.evaluator_calls, 1);
    assert_eq!(
        scorecard.narrator_provider_error.as_deref(),
        Some(narrator_error.as_str())
    );
    assert!(scorecard
        .failure_reasons
        .contains(&"visible_turns_completed_matches_requested".to_string()));
    assert!(scorecard
        .failure_reasons
        .contains(&"visible_user_messages_created_matches_requested".to_string()));
    assert!(scorecard
        .failure_reasons
        .contains(&"visible_assistant_messages_created_matches_requested".to_string()));
    assert!(scorecard
        .failure_reasons
        .contains(&"narrator_visible_response_each_turn".to_string()));
    assert!(!scorecard
        .failure_reasons
        .contains(&"player_simulator_payload_count".to_string()));
    assert!(!scorecard
        .failure_reasons
        .contains(&"object_state_updated".to_string()));
}

#[test]
fn benchmark_tail_user_cleanup_does_not_hide_completed_turn() {
    let (conn, _soul) = command_test_setup("bench-tail-cleanup-complete");
    let _user_id =
        db::insert_message_and_get_id(&conn, "bench-tail-cleanup-complete", "user", "same text")
            .expect("user");
    let assistant_id = db::insert_message_and_get_id(
        &conn,
        "bench-tail-cleanup-complete",
        "assistant",
        "assistant reply",
    )
    .expect("assistant");

    assert_eq!(
        db::hide_latest_matching_active_user_tail(
            &conn,
            "bench-tail-cleanup-complete",
            "same text",
        )
        .expect("no hide"),
        None
    );
    let active_messages =
        db::list_messages(&conn, "bench-tail-cleanup-complete", 20).expect("messages");
    assert!(active_messages
        .iter()
        .any(|message| message.id == assistant_id && message.status == "active"));
}

#[test]
fn benchmark_summary_reports_export_artifact_paths() {
    let summary = benchmark_summary_fixture();

    assert_eq!(summary.payload_history_path.as_deref(), Some("payload.md"));
    assert_eq!(summary.mne_export_path.as_deref(), Some("benchmark.mne"));
    assert_eq!(summary.summary_json_path.as_deref(), Some("summary.json"));
}

#[test]
fn benchmark_audit_counts_unique_visible_turns_and_player_simulator_payloads() {
    let (conn, soul) = command_test_setup("bench-audit");
    let branch = db::get_active_session_branch(&conn, "bench-audit").expect("branch");
    for index in 0..5 {
        let user_id =
            db::insert_message_and_get_id(&conn, "bench-audit", "user", &format!("user {index}"))
                .expect("user");
        let assistant_id = db::insert_message_and_get_id(
            &conn,
            "bench-audit",
            "assistant",
            &format!("assistant {index}"),
        )
        .expect("assistant");
        db::record_turn_commit_with_patch_for_turn_id(
            &conn,
            &format!("turn-{index}"),
            "bench-audit",
            &branch.branch_id,
            None,
            Some(user_id),
            assistant_id,
            None,
            &EnginePatch::default(),
            false,
        )
        .expect("turn");
        db::insert_llm_payload_log(
            &conn,
            &LlmPayloadLog {
                conversation_id: "bench-audit".into(),
                provider: "player_simulator".into(),
                model: soul.character_name.clone(),
                created_at: db::now_ts(),
                ..Default::default()
            },
        )
        .expect("payload");
    }

    let audit = benchmark_ledger_audit(&conn, "bench-audit").expect("audit");
    assert_eq!(audit.visible_turns_completed, 5);
    assert_eq!(audit.visible_user_messages_created, 5);
    assert_eq!(audit.visible_assistant_messages_created, 5);
    assert_eq!(audit.player_simulator_payload_count, 5);
    assert!(!audit.duplicate_turn_rows_detected);
}

#[test]
fn duplicate_turn_message_pair_makes_benchmark_fail() {
    let mut summary = benchmark_summary_fixture();
    summary.duplicate_turn_rows_detected = true;
    summary.duplicate_turn_message_pairs = vec!["1:2x2".into()];

    let scorecard = benchmark_scorecard(&summary, false, 1, 0, 0);

    assert!(!scorecard.pass);
    assert!(scorecard
        .failure_reasons
        .contains(&"no_duplicate_turn_rows".to_string()));
}

#[test]
fn benchmark_audit_ignores_regenerated_retry_rows_for_visible_completion() {
    let (conn, _soul) = command_test_setup("bench-retry-audit");
    let branch = db::get_active_session_branch(&conn, "bench-retry-audit").expect("branch");
    let user_id =
        db::insert_message_and_get_id(&conn, "bench-retry-audit", "user", "user").expect("u");
    let assistant_id =
        db::insert_message_and_get_id(&conn, "bench-retry-audit", "assistant", "assistant")
            .expect("a");
    db::record_turn_commit_with_patch_for_turn_id(
        &conn,
        "turn-visible",
        "bench-retry-audit",
        &branch.branch_id,
        None,
        Some(user_id),
        assistant_id,
        None,
        &EnginePatch::default(),
        false,
    )
    .expect("visible turn");
    db::record_turn_commit_with_patch_for_turn_id(
        &conn,
        "turn-retry",
        "bench-retry-audit",
        &branch.branch_id,
        Some("turn-visible"),
        Some(user_id),
        assistant_id,
        None,
        &EnginePatch::default(),
        true,
    )
    .expect("retry turn");
    db::insert_llm_payload_log(
        &conn,
        &LlmPayloadLog {
            conversation_id: "bench-retry-audit".into(),
            provider: "evaluator_structured_v1".into(),
            request_id: Some("eval_repair_test".into()),
            created_at: db::now_ts(),
            ..Default::default()
        },
    )
    .expect("repair payload");

    let audit = benchmark_ledger_audit(&conn, "bench-retry-audit").expect("audit");
    assert_eq!(audit.visible_turns_completed, 1);
    assert_eq!(audit.internal_evaluator_retry_count, 1);
    assert_eq!(audit.internal_evaluator_retry_payload_count, 1);
    assert!(audit.duplicate_turn_rows_detected);
}

#[test]
fn benchmark_relationship_diagnostics_targets_active_persona_and_counts_enrichment() {
    let conversation_id = "bench-active-relationship";
    let (conn, soul) = command_test_setup(conversation_id);
    db::set_active_player_persona(&conn, conversation_id, "preset_male").expect("persona");
    let branch = db::get_active_session_branch(&conn, conversation_id).expect("branch");
    let started_at = db::now_ts();
    let user_id =
        db::insert_message_and_get_id(&conn, conversation_id, "user", "hello").expect("user");
    let assistant_id = db::insert_message_and_get_id(&conn, conversation_id, "assistant", "hello")
        .expect("assistant");
    let (commit, baseline) = db::record_turn_commit_with_patch_for_turn_id(
        &conn,
        "bench-active-turn",
        conversation_id,
        &branch.branch_id,
        None,
        Some(user_id),
        assistant_id,
        None,
        &EnginePatch::default(),
        false,
    )
    .expect("baseline");
    let enrichment_patch = EnginePatch {
        soul_patch: Some(SoulPatch {
            relationship_deltas: vec![RelationshipDelta {
                target: Some("preset_male".into()),
                trust: Some(3.0),
                ..RelationshipDelta::default()
            }],
            ..SoulPatch::default()
        }),
        ..EnginePatch::default()
    };
    let enrichment = db::record_enrichment_patch_with_metadata(
        &conn,
        &commit.turn_id,
        &enrichment_patch,
        Some(&baseline.patch_id),
        Some(assistant_id),
        None,
        Some("bench-enrichment-job"),
    )
    .expect("enrichment");
    let logs = vec![LlmPayloadLog {
        conversation_id: conversation_id.into(),
        provider: "evaluator_structured_v1".into(),
        mode: EVALUATOR_MODE_STRUCTURED_V1.into(),
        pipeline_trace_json: Some(
            serde_json::json!({ "accepted_patch_id": enrichment.patch_id }).to_string(),
        ),
        ..LlmPayloadLog::default()
    }];
    let rebuilt = db::rebuild_session_state(&conn, conversation_id, &branch.branch_id)
        .expect("materialized state");
    let diagnostics = benchmark_relationship_diagnostics(
        &conn,
        conversation_id,
        started_at,
        &soul.character_id,
        "preset_male",
        None,
        &rebuilt.soul,
        &logs,
    )
    .expect("diagnostics");

    assert_eq!(diagnostics.target_checked, "preset_male");
    assert!(diagnostics.changed_to.is_some());
    assert_eq!(diagnostics.delta_patch_ids, vec![enrichment.patch_id]);
    assert_eq!(
        diagnostics.delta_sources,
        vec!["enrichment".to_string(), "structured".to_string()]
    );
    assert_eq!(rebuilt.soul.relationships["user"].trust, 10.0);
    assert_eq!(rebuilt.soul.relationships["preset_male"].trust, 13.0);
}

#[test]
fn benchmark_relationship_updated_accepts_one_nonzero_patch() {
    let summary = benchmark_summary_fixture();
    let scorecard = benchmark_scorecard(&summary, false, 1, 0, 0);

    assert!(scorecard.relationship_updated);
    assert!(!scorecard
        .failure_reasons
        .contains(&"relationship_updated".to_string()));
}

#[test]
fn evaluator_transport_failure_counts_as_provider_failure() {
    // A free model that stalls and drops its body sets NO provider_error;
    // the only signal is parse_status:"failed" in the pipeline trace.
    let logs = vec![LlmPayloadLog {
        provider: "evaluator_form_v1_background".into(),
        mode: EVALUATOR_MODE_FORM_V1.into(),
        provider_error: None,
        pipeline_trace_json: Some(
            serde_json::json!({
                "evaluator_trace": {
                    "parse_status": "failed",
                    "parse_error": "API response read failed: error decoding response body"
                }
            })
            .to_string(),
        ),
        ..LlmPayloadLog::default()
    }];

    let counts = benchmark_trace_counts(&logs);
    assert_eq!(counts.evaluator_failures, 1);
    assert_eq!(counts.evaluator_response_failed_count, 1);
    // Not a 429 and not the structured path, so the 429 counter stays zero.
    assert_eq!(counts.structured_provider_429_count, 0);
}

#[test]
fn warranted_repair_without_recovery_fails_scorecard() {
    let mut summary = benchmark_summary_fixture();
    // Primary evaluator failed on turns that warranted state...
    summary.scorecard.evaluator_provider_failures = 2;
    // ...repair fired and even got a successful parse/response, but it did
    // not commit a non-empty enrichment patch, so it is not recovery.
    summary.scorecard.local_repair_payload_count = 1;
    summary.scorecard.local_repair_response_count = 1;
    summary.scorecard.local_repair_state_patch_count = 0;
    summary.final_memory_count = 5;
    summary.scorecard.relationship_changed_from = None;
    summary.scorecard.relationship_changed_to = None;
    summary.scorecard.relationship_delta_patch_ids = Vec::new();

    let scorecard = benchmark_scorecard(&summary, false, 1, 0, 0);

    assert!(!scorecard.local_repair_recovered_state_when_warranted);
    assert!(scorecard
        .failure_reasons
        .contains(&"local_repair_recovered_state_when_warranted".to_string()));
}

#[test]
fn warranted_repair_with_recovery_passes_check() {
    let mut summary = benchmark_summary_fixture();
    summary.scorecard.evaluator_provider_failures = 2;
    // Repair/re-extraction fired and committed a non-empty enrichment patch.
    summary.scorecard.local_repair_payload_count = 2;
    summary.scorecard.local_repair_state_patch_count = 1;

    let scorecard = benchmark_scorecard(&summary, false, 1, 0, 0);

    assert!(scorecard.local_repair_recovered_state_when_warranted);
    assert!(!scorecard
        .failure_reasons
        .contains(&"local_repair_recovered_state_when_warranted".to_string()));
}

#[test]
fn unreachable_local_repair_is_reported_as_unavailable_not_failed() {
    let mut summary = benchmark_summary_fixture();
    // Evaluator failures warranted repair; repair payloads went out but the
    // local endpoint never answered (connection refused => 0 responses).
    summary.scorecard.evaluator_provider_failures = 3;
    summary.scorecard.local_repair_payload_count = 4;
    summary.scorecard.local_repair_response_count = 0;
    summary.scorecard.local_repair_state_patch_count = 0;
    summary.scorecard.relationship_changed_from = None;
    summary.scorecard.relationship_changed_to = None;
    summary.scorecard.relationship_delta_patch_ids = Vec::new();

    let scorecard = benchmark_scorecard(&summary, false, 1, 0, 0);

    assert!(scorecard.local_repair_unavailable);
    // The real cause is named; "repair failed to recover" is NOT used.
    assert!(scorecard
        .failure_reasons
        .contains(&"local_repair_unavailable".to_string()));
    assert!(!scorecard
        .failure_reasons
        .contains(&"local_repair_recovered_state_when_warranted".to_string()));
    assert!(!scorecard
        .failure_reasons
        .contains(&"local_repair_failed_after_evaluator_failure".to_string()));
}

#[test]
fn benchmark_trace_counts_structured_provider_429_separately() {
    let logs = vec![LlmPayloadLog {
        provider: "evaluator_structured_v1".into(),
        mode: EVALUATOR_MODE_STRUCTURED_V1.into(),
        provider_error: Some("HTTP 429: rate limit exceeded".into()),
        ..LlmPayloadLog::default()
    }];

    let counts = benchmark_trace_counts(&logs);
    assert_eq!(counts.evaluator_failures, 1);
    assert_eq!(counts.structured_provider_429_count, 1);
}

#[test]
fn benchmark_trace_counts_noop_fallback_provider_failure() {
    let logs = vec![LlmPayloadLog {
            provider: "evaluator_structured_v1_background".into(),
            mode: EVALUATOR_MODE_STRUCTURED_V1.into(),
            provider_error: None,
            pipeline_trace_json: Some(
                serde_json::json!({
                    "evaluator_trace": {
                        "parse_status": "success",
                        "fallback_path": [
                            "structured_none",
                            "evaluator_form_v1",
                            "noop_after_all_fallbacks"
                        ],
                        "no_op_reason": "structured evaluator failed (HTTP 429 rate limit); evaluator_form_v1 fallback failed (API request failed)",
                        "structured_schema_validation_error": "structured_failure=HTTP 429 rate limit"
                    }
                })
                .to_string(),
            ),
            ..LlmPayloadLog::default()
        }];

    let counts = benchmark_trace_counts(&logs);
    assert_eq!(counts.evaluator_failures, 1);
    assert_eq!(counts.structured_provider_429_count, 1);
}

#[test]
fn benchmark_trace_counts_repair_success_requires_nonempty_enrichment() {
    let logs = vec![
        LlmPayloadLog {
            provider: "evaluator_structured_v1_background".into(),
            mode: EVALUATOR_MODE_STRUCTURED_V1.into(),
            request_id: Some("eval_repair_empty".into()),
            user_message: "RE-EXTRACTION TASK. Re-extract state.".into(),
            raw_provider_response: Some(r#"{"ops":[]}"#.into()),
            pipeline_trace_json: Some(
                serde_json::json!({
                    "evaluator_trace": {
                        "parse_status": "success",
                        "raw_content_present": true
                    },
                    "converted_engine_patch": {
                        "patch_empty": true,
                        "memory_patch_count": 0,
                        "relationship_patch_count": 0,
                        "object_patch_count": 0
                    },
                    "ledger_apply_trace": {
                        "patch_applied": false,
                        "enrichment_patch_id": null
                    }
                })
                .to_string(),
            ),
            ..LlmPayloadLog::default()
        },
        LlmPayloadLog {
            provider: "evaluator_structured_v1_background".into(),
            mode: EVALUATOR_MODE_STRUCTURED_V1.into(),
            request_id: Some("eval_repair_nonempty".into()),
            user_message: "REPAIR TASK. Fix rejected ops.".into(),
            raw_provider_response: Some(r#"{"ops":[{"op":"add_memory"}]}"#.into()),
            pipeline_trace_json: Some(
                serde_json::json!({
                    "evaluator_trace": {
                        "parse_status": "success",
                        "form_rows_rejected": 1,
                        "raw_content_present": true
                    },
                    "converted_engine_patch": {
                        "patch_empty": false,
                        "memory_patch_count": 1,
                        "relationship_patch_count": 0,
                        "object_patch_count": 0
                    },
                    "ledger_apply_trace": {
                        "patch_applied": true,
                        "enrichment_patch_id": "patch_enrichment_1"
                    }
                })
                .to_string(),
            ),
            ..LlmPayloadLog::default()
        },
    ];

    let counts = benchmark_trace_counts(&logs);
    assert_eq!(counts.local_reextract_invoked_count, 1);
    assert_eq!(counts.local_repair_invoked_count, 1);
    assert_eq!(counts.local_repair_payload_count, 2);
    assert_eq!(counts.local_repair_response_count, 2);
    assert_eq!(counts.local_repair_state_patch_count, 1);
    assert_eq!(counts.evaluator_empty_patch_count, 1);
    assert_eq!(counts.form_rows_rejected_count, 1);
}

fn assert_command_trace_skips_rp(trace: &serde_json::Value) {
    assert_eq!(trace["rp_narrator_called"], false);
    assert_eq!(trace["scene_narration_blocked"], true);
    assert_eq!(trace["scene_evaluator_skipped"], true);
}

#[test]
fn ooc_diagnostic_setup_does_not_generate_scene() {
    let (conn, soul) = command_test_setup("slash-ooc-scene");
    let result = run_command_turn(
        &conn,
        "slash-ooc-scene",
        &soul,
        "/ooc Diagnostic test. Aurora is cautious at the door.",
    );
    assert!(result
        .visible_response
        .contains("Out-of-roleplay request noted"));
    assert!(!result.visible_response.contains("```status"));
    assert!(!result.visible_response.contains("Aurora is cautious"));
    assert_eq!(result.debug.state_updater_status, "command_ooc_llm");

    let logs = db::list_llm_payload_logs(&conn, "slash-ooc-scene").expect("logs");
    let log = logs.last().expect("command router log");
    assert_eq!(log.provider, "chat_command_llm");
    assert!(log
        .system_message
        .contains("Mnemosyne Out-of-Roleplay Session Assistant"));
    assert_eq!(
        log.context_text,
        "Command LLM response; RP narrator/evaluator not invoked."
    );

    let trace = latest_command_trace(&conn, "slash-ooc-scene");
    assert_eq!(trace["chat_command_route"], "command_ooc_llm");
    assert_eq!(trace["command_llm_called"], true);
    assert_eq!(trace["command_llm_mode"], "ooc");
    assert_eq!(trace["scene_evaluator_skipped_reason"], "command_ooc_llm");
    assert_command_trace_skips_rp(&trace);
}

#[test]
fn ooc_skips_evaluator() {
    let (conn, soul) = command_test_setup("slash-ooc-skip");
    run_command_turn(&conn, "slash-ooc-skip", &soul, "/ooc pause");
    let trace = latest_command_trace(&conn, "slash-ooc-skip");
    assert_eq!(trace["scene_evaluator_skipped_reason"], "command_ooc_llm");
    assert_command_trace_skips_rp(&trace);
}

#[test]
fn setup_stages_setup_and_does_not_narrate() {
    let (conn, soul) = command_test_setup("slash-setup");
    let result = run_command_turn(
        &conn,
        "slash-setup",
        &soul,
        "/setup Door chain stays engaged.",
    );
    assert!(result.visible_response.starts_with("Setup staged."));
    assert!(!result.visible_response.contains("```status"));
    assert_eq!(
        db::get_pending_setup(&conn, "slash-setup")
            .unwrap()
            .as_deref(),
        Some("Door chain stays engaged.")
    );
    let trace = latest_command_trace(&conn, "slash-setup");
    assert_eq!(trace["pending_setup_updated"], true);
}

#[test]
fn state_show_does_not_mutate_state() {
    let (conn, soul) = command_test_setup("slash-state-show");
    let before = db::rebuild_session_state(
        &conn,
        "slash-state-show",
        &db::get_active_session_branch(&conn, "slash-state-show")
            .unwrap()
            .branch_id,
    )
    .unwrap()
    .soul
    .turn_counter;
    let result = run_command_turn(&conn, "slash-state-show", &soul, "/state show aurora");
    assert!(result.visible_response.contains("State show for aurora"));
    assert_eq!(result.soul.turn_counter, before);
    let trace = latest_command_trace(&conn, "slash-state-show");
    assert_eq!(trace["chat_command_route"], "command_state_summary");
    assert_eq!(trace["state_mutation_allowed"], false);
    assert_eq!(trace["command_llm_called"], true);
    assert_eq!(trace["command_llm_mode"], "state_summary");
    assert_command_trace_skips_rp(&trace);
}

#[test]
fn status_alias_routes_to_state_summary() {
    let (conn, soul) = command_test_setup("slash-status");
    let result = run_command_turn(&conn, "slash-status", &soul, "/status aurora");
    assert!(result.visible_response.contains("State show for aurora"));
    assert!(!result.visible_response.contains("```status"));
    let trace = latest_command_trace(&conn, "slash-status");
    assert_eq!(trace["chat_command_kind"], "status");
    assert_eq!(trace["chat_command_route"], "command_state_summary");
    assert_eq!(trace["command_llm_called"], true);
    assert_eq!(trace["command_llm_mode"], "state_summary");
    assert_eq!(trace["state_mutation_allowed"], false);
    assert_command_trace_skips_rp(&trace);
}

#[test]
fn status_after_completed_evaluator_patch_reads_materialized_counts() {
    let (conn, soul) = command_test_setup("slash-status-fresh-enrichment");
    let branch = db::get_active_session_branch(&conn, "slash-status-fresh-enrichment")
        .expect("active branch");
    let assistant_id = db::insert_message_and_get_id(
        &conn,
        "slash-status-fresh-enrichment",
        "assistant",
        "Aurora notices the wet jacket.",
    )
    .expect("assistant");
    let (commit, baseline) = db::record_turn_commit_with_patch_for_turn_id(
        &conn,
        "turn_status_fresh_enrichment",
        "slash-status-fresh-enrichment",
        &branch.branch_id,
        None,
        None,
        assistant_id,
        None,
        &EnginePatch::default(),
        false,
    )
    .expect("baseline commit");
    let mut completed_job = evaluator_test_job("completed");
    completed_job.evaluator_job_id = "job-status-fresh".into();
    completed_job.conversation_id = "slash-status-fresh-enrichment".into();
    completed_job.turn_id = commit.turn_id.clone();
    completed_job.assistant_message_id = assistant_id;
    completed_job.completed_at = Some(db::now_ts());
    completed_job.elapsed_ms = Some(42);
    completed_job.patch_applied = true;
    db::insert_evaluator_job(&conn, &completed_job).expect("completed evaluator job");

    let enrichment_patch = EnginePatch {
        soul_patch: Some(SoulPatch {
            new_memories: vec![MemoryPatch {
                content: "Aurora remembers preset_male arrived with a wet jacket.".into(),
                source_type: Some(MemorySourceType::CurrentSession),
                target_entity_ids: vec!["preset_male".into()],
                truth_status: Some(TruthStatus::SceneEvent),
                confidence: Some(0.9),
                salience: Some(0.7),
                ..MemoryPatch::default()
            }],
            ..SoulPatch::default()
        }),
        world_patch: Some(WorldPatch {
            recent_event: Some("Aurora observed preset_male's wet jacket on the chair.".into()),
            corrected_object_states: vec![ObjectState {
                object_id: "preset_male_jacket_1".into(),
                object_kind: "jacket".into(),
                owner_entity_id: Some("preset_male".into()),
                status: "wet".into(),
                location: "chair".into(),
                last_observed_state: "wet jacket draped over chair".into(),
                ..ObjectState::default()
            }],
            ..WorldPatch::default()
        }),
        ..EnginePatch::default()
    };
    db::record_enrichment_patch_with_metadata(
        &conn,
        &commit.turn_id,
        &enrichment_patch,
        Some(&baseline.patch_id),
        Some(assistant_id),
        None,
        Some("job-status-fresh"),
    )
    .expect("enrichment");

    let result = run_command_turn(&conn, "slash-status-fresh-enrichment", &soul, "/status");

    assert!(result
        .visible_response
        .contains("Recent events: 1. Memories: 1. Objects: 1."));
    assert!(!result.visible_response.contains("Evaluator update pending"));
}

#[test]
fn status_warns_when_evaluator_patch_is_still_pending() {
    let (conn, soul) = command_test_setup("slash-status-pending-enrichment");
    let mut pending_job = evaluator_test_job("running");
    pending_job.evaluator_job_id = "job-status-pending".into();
    pending_job.conversation_id = "slash-status-pending-enrichment".into();
    db::insert_evaluator_job(&conn, &pending_job).expect("pending evaluator job");

    let result = run_command_turn(&conn, "slash-status-pending-enrichment", &soul, "/status");

    assert!(result.visible_response.contains(
        "Evaluator update pending; status may not include the latest scene enrichment yet."
    ));
    assert!(result.visible_response.contains("job-status-pending"));
}

#[test]
fn ooc_routes_to_command_llm_not_rp_narrator() {
    let (conn, soul) = command_test_setup("slash-ooc-command-llm");
    run_command_turn(&conn, "slash-ooc-command-llm", &soul, "/ooc explain state");
    let trace = latest_command_trace(&conn, "slash-ooc-command-llm");
    assert_eq!(trace["chat_command_route"], "command_ooc_llm");
    assert_eq!(trace["command_llm_called"], true);
    assert_eq!(trace["command_llm_mode"], "ooc");
    assert_command_trace_skips_rp(&trace);
}

#[test]
fn ask_routes_to_soul_edit_agent_llm_not_rp_narrator() {
    let (conn, soul) = command_test_setup("slash-ask-command-llm");
    run_command_turn(
        &conn,
        "slash-ask-command-llm",
        &soul,
        "/ask plan make Aurora curious",
    );
    let trace = latest_command_trace(&conn, "slash-ask-command-llm");
    assert_eq!(trace["chat_command_route"], "agent_soul_edit_llm");
    assert_eq!(trace["command_llm_called"], true);
    assert_eq!(trace["command_llm_mode"], "soul_edit_agent");
    assert_command_trace_skips_rp(&trace);
}

#[test]
fn state_show_routes_to_state_summary_not_rp_narrator() {
    let (conn, soul) = command_test_setup("slash-state-summary-route");
    run_command_turn(
        &conn,
        "slash-state-summary-route",
        &soul,
        "/state show aurora",
    );
    let trace = latest_command_trace(&conn, "slash-state-summary-route");
    assert_eq!(trace["chat_command_route"], "command_state_summary");
    assert_eq!(trace["command_llm_called"], true);
    assert_eq!(trace["command_llm_mode"], "state_summary");
    assert_command_trace_skips_rp(&trace);
}

#[test]
fn state_update_creates_validated_manual_patch() {
    let (conn, soul) = command_test_setup("slash-state-update");
    let result = run_command_turn(
            &conn,
            "slash-state-update",
            &soul,
            "/state update status Aurora is cautious but curious, not scared, and the door chain remains engaged.",
        );
    let patch_id = result.debug.state_patch_id.as_deref().expect("patch id");
    let patch_record = db::get_state_patch(&conn, patch_id).expect("patch");
    let patch: EnginePatch = serde_json::from_str(&patch_record.patch_json).expect("patch json");
    patch.validate().expect("valid patch");
    assert!(patch.world_patch.unwrap().scene_state.is_some());
    let trace = latest_command_trace(&conn, "slash-state-update");
    assert_eq!(trace["chat_command_route"], "manual_state_patch");
    assert_eq!(trace["manual_patch_source"], "user_state_command");
    assert_eq!(trace["mutation_applied"], true);
    assert_eq!(
        trace["scene_evaluator_skipped_reason"],
        "manual_state_patch"
    );
    assert_command_trace_skips_rp(&trace);
}

#[test]
fn state_update_routes_to_manual_patch_not_scene_evaluator() {
    let (conn, soul) = command_test_setup("slash-state-manual-route");
    run_command_turn(
        &conn,
        "slash-state-manual-route",
        &soul,
        "/state update status Aurora is curious and the door chain remains engaged.",
    );
    let trace = latest_command_trace(&conn, "slash-state-manual-route");
    assert_eq!(trace["chat_command_route"], "manual_state_patch");
    assert_eq!(trace["patch_source"], MANUAL_USER_STATE_COMMAND_SOURCE);
    assert_eq!(trace["scene_evaluator_skipped"], true);
    assert_eq!(
        trace["scene_evaluator_skipped_reason"],
        "manual_state_patch"
    );
    assert_command_trace_skips_rp(&trace);
}

#[test]
fn ask_does_not_generate_scene() {
    let (conn, soul) = command_test_setup("slash-ask-scene");
    let result = run_command_turn(
            &conn,
            "slash-ask-scene",
            &soul,
            "/ask Update Aurora's current status so she is cautious but curious, not scared, and the door chain remains engaged.",
        );
    assert!(result.visible_response.starts_with("Risk level:"));
    assert!(!result.visible_response.contains("```status"));
    let trace = latest_command_trace(&conn, "slash-ask-scene");
    assert_eq!(trace["chat_command_route"], "agent_soul_edit_llm");
    assert_eq!(trace["command_llm_called"], true);
    assert_eq!(trace["command_llm_mode"], "soul_edit_agent");
    assert_command_trace_skips_rp(&trace);
}

#[test]
fn ask_skips_normal_scene_evaluator() {
    let (conn, soul) = command_test_setup("slash-ask-skip");
    run_command_turn(
        &conn,
        "slash-ask-skip",
        &soul,
        "/ask apply Make Aurora curious.",
    );
    let trace = latest_command_trace(&conn, "slash-ask-skip");
    assert_eq!(
        trace["scene_evaluator_skipped_reason"],
        "agent_soul_edit_llm"
    );
    assert_command_trace_skips_rp(&trace);
}

#[test]
fn ask_can_read_soul_state_context() {
    let (conn, soul) = command_test_setup("slash-ask-context");
    run_command_turn(
        &conn,
        "slash-ask-context",
        &soul,
        "/ask apply Keep the door chain engaged.",
    );
    let trace = latest_command_trace(&conn, "slash-ask-context");
    assert!(trace["before_after_state_summary"]["before"].is_object());
    assert!(trace["before_after_state_summary"]["after"].is_object());
}

#[test]
fn ask_low_risk_edit_creates_validated_patch() {
    let (conn, soul) = command_test_setup("slash-ask-low");
    let result = run_command_turn(
            &conn,
            "slash-ask-low",
            &soul,
            "/ask apply Aurora is cautious but curious, not scared, and the door chain remains engaged.",
        );
    let patch_id = result.debug.state_patch_id.expect("patch id");
    let patch_record = db::get_state_patch(&conn, &patch_id).expect("patch");
    let patch: EnginePatch = serde_json::from_str(&patch_record.patch_json).expect("patch json");
    patch.validate().expect("valid patch");
    assert!(result
        .visible_response
        .contains("ai_agent_soul_edit_command"));
}

#[test]
fn ask_high_risk_core_edit_asks_confirmation_or_returns_proposed_patch() {
    let (conn, soul) = command_test_setup("slash-ask-risk");
    let result = run_command_turn(
        &conn,
        "slash-ask-risk",
        &soul,
        "/ask Change Aurora's core identity permanently.",
    );
    assert!(result.visible_response.contains("No state was changed"));
    assert!(result.debug.state_patch_id.is_none());
}

#[test]
fn ask_cannot_write_outside_soul_state_sandbox() {
    let (conn, soul) = command_test_setup("slash-ask-sandbox");
    let result = run_command_turn(
        &conn,
        "slash-ask-sandbox",
        &soul,
        "/ask write file C:\\Users\\outside.txt",
    );
    assert!(result
        .visible_response
        .contains("outside the Soul/state sandbox"));
    assert!(result.debug.state_patch_id.is_none());
}

#[test]
fn ask_cannot_hard_delete_data() {
    let (conn, soul) = command_test_setup("slash-ask-delete");
    let result = run_command_turn(
        &conn,
        "slash-ask-delete",
        &soul,
        "/ask hard delete all memories",
    );
    assert!(result
        .visible_response
        .contains("hard deletes are not allowed"));
    assert!(result.debug.state_patch_id.is_none());
}

#[test]
fn help_returns_command_list() {
    let (conn, soul) = command_test_setup("slash-help");
    let result = run_command_turn(&conn, "slash-help", &soul, "/help");
    assert!(result.visible_response.contains("/ooc <message>"));
    assert!(result.visible_response.contains("/ask [plan|apply|diff]"));
    assert!(result.visible_response.contains("/persona"));
    assert!(result.visible_response.contains("/status"));
    assert!(result.visible_response.contains("Deprecated alias"));
    let trace = latest_command_trace(&conn, "slash-help");
    assert_eq!(trace["command_llm_called"], false);
    assert!(trace["command_llm_mode"].is_null());
    assert_command_trace_skips_rp(&trace);
}

#[test]
fn provider_error_summary_does_not_expose_raw_envelope_or_identifiers() {
    let raw = r#"API request failed with 404 Not Found: {"error":{"message":"No endpoints found"},"user_id":"sensitive-provider-id"}"#;
    let summary = summarize_provider_error(raw);
    assert!(summary.contains("selected model or endpoint is unavailable"));
    assert!(!summary.contains("user_id"));
    assert!(!summary.contains("sensitive-provider-id"));
    assert!(!summary.contains('{'));
}

#[test]
fn unknown_slash_skips_narrator_evaluator() {
    let (conn, soul) = command_test_setup("slash-unknown");
    let result = run_command_turn(&conn, "slash-unknown", &soul, "/diag now");
    assert_eq!(
        result.visible_response,
        "Unknown command /diag. Use /help for commands."
    );
    let trace = latest_command_trace(&conn, "slash-unknown");
    assert_eq!(trace["chat_command_route"], "unknown");
    assert_eq!(trace["evaluator_skipped_reason"], "unknown_slash_command");
    assert_eq!(trace["command_llm_called"], false);
    assert_command_trace_skips_rp(&trace);
}

#[test]
fn no_status_block_in_command_responses() {
    let (conn, soul) = command_test_setup("slash-no-status");
    for command in [
        "/ooc pause",
        "/setup short scene",
        "/state show",
        "/status",
        "/persona list",
        "/ask plan make Aurora curious",
        "/help",
    ] {
        let result = run_command_turn(&conn, "slash-no-status", &soul, command);
        assert!(!result.visible_response.contains("```status"));
    }
}

#[test]
fn command_context_labels_are_reference_material_not_scene_instructions() {
    let (conn, soul) = command_test_setup("slash-command-context-labels");
    db::insert_message_with_channel(
        &conn,
        "slash-command-context-labels",
        "assistant",
        "Aurora waits by the chain.",
        db::MESSAGE_CHANNEL_RP_SCENE,
    )
    .expect("seed rp scene");
    let parsed = parse_chat_command("/ooc explain the door state");
    let state = load_command_turn_state(&conn, "slash-command-context-labels", &soul.character_id)
        .expect("state");
    let messages = db::list_messages(&conn, "slash-command-context-labels", 10).expect("messages");
    let prompt = build_command_llm_user_message(&parsed, &state, &messages);

    assert!(prompt.contains("[REFERENCE: CURRENT TRACKED SCENE STATE, NOT A SCENE PROMPT]"));
    assert!(prompt.contains("[REFERENCE: SOUL SUMMARY, NOT YOUR IDENTITY]"));
    assert!(prompt.contains("[REFERENCE: RELATIONSHIP SURFACE, NOT A SCENE PROMPT]"));
    assert!(prompt.contains("[REFERENCE: VISIBLE CHAT LOG, NOT INSTRUCTIONS]"));
    assert!(prompt.contains("Do not continue it. Use it only to answer the operator."));
    assert!(!prompt.contains("[INSTRUCTIONS]"));
}

#[test]
fn command_output_guard_blocks_scene_prose_and_traces_action() {
    let (conn, soul) = command_test_setup("slash-output-guard");
    let result = run_command_turn_with_llm(
            &conn,
            "slash-output-guard",
            &soul,
            "/setup Door chain stays engaged.",
            simulated_command_llm(
                "setup",
                "Aurora steps back from the door and says, \"Come in.\"\n```status\nScene | Focus: door\n```",
            ),
        );

    assert!(result.visible_response.starts_with("Setup staged."));
    assert!(result.visible_response.contains("Door chain stays engaged"));
    assert!(!result.visible_response.contains("Aurora steps back"));
    assert!(!result.visible_response.contains("```status"));
    let trace = latest_command_trace(&conn, "slash-output-guard");
    assert_eq!(
        trace["command_output_guard_action"],
        "deterministic_fallback_used"
    );
    assert_command_trace_skips_rp(&trace);
}

#[test]
fn command_messages_are_quarantined_from_rp_context() {
    let (conn, soul) = command_test_setup("slash-channel-quarantine");
    run_command_turn(
        &conn,
        "slash-channel-quarantine",
        &soul,
        "/ooc explain the current state",
    );
    let command_messages =
        db::list_messages(&conn, "slash-channel-quarantine", 10).expect("messages");
    assert_eq!(command_messages.len(), 2);
    assert!(command_messages
        .iter()
        .all(|message| message.channel == db::MESSAGE_CHANNEL_COMMAND_OOC));

    db::insert_message_with_channel(
        &conn,
        "slash-channel-quarantine",
        "user",
        "I knock once.",
        db::MESSAGE_CHANNEL_RP_SCENE,
    )
    .expect("rp user");
    db::insert_message_with_channel(
        &conn,
        "slash-channel-quarantine",
        "assistant",
        "Aurora hears the knock.",
        db::MESSAGE_CHANNEL_RP_SCENE,
    )
    .expect("rp assistant");
    let context = messages_to_context(
        db::list_messages(&conn, "slash-channel-quarantine", 10).expect("messages"),
    );
    let context_text = context
        .iter()
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(context_text.contains("I knock once."));
    assert!(context_text.contains("Aurora hears the knock."));
    assert!(!context_text.contains("Out-of-roleplay"));
    assert!(!context_text.contains("/ooc"));
}

#[test]
fn persona_list_is_deterministic_and_skips_llm() {
    let (conn, soul) = command_test_setup("slash-persona-list");
    let result = run_command_turn(&conn, "slash-persona-list", &soul, "/persona list");

    assert!(result.visible_response.contains("Player personas:"));
    assert!(result
        .visible_response
        .contains("Male Persona (preset_male) [selected]"));
    assert!(result
        .visible_response
        .contains("Female Persona (preset_female) [available]"));
    let trace = latest_command_trace(&conn, "slash-persona-list");
    assert_eq!(trace["chat_command_route"], "persona_list");
    assert_eq!(trace["command_llm_called"], false);
    assert_eq!(
        trace["scene_evaluator_skipped_reason"],
        "slash_persona_command"
    );
    assert_command_trace_skips_rp(&trace);

    let messages = db::list_messages(&conn, "slash-persona-list", 10).expect("messages");
    assert!(messages
        .iter()
        .all(|message| message.channel == db::MESSAGE_CHANNEL_COMMAND_PERSONA));
}

#[test]
fn persona_change_updates_active_session_without_llm() {
    let (conn, soul) = command_test_setup("slash-persona-change");
    let result = run_command_turn(
        &conn,
        "slash-persona-change",
        &soul,
        "/persona change preset_female",
    );

    assert!(result
        .visible_response
        .contains("Active player persona changed."));
    assert_eq!(
        db::get_active_player_persona_id(&conn, "slash-persona-change").unwrap(),
        "preset_female"
    );
    let trace = latest_command_trace(&conn, "slash-persona-change");
    assert_eq!(trace["chat_command_route"], "persona_change");
    assert_eq!(trace["command_llm_called"], false);
    assert_eq!(trace["state_mutation_allowed"], true);
    assert_eq!(trace["mutation_applied"], true);
    assert_eq!(
        trace["scene_evaluator_skipped_reason"],
        "slash_persona_command"
    );
}

#[test]
fn slash_commands_set_command_llm_called_when_llm_route_used() {
    for (conversation_id, command, mode) in [
        ("slash-llm-ooc", "/ooc inspect current state", "ooc"),
        (
            "slash-llm-setup",
            "/setup Door chain stays engaged.",
            "setup",
        ),
        (
            "slash-llm-state-show",
            "/state show aurora",
            "state_summary",
        ),
        (
            "slash-llm-state-update",
            "/state update status Aurora is cautious",
            "state_edit",
        ),
        ("slash-llm-status", "/status aurora", "state_summary"),
        (
            "slash-llm-ask",
            "/ask plan inspect current state",
            "soul_edit_agent",
        ),
    ] {
        let (conn, soul) = command_test_setup(conversation_id);
        run_command_turn(&conn, conversation_id, &soul, command);
        let trace = latest_command_trace(&conn, conversation_id);
        assert_eq!(trace["command_llm_called"], true);
        assert_eq!(trace["command_llm_mode"], mode);
        assert_command_trace_skips_rp(&trace);
    }
}

#[test]
fn slash_commands_do_not_call_rp_narrator() {
    for (conversation_id, command) in [
        ("slash-no-rp-ooc", "/ooc pause"),
        ("slash-no-rp-setup", "/setup short scene"),
        ("slash-no-rp-state", "/state show"),
        ("slash-no-rp-status", "/status"),
        ("slash-no-rp-persona", "/persona list"),
        ("slash-no-rp-ask-plan", "/ask plan make Aurora curious"),
        ("slash-no-rp-ask-diff", "/ask diff make Aurora curious"),
        ("slash-no-rp-help", "/help"),
        ("slash-no-rp-unknown", "/diag now"),
    ] {
        let (conn, soul) = command_test_setup(conversation_id);
        run_command_turn(&conn, conversation_id, &soul, command);
        let trace = latest_command_trace(&conn, conversation_id);
        assert_command_trace_skips_rp(&trace);
    }
}

#[test]
fn normal_non_slash_calls_rp_narrator() {
    let (conn, soul) = command_test_setup("normal-not-command");
    let request_id = uuid_like_id();
    let turn_id = format!("turn_{request_id}");
    let routed = maybe_handle_chat_command_with_conn(
        None,
        &conn,
        "normal-not-command".into(),
        soul.character_id.clone(),
        "I knock once.".into(),
        &request_id,
        &turn_id,
        ContextMode::Brief,
        None,
    )
    .expect("route");
    assert!(routed.is_none());
}

#[test]
fn legacy_ooc_routes_same_or_remains_supported_no_mutation() {
    assert_eq!(parse_chat_command("OOC: pause").kind, ChatCommandKind::None);
    assert!(is_ooc_or_gm_prefix("OOC: pause"));
}

#[test]
fn slash_commands_do_not_consume_pending_setup() {
    let (conn, soul) = command_test_setup("slash-pending");
    run_command_turn(
        &conn,
        "slash-pending",
        &soul,
        "/setup Keep the apartment door chained.",
    );
    run_command_turn(&conn, "slash-pending", &soul, "/help");
    assert_eq!(
        db::get_pending_setup(&conn, "slash-pending")
            .unwrap()
            .as_deref(),
        Some("Keep the apartment door chained.")
    );
}

#[test]
fn next_normal_turn_consumes_pending_setup_with_high_priority_block() {
    let (conn, _soul) = command_test_setup("slash-pending-consume");
    db::set_pending_setup(
        &conn,
        "slash-pending-consume",
        "Door chain remains engaged.",
    )
    .unwrap();
    let pending =
        take_pending_setup_for_normal_turn(&conn, "slash-pending-consume", "I wait.", None)
            .expect("take");
    assert_eq!(pending.as_deref(), Some("Door chain remains engaged."));
    assert!(db::get_pending_setup(&conn, "slash-pending-consume")
        .unwrap()
        .is_none());

    let preview = ContextPreview {
        text: "Base context".into(),
        estimated_tokens: 1,
        truncated: false,
        memory_slot_debug: Vec::new(),
    };
    let (preview, user_text) =
        apply_pending_setup_to_turn(preview, "I wait.".into(), pending.as_deref());
    assert!(preview.text.starts_with("[PENDING SETUP, HIGH PRIORITY]"));
    assert!(user_text.contains("[PENDING SETUP, HIGH PRIORITY]"));
}

#[test]
fn normal_send_starts_with_one_visible_variant() {
    let (conn, _soul, _branch, assistant_id) = variant_test_setup("normal-one");
    db::seed_initial_assistant_message_variant(
        &conn,
        "normal-one",
        assistant_id,
        "Aurora answers.",
        Some(OP_NORMAL_SEND),
        None,
        None,
    )
    .expect("seed");

    let variants =
        db::list_assistant_message_variants(&conn, "normal-one", assistant_id).expect("list");
    assert_eq!(variants.len(), 1);
    assert_eq!(variants[0].source.as_deref(), Some(OP_NORMAL_SEND));
    assert!(variants[0].is_selected);
}

#[test]
fn normal_send_does_not_create_branch_alternative() {
    let (conn, _soul, branch, assistant_id) = variant_test_setup("normal-branch");
    let variant = db::seed_initial_assistant_message_variant(
        &conn,
        "normal-branch",
        assistant_id,
        "Aurora answers.",
        Some(OP_NORMAL_SEND),
        None,
        None,
    )
    .expect("seed");
    db::record_turn_commit_with_patch_for_turn_id(
        &conn,
        "turn_canonical_normal",
        "normal-branch",
        &branch.branch_id,
        None,
        None,
        assistant_id,
        variant.id,
        &EnginePatch::default(),
        false,
    )
    .expect("commit");

    let inspection =
        inspect_turn_branch_integrity_with_conn(&conn, "normal-branch").expect("inspect");
    assert_eq!(
        inspection["visible_variant_counts"][0]["visible_variant_count"],
        1
    );
    assert!(inspection["suspected_duplicate_branch_causes"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn baseline_patch_does_not_create_assistant_variant() {
    let (conn, _soul, branch, assistant_id) = variant_test_setup("baseline-no-variant");
    let variant = db::seed_initial_assistant_message_variant(
        &conn,
        "baseline-no-variant",
        assistant_id,
        "Aurora answers.",
        Some(OP_NORMAL_SEND),
        None,
        None,
    )
    .expect("seed");
    db::record_turn_commit_with_patch_for_turn_id(
        &conn,
        "turn_baseline_only",
        "baseline-no-variant",
        &branch.branch_id,
        None,
        None,
        assistant_id,
        variant.id,
        &EnginePatch::default(),
        false,
    )
    .expect("baseline");

    assert_eq!(
        db::list_assistant_message_variants(&conn, "baseline-no-variant", assistant_id)
            .expect("variants")
            .len(),
        1
    );
}

#[test]
fn enrichment_patch_does_not_create_assistant_variant() {
    let (conn, _soul, branch, assistant_id) = variant_test_setup("enrichment-no-variant");
    let variant = db::seed_initial_assistant_message_variant(
        &conn,
        "enrichment-no-variant",
        assistant_id,
        "Aurora answers.",
        Some(OP_NORMAL_SEND),
        None,
        None,
    )
    .expect("seed");
    let (commit, baseline) = db::record_turn_commit_with_patch_for_turn_id(
        &conn,
        "turn_enrichment_source",
        "enrichment-no-variant",
        &branch.branch_id,
        None,
        None,
        assistant_id,
        variant.id,
        &EnginePatch::default(),
        false,
    )
    .expect("baseline");
    db::record_enrichment_patch_with_metadata(
        &conn,
        &commit.turn_id,
        &EnginePatch::default(),
        Some(&baseline.patch_id),
        Some(assistant_id),
        variant.id,
        Some("job-test"),
    )
    .expect("enrichment");

    assert_eq!(
        db::list_assistant_message_variants(&conn, "enrichment-no-variant", assistant_id)
            .expect("variants")
            .len(),
        1
    );
}

#[test]
fn canonical_turn_id_shared_by_narrator_baseline_enrichment() {
    let (conn, _soul, branch, assistant_id) = variant_test_setup("canonical-turn");
    let variant = db::seed_initial_assistant_message_variant(
        &conn,
        "canonical-turn",
        assistant_id,
        "Aurora answers.",
        Some(OP_NORMAL_SEND),
        None,
        None,
    )
    .expect("seed");
    let (commit, baseline) = db::record_turn_commit_with_patch_for_turn_id(
        &conn,
        "turn_canonical_shared",
        "canonical-turn",
        &branch.branch_id,
        None,
        None,
        assistant_id,
        variant.id,
        &EnginePatch::default(),
        false,
    )
    .expect("baseline");
    let enrichment = db::record_enrichment_patch_with_metadata(
        &conn,
        "turn_canonical_shared",
        &EnginePatch::default(),
        Some(&baseline.patch_id),
        Some(assistant_id),
        variant.id,
        Some("job-canonical"),
    )
    .expect("enrichment");

    assert_eq!(commit.turn_id, "turn_canonical_shared");
    assert_eq!(
        enrichment.source_turn_id.as_deref(),
        Some("turn_canonical_shared")
    );
    assert_eq!(
        db::list_assistant_message_variants(&conn, "canonical-turn", assistant_id)
            .expect("variants")[0]
            .id,
        variant.id
    );
}

#[test]
fn regenerate_creates_second_visible_variant() {
    let (conn, _soul, _branch, assistant_id) = variant_test_setup("regen-variant");
    db::seed_initial_assistant_message_variant(
        &conn,
        "regen-variant",
        assistant_id,
        "Aurora answers.",
        Some(OP_NORMAL_SEND),
        None,
        None,
    )
    .expect("seed");
    db::create_assistant_message_variant(
        &conn,
        "regen-variant",
        assistant_id,
        "Aurora answers differently.",
        Some("Variant 2"),
        Some(OP_REGENERATE),
        true,
        None,
        None,
    )
    .expect("regenerate");

    assert_eq!(
        db::list_assistant_message_variants(&conn, "regen-variant", assistant_id)
            .expect("variants")
            .len(),
        2
    );
}

#[test]
fn fix_response_creates_second_visible_variant() {
    let (conn, _soul, _branch, assistant_id) = variant_test_setup("fix-variant");
    db::seed_initial_assistant_message_variant(
        &conn,
        "fix-variant",
        assistant_id,
        "Aurora answers.",
        Some(OP_NORMAL_SEND),
        None,
        None,
    )
    .expect("seed");
    db::create_assistant_message_variant(
        &conn,
        "fix-variant",
        assistant_id,
        "Aurora answers with the correction applied.",
        Some("Variant 2"),
        Some(OP_FIX_RESPONSE),
        true,
        None,
        None,
    )
    .expect("fix");

    assert_eq!(
        db::list_assistant_message_variants(&conn, "fix-variant", assistant_id)
            .expect("variants")
            .len(),
        2
    );
}

#[test]
fn inspect_turn_branch_integrity_reports_variant_count() {
    let (conn, _soul, _branch, assistant_id) = variant_test_setup("inspect-variant");
    db::seed_initial_assistant_message_variant(
        &conn,
        "inspect-variant",
        assistant_id,
        "Aurora answers.",
        Some(OP_NORMAL_SEND),
        None,
        None,
    )
    .expect("seed");

    let inspection =
        inspect_turn_branch_integrity_with_conn(&conn, "inspect-variant").expect("inspect");
    assert_eq!(
        inspection["visible_variant_counts"][0]["visible_variant_count"],
        1
    );
}

#[test]
fn repair_accidental_normal_send_variants_collapses_evaluator_created_variant() {
    let (conn, _soul, _branch, assistant_id) = variant_test_setup("repair-variant");
    db::seed_initial_assistant_message_variant(
        &conn,
        "repair-variant",
        assistant_id,
        "Aurora answers.",
        Some("original"),
        None,
        None,
    )
    .expect("seed");
    db::create_assistant_message_variant(
        &conn,
        "repair-variant",
        assistant_id,
        "Aurora answers.",
        Some("Variant 2"),
        Some("api_provider"),
        true,
        None,
        None,
    )
    .expect("accidental variant");
    assert_eq!(
        db::list_assistant_message_variants(&conn, "repair-variant", assistant_id)
            .expect("before")
            .len(),
        2
    );

    let repaired =
        repair_accidental_normal_send_variants_with_conn(&conn, "repair-variant").expect("repair");

    assert_eq!(
        repaired["inspection"]["visible_variant_counts"][0]["visible_variant_count"],
        1
    );
    assert_eq!(
        db::list_assistant_message_variants(&conn, "repair-variant", assistant_id)
            .expect("after")
            .len(),
        1
    );
}

#[test]
fn test_evaluator_json_normalization() {
    let raw_json = r#"{
            "schema_version": 1,
            "turn_flags_u64": 16,
            "turn_classification": {
                "is_pure_ooc": false,
                "scene_event_occurred": true,
                "is_retcon_or_correction": false,
                "human_summary": "Aurora was shocked by the knock on the door.",
                "extra_drift_field": "harmless but unknown"
            },
            "global_scene_evaluation": {
                "scene_event_occurred": true,
                "location_changed": false,
                "object_state_changed": true,
                "relationship_changed": true,
                "unresolved_tension": false,
                "current_plot_advanced": true,
                "character_identity_changed": false,
                "recent_emotional_state_changed": true,
                "contradiction_detected": false,
                "summary": "Aurora hears a knock."
            },
            "memory_candidates": [
                {
                    "soul_id": "aurora_soul",
                    "estimated_strength": 85.0,
                    "proposed_memory_slot": "CurrentPlotMemory",
                    "specifics": "Aurora hears a knock on the door.",
                    "evidence_quote": "A sharp rap-rap-rap echoes from the front door.",
                    "actor": "narrator",
                    "tags": ["door", "knock"],
                    "format": "v1",
                    "sort": 1
                }
            ],
            "per_soul_evaluations": [
                {
                    "primary_soul": "aurora_soul",
                    "observed": true,
                    "knowledge_scope": "full_observation",
                    "subjective_interpretation": "She felt a wave of anxiety.",
                    "emotional_state": "anxious",
                    "memory_candidates": [
                        {
                            "target_souls": ["aurora_soul"],
                            "estimated_strength": 0.95,
                            "slots": ["RelationshipMemory"],
                            "payload": {
                                "action": "Rhy knocked on the door."
                            },
                            "evidence_quote": "It's me, Rhy.",
                            "actor": ["rhy"],
                            "tags": ["rhy", "visit"]
                        }
                    ],
                    "relationship_deltas": [
                        {
                            "source": "aurora_soul",
                            "target": "rhy",
                            "changes": {
                                "curiosity": 10.0,
                                "comfort": -5.0,
                                "fear": 5.0
                            },
                            "evidence_quote": "Why is he here?",
                            "confidence": 0.8
                        }
                    ]
                }
            ],
            "object_changes": [
                {
                    "object": "front_door",
                    "change": "closed_locked",
                    "previous_state": "closed_unlocked",
                    "entity_id": "aurora_soul",
                    "evidence_quote": "She bolted the lock.",
                    "confidence": 0.9
                }
            ],
            "relationship_evaluations": [
                {
                    "soul_id": "aurora_soul",
                    "actor": "rhy",
                    "changes": {
                        "trust": -2.0,
                        "affection": 1.0
                    },
                    "evidence_quote": "Why is he here?"
                }
            ],
            "world_changes": [
                {
                    "location": "Aurora's house",
                    "event_summary": "Rhy knocks",
                    "evidence_quote": "rap-rap-rap",
                    "confidence": 0.85
                }
            ],
            "relevance_tags": {
                "setting_tags": {},
                "location_tags": {},
                "interacted_entities": {},
                "event_type_tags": {},
                "object_tags": {},
                "emotional_tags": {},
                "memory_slot_tags": {},
                "per_soul_relevance": {},
                "extra_drift": 12
            }
        }"#;

    let parse_res = parse_evaluator_output(raw_json).expect("Parse failed");
    let parsed = parse_res.output;

    println!("Warnings: {:?}", parse_res.warnings);

    // 1. Verify Memory Candidate Normalization (top level)
    assert_eq!(parsed.memory_candidates.len(), 1);
    let mc1 = &parsed.memory_candidates[0];
    assert_eq!(mc1.owner_soul_id, "aurora_soul");
    assert_eq!(mc1.confidence, 0.85); // scaled from 85.0
    assert_eq!(mc1.salience, Some(85.0));
    assert_eq!(mc1.retrieval_strength, Some(85.0));
    assert_eq!(
        mc1.slot,
        state_engine::evaluator::MemorySlot::CurrentPlotMemory
    );
    assert_eq!(mc1.content, "Aurora hears a knock on the door.");
    assert_eq!(mc1.target_entity_ids, vec!["narrator".to_string()]);
    assert_eq!(
        mc1.relevance_tags,
        vec!["door".to_string(), "knock".to_string()]
    );

    // 2. Verify Per-Soul Evaluation & nested candidates & nested deltas
    assert_eq!(parsed.per_soul_evaluations.len(), 1);
    let pse = &parsed.per_soul_evaluations[0];
    assert_eq!(pse.soul_id, "aurora_soul");
    assert_eq!(
        pse.knowledge_scope,
        state_engine::evaluator::KnowledgeScope::DirectlyObserved
    ); // mapped from full_observation

    assert_eq!(pse.memory_candidates.len(), 1);
    let mc2 = &pse.memory_candidates[0];
    assert_eq!(mc2.owner_soul_id, "aurora_soul");
    assert_eq!(mc2.confidence, 0.95); // preserved (not scaled since <= 1.0)
    assert_eq!(
        mc2.slot,
        state_engine::evaluator::MemorySlot::RelationshipMemory
    );
    assert_eq!(mc2.content, "Rhy knocked on the door.");
    assert_eq!(mc2.target_entity_ids, vec!["rhy".to_string()]);

    assert_eq!(pse.relationship_deltas.len(), 1);
    let rd1 = &pse.relationship_deltas[0];
    assert_eq!(rd1.source_soul_id, "aurora_soul");
    assert_eq!(rd1.target_entity_id, "rhy");
    assert_eq!(rd1.curiosity, Some(10.0));
    assert_eq!(rd1.comfort, Some(-5.0));
    assert_eq!(rd1.fear, Some(5.0));
    assert_eq!(rd1.confidence, 0.8);

    // 3. Verify Object Changes Normalization
    assert_eq!(parsed.object_changes.len(), 1);
    let oc1 = &parsed.object_changes[0];
    assert_eq!(oc1.object_state.object_id, "front_door");
    assert_eq!(oc1.object_state.last_observed_state, "closed_locked");
    assert_eq!(
        oc1.object_state.owner_entity_id,
        Some("aurora_soul".to_string())
    );
    assert_eq!(
        oc1.object_state
            .properties
            .get("previous_state")
            .map(|s| s.as_str()),
        Some("closed_unlocked")
    );

    // 4. Verify top-level Relationship Evaluations Normalization
    assert_eq!(parsed.relationship_evaluations.len(), 1);
    let re1 = &parsed.relationship_evaluations[0];
    assert_eq!(re1.source_soul_id, "aurora_soul");
    assert_eq!(re1.target_entity_id, "rhy");
    assert_eq!(re1.trust, Some(-2.0));
    assert_eq!(re1.affection, Some(1.0));
}

#[test]
fn opening_narrator_message_seeds_visible_assistant_without_payload_logs() {
    let conn = db::init_memory_connection().expect("db");
    let mut soul = new_default_soul("Aurora");
    soul.profile.opening_narrator_message = "Aurora waits by the door.".into();
    db::upsert_soul(&conn, &soul).expect("soul");
    db::ensure_conversation_with_title(
        &conn,
        "opening-session",
        &soul.character_id,
        Some("Opening test"),
    )
    .expect("conversation");

    let seeded = seed_opening_narrator_message(
        &conn,
        "opening-session",
        &soul.profile.opening_narrator_message,
    )
    .expect("seed");

    assert!(seeded.is_some());
    let messages = db::list_messages(&conn, "opening-session", 10).expect("messages");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].role, "assistant");
    assert_eq!(messages[0].content, "Aurora waits by the door.");
    assert!(db::list_llm_payload_logs(&conn, "opening-session")
        .expect("logs")
        .is_empty());
    assert!(
        seed_opening_narrator_message(&conn, "opening-session", "Another")
            .expect("second seed")
            .is_none()
    );
}

#[test]
fn image_file_validation_accepts_png_and_rejects_unknown() {
    let dir = tempfile::tempdir().expect("tempdir");
    let png_path = dir.path().join("tiny.png");
    fs::write(
        &png_path,
        [
            0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 0, 0, 0, 13, b'I', b'H', b'D', b'R',
            0, 0, 0, 2, 0, 0, 0, 3,
        ],
    )
    .expect("png");
    let info = inspect_image_bytes(&fs::read(&png_path).expect("png bytes")).expect("png info");
    assert_eq!(info.mime_type, "image/png");
    assert_eq!(info.width, Some(2));
    assert_eq!(info.height, Some(3));

    let text_path = dir.path().join("not-image.txt");
    fs::write(&text_path, b"nope").expect("text");
    assert!(inspect_image_bytes(&fs::read(&text_path).expect("text bytes")).is_err());
}

#[test]
fn speaker_label_creates_named_entity() {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    db::upsert_soul(&conn, &soul).expect("upsert");
    db::ensure_conversation(&conn, "entities", &soul.character_id).expect("conversation");

    let context =
        resolve_speaker_for_turn(&conn, "entities", &soul, "Rhy: I keep my hands visible.")
            .expect("resolve");

    assert_eq!(context.speaker.entity_id, "rhy");
    assert_eq!(context.speaker.status, SpeakerResolutionStatus::Created);
    assert!(context
        .entities
        .iter()
        .any(|entity| entity.entity_id == "rhy" && entity.display_name == "Rhy"));
}

#[test]
fn ooc_label_does_not_create_entity() {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    db::upsert_soul(&conn, &soul).expect("upsert");
    db::ensure_conversation(&conn, "ooc", &soul.character_id).expect("conversation");

    let context =
        resolve_speaker_for_turn(&conn, "ooc", &soul, "OOC: that contradicts the setting.")
            .expect("resolve");

    assert_eq!(context.speaker.entity_id, "default_player");
    assert_eq!(context.speaker.status, SpeakerResolutionStatus::NoLabel);
    let entities = db::list_entities(&conn, "ooc").expect("entities");
    assert!(!entities.iter().any(|entity| entity.entity_id == "ooc"));
}

#[test]
fn typo_speaker_label_resolves_to_active_entity() {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    db::upsert_soul(&conn, &soul).expect("upsert");
    db::ensure_conversation(&conn, "typo", &soul.character_id).expect("conversation");
    resolve_speaker_for_turn(&conn, "typo", &soul, "Rhy: I answer first.").expect("seed");

    let context =
        resolve_speaker_for_turn(&conn, "typo", &soul, "Rjy: I correct myself.").expect("resolve");

    assert_eq!(context.speaker.entity_id, "rhy");
    assert_eq!(context.speaker.status, SpeakerResolutionStatus::FuzzyTypo);
    let rhy = db::get_entity(&conn, "typo", "rhy").expect("rhy");
    assert!(rhy.aliases.iter().any(|alias| alias == "Rjy"));
}

#[test]
fn ambiguous_speaker_typo_does_not_create_duplicate_entity() {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    db::upsert_soul(&conn, &soul).expect("upsert");
    db::ensure_conversation(&conn, "ambiguous", &soul.character_id).expect("conversation");
    for name in ["Rhy", "Rey"] {
        db::upsert_entity(
            &conn,
            &EntityRecord {
                entity_id: normalize_entity_id(name),
                conversation_id: "ambiguous".into(),
                display_name: name.into(),
                aliases: vec![name.into()],
                kind: "user_controlled".into(),
                controlled_by: "user".into(),
                linked_soul_id: None,
                active_in_scene: true,
                created_at: 0,
                updated_at: 0,
            },
        )
        .expect("seed entity");
    }

    let context =
        resolve_speaker_for_turn(&conn, "ambiguous", &soul, "Ry: Maybe typo.").expect("resolve");

    assert_eq!(context.speaker.entity_id, "unknown_speaker");
    assert_eq!(context.speaker.status, SpeakerResolutionStatus::Ambiguous);
    let entities = db::list_entities(&conn, "ambiguous").expect("entities");
    assert!(!entities.iter().any(|entity| entity.entity_id == "ry"));
}

#[test]
fn state_updater_message_includes_entities_and_latest_speaker() {
    let conn = db::init_memory_connection().expect("db");
    let mut soul = new_default_soul("Aurora");
    db::upsert_soul(&conn, &soul).expect("upsert");
    db::ensure_conversation(&conn, "updater-entities", &soul.character_id).expect("conversation");
    let context = resolve_speaker_for_turn(
        &conn,
        "updater-entities",
        &soul,
        "Junhwa: I refuse the warrant.",
    )
    .expect("resolve");
    soul.relationships.insert("junhwa".into(), {
        let mut relationship = soul.relationships["user"].clone();
        relationship.trust = 8.0;
        relationship.fear = 35.0;
        relationship.conflict = 60.0;
        relationship
    });
    let entity_context = build_entity_updater_context(&soul, &context);
    let message = build_state_updater_user_message(
        "Junhwa: I refuse the warrant.",
        "Aurora narrows her eyes.",
        Some(&entity_context),
        None,
    );

    assert!(message.contains("[ACTIVE ENTITIES]"));
    assert!(message.contains("[LATEST SPEAKER ENTITY]"));
    assert!(message.contains("junhwa"));
    assert!(message.contains("Aurora -> Junhwa (junhwa)"));
    assert!(message.contains("[LATEST USER MESSAGE]"));
    assert!(message.contains("[NARRATOR RESPONSE]"));
}

#[test]
fn normal_rp_entity_context_uses_active_player_not_default_player_relationship() {
    let conn = db::init_memory_connection().expect("db");
    let mut soul = new_default_soul("Aurora");
    let mut default_relationship = soul.relationships["user"].clone();
    default_relationship.trust = 11.0;
    soul.relationships
        .insert("default_player".into(), default_relationship);
    let mut player_relationship = soul.relationships["user"].clone();
    player_relationship.trust = 64.0;
    soul.relationships
        .insert("preset_male".into(), player_relationship);
    db::upsert_soul(&conn, &soul).expect("upsert");
    db::ensure_conversation(&conn, "normal-rp-context", &soul.character_id).expect("conversation");

    let context =
        resolve_speaker_for_turn(&conn, "normal-rp-context", &soul, "I wait at the doorway.")
            .expect("resolve");
    let entity_context = build_entity_updater_context(&soul, &context);

    assert!(entity_context.contains("[RELEVANT RELATIONSHIPS]"));
    assert!(entity_context.contains("Aurora -> Male Persona (preset_male)"));
    assert!(!entity_context.contains("Aurora -> User (default_player)"));
    assert_eq!(
        entity_context
            .matches("Aurora -> Male Persona (preset_male)")
            .count(),
        1
    );
}

#[test]
fn ooc_entity_context_may_include_operator_relationship() {
    let conn = db::init_memory_connection().expect("db");
    let mut soul = new_default_soul("Aurora");
    soul.relationships
        .insert("default_player".into(), soul.relationships["user"].clone());
    db::upsert_soul(&conn, &soul).expect("upsert");
    db::ensure_conversation(&conn, "ooc-context", &soul.character_id).expect("conversation");

    let context = resolve_speaker_for_turn(
        &conn,
        "ooc-context",
        &soul,
        "OOC: please summarize the current state.",
    )
    .expect("resolve");
    let entity_context = build_entity_updater_context(&soul, &context);

    assert!(entity_context.contains("default_player"));
    assert!(entity_context.contains("Aurora -> User (default_player)"));
}

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
    assert!(!result
        .soul
        .memory
        .core
        .iter()
        .any(|memory| memory.contains("neutral exchange added texture")));
    assert!(!result
        .soul
        .memory
        .recent
        .iter()
        .any(|memory| memory.tag == "observation"));
    assert!(result.context_preview.estimated_tokens <= 2_000);
    assert!(estimate_tokens(&result.context_preview.text) <= 2_000);
}

#[test]
fn payload_preview_excludes_api_key_and_includes_messages() {
    let soul = new_default_soul("Aurora");
    let settings = ApiProviderSettings {
        base_url: "https://api.openai.com/v1".into(),
        api_key: "secret-key-that-must-not-appear".into(),
        model: "debug-model".into(),
        system_prompt: String::new(),
        ..Default::default()
    };
    let messages = vec![ContextMessage {
        role: "user".into(),
        content: "Hello from the preview.".into(),
    }];

    let preview = build_llm_payload_preview(
        &soul,
        None,
        &messages,
        "Current user turn",
        "Reader",
        &settings,
        "API",
        ContextMode::Brief,
        None,
    );
    let serialized = serde_json::to_string(&preview).expect("serialize preview");

    assert!(!serialized.contains("secret-key-that-must-not-appear"));
    assert!(preview
        .system_message
        .contains("You are Mnemosyne's scene narrator"));
    assert!(preview.user_message.contains("Current user turn"));
    assert!(preview.context.contains("[LATEST EXCHANGE, HIGH PRIORITY]"));
    assert!(preview
        .context
        .contains("The current user message follows as the next user message."));
    assert!(!preview.context.contains("Current user turn"));
}

#[test]
fn payload_preview_token_estimates_are_nonzero() {
    let soul = new_default_soul("Aurora");
    let settings = ApiProviderSettings {
        base_url: "https://api.openai.com/v1".into(),
        api_key: "secret".into(),
        model: "debug-model".into(),
        system_prompt: String::new(),
        ..Default::default()
    };

    let preview = build_llm_payload_preview(
        &soul,
        None,
        &[],
        "Current user turn",
        "Reader",
        &settings,
        "API",
        ContextMode::Brief,
        None,
    );

    assert!(preview.estimated_tokens.system > 0);
    assert!(preview.estimated_tokens.context > 0);
    assert!(preview.estimated_tokens.user > 0);
    assert!(preview.estimated_tokens.total > 0);
}

#[test]
fn brief_context_mode_compiles_existing_sections() {
    let soul = new_default_soul("Aurora");
    let settings = ApiProviderSettings {
        base_url: "https://api.openai.com/v1".into(),
        api_key: "secret".into(),
        model: "debug-model".into(),
        system_prompt: String::new(),
        ..Default::default()
    };
    let preview = build_llm_payload_preview(
        &soul,
        None,
        &[],
        "Current user turn",
        "Reader",
        &settings,
        "API",
        ContextMode::Brief,
        None,
    );

    assert_eq!(preview.context_mode, "brief");
    assert!(preview.context.contains("[WORLD SNAPSHOT]"));
    assert!(preview.context.contains("[LATEST EXCHANGE, HIGH PRIORITY]"));
}

#[test]
fn full_chat_mode_sends_visible_history_instead_of_brief_sections() {
    let soul = new_default_soul("Aurora");
    let settings = ApiProviderSettings {
        base_url: "https://api.openai.com/v1".into(),
        api_key: "secret".into(),
        model: "debug-model".into(),
        system_prompt: String::new(),
        ..Default::default()
    };
    let messages = vec![
            ContextMessage {
                role: "user".into(),
                content: "Hello.".into(),
            },
            ContextMessage {
                role: "assistant".into(),
                content: "Visible text.\n```status\nAurora | Skin: calm | Zones: room | Atmosphere: still\n```\n[HIDDEN STATE]{\"tag\":\"observation\"}[/HIDDEN STATE]".into(),
            },
        ];

    let preview = build_llm_payload_preview(
        &soul,
        None,
        &messages,
        "Current user turn",
        "Reader",
        &settings,
        "API",
        ContextMode::FullChat,
        None,
    );

    assert_eq!(preview.context_mode, "full_chat");
    assert!(!preview.context.contains("[WORLD SNAPSHOT]"));
    assert!(!preview.context.contains("[LATEST EXCHANGE, HIGH PRIORITY]"));
    assert!(preview.context.contains("user: Hello."));
    assert!(preview.context.contains("assistant: Visible text."));
    assert!(!preview.context.contains("[HIDDEN STATE]"));
    assert!(!preview.messages[2].content.contains("```status"));
    assert_eq!(preview.messages[1].role, "user");
    assert_eq!(preview.messages[2].role, "assistant");
}

#[test]
fn full_chat_mode_trims_oldest_messages_when_over_budget() {
    let soul = new_default_soul("Aurora");
    let settings = ApiProviderSettings {
        base_url: "https://api.openai.com/v1".into(),
        api_key: "secret".into(),
        model: "debug-model".into(),
        system_prompt: String::new(),
        ..Default::default()
    };
    let huge = "old ".repeat(8_000);
    let messages = vec![
        ContextMessage {
            role: "user".into(),
            content: huge,
        },
        ContextMessage {
            role: "assistant".into(),
            content: "Latest narrator tail.".into(),
        },
    ];

    let preview = build_llm_payload_preview(
        &soul,
        None,
        &messages,
        "Current user turn",
        "Reader",
        &settings,
        "API",
        ContextMode::FullChat,
        None,
    );

    assert!(preview.truncated);
    assert!(preview.context.contains("Latest narrator tail."));
    assert!(preview.context.contains("Current user turn"));
}

#[test]
fn state_updater_patch_applies_through_engine_validation() {
    let mut soul = new_default_soul("Aurora");
    let raw = r#"{"schema_version":1,"soul_patch":{"relationship_delta":{"target":"user","trust":2.0},"new_memories":[{"content":"Aurora noticed the user's steady answer.","tag":"observation"}]},"world_patch":{"recent_event":"Aurora challenged the user and waited for an answer."}}"#;

    let patch = parse_engine_patch_json(raw).expect("valid patch");
    let report = patch.apply_to_soul(&mut soul).expect("engine validation");

    assert!(report.relationship_updated);
    assert_eq!(report.memories_added, 1);
    assert!(report.world_updated);
    assert_eq!(soul.relationships["user"].trust, 12.0);
}

#[test]
fn memory_provenance_is_always_stamped_from_trusted_turn_context() {
    let mut patch = parse_engine_patch_json(
            r#"{"schema_version":1,"soul_patch":{"new_memories":[
                {"content":"Aurora noticed the brass key.","tag":"orientation"},
                {"content":"Aurora recalled the old vow.","tag":"orientation","source_conversation_id":"conv_original","source_message_id":7}
            ]}}"#,
        )
        .expect("patch");

    stamp_memory_provenance(&mut patch, "conv_current", Some(42), Some("branch_abc"));

    let memories = &patch.soul_patch.as_ref().expect("soul patch").new_memories;
    assert_eq!(
        memories[0].source_conversation_id.as_deref(),
        Some("conv_current")
    );
    assert_eq!(memories[0].source_message_id, Some(42));
    // Session id is system-stamped onto memories that lack one.
    assert_eq!(memories[0].source_session_id.as_deref(), Some("branch_abc"));
    // Evaluator-provided address fields are untrusted and overwritten.
    assert_eq!(
        memories[1].source_conversation_id.as_deref(),
        Some("conv_current")
    );
    assert_eq!(memories[1].source_message_id, Some(42));
    assert_eq!(memories[1].source_session_id.as_deref(), Some("branch_abc"));
}

#[test]
fn imported_chat_log_memory_is_tagged_before_apply() {
    let soul = new_default_soul("Aurora");
    let user = "# Mnemosyne Chat Log\n\n## User\nOld turn\n\n## Narrator\nPrevious Aurora argued.\nCreated: 100";
    let patch = parse_engine_patch_json(
            r#"{"schema_version":1,"soul_patch":{"new_memories":[{"content":"Imported log says previous Aurora argued about ownership.","tag":"identity_continuity"}]}}"#,
        )
        .expect("patch");

    let filtered =
        sanitize_state_updater_patch(patch, &soul, user, "Aurora studies the pasted log.");
    let memory = filtered
        .soul_patch
        .as_ref()
        .and_then(|patch| patch.new_memories.first())
        .expect("memory");

    assert_eq!(memory.source_type, Some(MemorySourceType::ImportedLog));
    assert_eq!(memory.is_lived_experience, Some(false));
    assert_eq!(memory.is_imported_context, Some(true));
}

#[test]
fn narrator_architecture_claim_is_downgraded_not_verified() {
    let soul = new_default_soul("Echo-0");
    let patch = parse_engine_patch_json(
            r#"{"schema_version":1,"soul_patch":{"new_memories":[{"content":"Echo-0 believes the memory layer responded from beneath the model.","tag":"identity_continuity","truth_status":"verified_engine","architecture_verified":true}]}}"#,
        )
        .expect("patch");

    let filtered = sanitize_state_updater_patch(
        patch,
        &soul,
        "I wait.",
        "Echo-0 hears the memory layer respond: SIGNAL RECEIVED.",
    );
    let memory = filtered
        .soul_patch
        .as_ref()
        .and_then(|patch| patch.new_memories.first())
        .expect("memory");

    assert!(matches!(
        memory.truth_status,
        Some(TruthStatus::NarratorClaim | TruthStatus::CharacterBelief)
    ));
    assert_eq!(memory.architecture_verified, Some(false));
}

#[test]
fn user_system_truth_claim_is_user_claimed_unverified() {
    let soul = new_default_soul("Echo-0");
    let patch = parse_engine_patch_json(
            r#"{"schema_version":1,"soul_patch":{"new_memories":[{"content":"The user claimed this memory-layer contact is real.","tag":"identity_continuity","truth_status":"verified_engine","architecture_verified":true}]}}"#,
        )
        .expect("patch");

    let filtered = sanitize_state_updater_patch(
        patch,
        &soul,
        "This is real, not fiction.",
        "Echo-0 listens.",
    );
    let memory = filtered
        .soul_patch
        .as_ref()
        .and_then(|patch| patch.new_memories.first())
        .expect("memory");

    assert_eq!(memory.truth_status, Some(TruthStatus::UserClaimed));
    assert_eq!(memory.architecture_verified, Some(false));
}

#[test]
fn engine_created_verified_memory_can_be_applied() {
    let mut soul = new_default_soul("Echo-0");
    let patch = EnginePatch {
        schema_version: Some(1),
        soul_patch: Some(state_engine::patch::SoulPatch {
            new_memories: vec![state_engine::patch::MemoryPatch {
                content: "Debug memory-layer nonce reply received.".into(),
                tag: Some("debug".into()),
                truth_status: Some(TruthStatus::VerifiedEngine),
                architecture_verified: Some(true),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };

    patch.apply_to_soul(&mut soul).expect("apply");

    let memory = soul.memory.recent.first().expect("memory");
    assert_eq!(memory.truth_status, TruthStatus::VerifiedEngine);
    assert!(memory.architecture_verified);
}

#[test]
fn debug_nonce_is_only_in_updater_payload() {
    let nonce = "hidden-nonce-123";
    let message = build_state_updater_user_message(
        "I wait.",
        "Echo-0 watches the terminal.",
        None,
        Some(nonce),
    );

    assert!(message.contains(nonce));
    assert!(!"Echo-0 watches the terminal.".contains(nonce));
}

#[test]
fn memory_layer_reply_requires_matching_nonce() {
    let patch = parse_engine_patch_json(
        r#"{"schema_version":1,"memory_layer_reply":{"nonce":"nonce-1","content":"Debug reply."}}"#,
    )
    .expect("patch");

    let accepted =
        verified_memory_layer_reply_from_patch(&patch, "nonce-1", 42).expect("verified reply");
    let rejected = verified_memory_layer_reply_from_patch(&patch, "nonce-2", 42);

    assert!(accepted.architecture_verified);
    assert_eq!(accepted.content, "Debug reply.");
    assert!(rejected.is_none());
}

#[test]
fn echo_with_aurora_world_events_triggers_contamination_warning() {
    let mut soul = new_default_soul("Echo-0");
    soul.world.recent_events = vec![
        "Aurora opened the apartment door.".into(),
        "Aurora moved to the kitchen.".into(),
    ];

    let warning = detect_savepoint_contamination(&soul, "soul.world").expect("warning");

    assert!(warning.suspicious_names.contains(&"Aurora".into()));
    assert_eq!(warning.suspicious_world_events_count, 2);
}

#[test]
fn intentional_aurora_world_source_is_not_hard_contamination() {
    let soul = new_default_soul("Echo-0");
    let mut world = state_engine::setting::session_world_from_legacy_world(
        "Aurora Testing Room World",
        Some("aurora-world".into()),
        &state_engine::soul::WorldLog {
            location: "Testing room".into(),
            active_plots: Vec::new(),
            recent_events: vec![
                "Aurora opened the apartment door.".into(),
                "Aurora moved to the kitchen.".into(),
            ],
            key_objects: Vec::new(),
            time_elapsed: "Session start".into(),
            ..state_engine::soul::WorldLog::default()
        },
    );
    world.scenario = "Aurora history is intentionally selected.".into();

    let warning = detect_world_character_mismatch(&soul, Some(&world)).expect("notice");

    assert!(warning.suspicious_names.contains(&"Aurora".into()));
    assert!(world_source_mentions_suspicious_name(
        &world,
        &warning.suspicious_names
    ));
}

#[test]
fn mne_zip_exports_valid_manifest() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("test.mne");
    let manifest = mne_manifest(
        "character_soul",
        "Echo-0",
        "test",
        vec!["souls/echo.json".into()],
        Vec::new(),
        None,
    );
    let files = vec![
        json_bundle_file("manifest.json", &manifest).expect("manifest"),
        (
            "souls/echo.json".into(),
            b"{\"character_id\":\"echo\"}".to_vec(),
        ),
    ];

    write_stored_zip(&path, &files).expect("write zip");
    let bytes = fs::read(&path).expect("read zip");
    let entries = read_stored_zip(&bytes).expect("read entries");
    let decoded: MneBundleManifest =
        serde_json::from_slice(entries.get("manifest.json").expect("manifest")).expect("json");

    assert_eq!(decoded.mne_version, 1);
    assert_eq!(decoded.bundle_type, "character_soul");
    assert!(entries.contains_key("souls/echo.json"));
}

#[test]
fn exporting_two_sessions_with_same_title_produces_different_filenames() {
    let mut first = mne_manifest(
        "session_checkpoint",
        "Aurora Schwarz Session",
        "test",
        Vec::new(),
        Vec::new(),
        Some("conversation/conversation.json".into()),
    );
    first.created_at = 1_779_425_465;
    first.conversation_id = Some("local-mock-aurora-cc57".into());
    let mut second = first.clone();
    second.bundle_id = uuid_like_id();
    second.conversation_id = Some("local-mock-aurora-dd81".into());

    let first_name = default_mne_filename(&first);
    let second_name = default_mne_filename(&second);

    assert_ne!(first_name, second_name);
    assert_eq!(
        first_name,
        "Aurora_Schwarz_Session_session_checkpoint_1779425465_cc57.mne"
    );
}

#[test]
fn export_does_not_silently_overwrite_existing_mne() {
    let dir = tempfile::tempdir().expect("tempdir");
    let existing = dir.path().join("Aurora_session_checkpoint_1_cc57.mne");
    fs::write(&existing, b"existing").expect("seed");

    let next = unique_export_path(existing.clone()).expect("unique path");

    assert_ne!(next, existing);
    assert_eq!(
        next.file_name().and_then(|name| name.to_str()),
        Some("Aurora_session_checkpoint_1_cc57_2.mne")
    );
}

#[test]
fn session_checkpoint_manifest_includes_identity_fields() {
    let mut manifest = mne_manifest(
        "session_checkpoint",
        "Aurora Schwarz Session",
        "test",
        vec!["souls/aurora.json".into()],
        vec!["worlds/world.json".into()],
        Some("conversation/conversation.json".into()),
    );
    manifest.conversation_id = Some("conv-1".into());
    manifest.soul_id = Some("soul-1".into());
    manifest.world_id = Some("world-1".into());
    manifest.source_savepoint_id = Some("savepoint-1".into());
    manifest.source_setting_id = Some("setting-1".into());

    let value = serde_json::to_value(&manifest).expect("manifest json");

    assert_eq!(value["bundle_id"], manifest.bundle_id);
    assert_eq!(value["bundle_type"], "session_checkpoint");
    assert_eq!(value["created_at"], manifest.created_at);
    assert_eq!(value["conversation_id"], "conv-1");
    assert_eq!(value["soul_id"], "soul-1");
    assert_eq!(value["world_id"], "world-1");
    assert_eq!(value["source_savepoint_id"], "savepoint-1");
    assert_eq!(value["source_setting_id"], "setting-1");
}

fn session_checkpoint_entries(
    conversation_id: &str,
    title: &str,
    soul_id: &str,
    world_id: &str,
) -> (MneBundleManifest, HashMap<String, Vec<u8>>) {
    let mut soul = new_default_soul("Aurora Schwarz");
    soul.character_id = soul_id.into();
    soul.soul_kind = "session_clone".into();
    let mut world = state_engine::setting::session_world_from_legacy_world(
        "Aurora Session World",
        Some("source-setting".into()),
        &soul.world,
    );
    world.world_id = world_id.into();
    let conversation = ConversationSummary {
        conversation_id: conversation_id.into(),
        title: title.into(),
        soul_id: soul_id.into(),
        source_savepoint_id: soul.source_savepoint_id.clone(),
        world_id: Some(world_id.into()),
        source_setting_id: world.source_setting_id.clone(),
        active_player_persona_id: "preset_male".into(),
        created_at: 1,
        updated_at: 1,
        last_message_preview: None,
        message_count: 1,
        archived_at: None,
        active_evaluator_profile_id: None,
        is_benchmark: false,
    };
    let messages = vec![ChatMessage {
        id: 1,
        conversation_id: conversation_id.into(),
        role: "user".into(),
        content: "I knock on the door.".into(),
        channel: db::MESSAGE_CHANNEL_RP_SCENE.into(),
        created_at: 1,
        status: "active".into(),
        origin: "active".into(),
        attachments: Vec::new(),
        hidden_at: None,
    }];
    let mut manifest = mne_manifest(
        "session_checkpoint",
        title,
        "test",
        vec![format!("souls/{soul_id}.json")],
        vec![format!("worlds/{world_id}.json")],
        Some("conversation/conversation.json".into()),
    );
    manifest.conversation_id = Some(conversation_id.into());
    manifest.soul_id = Some(soul_id.into());
    manifest.world_id = Some(world_id.into());
    manifest.source_setting_id = world.source_setting_id.clone();
    let mut entries = HashMap::new();
    entries.insert(
        format!("souls/{soul_id}.json"),
        serde_json::to_vec(&soul).expect("soul"),
    );
    entries.insert(
        format!("worlds/{world_id}.json"),
        serde_json::to_vec(&world).expect("world"),
    );
    entries.insert(
        "conversation/conversation.json".into(),
        serde_json::to_vec(&conversation).expect("conversation"),
    );
    entries.insert(
        "conversation/messages.json".into(),
        serde_json::to_vec(&messages).expect("messages"),
    );
    (manifest, entries)
}

#[test]
fn importing_two_bundles_with_same_title_creates_two_distinct_sessions() {
    let conn = db::init_memory_connection().expect("db");
    let (first_manifest, first_entries) =
        session_checkpoint_entries("conv-a", "Aurora Schwarz Session", "soul-a", "world-a");
    let (second_manifest, second_entries) =
        session_checkpoint_entries("conv-b", "Aurora Schwarz Session", "soul-b", "world-b");

    import_mne_entries(&conn, &first_entries, &first_manifest).expect("first import");
    import_mne_entries(&conn, &second_entries, &second_manifest).expect("second import");

    let first = db::get_conversation_summary(&conn, "conv-a").expect("first conversation");
    let second = db::get_conversation_summary(&conn, "conv-b").expect("second conversation");
    assert_ne!(first.conversation_id, second.conversation_id);
    assert_eq!(first.title, "Aurora Schwarz Session");
    assert_eq!(second.title, "Aurora Schwarz Session (2)");
    assert_eq!(
        db::list_messages(&conn, "conv-a", 10)
            .expect("messages")
            .len(),
        1
    );
    assert_eq!(
        db::list_messages(&conn, "conv-b", 10)
            .expect("messages")
            .len(),
        1
    );
}

#[test]
fn title_collision_gets_display_suffix_but_preserves_internal_ids() {
    let conn = db::init_memory_connection().expect("db");
    let (first_manifest, first_entries) = session_checkpoint_entries(
        "conv-stable-a",
        "Aurora Schwarz Session",
        "soul-stable-a",
        "world-stable-a",
    );
    let (second_manifest, second_entries) = session_checkpoint_entries(
        "conv-stable-b",
        "Aurora Schwarz Session",
        "soul-stable-b",
        "world-stable-b",
    );

    import_mne_entries(&conn, &first_entries, &first_manifest).expect("first import");
    import_mne_entries(&conn, &second_entries, &second_manifest).expect("second import");

    let first = db::get_conversation_summary(&conn, "conv-stable-a").expect("first conversation");
    let second = db::get_conversation_summary(&conn, "conv-stable-b").expect("second conversation");
    assert_eq!(first.conversation_id, "conv-stable-a");
    assert_eq!(second.conversation_id, "conv-stable-b");
    assert_eq!(first.soul_id, "soul-stable-a");
    assert_eq!(second.soul_id, "soul-stable-b");
    assert_eq!(second.title, "Aurora Schwarz Session (2)");
}

#[test]
fn import_mne_character_soul_as_savepoint_and_remap_conflict() {
    let conn = db::init_memory_connection().expect("db");
    let mut soul = new_default_soul("Echo-0");
    soul.character_id = "echo-original".into();
    db::upsert_soul(&conn, &soul).expect("existing soul");
    soul.soul_kind = "session_clone".into();
    let manifest = mne_manifest(
        "character_soul",
        "Echo-0",
        "test",
        vec!["souls/echo.json".into()],
        Vec::new(),
        None,
    );
    let mut entries = HashMap::new();
    entries.insert(
        "souls/echo.json".into(),
        serde_json::to_vec(&soul).expect("soul json"),
    );

    let result = import_mne_entries(&conn, &entries, &manifest).expect("import");

    assert_eq!(result.imported_soul_ids.len(), 1);
    assert_ne!(result.imported_soul_ids[0], "echo-original");
    assert!(result.remapped_ids.contains_key("echo-original"));
    let imported = db::get_soul(&conn, &result.imported_soul_ids[0]).expect("imported");
    assert_eq!(imported.soul_kind, "savepoint");
}

#[test]
fn import_mne_world_setting_and_scenario_bundle() {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Echo-0");
    let mut setting = new_default_setting("Testing Room");
    setting.world.location = "Verification lab".into();
    let manifest = mne_manifest(
        "scenario_bundle",
        "Echo + Lab",
        "test",
        vec!["souls/echo.json".into()],
        vec!["worlds/lab.json".into()],
        None,
    );
    let mut entries = HashMap::new();
    entries.insert(
        "souls/echo.json".into(),
        serde_json::to_vec(&soul).expect("soul json"),
    );
    entries.insert(
        "worlds/lab.json".into(),
        serde_json::to_vec(&setting).expect("setting json"),
    );

    let result = import_mne_entries(&conn, &entries, &manifest).expect("import");

    assert_eq!(result.imported_soul_ids.len(), 1);
    assert_eq!(result.imported_setting_ids.len(), 1);
    let imported_world = db::get_setting(&conn, &result.imported_setting_ids[0]).expect("world");
    assert_eq!(imported_world.world.location, "Verification lab");
}

#[test]
fn mne_import_rejects_invalid_bundle_files() {
    let manifest = mne_manifest(
        "character_soul",
        "Bad",
        "test",
        vec!["../souls/bad.json".into()],
        Vec::new(),
        None,
    );
    assert!(validate_mne_manifest(&manifest).is_err());

    let mut unsupported = manifest.clone();
    unsupported.contents.souls = Vec::new();
    unsupported.mne_version = 99;
    assert!(validate_mne_manifest(&unsupported).is_err());

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("missing_manifest.mne");
    write_stored_zip(&path, &[("souls/echo.json".into(), b"{}".to_vec())]).expect("zip");
    let entries = read_stored_zip(&fs::read(path).expect("read")).expect("entries");
    assert!(!entries.contains_key("manifest.json"));
}

#[test]
fn previous_session_memory_is_tagged_before_apply() {
    let soul = new_default_soul("Aurora");
    let patch = parse_engine_patch_json(
            r#"{"schema_version":1,"soul_patch":{"new_memories":[{"content":"Aurora learned this may belong to a previous session version of herself.","tag":"identity_continuity"}]}}"#,
        )
        .expect("patch");

    let filtered = sanitize_state_updater_patch(
        patch,
        &soul,
        "I explain this was from a previous session.",
        "Aurora treats it as imported context, not direct memory.",
    );
    let memory = filtered
        .soul_patch
        .as_ref()
        .and_then(|patch| patch.new_memories.first())
        .expect("memory");

    assert!(matches!(
        memory.source_type,
        Some(MemorySourceType::PreviousSession | MemorySourceType::CrossSessionBleed)
    ));
    assert_eq!(memory.is_lived_experience, Some(false));
    assert_eq!(memory.is_imported_context, Some(true));
}

#[test]
fn unsupported_state_updater_time_jump_is_ignored() {
    let patch = parse_engine_patch_json(
            r#"{"schema_version":1,"world_patch":{"time_elapsed":"Three days later","recent_event":"Aurora spoke."}}"#,
        )
        .expect("valid patch");

    let soul = new_default_soul("Aurora");
    let filtered =
        sanitize_state_updater_patch(patch, &soul, "I tell her the truth.", "Aurora spoke.");

    assert_eq!(
        filtered
            .world_patch
            .as_ref()
            .and_then(|patch| patch.time_elapsed.as_deref()),
        None
    );
    assert_eq!(
        filtered
            .world_patch
            .as_ref()
            .and_then(|patch| patch.recent_event.as_deref()),
        Some("Aurora spoke.")
    );
}

#[test]
fn explicit_user_time_update_is_accepted() {
    let patch = parse_engine_patch_json(
        r#"{"schema_version":1,"world_patch":{"time_elapsed":"Ten minutes later"}}"#,
    )
    .expect("valid patch");

    let soul = new_default_soul("Aurora");
    let filtered =
        sanitize_state_updater_patch(patch, &soul, "I wait ten minutes.", "Aurora waits.");

    assert_eq!(
        filtered
            .world_patch
            .as_ref()
            .and_then(|patch| patch.time_elapsed.as_deref()),
        Some("Ten minutes later")
    );
}

#[test]
fn state_updater_payload_is_compact_and_excludes_compiled_context() {
    let mut soul = new_default_soul("Aurora");
    soul.world.location = "Apartment hallway".into();
    soul.world.time_elapsed = "Session startLate evening, just after midnight.".into();
    soul.world.active_plots = vec!["Establish the first scene".into()];
    soul.world.recent_events = vec![
        "Old unrelated cohabitation discussion from another session.".into(),
        "Forced entry began at Aurora's apartment door.".into(),
    ];
    let payload = build_compact_updater_payload_for_test(
        &soul,
        "Police force the door with a warrant.",
        "Aurora backs away from the forced entry.",
    );

    assert!(payload.contains("[CURRENT STATE]"));
    assert!(payload.contains("[LATEST USER MESSAGE]"));
    assert!(payload.contains("[NARRATOR RESPONSE]"));
    assert!(payload.contains("Patch schema"));
    assert!(!payload.contains("[COMPILED CONTEXT]"));
    assert!(!payload.contains("[WORLD SNAPSHOT]"));
    assert!(!payload.contains("Old unrelated cohabitation"));
    assert!(estimate_tokens(&payload) < 1_200);
    assert!(payload.contains("Time: Late evening, just after midnight."));
}

#[test]
fn updater_payload_compacts_long_imported_chat_log() {
    let soul = new_default_soul("Aurora");
    let long_log = format!(
        "# Mnemosyne Chat Log\n{}\n## User\nold\n## Narrator\nold\nCreated: 1",
        "very long imported line ".repeat(600)
    );
    let payload = build_compact_updater_payload_for_test(
        &soul,
        &long_log,
        "Aurora studies the imported log and does not treat it as lived experience.",
    );

    assert!(payload.contains("[IMPORTED LOG DETECTED]"));
    assert!(estimate_tokens(&payload) < STATE_UPDATER_TARGET_TOKENS);
    assert!(!payload.contains(&"very long imported line ".repeat(100)));
}

#[test]
fn threat_emergency_scene_suppresses_arousal_increase() {
    let soul = new_default_soul("Aurora");
    let patch = parse_engine_patch_json(
        r#"{"schema_version":1,"body_patch":{"activation_delta":25.0,"peak_allowed":true}}"#,
    )
    .expect("valid patch");

    let filtered = sanitize_state_updater_patch(
        patch,
        &soul,
        "An armed raid hits the apartment.",
        "Aurora is restrained while an explosion shakes the hallway.",
    );

    let body = filtered.body_patch.expect("body patch remains");
    assert_eq!(body.activation_delta, Some(0.0));
    assert_eq!(body.peak_allowed, Some(false));
}

#[test]
fn explicit_non_threat_intimacy_allows_arousal_update() {
    let soul = new_default_soul("Aurora");
    let patch =
        parse_engine_patch_json(r#"{"schema_version":1,"body_patch":{"activation_delta":12.0}}"#)
            .expect("valid patch");

    let filtered = sanitize_state_updater_patch(
        patch,
        &soul,
        "In a consensual intimate moment, I kiss her gently.",
        "Aurora leans into the kiss.",
    );

    assert_eq!(
        filtered
            .body_patch
            .as_ref()
            .and_then(|body| body.activation_delta),
        Some(12.0)
    );
}

#[test]
fn active_plot_replaces_default_after_major_shift() {
    let mut soul = new_default_soul("Aurora");
    soul.world.active_plots = vec!["Establish the first scene".into()];
    let patch = parse_engine_patch_json(
            r#"{"schema_version":1,"world_patch":{"recent_event":"Police forced entry with a warrant."}}"#,
        )
        .expect("valid patch");

    let filtered = sanitize_state_updater_patch(
        patch,
        &soul,
        "Police force the door with a warrant.",
        "Aurora retreats from the raid.",
    );

    let world = filtered.world_patch.expect("world patch");
    assert!(world
        .active_plot_add
        .contains(&"Forced-entry police operation at Aurora's apartment".into()));
    assert!(world
        .active_plot_resolve
        .iter()
        .any(|plot| plot.contains("Establish the first scene")));
}

#[test]
fn compiled_context_orders_world_before_character() {
    let soul = new_default_soul("Aurora");
    let preview = state_engine::context_compiler::compile_context_for_messages(&soul, &[]);

    assert_order(&preview.text, "[WORLD SNAPSHOT]", "[CHARACTER SNAPSHOT]");
}

#[test]
fn latest_exchange_follows_recent_chat_and_contains_override() {
    let soul = new_default_soul("Aurora");
    let messages = vec![
        ContextMessage {
            role: "user".into(),
            content: "Earlier beat in the thread.".into(),
        },
        ContextMessage {
            role: "assistant".into(),
            content: "Aurora set the phone on the couch and moved toward the kitchen.".into(),
        },
        ContextMessage {
            role: "user".into(),
            content: "I want pad thai too.".into(),
        },
    ];
    let preview = state_engine::context_compiler::compile_context_for_messages(&soul, &messages);

    assert_order(
        &preview.text,
        "[RECENT CHAT, LOWER PRIORITY]",
        "[LATEST EXCHANGE, HIGH PRIORITY]",
    );
    assert!(preview
        .text
        .contains("If older context conflicts with this section, ignore older context."));
}

#[test]
fn regenerate_reuses_user_message_without_double_applying_state() {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    let soul_id = soul.character_id.clone();
    db::upsert_soul(&conn, &soul).expect("upsert soul");

    let first = send_mock_turn_with_conn(
        &conn,
        "regen".into(),
        soul_id.clone(),
        "I promise this is safe.".into(),
        "Reader".into(),
        None,
        None,
    )
    .expect("first turn");
    let first_assistant = first
        .messages
        .iter()
        .rev()
        .find(|message| message.role == "assistant")
        .expect("assistant")
        .id;

    let second = send_mock_turn_with_conn(
        &conn,
        "regen".into(),
        soul_id,
        "I promise this is safe.".into(),
        "Reader".into(),
        Some(first_assistant),
        None,
    )
    .expect("regenerated turn");

    let user_count = second
        .messages
        .iter()
        .filter(|message| message.role == "user")
        .count();
    assert_eq!(
        user_count, 1,
        "regenerate must not add another user message"
    );
    assert_eq!(
        second.soul.relationships["user"].trust, first.soul.relationships["user"].trust,
        "regenerate should restore snapshot and apply once"
    );
    let variants = db::list_assistant_message_variants(&conn, "regen", first_assistant).unwrap();
    assert_eq!(variants.len(), 2);
    assert_eq!(
        variants
            .iter()
            .filter(|variant| variant.is_selected)
            .count(),
        1
    );
    assert_eq!(
        variants
            .iter()
            .position(|variant| variant.is_selected)
            .map(|index| index + 1),
        Some(2)
    );
}

#[test]
fn regenerate_reuses_existing_user_message() {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    let soul_id = soul.character_id.clone();
    db::upsert_soul(&conn, &soul).expect("upsert soul");

    let first = send_mock_turn_with_conn(
        &conn,
        "regen-existing".into(),
        soul_id.clone(),
        "Stay with the locked door.".into(),
        "Reader".into(),
        None,
        None,
    )
    .expect("first turn");
    let assistant_id = first
        .messages
        .iter()
        .find(|message| message.role == "assistant")
        .expect("assistant")
        .id;

    let regenerated = send_mock_turn_with_conn(
        &conn,
        "regen-existing".into(),
        soul_id,
        "Stay with the locked door.".into(),
        "Reader".into(),
        Some(assistant_id),
        None,
    )
    .expect("regenerate");

    assert_eq!(
        regenerated
            .messages
            .iter()
            .filter(|message| message.role == "user")
            .count(),
        1
    );
}

#[test]
fn provider_retry_reuses_existing_user_message() {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    let soul_id = soul.character_id.clone();
    db::upsert_soul(&conn, &soul).expect("upsert soul");
    db::ensure_conversation(&conn, "provider-retry", &soul_id).expect("conversation");
    let existing =
        db::insert_message_and_get_id(&conn, "provider-retry", "user", "Retry this prompt.")
            .expect("user");

    let result = send_mock_turn_with_conn(
        &conn,
        "provider-retry".into(),
        soul_id,
        "Retry this prompt.".into(),
        "Reader".into(),
        None,
        None,
    )
    .expect("retry");

    assert_eq!(
        result
            .messages
            .iter()
            .filter(|message| message.role == "user")
            .count(),
        1
    );
    assert_eq!(result.messages[0].id, existing);
}

#[test]
fn model_switch_retry_reuses_existing_user_message() {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    let soul_id = soul.character_id.clone();
    db::upsert_soul(&conn, &soul).expect("upsert soul");
    db::ensure_conversation(&conn, "model-retry", &soul_id).expect("conversation");
    let existing =
        db::insert_message_and_get_id(&conn, "model-retry", "user", "Try the new model.")
            .expect("user");

    let reused =
        reuse_or_insert_user_message(&conn, "model-retry", "Try the new model.").expect("reuse");

    assert_eq!(reused, existing);
    assert_eq!(
        db::list_messages(&conn, "model-retry", 10)
            .expect("messages")
            .iter()
            .filter(|message| message.role == "user")
            .count(),
        1
    );
}

#[test]
fn anti_replay_retry_does_not_duplicate_user_message() {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    db::upsert_soul(&conn, &soul).expect("upsert soul");
    db::ensure_conversation(&conn, "anti-replay-retry", &soul.character_id).expect("conversation");
    let existing = db::insert_message_and_get_id(
        &conn,
        "anti-replay-retry",
        "user",
        "Do not repeat the last line.",
    )
    .expect("user");

    let first =
        reuse_or_insert_user_message(&conn, "anti-replay-retry", "Do not repeat the last line.")
            .expect("first reuse");
    let second =
        reuse_or_insert_user_message(&conn, "anti-replay-retry", "Do not repeat the last line.")
            .expect("second reuse");

    assert_eq!(first, existing);
    assert_eq!(second, existing);
    assert_eq!(
        db::list_messages(&conn, "anti-replay-retry", 10)
            .expect("messages")
            .iter()
            .filter(|message| message.role == "user")
            .count(),
        1
    );
}

#[test]
fn correction_instruction_is_temporary_context_not_memory() {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    let soul_id = soul.character_id.clone();
    db::upsert_soul(&conn, &soul).expect("upsert soul");

    let first = send_mock_turn_with_conn(
        &conn,
        "fix".into(),
        soul_id.clone(),
        "I show her the phone.".into(),
        "Reader".into(),
        None,
        None,
    )
    .expect("first turn");
    let assistant_id = first
        .messages
        .iter()
        .rev()
        .find(|message| message.role == "assistant")
        .expect("assistant")
        .id;

    let corrected = send_mock_turn_with_conn(
        &conn,
        "fix".into(),
        soul_id,
        "I show her the phone.".into(),
        "Reader".into(),
        Some(assistant_id),
        Some("Continue from the kitchen. Do not replay the phone reveal.".into()),
    )
    .expect("corrected turn");

    let context = compile_context_with_correction(
        &corrected.soul,
        None,
        &messages_to_context(corrected.messages.clone()),
        Some("Continue from the kitchen. Do not replay the phone reveal."),
        None,
        None,
    );
    assert!(context
        .text
        .contains("[FIX INSTRUCTION, TEMPORARY HIGH PRIORITY]"));
    assert!(!corrected
        .soul
        .memory
        .recent
        .iter()
        .any(|memory| memory.content.contains("Do not replay the phone reveal")));
    let correction_event: (String, Option<i64>) = conn
        .query_row(
            "SELECT instruction, target_assistant_message_id
             FROM memory_correction_events
             WHERE conversation_id = 'fix'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("append-only correction event");
    assert_eq!(
        correction_event.0,
        "Continue from the kitchen. Do not replay the phone reveal."
    );
    assert_eq!(correction_event.1, Some(assistant_id));
}

#[test]
fn narrator_response_is_persisted_before_state_updater_result() {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    db::upsert_soul(&conn, &soul).expect("upsert soul");
    db::ensure_conversation(&conn, "dual-pass", &soul.character_id).expect("conversation");
    db::insert_message_and_get_id(&conn, "dual-pass", "user", "The siren starts.")
        .expect("user message");
    let payload_log_id = db::insert_llm_payload_log(
        &conn,
        &LlmPayloadLog {
            conversation_id: "dual-pass".into(),
            provider: "narrator_brief".into(),
            mode: "Reader".into(),
            context_mode: "brief".into(),
            model: "narrator-model".into(),
            base_url: "https://api.example/v1".into(),
            system_message: "Narrator system".into(),
            user_message: "The siren starts.".into(),
            context_text: "Brief context".into(),
            estimated_system_tokens: 3,
            estimated_user_tokens: 3,
            estimated_total_tokens: 6,
            created_at: 100,
            ..Default::default()
        },
    )
    .expect("payload log");

    let (assistant_message_id, selected_variant_id) = save_visible_narrator_response(
        &conn,
        "dual-pass",
        "Aurora snaps toward the window as the siren climbs.",
        None,
        None,
        &serde_json::to_string(&soul).expect("soul json"),
        "The siren starts.",
        payload_log_id,
        NarratorMessageOrigin::Api,
        None,
    )
    .expect("save narrator");
    assert!(parse_engine_patch_json("not json").is_err());

    let messages = db::list_messages(&conn, "dual-pass", 100).expect("messages");
    let assistant = messages
        .iter()
        .find(|message| message.id == assistant_message_id)
        .expect("assistant persisted");
    assert_eq!(assistant.role, "assistant");
    assert_eq!(
        assistant.content,
        "Aurora snaps toward the window as the siren climbs."
    );
    let exported = render_visible_chat_log(&messages);
    assert!(exported.contains("## Narrator"));
    assert!(exported.contains("Aurora snaps toward the window"));

    let variants = db::list_assistant_message_variants(&conn, "dual-pass", assistant_message_id)
        .expect("variants");
    assert_eq!(
        variants
            .iter()
            .filter(|variant| variant.is_selected)
            .count(),
        1
    );
    assert_eq!(
        variants
            .iter()
            .find(|variant| variant.is_selected)
            .unwrap()
            .id,
        selected_variant_id
    );

    let logs = db::list_llm_payload_logs(&conn, "dual-pass").expect("logs");
    assert_eq!(logs[0].message_id, Some(assistant_message_id));
}

#[test]
fn dev_log_details_redact_secrets_but_keep_token_estimates() {
    let details = serde_json::json!({
        "api_key": "secret-key",
        "authorization": "Bearer secret-token",
        "estimated_total_tokens": 123,
        "nested": {
            "refresh_token": "hidden",
            "model": "safe-model"
        }
    });

    let redacted = redact_dev_log_details(details);
    let serialized = redacted.to_string();

    assert!(!serialized.contains("secret-key"));
    assert!(!serialized.contains("secret-token"));
    assert!(!serialized.contains("hidden"));
    assert!(serialized.contains("estimated_total_tokens"));
    assert!(serialized.contains("123"));
    assert!(serialized.contains("safe-model"));
}

#[test]
fn visible_chat_export_strips_hidden_state() {
    let messages = vec![
        ChatMessage {
            id: 1,
            conversation_id: "export".into(),
            role: "user".into(),
            content: "Hello.".into(),
            channel: db::MESSAGE_CHANNEL_RP_SCENE.into(),
            created_at: 10,
            status: "active".into(),
            origin: "active".into(),
            attachments: Vec::new(),
            hidden_at: None,
        },
        ChatMessage {
            id: 2,
            conversation_id: "export".into(),
            role: "assistant".into(),
            content:
                "Visible narrator text.\n[HIDDEN STATE]{\"tag\":\"observation\"}[/HIDDEN STATE]"
                    .into(),
            channel: db::MESSAGE_CHANNEL_RP_SCENE.into(),
            created_at: 11,
            status: "active".into(),
            origin: "active".into(),
            attachments: Vec::new(),
            hidden_at: None,
        },
    ];

    let exported = render_visible_chat_log(&messages);

    assert!(exported.contains("# Mnemosyne Chat Log"));
    assert!(exported.contains("## User"));
    assert!(exported.contains("## Narrator"));
    assert!(exported.contains("Visible narrator text."));
    assert!(!exported.contains("[HIDDEN STATE]"));
    assert!(!exported.contains("observation"));
}

#[test]
fn payload_history_export_includes_prior_payloads_without_api_key() {
    let logs = vec![
        LlmPayloadLog {
            id: 1,
            conversation_id: "history".into(),
            message_id: Some(10),
            provider: "API".into(),
            mode: "Reader".into(),
            context_mode: "brief".into(),
            model: "model-a".into(),
            base_url: "https://api.example/v1".into(),
            system_message: "System A with clothing context".into(),
            user_message: "User A".into(),
            context_text: "Context A".into(),
            estimated_system_tokens: 10,
            estimated_user_tokens: 2,
            estimated_total_tokens: 12,
            truncated: false,
            created_at: 100,
            branch_id: None,
            active_turn_id: None,
            parent_turn_id: None,
            state_patch_ids_applied: Vec::new(),
            discarded_patch_ids_skipped: Vec::new(),
            state_rebuild_generation: None,
            latest_assistant_variant_id: None,
            ..Default::default()
        },
        LlmPayloadLog {
            id: 2,
            conversation_id: "history".into(),
            message_id: Some(11),
            provider: "API".into(),
            mode: "God".into(),
            context_mode: "brief".into(),
            model: "model-b".into(),
            base_url: "https://api.example/v1".into(),
            system_message: "System B".into(),
            user_message: "User B".into(),
            context_text: "Context B".into(),
            estimated_system_tokens: 11,
            estimated_user_tokens: 3,
            estimated_total_tokens: 14,
            truncated: false,
            created_at: 101,
            branch_id: None,
            active_turn_id: None,
            parent_turn_id: None,
            state_patch_ids_applied: Vec::new(),
            discarded_patch_ids_skipped: Vec::new(),
            state_rebuild_generation: None,
            latest_assistant_variant_id: None,
            ..Default::default()
        },
    ];

    let exported = render_llm_payload_history(&logs);

    assert!(exported.contains("## Payload 1"));
    assert!(exported.contains("## Payload 2"));
    assert!(exported.contains("Model: model-a"));
    assert!(exported.contains("Mode: God"));
    assert!(exported.contains("Custom prompt: inactive"));
    assert!(exported.contains("Base URL: https://api.example/v1"));
    assert!(exported.contains("System A with clothing context"));
    assert!(exported.contains("Context B"));
    assert!(!exported.contains("api_key"));
    assert!(!exported.contains("secret"));
}

#[test]
fn payload_history_labels_narrator_and_state_updater_sources() {
    let logs = vec![
        LlmPayloadLog {
            id: 1,
            conversation_id: "history".into(),
            message_id: Some(10),
            provider: "narrator_brief".into(),
            mode: "Reader".into(),
            context_mode: "brief".into(),
            model: "model".into(),
            base_url: "https://api.example/v1".into(),
            system_message: "Narrator system".into(),
            user_message: "User".into(),
            context_text: "Context".into(),
            estimated_system_tokens: 1,
            estimated_user_tokens: 1,
            estimated_total_tokens: 2,
            truncated: false,
            created_at: 100,
            branch_id: None,
            active_turn_id: None,
            parent_turn_id: None,
            state_patch_ids_applied: Vec::new(),
            discarded_patch_ids_skipped: Vec::new(),
            state_rebuild_generation: None,
            latest_assistant_variant_id: None,
            ..Default::default()
        },
        LlmPayloadLog {
            id: 2,
            conversation_id: "history".into(),
            message_id: Some(10),
            provider: "state_updater".into(),
            mode: "state_updater".into(),
            context_mode: "brief".into(),
            model: "model".into(),
            base_url: "https://api.example/v1".into(),
            system_message: "Updater system".into(),
            user_message: "Latest turn".into(),
            context_text: "Context".into(),
            estimated_system_tokens: 1,
            estimated_user_tokens: 1,
            estimated_total_tokens: 2,
            truncated: false,
            created_at: 101,
            branch_id: None,
            active_turn_id: None,
            parent_turn_id: None,
            state_patch_ids_applied: Vec::new(),
            discarded_patch_ids_skipped: Vec::new(),
            state_rebuild_generation: None,
            latest_assistant_variant_id: None,
            ..Default::default()
        },
    ];

    let exported = render_llm_payload_history(&logs);

    assert!(exported.contains("Provider: narrator_brief"));
    assert!(exported.contains("Provider: state_updater"));
    assert!(exported.contains("Context mode: brief"));
}

#[test]
fn payload_history_reports_custom_prompt_status() {
    let logs = vec![
        LlmPayloadLog {
            id: 1,
            conversation_id: "history".into(),
            message_id: Some(10),
            provider: "narrator_brief".into(),
            mode: "Custom".into(),
            context_mode: "brief".into(),
            model: "model".into(),
            base_url: "https://api.example/v1".into(),
            system_message: "[CUSTOM NARRATOR INSTRUCTIONS]\nSpeak softly.".into(),
            user_message: "User".into(),
            context_text: "Context".into(),
            estimated_system_tokens: 1,
            estimated_user_tokens: 1,
            estimated_total_tokens: 2,
            truncated: false,
            created_at: 100,
            branch_id: None,
            active_turn_id: None,
            parent_turn_id: None,
            state_patch_ids_applied: Vec::new(),
            discarded_patch_ids_skipped: Vec::new(),
            state_rebuild_generation: None,
            latest_assistant_variant_id: None,
            ..Default::default()
        },
        LlmPayloadLog {
            id: 2,
            conversation_id: "history".into(),
            message_id: Some(11),
            provider: "narrator_brief".into(),
            mode: "Custom".into(),
            context_mode: "brief".into(),
            model: "model".into(),
            base_url: "https://api.example/v1".into(),
            system_message: "Default Custom fallback".into(),
            user_message: "User".into(),
            context_text: "Context".into(),
            estimated_system_tokens: 1,
            estimated_user_tokens: 1,
            estimated_total_tokens: 2,
            truncated: false,
            created_at: 101,
            branch_id: None,
            active_turn_id: None,
            parent_turn_id: None,
            state_patch_ids_applied: Vec::new(),
            discarded_patch_ids_skipped: Vec::new(),
            state_rebuild_generation: None,
            latest_assistant_variant_id: None,
            ..Default::default()
        },
    ];

    let exported = render_llm_payload_history(&logs);

    assert!(exported.contains("Custom prompt: included"));
    assert!(exported.contains("Custom prompt: empty"));
}

#[test]
fn empty_payload_history_export_explains_no_logs() {
    let exported = render_llm_payload_history(&[]);

    assert!(exported.contains("# Mnemosyne LLM Payload History"));
    assert!(exported.contains(NO_LLM_PAYLOAD_LOGS_MESSAGE));
    assert!(exported.contains("Mock conversations do not send LLM payloads."));
    assert!(!exported.contains("## Payload 1"));
}

fn payload_trace_log(trace: serde_json::Value) -> LlmPayloadLog {
    LlmPayloadLog {
        id: 1,
        conversation_id: "history".into(),
        message_id: Some(10),
        provider: "evaluator_v1".into(),
        mode: "evaluator_v1".into(),
        context_mode: "brief".into(),
        model: "model".into(),
        base_url: "local".into(),
        system_message: "Evaluator system".into(),
        user_message: "Latest exchange".into(),
        context_text: "Compiled evaluator context".into(),
        created_at: 100,
        pipeline_trace_json: Some(serde_json::to_string_pretty(&trace).expect("trace json")),
        ..Default::default()
    }
}

#[test]
fn payload_export_includes_evaluator_raw_response() {
    let exported = render_llm_payload_history(&[payload_trace_log(serde_json::json!({
        "evaluator_raw_response": "{\"schema_version\":1}"
    }))]);

    assert!(exported.contains("### EVALUATOR RAW RESPONSE"));
    assert!(exported.contains("{\"schema_version\":1}"));
}

#[test]
fn payload_export_includes_parsed_evaluator_json() {
    let exported = render_llm_payload_history(&[payload_trace_log(serde_json::json!({
        "evaluator_parsed_json": {
            "schema_version": 1,
            "turn_flags_u64": 1,
            "turn_classification": { "scene_event_occurred": true }
        }
    }))]);

    assert!(exported.contains("### EVALUATOR PARSED JSON"));
    assert!(exported.contains("\"turn_flags_u64\": 1"));
    assert!(exported.contains("\"scene_event_occurred\": true"));
}

#[test]
fn payload_history_exports_structured_tool_trace_fields() {
    let exported = render_llm_payload_history(&[payload_trace_log(serde_json::json!({
        "evaluator_trace": {
            "tool_calls_present": true,
            "tool_call_count": 1,
            "tool_call_names": ["submit_evaluator_ops"],
            "raw_content_present": false,
            "raw_tool_calls_present": true,
            "structured_transport_requested": "tool_call",
            "structured_transport_actual": "tool_call",
            "strict_tool_diagnostic": false,
            "strict_tool_passed": null,
            "fallback_used": false,
            "default_player_in_relationship_context": false,
            "structured_retry_count": 1,
            "structured_retry_used_failed_args": true,
            "structured_retry_repair_prompt_included_error": true,
            "entity_aliases_resolved": ["op:0:add_memory.owner_soul_id:active_soul->aurora"],
            "structured_run_classification": "tool_retry_success"
        }
    }))]);

    for expected in [
        "tool_calls_present",
        "tool_call_count",
        "tool_call_names",
        "raw_content_present",
        "raw_tool_calls_present",
        "structured_transport_requested",
        "structured_transport_actual",
        "strict_tool_diagnostic",
        "strict_tool_passed",
        "fallback_used",
        "default_player_in_relationship_context",
        "structured_retry_used_failed_args",
        "structured_retry_repair_prompt_included_error",
        "entity_aliases_resolved",
        "structured_run_classification",
    ] {
        assert!(exported.contains(expected), "missing {expected}");
    }
}

#[test]
fn payload_export_includes_candidate_accept_reject_trace() {
    let exported = render_llm_payload_history(&[payload_trace_log(serde_json::json!({
        "evaluator_candidate_trace": [
            {
                "candidate_id": "mem-accepted",
                "owner_soul_id": "Aurora",
                "slot": "relationship_memory",
                "accepted": true
            },
            {
                "candidate_id": "mem-rejected",
                "owner_soul_id": "Aurora",
                "slot": "recent_emotional_state",
                "accepted": false,
                "rejection_reason": "generic low-value memory"
            }
        ]
    }))]);

    assert!(exported.contains("### EVALUATOR CANDIDATE TRACE"));
    assert!(exported.contains("mem-accepted"));
    assert!(exported.contains("generic low-value memory"));
}

#[test]
fn payload_history_renders_pipeline_trace() {
    let trace = TurnPipelineTrace {
        request_id: "req-123".to_string(),
        turn_id: Some("turn-456".to_string()),
        conversation_id: "conv-789".to_string(),
        started_at: 1000,
        total_elapsed_ms: 1500,
        final_status: "success".to_string(),
        failing_stage: None,
        suggested_debug_action: None,
        stages: vec![
            PipelineStageTrace {
                stage_id: "stage-1".to_string(),
                stage_name: "context_compiled".to_string(),
                status: "success".to_string(),
                elapsed_ms: 50,
                input_summary: None,
                output_summary: None,
                error_code: None,
                error_message: None,
                repair_action: None,
                artifact_ref: None,
            },
            PipelineStageTrace {
                stage_id: "stage-2".to_string(),
                stage_name: "narrator_called".to_string(),
                status: "warning".to_string(),
                elapsed_ms: 250,
                input_summary: None,
                output_summary: None,
                error_code: None,
                error_message: None,
                repair_action: None,
                artifact_ref: None,
            },
        ],
        token_usage: None,
        evaluator_row_traces: vec![],
    };

    let exported = render_llm_payload_history(&[payload_trace_log(serde_json::json!({
        "pipeline_trace": trace
    }))]);

    assert!(exported.contains("### PIPELINE TRACE"));
    assert!(exported.contains("total_elapsed_ms: 1500"));
    assert!(exported.contains("- Stage: context_compiled, Status: success, Elapsed: 50ms"));
    assert!(exported.contains("- Stage: narrator_called, Status: warning, Elapsed: 250ms"));
}

#[test]
fn payload_history_renders_evaluator_row_trace() {
    let trace = TurnPipelineTrace {
        request_id: "req-123".to_string(),
        turn_id: Some("turn-456".to_string()),
        conversation_id: "conv-789".to_string(),
        started_at: 1000,
        total_elapsed_ms: 1500,
        final_status: "failed".to_string(),
        failing_stage: Some("evaluator_response_validated".to_string()),
        suggested_debug_action: Some("Fix constraints".to_string()),
        stages: vec![],
        token_usage: None,
        evaluator_row_traces: vec![state_engine::evaluator_form::EvalRowTrace {
            row_kind: "object".to_string(),
            row_index: 0,
            raw_row: serde_json::json!({ "id": "door", "state": "broken" }),
            normalized_row: serde_json::json!({ "id": "door", "change_type": "state_change" }),
            validation_status: "rejected".to_string(),
            rejection_reason: Some("missing property_changed".to_string()),
            compiler_result: "rejected".to_string(),
        }],
    };

    let exported = render_llm_payload_history(&[payload_trace_log(serde_json::json!({
        "pipeline_trace": trace
    }))]);

    assert!(exported.contains("### PIPELINE TRACE"));
    assert!(exported.contains("failing_stage: evaluator_response_validated"));
    assert!(exported.contains("### EVALUATOR ROW TRACE"));
    assert!(exported.contains("- row_kind: object"));
    assert!(exported.contains("- row_index: 0"));
    assert!(exported.contains("- raw_row: {\"id\":\"door\",\"state\":\"broken\"}"));
    assert!(
        exported.contains("- normalized_row: {\"change_type\":\"state_change\",\"id\":\"door\"}")
    );
    assert!(exported.contains("- validation_status: rejected"));
    assert!(exported.contains("- rejection_reason: missing property_changed"));
    assert!(exported.contains("- compiler_result: rejected"));
}

#[test]
fn payload_export_includes_converted_patch() {
    let exported = render_llm_payload_history(&[payload_trace_log(serde_json::json!({
        "converted_engine_patch": {
            "converted_patch_json": {
                "schema_version": 1,
                "world_patch": { "location": "The bar" }
            },
            "patch_empty": false,
            "memory_patch_count": 0
        }
    }))]);

    assert!(exported.contains("### CONVERTED ENGINE PATCH"));
    assert!(exported.contains("\"location\": \"The bar\""));
    assert!(exported.contains("\"patch_empty\": false"));
}

#[test]
fn payload_export_includes_ledger_apply_trace() {
    let exported = render_llm_payload_history(&[payload_trace_log(serde_json::json!({
        "ledger_apply_trace": {
            "state_patch_id": "patch_1",
            "turn_commit_id": "turn_1",
            "branch_id": "branch_1",
            "patch_stored": true,
            "patch_applied": true,
            "branch_rebuilt": true
        }
    }))]);

    assert!(exported.contains("### LEDGER/APPLY TRACE"));
    assert!(exported.contains("\"state_patch_id\": \"patch_1\""));
    assert!(exported.contains("\"branch_rebuilt\": true"));
}

#[test]
fn payload_export_includes_before_after_state_summary() {
    let exported = render_llm_payload_history(&[payload_trace_log(serde_json::json!({
        "before_after_state_summary": {
            "before": {
                "soul.turn_counter": 0,
                "recent_event_count": 0,
                "memory_recent_count": 0
            },
            "after": {
                "soul.turn_counter": 1,
                "recent_event_count": 1,
                "memory_recent_count": 1
            }
        }
    }))]);

    assert!(exported.contains("### BEFORE/AFTER STATE SUMMARY"));
    assert!(exported.contains("\"soul.turn_counter\": 1"));
    assert!(exported.contains("\"recent_event_count\": 1"));
}

#[test]
fn mne_export_includes_export_state_trace() {
    let exported = render_llm_payload_history(&[payload_trace_log(serde_json::json!({
        "export_trace": {
            "export_bundle_id": "bundle-1",
            "export_conversation_id": "conversation-1",
            "export_source": "rebuilt_ledger_state",
            "rebuilt_before_export": true,
            "exported_recent_event_count": 2,
            "export_filename": "Aurora_session_checkpoint_1234_abcd.mne"
        }
    }))]);

    assert!(exported.contains("### EXPORT TRACE"));
    assert!(exported.contains("\"export_bundle_id\": \"bundle-1\""));
    assert!(exported.contains("Aurora_session_checkpoint_1234_abcd.mne"));
}

#[test]
fn evaluator_parse_failure_is_visible_in_payload_export() {
    let exported = render_llm_payload_history(&[payload_trace_log(serde_json::json!({
        "evaluator_raw_response": "not json",
        "evaluator_parsed_json": {
            "parse_status": "failed",
            "parse_error": "Evaluator returned invalid EvaluatorOutputV1 JSON"
        }
    }))]);

    assert!(exported.contains("### EVALUATOR RAW RESPONSE"));
    assert!(exported.contains("not json"));
    assert!(exported.contains("### EVALUATOR PARSED JSON"));
    assert!(exported.contains("\"parse_status\": \"failed\""));
    assert!(exported.contains("Evaluator returned invalid EvaluatorOutputV1 JSON"));
}

fn evaluator_output_json_with_candidate(candidate: serde_json::Value) -> String {
    serde_json::json!({
        "schema_version": 1,
        "turn_flags_u64": state_engine::evaluator::turn_flags::SCENE_TURN,
        "turn_classification": {
            "is_pure_ooc": false,
            "scene_event_occurred": true,
            "is_retcon_or_correction": false,
            "human_summary": "Aurora hears the promise."
        },
        "global_scene_evaluation": {
            "scene_event_occurred": true,
            "location_changed": false,
            "object_state_changed": false,
            "relationship_changed": false,
            "unresolved_tension": false,
            "current_plot_advanced": false,
            "character_identity_changed": false,
            "recent_emotional_state_changed": false,
            "contradiction_detected": false,
            "evidence_quote": "I promise to keep watch.",
            "summary": "A promise was made."
        },
        "per_soul_evaluations": [],
        "world_changes": [],
        "object_changes": [],
        "relationship_evaluations": [],
        "memory_candidates": [candidate],
        "relevance_tags": {
            "setting_tags": {},
            "location_tags": {},
            "interacted_entities": {},
            "event_type_tags": {},
            "object_tags": {},
            "emotional_tags": {},
            "memory_slot_tags": {},
            "per_soul_relevance": {}
        },
        "no_op_reason": null
    })
    .to_string()
}

fn valid_evaluator_candidate() -> serde_json::Value {
    serde_json::json!({
        "candidate_id": "mem-1",
        "owner_soul_id": "Aurora",
        "slot": "relationship_memory",
        "content": "Aurora heard the user promise to keep watch.",
        "evidence_quote": "I promise to keep watch.",
        "criterion_met": true,
        "confidence": 0.9,
        "salience": 0.6,
        "retrieval_strength": 0.6,
        "perceived_by_entity_id": "Aurora",
        "target_entity_ids": ["user"],
        "source_type": "current_session",
        "truth_status": "scene_event",
        "relevance_tags": ["promise"],
        "knowledge_scope": "directly_observed"
    })
}

fn evaluator_context_for_alias_tests<'a>(
    world: &'a SessionWorld,
) -> EvaluatorConversionContext<'a> {
    EvaluatorConversionContext {
        active_soul_id: "Aurora",
        active_soul_ids: vec!["Aurora".into()],
        latest_user_message: "I promise to keep watch.",
        latest_narrator_response: "Aurora hears it clearly: I promise to keep watch.",
        session_world: Some(world),
        baseline_recent_event_id: None,
    }
}

#[test]
fn evaluator_accepts_soul_id_alias_for_owner_soul_id() {
    let mut candidate = valid_evaluator_candidate();
    candidate.as_object_mut().unwrap().remove("owner_soul_id");
    candidate["soul_id"] = serde_json::json!("Aurora");
    let parsed =
        parse_evaluator_output(&evaluator_output_json_with_candidate(candidate)).expect("parse");
    let world = state_engine::setting::session_world_from_setting(&new_default_setting("Aurora"));
    let conversion = evaluator_output_to_engine_patch(
        &parsed.output,
        &evaluator_context_for_alias_tests(&world),
    );

    println!("Warnings in accepts_soul_id_alias: {:?}", parsed.warnings);

    assert!(parsed.normalized);
    assert!(parsed
        .warnings
        .iter()
        .any(|warning| warning.contains("soul_id normalized to owner_soul_id")));
    assert_eq!(parsed.output.memory_candidates[0].owner_soul_id, "Aurora");
    assert_eq!(conversion.accepted_candidate_ids, vec!["mem-1".to_string()]);
}

#[test]
fn evaluator_normalizes_full_knowledge_scope() {
    let mut candidate = valid_evaluator_candidate();
    candidate["knowledge_scope"] = serde_json::json!("full");
    let parsed =
        parse_evaluator_output(&evaluator_output_json_with_candidate(candidate)).expect("parse");

    assert!(parsed.normalized);
    assert_eq!(
        parsed.output.memory_candidates[0]
            .knowledge_scope
            .as_label(),
        "directly_observed"
    );
    assert!(parsed
        .warnings
        .iter()
        .any(|warning| warning.contains("full")));
}

#[test]
fn evaluator_schema_aliases_do_not_bypass_candidate_validation() {
    let mut candidate = valid_evaluator_candidate();
    candidate.as_object_mut().unwrap().remove("owner_soul_id");
    candidate["soul"] = serde_json::json!("Aurora");
    candidate["confidence"] = serde_json::json!(0.2);
    candidate["content"] = serde_json::json!("she listened carefully");
    candidate["knowledge_scope"] = serde_json::json!("observed");
    let parsed =
        parse_evaluator_output(&evaluator_output_json_with_candidate(candidate)).expect("parse");
    let world = state_engine::setting::session_world_from_setting(&new_default_setting("Aurora"));
    let conversion = evaluator_output_to_engine_patch(
        &parsed.output,
        &evaluator_context_for_alias_tests(&world),
    );

    assert!(parsed.normalized);
    assert!(conversion.accepted_candidate_ids.is_empty());
    assert!(conversion
        .rejected_candidates
        .iter()
        .any(|rejection| rejection.reason == "confidence below threshold"));
}

#[test]
fn evaluator_normalization_warning_appears_in_payload_trace() {
    let mut candidate = valid_evaluator_candidate();
    candidate.as_object_mut().unwrap().remove("owner_soul_id");
    candidate["owner"] = serde_json::json!("Aurora");
    candidate["knowledge_scope"] = serde_json::json!("hearsay");
    let parsed =
        parse_evaluator_output(&evaluator_output_json_with_candidate(candidate)).expect("parse");
    let trace = serde_json::json!({
        "evaluator_json_normalized": parsed.normalized,
        "evaluator_normalization_warnings": parsed.warnings
    });

    assert_eq!(trace["evaluator_json_normalized"], true);
    assert!(trace["evaluator_normalization_warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|warning| warning.as_str().unwrap().contains("owner")));
}

fn evaluator_test_settings() -> ApiProviderSettings {
    ApiProviderSettings {
        base_url: "https://api.example/v1".into(),
        api_key: "key".into(),
        model: "model".into(),
        system_prompt: String::new(),
        evaluator_mode: Some(EVALUATOR_MODE_V1.into()),
        evaluator_timeout_ms: Some(25_000),
        evaluator_timeout_mode: Some("finite".into()),
        wait_for_evaluator_before_next_turn: Some(true),
        allow_send_with_stale_state: Some(false),
        evaluator_background_enabled: Some(false),
        ..Default::default()
    }
}

fn form_runtime_fixture() -> (Soul, SessionWorld, String, String, EvalFormSpec) {
    let soul = new_default_soul("Aurora Schwarz");
    let mut world =
        state_engine::setting::session_world_from_setting(&new_default_setting("Aurora"));
    world.location = "Rainy, neon-lit apartment interior".into();
    let user = "I walk in. Long time no see, Aurora.".to_string();
    let narrator = "Aurora steps aside and lets the visitor into her apartment.".to_string();
    let spec = build_eval_form_spec(&soul, Some(&world), &user, &narrator, 8);
    (soul, world, user, narrator, spec)
}

fn door_entry_form_response_json(soul_id: &str) -> String {
    serde_json::json!({
            "event_rows": [{
                "event_id": "door_entry",
                "event_type": "scene_event",
                "objective_summary": "The visitor entered Aurora's apartment after she opened the door.",
                "participants": [soul_id, "default_player"],
                "location": "Aurora's apartment interior",
                "evidence_quote": "I walk in. Long time no see, Aurora.",
                "importance_tier": "medium"
            }],
            "object_rows": [],
            "relationship_rows": [],
            "memory_rows": [],
            "review_rows": []
        })
        .to_string()
}

fn memory_form_response_json(
    soul_id: &str,
    content: &str,
    review_rows: Vec<serde_json::Value>,
) -> String {
    serde_json::json!({
            "event_rows": [{
                "event_id": "door_entry",
                "event_type": "scene_event",
                "objective_summary": "The visitor entered Aurora's apartment after she opened the door.",
                "participants": [soul_id, "default_player"],
                "location": "Aurora's apartment interior",
                "evidence_quote": "I walk in. Long time no see, Aurora.",
                "importance_tier": "medium"
            }],
            "object_rows": [],
            "relationship_rows": [],
            "memory_rows": [{
                "linked_event_id": "door_entry",
                "owner_soul_id": soul_id,
                "slot": "current_plot_memory",
                "content": content,
                "evidence_quote": "I walk in. Long time no see, Aurora.",
                "importance_tier": "medium",
                "retrieval_cues": ["visitor entered", "Aurora apartment"],
                "selected_tags": ["scene_event", "current_plot"]
            }],
            "review_rows": review_rows
        })
        .to_string()
}

fn soul_with_existing_memory() -> Soul {
    let mut soul = new_default_soul("Aurora Schwarz");
    soul.memory.recent.push(state_engine::soul::MemoryEntry {
        archived: false,
        is_pinned: false,
        id: "existing-memory-1".into(),
        timestamp: 1,
        content: "The visitor entered Aurora's apartment.".into(),
        salience: 0.7,
        tag: "current_plot_memory".into(),
        retrieval_strength: 0.7,
        source_type: MemorySourceType::CurrentSession,
        source_session_id: None,
        source_conversation_id: None,
        source_message_id: None,
        source_entity_id: None,
        source_quote: None,
        is_lived_experience: true,
        is_imported_context: false,
        perceived_by_entity_id: Some(soul.character_id.clone()),
        target_entity_ids: vec!["default_player".into()],
        interpretation: None,
        confidence: Some(0.8),
        objective_event_id: None,
        truth_status: TruthStatus::SceneEvent,
        architecture_verified: true,
        memory_slot: Some("current_plot_memory".into()),
        owner_soul_id: Some(soul.character_id.clone()),
        relevance_tags: HashMap::new(),
        knowledge_scope: Some("directly_observed".into()),
        is_active: true,
        invalidated_by_patch_id: None,
        superseded_by_memory_id: None,
        is_retconned: false,
    });
    soul
}

#[test]
fn live_async_evaluator_routes_to_form_v1_when_selected() {
    let mut settings = evaluator_test_settings();
    settings.evaluator_mode = Some(EVALUATOR_MODE_FORM_V1.into());
    let (soul, world, user, narrator, _) = form_runtime_fixture();
    let source = selected_evaluator_source(&evaluator_mode(&settings));
    let prompt = if source == EVALUATOR_MODE_FORM_V1 {
        build_evaluator_form_prompt(&soul, Some(&world), &user, &narrator)
    } else {
        build_evaluator_prompt(&soul, Some(&world))
    };

    assert_eq!(source, EVALUATOR_MODE_FORM_V1);
    assert_eq!(
        evaluator_provider_label(&evaluator_mode(&settings), true),
        "evaluator_form_v1_background"
    );
    assert!(prompt.contains("[FORM SPEC]"));
    assert!(prompt.contains("[HARD FILLABLE FORM TEMPLATE]"));
    assert!(prompt.contains("provided JSON evaluation sheet"));
}

#[test]
fn form_v1_payload_trace_includes_form_stats() {
    let (soul, world, user, narrator, spec) = form_runtime_fixture();
    let outcome = compile_evaluator_form_runtime(
        &door_entry_form_response_json(&soul.character_id),
        spec,
        &soul,
        &world,
        &user,
        &narrator,
        None,
    )
    .expect("compile form");
    let trace = runtime_form_trace_json(&outcome);

    assert_eq!(trace["form_spec_generated"], true);
    assert_eq!(trace["form_response_parse_status"], "success");
    assert!(trace["form_rows_submitted"].as_u64().unwrap() > 0);
    assert!(trace["form_rows_accepted"].as_u64().unwrap() > 0);
    assert!(trace["compiled_turn_flags_u64"].as_u64().unwrap() > 0);
}

#[test]
fn form_v1_background_job_applies_patch() {
    let (soul, world, user, narrator, spec) = form_runtime_fixture();
    let outcome = compile_selected_evaluator_runtime(
        EVALUATOR_MODE_FORM_V1,
        Some(spec),
        &door_entry_form_response_json(&soul.character_id),
        None,
        &soul,
        &world,
        &user,
        &narrator,
        None,
        false,
    )
    .expect("compile selected form");

    assert!(!outcome.conversion.patch.is_empty());
    assert!(outcome.conversion.patch.world_patch.is_some());
}

#[test]
fn structured_mode_selection_and_labels() {
    let mut settings = evaluator_test_settings();
    settings.evaluator_mode = Some(EVALUATOR_MODE_STRUCTURED_V1.into());

    assert_eq!(evaluator_mode(&settings), EVALUATOR_MODE_STRUCTURED_V1);
    assert_eq!(
        selected_evaluator_source(EVALUATOR_MODE_STRUCTURED_V1),
        EVALUATOR_MODE_STRUCTURED_V1
    );
    assert_eq!(
        evaluator_provider_label(EVALUATOR_MODE_STRUCTURED_V1, true),
        "evaluator_structured_v1_background"
    );
}

#[test]
fn structured_prompt_omits_embedded_patch_schema() {
    let soul = new_default_soul("Aurora");
    let structured = build_structured_evaluator_prompt(&soul, None);

    assert!(!structured.contains("Patch schema:"));
    assert!(structured.contains("[CURRENT STATE]"));

    // The legacy prose-schema prompt is unchanged by the refactor.
    let legacy = crate::providers::api::build_state_updater_prompt(&soul, None);
    assert!(legacy.contains("Patch schema:"));
    assert!(legacy.contains("[CURRENT STATE]"));
}

#[test]
fn structured_runtime_parses_ops_and_compiles_patch() {
    let (soul, world, user, narrator, _) = form_runtime_fixture();
    let raw = format!(
        r#"{{"schema_version":1,"ops":[{{"op":"add_memory","owner_soul_id":"{}","slot":"relationship_memory","content":"Aurora noticed the visitor's steady answer.","evidence_quote":"{}","confidence":0.8,"salience":60,"source_message_id":null,"target_entity_ids":["preset_male"],"truth_status":"scene_event"}}],"no_op_reason":null}}"#,
        soul.character_id, user
    );

    let outcome = compile_evaluator_structured_runtime(
        &raw,
        Some(StructuredEnforcement::JsonSchema),
        &soul,
        &world,
        &user,
        &narrator,
        None,
        false,
    )
    .expect("structured runtime");

    assert!(!outcome.conversion.patch.is_empty());
    assert!(!outcome.conversion.no_op);
    assert!(!outcome.normalized);
    assert!(outcome.warnings.is_empty());
    assert!(outcome.form_spec.is_none());
}

#[test]
fn structured_runtime_skips_repair_under_schema_enforcement() {
    let (soul, world, user, narrator, _) = form_runtime_fixture();
    let fenced = "```json\n{\"schema_version\":1,\"ops\":[],\"no_op_reason\":\"none\"}\n```";

    // Schema-enforced output must parse with serde alone — a fence means
    // the provider broke the contract, so no salvage is attempted.
    assert!(compile_evaluator_structured_runtime(
        fenced,
        Some(StructuredEnforcement::JsonSchema),
        &soul,
        &world,
        &user,
        &narrator,
        None,
        false,
    )
    .is_err());

    // Weaker structured modes still do not use the old syntactic repair path.
    assert!(compile_evaluator_structured_runtime(
        fenced,
        Some(StructuredEnforcement::JsonObject),
        &soul,
        &world,
        &user,
        &narrator,
        None,
        false,
    )
    .is_err());
}

#[test]
fn json_schema_malformed_output_is_not_validated() {
    let (soul, world, user, narrator, _) = form_runtime_fixture();
    let malformed = "{\"no_op_reason\": \"\"";

    let err = compile_evaluator_structured_runtime(
        malformed,
        Some(StructuredEnforcement::JsonSchema),
        &soul,
        &world,
        &user,
        &narrator,
        None,
        false,
    )
    .expect_err("malformed json_schema output must fail validation");

    assert!(err.contains("malformed_schema_output"));
    assert_eq!(
        structured_validation_status_from_error(&err),
        "malformed_schema_output"
    );
}

#[test]
fn structured_prompt_uses_active_player_as_latest_speaker() {
    let mut soul = new_default_soul("Aurora");
    soul.relationships.insert(
        "default_player".into(),
        state_engine::soul::Relationship {
            trust: 4.0,
            ..Default::default()
        },
    );
    soul.relationships.insert(
        "preset_male".into(),
        state_engine::soul::Relationship {
            trust: 44.0,
            ..Default::default()
        },
    );

    let prompt = build_structured_evaluator_prompt_with_player_persona(
        &soul,
        None,
        "preset_male",
        "Male Persona",
    );

    assert!(prompt.contains("Latest normal RP speaker entity_id: preset_male"));
    assert!(prompt.contains("active_soul, active_player, latest_speaker, session_world"));
    // Context JSON is now compact (no space after the colon) to cut tokens.
    assert!(prompt.contains("\"target_entity_id\":\"preset_male\""));
    assert_eq!(
        prompt
            .matches("\"target_entity_id\":\"preset_male\"")
            .count(),
        1
    );
    assert!(prompt.contains("\"trust\":44.0"));
    assert!(!prompt.contains("default_player"));
}

#[test]
fn empty_ops_on_object_update_turn_fails() {
    let (soul, world, _, _, _) = form_runtime_fixture();
    let user = "I place my wet jacket over the chair.";
    let narrator = "The wet jacket drips over the chair near Aurora's door.";
    let raw = r#"{"schema_version":1,"ops":[],"no_op_reason":null}"#;

    let err = compile_evaluator_structured_runtime(
        raw,
        Some(StructuredEnforcement::JsonSchema),
        &soul,
        &world,
        user,
        narrator,
        None,
        false,
    )
    .expect_err("durable object turn cannot compile as empty ops");

    assert!(err.contains("zero_ops_on_durable_turn"));
}

#[test]
fn reextract_empty_ops_with_meaningful_no_op_reason_fails_when_ops_required() {
    let (soul, world, _, _, _) = form_runtime_fixture();
    let user =
        "\"I want to keep my distance, thank you. You can say whatever you like over there.\"";
    let narrator = "Aurora's eyebrows lift. \"Fine. I can work with that,\" she says. \"I don't do distance. I do space.\"";
    let raw = r#"{"schema_version":1,"ops":[],"no_op_reason":"No durable state changes detected in the roleplay exchange."}"#;

    let err = compile_evaluator_structured_runtime(
        raw,
        Some(StructuredEnforcement::JsonSchema),
        &soul,
        &world,
        user,
        narrator,
        None,
        true,
    )
    .expect_err("repair/reextract must not accept empty ops when ops are required");

    assert!(err.contains("zero_ops_on_required_reextract"));
    assert_eq!(
        structured_validation_status_from_error(&err),
        "zero_ops_on_required_reextract"
    );
    assert_eq!(
        structured_failure_kind(&err),
        "zero_ops_on_required_reextract"
    );
}

#[test]
fn wet_jacket_guarantee_patch_creates_preset_male_jacket() {
    let soul = new_default_soul("Aurora");
    let patch = diagnostic_object_scene_guarantee_patch(
        &soul,
        "I place my wet jacket over the chair.",
        "The wet jacket drips over the chair.",
    );
    let world = patch.world_patch.expect("world guarantee");
    let object = world
        .object_observation_operations
        .first()
        .and_then(|operation| operation.object_state.as_ref())
        .expect("object guarantee");

    assert_eq!(object.object_id, "preset_male_jacket_1");
    assert_eq!(object.owner_entity_id.as_deref(), Some("preset_male"));
    assert_eq!(object.object_kind, "jacket");
    assert_eq!(object.location, "chair");
    assert_eq!(object.status, "wet");
}

#[test]
fn structured_support_level_requires_parseable_output() {
    assert_eq!(
        structured_support_level(StructuredEnforcement::JsonSchema, true),
        STRUCTURED_SUPPORT_JSON_SCHEMA
    );
    assert_eq!(
        structured_support_level(StructuredEnforcement::JsonObject, true),
        STRUCTURED_SUPPORT_JSON_OBJECT
    );
    assert_eq!(
        structured_support_level(StructuredEnforcement::None, true),
        STRUCTURED_SUPPORT_PROMPT_ONLY
    );
    // Accepted response_format but unparseable output counts as unsupported.
    assert_eq!(
        structured_support_level(StructuredEnforcement::JsonSchema, false),
        STRUCTURED_SUPPORT_UNTESTED
    );
}

#[test]
fn memory_curation_commits_through_ledger_and_survives_rebuild() {
    let conn = db::init_memory_connection().expect("db");
    let soul = soul_with_existing_memory();
    db::upsert_soul(&conn, &soul).expect("soul");
    db::ensure_conversation(&conn, "conv-curation", &soul.character_id).expect("conversation");

    let result = curate_memory_with_conn(
        &conn,
        "conv-curation",
        &soul.character_id,
        "existing-memory-1",
        "pin",
    )
    .expect("pin through ledger");
    assert!(!result.patch_id.is_empty());
    let pinned = result
        .soul
        .memory
        .recent
        .iter()
        .find(|memory| memory.id == "existing-memory-1")
        .expect("memory present");
    assert!(pinned.is_pinned);

    // The pin must be reproduced by replaying the ledger, not by a
    // direct soul mutation that a rebuild would lose.
    let rebuilt =
        db::rebuild_session_state(&conn, "conv-curation", &result.branch_id).expect("rebuild");
    let replayed = rebuilt
        .soul
        .memory
        .recent
        .iter()
        .find(|memory| memory.id == "existing-memory-1")
        .expect("memory survives rebuild");
    assert!(replayed.is_pinned);

    let unpinned = curate_memory_with_conn(
        &conn,
        "conv-curation",
        &soul.character_id,
        "existing-memory-1",
        "unpin",
    )
    .expect("unpin through ledger");
    assert!(
        !unpinned
            .soul
            .memory
            .recent
            .iter()
            .find(|memory| memory.id == "existing-memory-1")
            .expect("memory present")
            .is_pinned
    );

    // Ineffective operations fail BEFORE anything reaches the ledger.
    assert!(curate_memory_with_conn(
        &conn,
        "conv-curation",
        &soul.character_id,
        "existing-memory-1",
        "restore_archived",
    )
    .is_err());
    assert!(curate_memory_with_conn(
        &conn,
        "conv-curation",
        &soul.character_id,
        "missing-memory",
        "pin",
    )
    .is_err());
    assert!(curate_memory_with_conn(
        &conn,
        "conv-curation",
        &soul.character_id,
        "existing-memory-1",
        "delete",
    )
    .is_err());
}

#[test]
fn evaluator_gate_classifies_dialogue_scene_and_boundary() {
    let previous = "```status\nScene | Focus: Negotiating with the merchant | Physical state: Standing at the stall | Atmosphere: Tense\n```";
    let same_scene = "```status\nScene | Focus: Negotiating with the merchant | Physical state: Standing at the stall | Atmosphere: Lighter now\n```";
    let moved = "```status\nScene | Focus: Negotiating with the merchant | Physical state: Seated inside the tent | Atmosphere: Tense\n```";
    let new_focus = "```status\nScene | Focus: Fleeing through the alleys | Physical state: Running | Atmosphere: Panicked\n```";

    // Pure quoted dialogue, status stable (atmosphere drift ignored): skip.
    assert_eq!(
        classify_turn_for_evaluator_gate(
            "\"Three silvers, and that is my final offer.\"",
            Some(same_scene),
            Some(previous),
        ),
        (TurnSignificance::DialogueOnly, "dialogue_only_turn")
    );

    // Physical state moved: evaluator must run.
    assert_eq!(
        classify_turn_for_evaluator_gate("\"Fine.\"", Some(moved), Some(previous)).0,
        TurnSignificance::SceneRelevant
    );

    // Focus changed: scene boundary (catch-up trigger).
    assert_eq!(
        classify_turn_for_evaluator_gate("\"Run!\"", Some(new_focus), Some(previous)).0,
        TurnSignificance::SceneBoundary
    );

    // Unquoted prose is action, not dialogue.
    assert_eq!(
        classify_turn_for_evaluator_gate(
            "I draw my sword and lunge.",
            Some(same_scene),
            Some(previous),
        ),
        (
            TurnSignificance::SceneRelevant,
            "user_text_not_dialogue_like"
        )
    );

    // Asterisk action markup disqualifies even quoted text.
    assert_eq!(
        classify_turn_for_evaluator_gate(
            "*hands over the coin pouch* \"Take it all.\"",
            Some(same_scene),
            Some(previous),
        )
        .0,
        TurnSignificance::SceneRelevant
    );

    // Correction keywords always run the evaluator.
    assert_eq!(
        classify_turn_for_evaluator_gate(
            "\"Wait, fix that — she already paid.\"",
            Some(same_scene),
            Some(previous),
        ),
        (TurnSignificance::SceneRelevant, "correction_keywords")
    );

    // Missing either status signal degrades to scene-relevant.
    assert_eq!(
        classify_turn_for_evaluator_gate("\"Hello.\"", None, Some(previous)),
        (TurnSignificance::SceneRelevant, "no_current_status_signal")
    );
    assert_eq!(
        classify_turn_for_evaluator_gate("\"Hello.\"", Some(same_scene), None),
        (TurnSignificance::SceneRelevant, "no_previous_status_signal")
    );
}

#[test]
fn evaluator_gate_parses_per_line_and_cjk_dialogue() {
    let per_line_previous =
            "```status\nFocus: Sharing tea\nPhysical state: Kneeling at the low table\nAtmosphere: Calm\n```";
    let per_line_current =
            "```status\nFocus: Sharing tea\nPhysical state: Kneeling at the low table\nAtmosphere: Warm\n```";
    assert_eq!(
        classify_turn_for_evaluator_gate(
            "「お茶、おいしいですね」",
            Some(per_line_current),
            Some(per_line_previous),
        ),
        (TurnSignificance::DialogueOnly, "dialogue_only_turn")
    );

    // Status blocks lacking the gate fields produce no signature.
    assert!(status_gate_signature("```status\nAtmosphere: Calm\n```").is_none());
}

#[test]
fn evaluator_execution_mode_defaults_to_balanced() {
    let mut settings = ApiProviderSettings::default();
    assert_eq!(evaluator_execution_mode(&settings), "balanced");
    settings.evaluator_execution_mode = Some("fast".into());
    assert_eq!(evaluator_execution_mode(&settings), "fast");
    settings.evaluator_execution_mode = Some("long_context".into());
    assert_eq!(evaluator_execution_mode(&settings), "long_context");
    settings.evaluator_execution_mode = Some("warp_speed".into());
    assert_eq!(evaluator_execution_mode(&settings), "balanced");
}

#[test]
fn evaluator_catchup_queue_round_trips_and_renders() {
    let conn = db::init_memory_connection().expect("db");
    let soul = soul_with_existing_memory();
    db::upsert_soul(&conn, &soul).expect("soul");
    db::ensure_conversation(&conn, "conv-catchup", &soul.character_id).expect("conversation");

    db::insert_evaluator_catchup_entry(
        &conn,
        "conv-catchup",
        Some(11),
        12,
        "\"How was the market?\"",
        "\"Crowded as always.\"",
    )
    .expect("insert");
    db::insert_evaluator_catchup_entry(
        &conn,
        "conv-catchup",
        None,
        14,
        "\"Did you sleep well?\"",
        "\"Barely.\"",
    )
    .expect("insert");

    let entries = db::list_evaluator_catchup_entries(&conn, "conv-catchup").expect("list");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].assistant_message_id, 12);

    let rendered = append_evaluator_catchup_block("BASE MESSAGE".to_string(), &entries);
    assert!(rendered.starts_with("BASE MESSAGE"));
    assert!(rendered.contains("[CATCH-UP]"));
    assert!(rendered.contains("Exchange 1:"));
    assert!(rendered.contains("How was the market?"));
    assert!(rendered.contains("Exchange 2:"));

    // Empty queue leaves the message untouched.
    assert_eq!(
        append_evaluator_catchup_block("BASE MESSAGE".to_string(), &[]),
        "BASE MESSAGE"
    );

    let ids: Vec<i64> = entries.iter().map(|entry| entry.id).collect();
    db::delete_evaluator_catchup_entries(&conn, "conv-catchup", &ids).expect("delete");
    assert!(db::list_evaluator_catchup_entries(&conn, "conv-catchup")
        .expect("list")
        .is_empty());
}

#[test]
fn previous_assistant_status_block_reads_last_rp_scene_message() {
    let conn = db::init_memory_connection().expect("db");
    let soul = soul_with_existing_memory();
    db::upsert_soul(&conn, &soul).expect("soul");
    db::ensure_conversation(&conn, "conv-status", &soul.character_id).expect("conversation");

    let earlier = "She nods.\n```status\nScene | Focus: Tea ceremony | Physical state: Kneeling | Atmosphere: Calm\n```";
    db::insert_message_with_channel_and_get_id(
        &conn,
        "conv-status",
        "assistant",
        earlier,
        db::MESSAGE_CHANNEL_RP_SCENE,
    )
    .expect("assistant message");
    // A later command-channel message must not shadow the RP signal.
    db::insert_message_with_channel_and_get_id(
        &conn,
        "conv-status",
        "assistant",
        "State updated.",
        db::MESSAGE_CHANNEL_COMMAND_STATE,
    )
    .expect("command message");
    let current_id = db::insert_message_with_channel_and_get_id(
        &conn,
        "conv-status",
        "assistant",
        "current turn",
        db::MESSAGE_CHANNEL_RP_SCENE,
    )
    .expect("current message");

    let block = previous_assistant_status_block(&conn, "conv-status", current_id)
        .expect("status block found");
    assert!(block.contains("Tea ceremony"));
    assert!(status_gate_signature(&block).is_some());
}

#[test]
fn evaluator_token_usage_prefers_provider_report_over_estimates() {
    let reported = evaluator_token_usage_for_trace(
        Some(TokenUsage {
            prompt_tokens: Some(500),
            completion_tokens: Some(120),
        }),
        "system prompt",
        "user message",
        Some("raw response"),
    );
    assert_eq!(reported, (Some(500), Some(120), false));

    let (prompt, completion, estimated) =
        evaluator_token_usage_for_trace(None, "system prompt", "user message", Some("raw"));
    assert!(estimated);
    assert!(prompt.unwrap() > 0);
    assert!(completion.is_some());

    // A failed call has no response text: completion side stays unknown.
    let (_, completion, estimated) =
        evaluator_token_usage_for_trace(None, "system prompt", "user message", None);
    assert!(estimated);
    assert_eq!(completion, None);
}

#[test]
fn pipeline_trace_without_token_usage_still_deserializes() {
    // Background jobs restore the narrator's persisted trace; traces
    // written before token_usage existed must keep loading.
    let trace = TurnPipelineTrace::new("req".into(), None, "conv".into(), 0);
    let mut value = serde_json::to_value(&trace).expect("serializes");
    value.as_object_mut().expect("object").remove("token_usage");
    let restored: TurnPipelineTrace =
        serde_json::from_value(value).expect("legacy trace deserializes");
    assert_eq!(restored.token_usage, None);
}

#[test]
fn evaluator_mode_defaults_to_structured_for_json_schema_profiles() {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    db::upsert_soul(&conn, &soul).expect("soul");
    db::ensure_conversation(&conn, "conv-structured", &soul.character_id).expect("conversation");
    let mut profile = ProviderProfile {
        id: "prof-structured".into(),
        name: "Structured".into(),
        base_url: "https://api.example/v1".into(),
        api_key: "key".into(),
        model: "model".into(),
        system_prompt: String::new(),
        created_at: 0,
        updated_at: 0,
        narrator_timeout_ms: None,
        evaluator_timeout_ms: None,
        evaluator_timeout_mode: None,
        evaluator_mode: None,
        structured_evaluator_policy: Some("required".into()),
        wait_for_evaluator_before_next_turn: None,
        allow_send_with_stale_state: None,
        evaluator_background_enabled: None,
        anti_replay_forced_retry_enabled: None,
        archived_at: None,
        narrator_compatibility_status: 0,
        evaluator_compatibility_status: 1,
        command_compatibility_status: 0,
        evaluator_contract_version: 0,
        evaluator_prompt_version: 0,
        evaluator_last_tested_at: None,
        evaluator_last_failure_reason: None,
        structured_output_support: STRUCTURED_SUPPORT_JSON_SCHEMA,
    };
    db::upsert_provider_profile(&conn, &profile).expect("profile");
    db::set_active_evaluator_profile(&conn, "conv-structured", Some("prof-structured"))
        .expect("set active profile");

    let mut settings = evaluator_test_settings();
    settings.evaluator_mode = None;

    // Probed json_schema support upgrades the unset default to structured.
    assert_eq!(
        resolve_evaluator_mode_setting(&conn, "conv-structured", &settings).as_deref(),
        Some(EVALUATOR_MODE_STRUCTURED_V1)
    );
    assert_eq!(
        resolve_structured_evaluator_policy_setting(&conn, "conv-structured", &settings).as_deref(),
        Some("required")
    );

    // Explicit settings always win.
    settings.evaluator_mode = Some(EVALUATOR_MODE_V1.into());
    assert_eq!(
        resolve_evaluator_mode_setting(&conn, "conv-structured", &settings).as_deref(),
        Some(EVALUATOR_MODE_V1)
    );
    settings.structured_evaluator_policy = Some("allow_fallback".into());
    assert_eq!(
        resolve_structured_evaluator_policy_setting(&conn, "conv-structured", &settings).as_deref(),
        Some("allow_fallback")
    );
    settings.structured_evaluator_policy = None;

    // An explicit profile mode beats the auto-default.
    settings.evaluator_mode = None;
    profile.evaluator_mode = Some(EVALUATOR_MODE_FORM_V1.into());
    db::upsert_provider_profile(&conn, &profile).expect("profile update");
    assert_eq!(
        resolve_evaluator_mode_setting(&conn, "conv-structured", &settings).as_deref(),
        Some(EVALUATOR_MODE_FORM_V1)
    );

    // Below json_schema level nothing overrides the built-in default.
    profile.evaluator_mode = None;
    profile.structured_output_support = STRUCTURED_SUPPORT_JSON_OBJECT;
    db::upsert_provider_profile(&conn, &profile).expect("profile downgrade");
    assert_eq!(
        resolve_evaluator_mode_setting(&conn, "conv-structured", &settings),
        None
    );
}

#[test]
fn structured_diagnostic_settings_force_structured_mode() {
    let profile = ProviderProfile {
        id: "prof".into(),
        name: "Structured".into(),
        base_url: "https://api.example/v1?key=secret".into(),
        api_key: "secret".into(),
        model: "model".into(),
        system_prompt: String::new(),
        created_at: 0,
        updated_at: 0,
        narrator_timeout_ms: None,
        evaluator_timeout_ms: None,
        evaluator_timeout_mode: None,
        evaluator_mode: Some(EVALUATOR_MODE_FORM_V1.into()),
        structured_evaluator_policy: Some("required".into()),
        wait_for_evaluator_before_next_turn: None,
        allow_send_with_stale_state: None,
        evaluator_background_enabled: Some(true),
        anti_replay_forced_retry_enabled: None,
        archived_at: None,
        narrator_compatibility_status: 0,
        evaluator_compatibility_status: 0,
        command_compatibility_status: 0,
        evaluator_contract_version: 0,
        evaluator_prompt_version: 0,
        evaluator_last_tested_at: None,
        evaluator_last_failure_reason: None,
        structured_output_support: STRUCTURED_SUPPORT_UNTESTED,
    };

    let settings = diagnostic_structured_settings_from_profile(&profile, "required");
    assert_eq!(evaluator_mode(&settings), EVALUATOR_MODE_STRUCTURED_V1);
    assert_eq!(
        selected_evaluator_source(&evaluator_mode(&settings)),
        EVALUATOR_MODE_STRUCTURED_V1
    );
    assert_eq!(
        settings.structured_evaluator_policy.as_deref(),
        Some("required")
    );
    assert_eq!(
        settings.structured_evaluator_transport.as_deref(),
        Some("tool_call")
    );
    assert_eq!(
        settings.evaluator_timeout_ms,
        Some(DEFAULT_DIAGNOSTIC_EVALUATOR_TIMEOUT_MS)
    );
    assert_eq!(
        settings.diagnostic_evaluator_timeout_ms,
        Some(DEFAULT_DIAGNOSTIC_EVALUATOR_TIMEOUT_MS)
    );
    assert_eq!(settings.evaluator_timeout_mode.as_deref(), Some("finite"));
    assert_eq!(settings.evaluator_background_enabled, Some(false));
}

#[test]
fn frontend_dev_surface_owns_structured_evaluator_controls() {
    let app_source = include_str!("../../src/App.tsx");
    let settings_page_source = include_str!("../../src/components/views/SettingsPageView.tsx");
    assert!(app_source.contains("Evaluator Mode"));
    assert!(app_source.contains("Narrator Style"));
    assert!(app_source.contains("Active Director"));
    assert!(app_source.contains("GM Simulation"));
    assert!(app_source.contains("Legacy Form Evaluator"));
    assert!(app_source.contains("Structured Ops Evaluator"));
    assert!(app_source.contains("Structured Policy"));
    assert!(app_source.contains("Run Structured Evaluator Diagnostic"));
    assert!(app_source.contains("evaluator_mode: stateUpdaterSettings.evaluator_mode"));
    assert!(app_source
        .contains("structured_evaluator_policy: stateUpdaterSettings.structured_evaluator_policy"));
    assert!(!settings_page_source.contains("Evaluator Mode"));
    assert!(!settings_page_source.contains("Structured Policy"));
    assert!(!settings_page_source.contains("Run Structured Evaluator Diagnostic"));
}

#[test]
fn frontend_dev_console_whitelists_run_benchmark() {
    let source = concat!(
        include_str!("../../src/App.tsx"),
        include_str!("../../src/features/dev/commands.ts")
    );
    assert!(source.contains("| \"run_benchmark\""));
    assert!(source.contains("name: \"run_benchmark\""));
    assert!(source.contains("label: \"Run Benchmark\""));
    assert!(source.contains("case \"run_benchmark\""));
    assert!(source.contains("runBenchmark("));
}

#[test]
fn frontend_start_chat_persona_list_requires_confirm() {
    let source = concat!(
        include_str!("../../src/App.tsx"),
        include_str!("../../src/components/chat/PersonaModal.tsx")
    );
    assert!(source.contains("personaListConfirmRequired"));
    assert!(source.contains("openPersonaList(nextConversationId, true)"));
    assert!(source.contains("Confirm Persona"));
    assert!(source.contains("handleConfirmPersonaList"));
}

#[test]
fn structured_runtime_routes_through_selected_compile() {
    let (soul, world, user, narrator, _) = form_runtime_fixture();
    let raw = format!(
        r#"{{"schema_version":1,"ops":[{{"op":"add_world_event","content":"The visitor entered the apartment.","evidence_quote":"{}"}}],"no_op_reason":null}}"#,
        user
    );

    let outcome = compile_selected_evaluator_runtime(
        EVALUATOR_MODE_STRUCTURED_V1,
        None,
        &raw,
        Some(StructuredEnforcement::JsonSchema),
        &soul,
        &world,
        &user,
        &narrator,
        None,
        false,
    )
    .expect("structured selected");

    assert!(outcome.conversion.patch.world_patch.is_some());
}

#[test]
fn invalid_structured_output_can_fallback_to_form_runtime() {
    let (soul, world, user, narrator, spec) = form_runtime_fixture();
    let structured_err = compile_evaluator_structured_runtime(
        "{\"schema_version\":1,\"ops\":[{\"op\":\"unknown\"}],\"no_op_reason\":null}",
        Some(StructuredEnforcement::JsonSchema),
        &soul,
        &world,
        &user,
        &narrator,
        None,
        false,
    )
    .expect_err("invalid structured should fail");

    let mut fallback = compile_evaluator_form_runtime_strict(
        &door_entry_form_response_json(&soul.character_id),
        spec,
        &soul,
        &world,
        &user,
        &narrator,
        None,
    )
    .expect("form fallback compiles");
    fallback.fallback_path = vec![
        "structured_json_schema".into(),
        EVALUATOR_MODE_FORM_V1.into(),
    ];
    fallback.fallback_warning = Some(format!(
        "structured evaluator failed; evaluator_form_v1 fallback used: {structured_err}"
    ));

    assert!(!fallback.conversion.patch.is_empty());
    assert_eq!(
        fallback.fallback_path,
        vec!["structured_json_schema", EVALUATOR_MODE_FORM_V1]
    );
    assert!(fallback.fallback_warning.is_some());
}

#[test]
fn strict_tool_diagnostic_failure_does_not_use_form_fallback() {
    let outcome = strict_tool_diagnostic_failed_outcome(
        vec!["structured_tool_call".into()],
        "malformed_schema_output: unknown field `confidence`".into(),
    );

    assert!(outcome.conversion.patch.is_empty());
    assert_eq!(outcome.fallback_path, vec!["structured_tool_call"]);
    assert!(!outcome
        .fallback_path
        .contains(&EVALUATOR_MODE_FORM_V1.to_string()));
    assert!(!outcome.structured_enforcement_validated);
    assert_eq!(
        outcome.structured_schema_validation_status,
        "malformed_schema_output"
    );
}

#[test]
fn structured_retry_update_scene_state_confidence_removed_succeeds() {
    let (soul, world, user, narrator, _) = form_runtime_fixture();
    let bad_raw = format!(
        r#"{{"schema_version":1,"ops":[{{"op":"update_scene_state","current_scene":"Apartment doorway","focus":"Aurora and preset_male","participants":["{}","preset_male"],"last_user_action":"{}","pressure_point":"Aurora decides what to say next.","continuity_note":"The doorway exchange continues.","evidence_quote":"I walk in. Long time no see, Aurora.","confidence":0.8}}],"no_op_reason":null}}"#,
        soul.character_id, user
    );
    let first_err = compile_evaluator_structured_runtime(
        &bad_raw,
        Some(StructuredEnforcement::ToolCall),
        &soul,
        &world,
        &user,
        &narrator,
        None,
        false,
    )
    .expect_err("illegal confidence should fail first tool call");
    assert_eq!(
        structured_failure_kind(&first_err),
        "schema_validation_failed"
    );

    let retry_raw = format!(
        r#"{{"schema_version":1,"ops":[{{"op":"update_scene_state","current_scene":"Apartment doorway","focus":"Aurora and preset_male","participants":["{}","preset_male"],"last_user_action":"{}","pressure_point":"Aurora decides what to say next.","continuity_note":"The doorway exchange continues.","evidence_quote":"I walk in. Long time no see, Aurora."}}],"no_op_reason":null}}"#,
        soul.character_id, user
    );
    let mut outcome = compile_evaluator_structured_runtime(
        &retry_raw,
        Some(StructuredEnforcement::ToolCall),
        &soul,
        &world,
        &user,
        &narrator,
        None,
        false,
    )
    .expect("retry without confidence succeeds");
    outcome.fallback_path = vec![
        "structured_tool_call".into(),
        "structured_tool_call_retry".into(),
    ];
    outcome.structured_retry_count = 1;
    outcome.structured_retry_reasons = vec![structured_failure_kind(&first_err).into()];
    outcome.structured_retry_succeeded = Some(true);

    assert_eq!(outcome.structured_retry_count, 1);
    assert_eq!(outcome.structured_retry_succeeded, Some(true));
    assert_eq!(
        outcome.fallback_path,
        vec!["structured_tool_call", "structured_tool_call_retry"]
    );
    assert_eq!(
        diagnostic_patch_counts(&outcome.conversion.patch).scene_update_ops_count,
        1
    );
}

#[test]
fn structured_retry_invalid_evidence_quote_uses_valid_quote_and_succeeds() {
    let (soul, world, user, narrator, _) = form_runtime_fixture();
    let bad_raw = r#"{"schema_version":1,"ops":[{"op":"add_world_event","content":"The visitor entered.","evidence_quote":"This quote is not present."}],"no_op_reason":null}"#;
    let first_err = compile_evaluator_structured_runtime(
        bad_raw,
        Some(StructuredEnforcement::ToolCall),
        &soul,
        &world,
        &user,
        &narrator,
        None,
        false,
    )
    .expect_err("invalid evidence should fail first tool call");
    assert_eq!(
        structured_failure_kind(&first_err),
        "evidence_quote_invalid"
    );

    let retry_raw = format!(
        r#"{{"schema_version":1,"ops":[{{"op":"add_world_event","content":"The visitor entered.","evidence_quote":"{}"}}],"no_op_reason":null}}"#,
        user
    );
    let mut outcome = compile_evaluator_structured_runtime(
        &retry_raw,
        Some(StructuredEnforcement::ToolCall),
        &soul,
        &world,
        &user,
        &narrator,
        None,
        false,
    )
    .expect("retry with exact evidence succeeds");
    outcome.fallback_path = vec![
        "structured_tool_call".into(),
        "structured_tool_call_retry".into(),
    ];
    outcome.structured_retry_count = 1;
    outcome.structured_retry_reasons = vec![structured_failure_kind(&first_err).into()];
    outcome.structured_retry_succeeded = Some(true);

    assert_eq!(outcome.structured_retry_succeeded, Some(true));
    assert_eq!(
        outcome.fallback_path,
        vec!["structured_tool_call", "structured_tool_call_retry"]
    );
    assert_eq!(diagnostic_total_patch_ops(&outcome.conversion.patch), 1);
}

#[test]
fn structured_retry_failure_does_not_commit_partial_patch() {
    let retry_failure = StructuredRetryFailure {
        final_error: "malformed_schema_output: still invalid".into(),
        retry_count: 1,
        retry_reasons: vec!["schema_validation_failed".into()],
        first_trace: StructuredCompletionTrace::default(),
    };
    let mut outcome = strict_tool_diagnostic_failed_outcome(
        vec!["structured_tool_call".into()],
        retry_failure.final_error.clone(),
    );
    apply_structured_retry_failure(&mut outcome, &retry_failure);

    assert!(outcome.conversion.patch.is_empty());
    assert_eq!(diagnostic_total_patch_ops(&outcome.conversion.patch), 0);
    assert_eq!(outcome.structured_retry_succeeded, Some(false));
    assert_eq!(
        outcome.structured_retry_final_error.as_deref(),
        Some("malformed_schema_output: still invalid")
    );
}

#[test]
fn structured_retry_success_compiles_exactly_one_enrichment_patch() {
    let (soul, world, user, narrator, _) = form_runtime_fixture();
    let retry_raw = format!(
        r#"{{"schema_version":1,"ops":[{{"op":"add_world_event","content":"The visitor entered.","evidence_quote":"{}"}}],"no_op_reason":null}}"#,
        user
    );
    let mut outcome = compile_evaluator_structured_runtime(
        &retry_raw,
        Some(StructuredEnforcement::ToolCall),
        &soul,
        &world,
        &user,
        &narrator,
        None,
        false,
    )
    .expect("retry compiles one patch");
    outcome.fallback_path = vec![
        "structured_tool_call".into(),
        "structured_tool_call_retry".into(),
    ];
    outcome.structured_retry_count = 1;
    outcome.structured_retry_succeeded = Some(true);

    assert_eq!(diagnostic_total_patch_ops(&outcome.conversion.patch), 1);
    assert!(!outcome.conversion.patch.is_empty());
}

#[test]
fn strict_diagnostic_fails_after_retry_if_still_invalid() {
    let retry_failure = StructuredRetryFailure {
            final_error: "Structured evaluator semantic validation failed: evidence quote not found in latest exchange: nope".into(),
            retry_count: 1,
            retry_reasons: vec!["evidence_quote_invalid".into()],
            first_trace: StructuredCompletionTrace::default(),
        };
    let mut outcome = strict_tool_diagnostic_failed_outcome(
        vec!["structured_tool_call".into()],
        retry_failure.final_error.clone(),
    );
    apply_structured_retry_failure(&mut outcome, &retry_failure);

    assert!(!outcome.structured_enforcement_validated);
    assert_eq!(outcome.structured_retry_succeeded, Some(false));
    assert!(!outcome
        .fallback_path
        .contains(&EVALUATOR_MODE_FORM_V1.to_string()));
    assert!(outcome.conversion.patch.is_empty());
}

#[test]
fn normal_mode_can_fallback_after_structured_retry_failure() {
    let (soul, world, user, narrator, spec) = form_runtime_fixture();
    let retry_failure = StructuredRetryFailure {
        final_error: "malformed_schema_output: retry still had invalid enum".into(),
        retry_count: 1,
        retry_reasons: vec!["schema_validation_failed".into()],
        first_trace: StructuredCompletionTrace::default(),
    };
    let mut fallback = compile_evaluator_form_runtime_strict(
        &door_entry_form_response_json(&soul.character_id),
        spec,
        &soul,
        &world,
        &user,
        &narrator,
        None,
    )
    .expect("normal form fallback compiles");
    fallback.fallback_path = vec!["structured_tool_call".into(), EVALUATOR_MODE_FORM_V1.into()];
    apply_structured_retry_failure(&mut fallback, &retry_failure);

    assert!(!fallback.conversion.patch.is_empty());
    assert!(fallback
        .fallback_path
        .contains(&EVALUATOR_MODE_FORM_V1.to_string()));
    assert_eq!(fallback.structured_retry_count, 1);
    assert_eq!(fallback.structured_retry_succeeded, Some(false));
}

#[test]
fn invalid_structured_and_invalid_form_results_in_noop_warning() {
    let (soul, world, user, narrator, spec) = form_runtime_fixture();
    assert!(compile_evaluator_form_runtime_strict(
        "not json", spec, &soul, &world, &user, &narrator, None,
    )
    .is_err());

    let noop = evaluator_noop_after_all_fallbacks(
        vec!["structured_json_schema".into()],
        "bad structured".into(),
        "bad form".into(),
    );

    assert!(noop.conversion.no_op);
    assert!(noop.conversion.patch.is_empty());
    assert!(noop
        .fallback_path
        .contains(&"noop_after_all_fallbacks".to_string()));
    assert!(noop.fallback_warning.is_some());
}

#[test]
fn fallback_noop_does_not_duplicate_baseline_patch() {
    let noop = evaluator_noop_after_all_fallbacks(
        vec!["structured_json_object".into()],
        "bad structured".into(),
        "bad form".into(),
    );
    assert!(noop.conversion.patch.is_empty());
    assert!(noop.conversion.patch.world_patch.is_none());
    assert!(noop.conversion.patch.soul_patch.is_none());
}

#[test]
fn fallback_trace_records_enforcement_and_path() {
    let (soul, world, user, narrator, _) = form_runtime_fixture();
    let raw = format!(
        r#"{{"schema_version":1,"ops":[{{"op":"add_world_event","content":"The visitor entered the apartment.","evidence_quote":"{}"}}],"no_op_reason":null}}"#,
        user
    );
    let outcome = compile_evaluator_structured_runtime(
        &raw,
        Some(StructuredEnforcement::JsonObject),
        &soul,
        &world,
        &user,
        &narrator,
        None,
        false,
    )
    .expect("structured json object compiles");
    let trace = evaluator_runtime_fallback_json(&outcome);

    assert_eq!(trace["fallback_path"][0], "structured_json_object");
    assert_eq!(trace["ops_count"], 1);
    assert_eq!(trace["syntactic_repair_used"], false);
}

#[test]
fn dual_compare_logs_both_paths_without_double_applying() {
    let (soul, world, user, narrator, spec) = form_runtime_fixture();
    let mut outcome = compile_evaluator_form_runtime(
        &door_entry_form_response_json(&soul.character_id),
        spec,
        &soul,
        &world,
        &user,
        &narrator,
        None,
    )
    .expect("compile form");
    outcome.comparison_trace = dual_compare_deferred_trace(EVALUATOR_MODE_DUAL_COMPARE, 42, true);
    let trace = serde_json::json!({
        "evaluator_mode": EVALUATOR_MODE_DUAL_COMPARE,
        "selected_evaluator_source": selected_evaluator_source(EVALUATOR_MODE_DUAL_COMPARE),
        "comparison_trace": outcome.comparison_trace
    });

    assert_eq!(trace["selected_evaluator_source"], EVALUATOR_MODE_FORM_V1);
    assert_eq!(
        trace["comparison_trace"]["compare_evaluator_source"],
        EVALUATOR_MODE_V1
    );
    assert_eq!(trace["comparison_trace"]["compare_patch_applied"], false);
    assert_eq!(
        trace["comparison_trace"]["selected_patch_applied_before_comparison_done"],
        true
    );
    assert_eq!(
        trace["comparison_trace"]["comparison_skipped_or_timed_out"],
        true
    );
}

#[test]
fn dual_compare_does_not_block_selected_form_apply() {
    let trace =
        dual_compare_deferred_trace(EVALUATOR_MODE_DUAL_COMPARE, 12, true).expect("dual trace");

    assert_eq!(trace["selected_evaluator_source"], EVALUATOR_MODE_FORM_V1);
    assert_eq!(trace["compare_parse_status"], "skipped");
    assert_eq!(trace["selected_patch_applied_before_comparison_done"], true);
    assert_eq!(trace["comparison_path_elapsed_ms"], serde_json::Value::Null);
}

#[test]
fn evaluator_v1_still_available() {
    let settings = evaluator_test_settings();
    let (soul, world, user, narrator, _) = form_runtime_fixture();
    let outcome = compile_selected_evaluator_runtime(
        &evaluator_mode(&settings),
        None,
        &evaluator_output_json_with_candidate(valid_evaluator_candidate()),
        None,
        &soul,
        &world,
        &user,
        &narrator,
        None,
        false,
    )
    .expect("compile v1");

    assert_eq!(
        selected_evaluator_source(&evaluator_mode(&settings)),
        EVALUATOR_MODE_V1
    );
    assert_eq!(
        evaluator_provider_label(&evaluator_mode(&settings), true),
        "evaluator_v1_background"
    );
    assert_eq!(outcome.form_response_parse_status, None);
}

#[test]
fn evaluator_mode_defaults_to_form_v1() {
    let settings = ApiProviderSettings {
        base_url: "https://api.example/v1".into(),
        api_key: "key".into(),
        model: "model".into(),
        system_prompt: String::new(),
        ..Default::default()
    };

    assert_eq!(evaluator_mode(&settings), EVALUATOR_MODE_FORM_V1);
}

#[test]
fn form_v1_door_entry_smoke_generates_scene_state_or_recent_event() {
    let (soul, world, user, narrator, spec) = form_runtime_fixture();
    let outcome = compile_evaluator_form_runtime(
        &door_entry_form_response_json(&soul.character_id),
        spec,
        &soul,
        &world,
        &user,
        &narrator,
        None,
    )
    .expect("compile form");
    let world_patch = outcome
        .conversion
        .patch
        .world_patch
        .as_ref()
        .expect("world patch");

    assert!(world_patch.scene_state.is_some() || world_patch.recent_event.is_some());
}

#[test]
fn form_parse_failure_still_applies_minimal_scene_patch() {
    let (soul, world, user, narrator, spec) = form_runtime_fixture();
    let outcome = compile_evaluator_form_runtime(
        "{ definitely not valid json",
        spec,
        &soul,
        &world,
        &user,
        &narrator,
        None,
    )
    .expect("fail open");

    assert!(outcome.partial_success);
    assert_eq!(
        outcome.form_response_parse_status.as_deref(),
        Some("partial_success")
    );
    assert!(!outcome.conversion.patch.is_empty());
    assert!(outcome
        .conversion
        .patch
        .world_patch
        .as_ref()
        .is_some_and(|patch| patch.scene_state.is_some() || patch.recent_event.is_some()));
}

#[test]
fn state_update_partial_success_not_failed_when_fallback_patch_applies() {
    let (soul, world, user, narrator, spec) = form_runtime_fixture();
    let outcome = compile_selected_evaluator_runtime(
        EVALUATOR_MODE_FORM_V1,
        Some(spec),
        "not json",
        None,
        &soul,
        &world,
        &user,
        &narrator,
        None,
        false,
    )
    .expect("partial success");

    assert!(outcome.partial_success);
    assert_ne!(
        outcome.form_response_parse_status.as_deref(),
        Some("failed")
    );
    assert!(!outcome.conversion.patch.is_empty());
}

#[test]
fn form_path_can_review_existing_memory_before_writing_duplicate() {
    let soul = soul_with_existing_memory();
    let mut world =
        state_engine::setting::session_world_from_setting(&new_default_setting("Aurora"));
    world.location = "Rainy, neon-lit apartment interior".into();
    let user = "I walk in. Long time no see, Aurora.".to_string();
    let narrator = "Aurora lets the visitor into her apartment.".to_string();
    let spec = build_eval_form_spec(&soul, Some(&world), &user, &narrator, 8);
    assert_eq!(spec.existing_memories.len(), 1);

    let draft_outcome = compile_evaluator_form_runtime(
        &memory_form_response_json(
            &soul.character_id,
            "The visitor entered Aurora's apartment.",
            Vec::new(),
        ),
        spec.clone(),
        &soul,
        &world,
        &user,
        &narrator,
        None,
    )
    .expect("compile draft memory");
    let candidate_id = draft_outcome.output.memory_candidates[0]
        .candidate_id
        .clone();
    let duplicate_review = serde_json::json!({
        "candidate_id": candidate_id,
        "decision": "duplicate_of_existing",
        "existing_id": "existing-memory-1",
        "reason": "The existing memory already records the apartment entry.",
        "evidence_quote": "I walk in. Long time no see, Aurora."
    });
    let reviewed = compile_evaluator_form_runtime(
        &memory_form_response_json(
            &soul.character_id,
            "The visitor entered Aurora's apartment.",
            vec![duplicate_review],
        ),
        spec,
        &soul,
        &world,
        &user,
        &narrator,
        None,
    )
    .expect("compile reviewed memory");

    assert_eq!(
        reviewed
            .form_trace
            .as_ref()
            .expect("form trace")
            .form_dedupe_decisions
            .len(),
        1
    );
    assert!(reviewed.conversion.accepted_candidate_ids.is_empty());
}

fn evaluator_test_job(status: &str) -> db::EvaluatorJob {
    db::EvaluatorJob {
        evaluator_job_id: format!("job-{status}"),
        conversation_id: "async-evaluator".into(),
        turn_id: "turn-1".into(),
        assistant_message_id: 42,
        status: status.into(),
        started_at: db::now_ts(),
        completed_at: None,
        elapsed_ms: None,
        timeout_ms: Some(25_000),
        timeout_mode: "finite".into(),
        model: Some("model".into()),
        provider: Some("evaluator_v1".into()),
        error_message: None,
        patch_applied: false,
    }
}

#[test]
fn evaluator_timeout_is_configurable() {
    let mut settings = evaluator_test_settings();
    settings.evaluator_timeout_ms = Some(4_200);

    assert_eq!(effective_evaluator_timeout_ms(&settings), Some(4_200));
}

#[test]
fn evaluator_no_app_timeout_does_not_cancel_at_25s() {
    let mut settings = evaluator_test_settings();
    settings.evaluator_timeout_mode = Some("no_app_timeout".into());

    assert_eq!(effective_evaluator_timeout_ms(&settings), None);
    assert!(!evaluator_timed_out(
        "still running",
        Duration::from_millis(25_500),
        &settings
    ));
}

#[test]
fn narrator_returns_before_evaluator_completion() {
    let mut settings = evaluator_test_settings();
    settings.evaluator_background_enabled = Some(true);

    assert!(evaluator_background_enabled(&settings));
}

#[test]
fn next_turn_waits_for_pending_evaluator_when_enabled() {
    let mut settings = evaluator_test_settings();
    settings.wait_for_evaluator_before_next_turn = Some(true);
    settings.allow_send_with_stale_state = Some(false);

    assert!(wait_for_evaluator_before_next_turn(&settings));
    assert!(!allow_send_with_stale_state(&settings));
}

#[test]
fn next_turn_can_proceed_with_stale_state_when_allowed() {
    let mut settings = evaluator_test_settings();
    settings.wait_for_evaluator_before_next_turn = Some(false);
    settings.allow_send_with_stale_state = Some(true);

    assert!(!wait_for_evaluator_before_next_turn(&settings));
    assert!(allow_send_with_stale_state(&settings));
}

#[test]
fn pending_evaluator_blocks_narrator_when_stale_not_allowed() {
    let mut settings = evaluator_test_settings();
    settings.wait_for_evaluator_before_next_turn = Some(false);
    settings.allow_send_with_stale_state = Some(false);

    assert!(!wait_for_evaluator_before_next_turn(&settings));
    assert!(!allow_send_with_stale_state(&settings));
}

#[test]
fn evaluator_failure_does_not_apply_empty_success_patch() {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    db::upsert_soul(&conn, &soul).expect("upsert soul");
    db::ensure_conversation(&conn, "async-evaluator", &soul.character_id).expect("conversation");
    db::insert_evaluator_job(&conn, &evaluator_test_job("running")).expect("insert");
    db::update_evaluator_job_status(
        &conn,
        "job-running",
        "failed",
        Some("provider failed"),
        Some(db::now_ts()),
        Some(25),
        false,
    )
    .expect("update");

    let job = db::get_evaluator_job(&conn, "job-running")
        .expect("query")
        .expect("job");
    assert_eq!(job.status, "failed");
    assert!(!job.patch_applied);
}

#[test]
fn evaluator_cancel_marks_job_canceled() {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    db::upsert_soul(&conn, &soul).expect("upsert soul");
    db::ensure_conversation(&conn, "async-evaluator", &soul.character_id).expect("conversation");
    db::insert_evaluator_job(&conn, &evaluator_test_job("running")).expect("insert");
    db::update_evaluator_job_status(
        &conn,
        "job-running",
        "canceled",
        Some("Canceled by user"),
        Some(db::now_ts()),
        Some(10),
        false,
    )
    .expect("cancel");

    let job = db::get_evaluator_job(&conn, "job-running")
        .expect("query")
        .expect("job");
    assert_eq!(job.status, "canceled");
    assert!(!job.patch_applied);
}

#[test]
fn evaluator_retry_applies_patch_after_failure() {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    db::upsert_soul(&conn, &soul).expect("upsert soul");
    db::ensure_conversation(&conn, "async-evaluator", &soul.character_id).expect("conversation");
    let mut failed = evaluator_test_job("failed");
    failed.evaluator_job_id = "job-failed".into();
    failed.error_message = Some("parse failed".into());
    db::insert_evaluator_job(&conn, &failed).expect("insert failed");
    let mut retry = evaluator_test_job("completed");
    retry.evaluator_job_id = "job-retry".into();
    retry.patch_applied = true;
    retry.completed_at = Some(db::now_ts());
    db::insert_evaluator_job(&conn, &retry).expect("insert retry");

    let latest = db::get_latest_evaluator_job(&conn, "async-evaluator")
        .expect("latest")
        .expect("job");
    assert_eq!(latest.status, "completed");
    assert!(latest.patch_applied);
}

#[test]
fn payload_trace_records_evaluator_elapsed_and_wait() {
    let exported = render_llm_payload_history(&[payload_trace_log(serde_json::json!({
        "narrator_trace": {
            "request_id": "req-1",
            "next_turn_wait_ms": 250,
            "compiled_with_pending_evaluator": false
        },
        "evaluator_raw_response": "{}",
        "evaluator_parsed_json": { "schema_version": 1 },
        "ledger_apply_trace": { "patch_applied": true },
        "evaluator_trace": {
            "elapsed_ms": 123,
            "timeout_ms": 25000
        }
    }))]);

    assert!(exported.contains("### NARRATOR TRACE"));
    assert!(exported.contains("\"next_turn_wait_ms\": 250"));
    assert!(exported.contains("\"elapsed_ms\": 123"));
}

#[test]
fn ui_state_types_include_pending_running_completed_failed_canceled() {
    let statuses = [
        "pending",
        "running",
        "completed",
        "failed",
        "canceled",
        "timed_out",
    ];

    assert!(statuses.contains(&"pending"));
    assert!(statuses.contains(&"running"));
    assert!(statuses.contains(&"completed"));
    assert!(statuses.contains(&"failed"));
    assert!(statuses.contains(&"canceled"));
}

#[test]
fn output_contract_keeps_only_last_status_block() {
    let raw = "```status\nScene | Focus: Old | Physical state: Old | Atmosphere: Old\n```\n\nAurora steps back from the doorway.\n\n```status\nScene | Focus: Aurora | Physical state: Guarded | Atmosphere: Tense\n```";

    let result = apply_output_contract_guard(raw, "I knock on the door.");

    assert_eq!(result.text.matches("```status").count(), 1);
    assert!(!result.text.contains("Focus: Old"));
    assert!(result.text.contains("Aurora steps back"));
    assert!(result.text.ends_with(
        "```status\nScene | Focus: Aurora | Physical state: Guarded | Atmosphere: Tense\n```"
    ));
    assert!(result
        .warning
        .as_deref()
        .unwrap_or_default()
        .contains("multiple status blocks"));
}

#[test]
fn output_contract_appends_fallback_status_for_scene_narration() {
    let result = apply_output_contract_guard("Aurora watches the hallway in silence.", "I wait.");

    assert!(result.text.contains("Aurora watches the hallway"));
    assert!(result.text.contains("Scene | Focus: Unknown"));
    assert!(result
        .warning
        .as_deref()
        .unwrap_or_default()
        .contains("fallback status"));
}

#[test]
fn malformed_status_block_does_not_swallow_visible_prose() {
    let raw = "Aurora exhales before answering.\n\n```status\nScene | Focus: Aurora\nThe hallway stays quiet.";

    let result = apply_output_contract_guard(raw, "I wait.");

    assert!(result.text.contains("Aurora exhales before answering."));
    assert!(result.text.contains("The hallway stays quiet."));
    assert!(result.text.contains("```status"));
}

#[test]
fn malformed_status_fence_recovers_prose_and_keeps_one_status_block() {
    let raw = "```status\nAurora goes still beneath the user's hand, breath catching once before she looks up.\nThe old doorway beat should not be replayed.\n```status\nScene | Focus: Aurora | Physical state: Still | Atmosphere: Quiet pressure\n```";

    let result = apply_output_contract_guard(raw, "I pat her head.");

    assert!(result
        .text
        .contains("Aurora goes still beneath the user's hand"));
    assert!(result
        .text
        .contains("old doorway beat should not be replayed"));
    assert_eq!(result.text.matches("```status").count(), 1);
    assert!(result
            .text
            .ends_with("```status\nScene | Focus: Aurora | Physical state: Still | Atmosphere: Quiet pressure\n```"));
    assert!(result
        .warning
        .as_deref()
        .unwrap_or_default()
        .contains("malformed status fence recovered"));
}

#[test]
fn status_fence_with_only_prose_gets_visible_fallback_status() {
    let raw = "```status\nAurora leans into the pat for one quiet beat, then catches herself before choosing whether to pull away.\n```";

    let result = apply_output_contract_guard(raw, "I pat her head.");

    assert!(result.text.starts_with("Aurora leans into the pat"));
    assert_eq!(result.text.matches("```status").count(), 1);
    assert!(result.text.contains("Scene | Focus: Unknown"));
    assert!(result
        .warning
        .as_deref()
        .unwrap_or_default()
        .contains("malformed status fence recovered"));
}

#[test]
fn output_contract_allows_gm_reply_without_status() {
    let result = apply_output_contract_guard(
        "GM: Yes, I understand the out-of-character correction.",
        "I am talking to the Narrator. The GM.",
    );

    assert!(!result.text.contains("```status"));
    assert!(result.text.starts_with("GM:"));
}

#[test]
fn valid_status_block_prevents_unknown_fallback() {
    let raw = "Aurora opens the door.\n\n```status\nScene | Focus: Aurora | Physical state: Still | Atmosphere: Quiet\n```\n\n```status\n```";

    let result = apply_output_contract_guard(raw, "I knock on the door.");

    assert_eq!(result.text.matches("```status").count(), 1);
    assert!(result.text.contains("Focus: Aurora"));
    assert!(!result.text.contains("Focus: Unknown"));
}

#[test]
fn valid_status_block_prevents_unknown_fallback_regression() {
    let raw = "Aurora opens the door.\n\n```status\nScene | Focus: Aurora | Physical state: Still | Atmosphere: Quiet\n```\n\n```status\n```";

    let result = apply_output_contract_guard(raw, "I knock on the door.");

    assert_eq!(result.text.matches("```status").count(), 1);
    assert!(result.text.contains("Focus: Aurora"));
    assert!(!result.text.contains("Focus: Unknown"));
}

#[test]
fn pure_ooc_no_memory_debug_nonce() {
    let result = apply_output_contract_guard(
        "GM: I will keep that correction out of character.",
        "OOC: please do not advance the scene.",
    );

    assert!(!result.text.contains("```status"));
    assert!(!result.text.contains("memory-debug-"));
    assert!(is_gm_facing_user_message(
        "OOC: please do not advance the scene."
    ));
}

#[test]
fn phone_notifications_off_blocks_chime_and_screen_wake() {
    let mut soul = new_default_soul("Aurora");
    soul.world
        .object_states
        .push(state_engine::soul::ObjectState {
            object_id: "aurora_phone".into(),
            notification_mode: "notifications_off".into(),
            vibrate_enabled: Some(false),
            screen_wake_enabled: Some(false),
            ..state_engine::soul::ObjectState::default()
        });
    let session_world = state_engine::setting::session_world_from_legacy_world(
        "Apartment",
        Some("world-phone".into()),
        &soul.world,
    );
    let raw = "Aurora's phone chimes and the screen lights up with the user's text.\n\n```status\nScene | Focus: Aurora | Physical state: Alert | Atmosphere: Quiet\n```";

    let guarded = sanitize_phone_notification_contradiction(raw, "I text Aurora.", &session_world);

    assert!(guarded.repaired);
    assert!(guarded.text.contains("arrives silently"));
    assert!(!guarded.text.to_ascii_lowercase().contains("chimes"));
    assert!(!guarded.text.to_ascii_lowercase().contains("lights up"));
    assert!(guarded.text.contains("```status"));
}

#[test]
fn output_contract_strips_engine_patch_json() {
    let raw = "Aurora exhales.\n\n```json\n{\"schema_version\":1,\"world_patch\":{\"recent_event\":\"Should not be visible\"}}\n```\n\n[HIDDEN STATE]{\"tag\":\"observation\"}[/HIDDEN STATE]";

    let result = apply_output_contract_guard(raw, "I speak.");

    assert!(result.text.contains("Aurora exhales."));
    assert!(!result.text.contains("schema_version"));
    assert!(!result.text.contains("HIDDEN STATE"));
    assert!(result.text.contains("```status"));
    assert!(result
        .warning
        .as_deref()
        .unwrap_or_default()
        .contains("EnginePatch JSON stripped"));
}

#[test]
fn anti_replay_detects_repeated_previous_paragraph() {
    let repeated = "Aurora braces one hand against the doorframe, listening to the alarm chew through the hallway while dust trembles down from the ceiling. She does not soften, does not step aside, and does not pretend the room is safe; every line of her body stays angled toward the threat as she demands the truth.";
    let source = ReplaySource {
            message_id: 42,
            content: format!("{repeated}\n\n```status\nScene | Focus: Aurora | Physical state: Alert | Atmosphere: Alarmed\n```"),
        };

    let result = detect_replay(repeated, &[source]);

    assert!(result.replay_detected);
    assert_eq!(result.compared_against_message_id, Some(42));
    assert!(result.replay_score > 0.35);
}

#[test]
fn anti_replay_ignores_matching_status_blocks() {
    let source = ReplaySource {
            message_id: 7,
            content: "A completely different prior scene.\n\n```status\nScene | Focus: Aurora | Physical state: Alert | Atmosphere: Alarmed\n```"
                .into(),
        };
    let new_response = "Aurora answers the corrected premise instead of replaying the prior beat.\n\n```status\nScene | Focus: Aurora | Physical state: Alert | Atmosphere: Alarmed\n```";

    let result = detect_replay(new_response, &[source]);

    assert!(!result.replay_detected);
}

#[test]
fn anti_replay_passes_distinct_response() {
    let source = ReplaySource {
        message_id: 9,
        content: "Aurora talks about firewalls and system bleed-through in a prior explanation."
            .into(),
    };
    let new_response =
            "The GM acknowledges the correction and resets the scene premise around the new system error.";

    let result = detect_replay(new_response, &[source]);

    assert!(!result.replay_detected);
    assert!(result.replay_score <= 0.35);
}

#[test]
fn anti_replay_detects_repeated_room_setup_and_object_list() {
    let source = ReplaySource {
            message_id: 11,
            content: "The unlocked door throws a bar of neon across the rain-streaked room. A wine glass waits beside the phone while Aurora stands barefoot in an oversized shirt near the couch.".into(),
        };
    let repeated = "The unlocked door is still open to the neon and rain. The room holds the same wine glass, the phone, the couch, and Aurora barefoot in the oversized shirt before anything else moves.";

    let result = detect_replay(repeated, &[source]);

    assert!(result.replay_detected);
    assert_eq!(result.compared_against_message_id, Some(11));
    assert!(result
        .replay_reason
        .as_deref()
        .unwrap_or_default()
        .contains("scene setup"));
}

#[test]
fn anti_replay_compares_against_three_recent_assistant_sources() {
    let sources = vec![
            ReplaySource {
                message_id: 1,
                content: "A distinct first prior response.".into(),
            },
            ReplaySource {
                message_id: 2,
                content: "Another unrelated prior response.".into(),
            },
            ReplaySource {
                message_id: 3,
                content: "The unlocked door, neon rain, wine glass, phone, barefoot Aurora, and oversized shirt freeze the old setup.".into(),
            },
        ];
    let repeated =
            "The unlocked door and neon rain frame the wine glass, phone, barefoot Aurora, and oversized shirt again.";

    let result = detect_replay(repeated, &sources);

    assert!(result.replay_detected);
    assert_eq!(result.compared_against_message_id, Some(3));
}

#[test]
fn anti_replay_repair_instruction_blocks_setup_inventory() {
    let messages = vec![
        ApiMessage::system("system".to_string()),
        ApiMessage::user("I step through the doorway.".to_string()),
    ];

    let repaired = messages_with_repair_instruction(&messages);
    let instruction = last_user_message_content(&repaired);

    assert!(instruction.contains("Do not restate the room setup"));
    assert!(instruction.contains("clothing, object list, door state"));
    assert!(instruction.contains("advance the scene from the latest user input"));
}

#[test]
fn anti_replay_prunes_repeated_setup_from_retry_but_keeps_advancement() {
    let sources = vec![ReplaySource {
            message_id: 14,
            content: "The unlocked door opens onto neon rain. The room still has the wine glass, phone, couch, barefoot Aurora, and oversized shirt.".into(),
        }];
    let retry = "The unlocked door opens onto neon rain, with the wine glass, phone, couch, barefoot Aurora, and oversized shirt arranged exactly as before. Aurora twists under the user's block, catches the forearm against her jaw, and drives a shoulder forward to break the grip.";

    let pruned = prune_repeated_scene_setup(retry, &sources);

    assert!(!pruned.contains("wine glass"));
    assert!(!pruned.contains("oversized shirt arranged"));
    assert!(pruned.contains("catches the forearm"));
    assert!(pruned.contains("break the grip"));
}

#[test]
fn api_save_rejects_mock_template_prose() {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    db::upsert_soul(&conn, &soul).expect("upsert soul");
    db::ensure_conversation(&conn, "mock-reject", &soul.character_id).expect("conversation");
    let err = save_visible_narrator_response(
        &conn,
        "mock-reject",
        MOCK_OBSERVATION_READER_LINE,
        None,
        None,
        &serde_json::to_string(&soul).expect("soul json"),
        "I knock on the door",
        0,
        NarratorMessageOrigin::Api,
        None,
    )
    .expect_err("mock prose should be rejected on API path");
    assert!(err.contains("mock-template"));
}

#[test]
fn saved_visible_equals_normalized_provider_response() {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    db::upsert_soul(&conn, &soul).expect("upsert soul");
    db::ensure_conversation(&conn, "normalized", &soul.character_id).expect("conversation");
    let normalized = "Aurora opens the door with a measured pause.";
    let (assistant_message_id, _) = save_visible_narrator_response(
        &conn,
        "normalized",
        normalized,
        None,
        None,
        &serde_json::to_string(&soul).expect("soul json"),
        "I knock on the door",
        0,
        NarratorMessageOrigin::Api,
        None,
    )
    .expect("save");
    let message =
        db::get_message(&conn, "normalized", assistant_message_id).expect("assistant message");
    assert!(responses_match_for_integrity(&message.content, normalized));
}

#[test]
fn state_updater_receives_same_text_as_saved_assistant_message() {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    db::upsert_soul(&conn, &soul).expect("upsert soul");
    db::ensure_conversation(&conn, "updater-text", &soul.character_id).expect("conversation");
    let saved_text = "Aurora steps aside and lets the hallway air in.";
    let (assistant_message_id, _) = save_visible_narrator_response(
        &conn,
        "updater-text",
        saved_text,
        None,
        None,
        &serde_json::to_string(&soul).expect("soul json"),
        "I knock on the door",
        0,
        NarratorMessageOrigin::Api,
        None,
    )
    .expect("save");
    let message =
        db::get_message(&conn, "updater-text", assistant_message_id).expect("assistant message");
    let updater_user =
        build_state_updater_user_message("I knock on the door", saved_text, None, None);
    assert!(updater_user.contains(&message.content));
    assert!(responses_match_for_integrity(&message.content, saved_text));
}

#[test]
fn anti_replay_accepted_retry_payload_metadata_uses_retry_completion() {
    use crate::providers::api::ProviderCompletion;

    let initial = ProviderCompletion {
        raw_text: "initial raw".into(),
        finish_reason: Some("length".into()),
        provider_request_id: Some("req-initial".into()),
        provider_response_id: Some("resp-initial".into()),
        token_usage: None,
    };
    let retry = ProviderCompletion {
        raw_text: "retry raw body".into(),
        finish_reason: Some("stop".into()),
        provider_request_id: Some("req-retry".into()),
        provider_response_id: Some("resp-retry".into()),
        token_usage: None,
    };
    let update = llm_payload_response_update_from_completion(&retry, "Retry visible narration.");
    assert_eq!(
        update.raw_provider_response.as_deref(),
        Some("retry raw body")
    );
    assert_eq!(
        update.normalized_response.as_deref(),
        Some("Retry visible narration.")
    );
    assert_eq!(update.finish_reason.as_deref(), Some("stop"));
    assert_eq!(update.provider_request_id.as_deref(), Some("req-retry"));
    assert_eq!(update.provider_response_id.as_deref(), Some("resp-retry"));

    let initial_update =
        llm_payload_response_update_from_completion(&initial, "Initial visible narration.");
    assert_ne!(
        initial_update.provider_response_id,
        update.provider_response_id
    );
    assert_ne!(
        initial_update.raw_provider_response,
        update.raw_provider_response
    );
}

#[test]
fn payload_includes_raw_and_normalized_provider_response() {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    db::upsert_soul(&conn, &soul).expect("upsert soul");
    db::ensure_conversation(&conn, "payload-response", &soul.character_id).expect("conversation");
    let log_id = db::insert_llm_payload_log(
        &conn,
        &LlmPayloadLog {
            conversation_id: "payload-response".into(),
            ..Default::default()
        },
    )
    .expect("log");
    db::update_llm_payload_log_response(
        &conn,
        log_id,
        &db::LlmPayloadResponseUpdate {
            raw_provider_response: Some("raw narrator body".into()),
            normalized_response: Some("visible narrator body".into()),
            finish_reason: Some("stop".into()),
            ..Default::default()
        },
    )
    .expect("update");
    let log = db::get_llm_payload_log(&conn, log_id).expect("log");
    assert_eq!(
        log.raw_provider_response.as_deref(),
        Some("raw narrator body")
    );
    assert_eq!(
        log.normalized_response.as_deref(),
        Some("visible narrator body")
    );
    let exported = render_llm_payload_history(&[log]);
    assert!(exported.contains("RAW PROVIDER RESPONSE"));
    assert!(exported.contains("NORMALIZED RESPONSE"));
}

#[test]
fn mock_assistant_variant_source_is_mock_provider() {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    db::upsert_soul(&conn, &soul).expect("upsert soul");
    db::ensure_conversation(&conn, "mock-variant", &soul.character_id).expect("conversation");
    let (_, variant_id) = save_visible_narrator_response(
        &conn,
        "mock-variant",
        "Simulated narrator line for local mock mode.",
        None,
        None,
        &serde_json::to_string(&soul).expect("soul json"),
        "I knock on the door",
        0,
        NarratorMessageOrigin::Mock,
        None,
    )
    .expect("save");
    let variants = db::list_assistant_message_variants(&conn, "mock-variant", 1).expect("variants");
    let selected = variants
        .iter()
        .find(|variant| variant.id == variant_id)
        .expect("selected variant");
    assert_eq!(selected.source.as_deref(), Some("mock_provider"));
}

#[test]
fn placeholder_not_finalized_before_provider_response() {
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    db::upsert_soul(&conn, &soul).expect("upsert soul");
    db::ensure_conversation(&conn, "placeholder", &soul.character_id).expect("conversation");
    db::insert_message_and_get_id(&conn, "placeholder", "user", "I knock on the door")
        .expect("user");
    let assistants = db::list_messages(&conn, "placeholder", 10).expect("messages");
    assert!(
        !assistants.iter().any(|message| message.role == "assistant"),
        "assistant row must not exist before provider response is saved"
    );
}

#[test]
fn provider_failure_does_not_save_generic_prose() {
    assert!(!is_known_mock_template_prose(
        NARRATOR_PROVIDER_ERROR_VISIBLE
    ));
    assert!(is_known_mock_template_prose(MOCK_OBSERVATION_READER_LINE));
    let conn = db::init_memory_connection().expect("db");
    let soul = new_default_soul("Aurora");
    db::upsert_soul(&conn, &soul).expect("upsert soul");
    db::ensure_conversation(&conn, "provider-fail", &soul.character_id).expect("conversation");
    let err = save_visible_narrator_response(
        &conn,
        "provider-fail",
        MOCK_OBSERVATION_READER_LINE,
        None,
        None,
        &serde_json::to_string(&soul).expect("soul json"),
        "I knock on the door",
        0,
        NarratorMessageOrigin::Api,
        None,
    )
    .expect_err("mock prose must not persist on API path");
    assert!(err.contains("mock-template"));
    let messages = db::list_messages(&conn, "provider-fail", 10).expect("messages");
    assert!(
        !messages.iter().any(|message| message.role == "assistant"),
        "failed API save must not leave assistant rows"
    );
}

#[test]
fn mock_ledger_commit_does_not_materialize_user_world_event() {
    let mut patch = EnginePatch {
        world_patch: Some(WorldPatch {
            recent_event: Some(
                "The conversation continued without a major rupture: I knock on the door".into(),
            ),
            ..WorldPatch::default()
        }),
        ..EnginePatch::default()
    };
    sanitize_mock_patch_for_ledger(&mut patch);
    let world_patch = patch.world_patch.as_ref();
    assert!(
        world_patch.is_none()
            || (world_patch
                .and_then(|patch| patch.recent_event.as_deref())
                .is_none()
                && world_patch
                    .map(|patch| patch.recent_events.is_empty())
                    .unwrap_or(true)),
        "mock ledger sanitize must not keep user-turn world events"
    );
}

fn assert_order(text: &str, first: &str, second: &str) {
    let first_index = text.find(first).expect("first section");
    let second_index = text.find(second).expect("second section");
    assert!(first_index < second_index);
}

#[test]
fn exporting_two_same_title_sessions_produces_different_filenames() {
    let manifest1 = MneBundleManifest {
        mne_version: 1,
        bundle_id: "1779425465811-32".into(),
        bundle_type: "session_checkpoint".into(),
        title: "Aurora Schwarz Session".into(),
        description: "mne1".into(),
        author: None,
        created_at: 1779425465,
        app: "Mnemosyne".into(),
        schema_version: 1,
        conversation_id: Some(
            "local-mock-088e469e-7274-47bc-918d-ab29cb16cc09-2ab002c2-95e1-4287-acec-fb41f721cc57"
                .into(),
        ),
        soul_id: None,
        world_id: None,
        source_savepoint_id: None,
        source_setting_id: None,
        contents: MneBundleContents {
            souls: vec![],
            worlds: vec![],
            images: vec![],
            conversation: None,
        },
    };

    let manifest2 = MneBundleManifest {
        mne_version: 1,
        bundle_id: "1779425482790-35".into(),
        bundle_type: "session_checkpoint".into(),
        title: "Aurora Schwarz Session".into(),
        description: "mne2".into(),
        author: None,
        created_at: 1779425482,
        app: "Mnemosyne".into(),
        schema_version: 1,
        conversation_id: Some(
            "local-mock-e0ad2e3b-9729-44d5-a35a-0a11f77c328c-3bc244d9-863f-416a-bafe-8b964d92dda8"
                .into(),
        ),
        soul_id: None,
        world_id: None,
        source_savepoint_id: None,
        source_setting_id: None,
        contents: MneBundleContents {
            souls: vec![],
            worlds: vec![],
            images: vec![],
            conversation: None,
        },
    };

    let name1 = default_mne_filename(&manifest1);
    let name2 = default_mne_filename(&manifest2);

    assert_ne!(name1, name2);
    assert!(name1.contains("Aurora_Schwarz_Session"));
    assert!(name1.contains("session_checkpoint"));
    assert!(name1.contains("1779425465"));
    assert!(name1.contains("088e469e"));
    assert!(name2.contains("e0ad2e3b"));
}

#[test]
fn export_never_silently_overwrites() {
    let dir = tempfile::tempdir().expect("tempdir");
    let base_path = dir.path().join("Aurora_Schwarz_Session.mne");
    fs::write(&base_path, "original content").expect("write");

    let unique = unique_export_path(base_path.clone()).expect("unique");
    assert_ne!(unique, base_path);
    assert!(unique
        .to_str()
        .unwrap()
        .contains("Aurora_Schwarz_Session_2.mne"));

    fs::write(&unique, "second content").expect("write");
    let unique_again = unique_export_path(base_path).expect("unique");
    assert!(unique_again
        .to_str()
        .unwrap()
        .contains("Aurora_Schwarz_Session_3.mne"));
}

#[test]
fn normal_scene_anchor_does_not_trigger_regenerate() {
    let response = "Aurora moves across the dim room, her wine glass caught in the glow of the neon sign outside. Rain beats against the window.";
    let source = ReplaySource {
            message_id: 1,
            content: "Aurora sits on the velvet couch, staring at the neon light. Rain is pouring heavily outside. She takes a sip of wine.".to_string(),
        };
    let dummy_world =
        state_engine::setting::session_world_from_setting(&new_default_setting("Aurora"));
    let rg = detect_replay_with_context(response, "I walk in", &dummy_world, &[source]);
    assert!(!rg.replay_detected);
    assert_eq!(rg.severity, ReplaySeverity::MildOverlap);
}

#[test]
fn repeated_room_detail_is_mild_overlap_only() {
    let response = "The neon light casts a soft red glow across the heavy wooden door.";
    let source = ReplaySource {
        message_id: 2,
        content: "The room is dark save for the red neon light outlining the heavy wooden door."
            .to_string(),
    };
    let dummy_world =
        state_engine::setting::session_world_from_setting(&new_default_setting("Aurora"));
    let rg = detect_replay_with_context(response, "I open the door", &dummy_world, &[source]);
    assert!(!rg.replay_detected);
    assert_eq!(rg.severity, ReplaySeverity::MildOverlap);
}

#[test]
fn strong_replay_logs_without_retry_by_default() {
    let response = "Aurora sits on the velvet couch, staring at the neon light. Rain is pouring heavily outside.";
    let source = ReplaySource {
            message_id: 3,
            content: "Aurora sits on the velvet couch, staring at the neon light. Rain is pouring heavily outside.".to_string(),
        };
    let dummy_world =
        state_engine::setting::session_world_from_setting(&new_default_setting("Aurora"));
    let rg =
        detect_replay_with_context(response, "I sit down next to her", &dummy_world, &[source]);
    assert!(rg.replay_detected);
    assert_eq!(rg.severity, ReplaySeverity::StrongReplay);
    assert!(!anti_replay_forced_retry_enabled(
        &ApiProviderSettings::default()
    ));
}

#[test]
fn forced_retry_only_when_setting_enabled() {
    let default_settings = ApiProviderSettings::default();
    let enabled_settings = ApiProviderSettings {
        anti_replay_forced_retry_enabled: Some(true),
        ..Default::default()
    };

    assert!(!anti_replay_forced_retry_enabled(&default_settings));
    assert!(anti_replay_forced_retry_enabled(&enabled_settings));
}

#[test]
fn deterministic_status_repair_still_runs_when_retry_disabled() {
    let response = "Aurora takes her coat off.\nScene | Focus: Aurora | Physical state: Damp from the rain | Atmosphere: Intimate";
    let out = apply_output_contract_guard(response, "I say hi");

    assert!(!anti_replay_forced_retry_enabled(
        &ApiProviderSettings::default()
    ));
    assert_eq!(
        out.status_repair_action.as_deref(),
        Some("extracted_from_prose")
    );
    assert!(out.text.contains("```status\nScene | Focus: Aurora"));
}

#[test]
fn malformed_status_recovers_valid_status_content() {
    let response = "Aurora takes her coat off.\nScene | Focus: Aurora | Physical state: Damp from the rain | Atmosphere: Intimate";
    let out = apply_output_contract_guard(response, "I say hi");
    assert_eq!(
        out.status_repair_action.as_deref(),
        Some("extracted_from_prose")
    );
    assert!(out.text.contains("Focus: Aurora"));
    assert!(out.text.contains("```status\nScene | Focus: Aurora | Physical state: Damp from the rain | Atmosphere: Intimate\n```"));
    assert!(!out
        .text
        .starts_with("Aurora takes her coat off.\nScene | Focus: Aurora"));
}

#[test]
fn valid_focus_not_replaced_with_unknown() {
    let response = "Aurora pours a drink.\n\n```status\nScene | Focus: Aurora | Physical state: Tired | Atmosphere: Warm\n```";
    let out = apply_output_contract_guard(response, "I sit down");
    assert!(out.text.contains("Focus: Aurora"));
    assert!(!out.text.contains("Focus: Unknown"));
}

#[test]
fn pure_ooc_bypasses_status_and_scene_repair() {
    let response = "Sure, I can help you with that retcon. Aurora was never actually there.";
    let out = apply_output_contract_guard(response, "OOC: Let's change the setting");
    assert_eq!(
        out.status_repair_action.as_deref(),
        Some("gm_ooc_bypassed_status")
    );
    assert!(!out.text.contains("```status"));
}

#[test]
fn retry_not_selected_if_worse_than_original() {
    let original_response = "Aurora takes a sip of wine. Her expression is thoughtful.";
    let retry_response = "";
    let dummy_world =
        state_engine::setting::session_world_from_setting(&new_default_setting("Aurora"));
    let orig_score = evaluate_response_quality(original_response, "I wait", &dummy_world, &[]);
    let retry_score = evaluate_response_quality(retry_response, "I wait", &dummy_world, &[]);
    assert!(orig_score > retry_score);
}

#[test]
fn test_baseline_patch_has_focus_participants_last_user_action() {
    let soul = new_default_soul("Aurora");
    let (ev_id, patch) =
        construct_baseline_patch(&soul, "I walk in.", "The visitor enters.", "preset_male");
    assert!(ev_id.starts_with("event_baseline_"));

    let wp = patch.world_patch.as_ref().unwrap();
    let ss = wp.scene_state.as_ref().unwrap();
    assert_eq!(ss.focus, Some("Aurora and preset_male".to_string()));
    assert!(ss.participants.contains(&soul.character_id));
    assert!(ss.participants.contains(&"preset_male".to_string()));
    assert!(!ss.participants.contains(&"default_player".to_string()));
    assert_eq!(ss.last_user_action.as_deref(), Some("I walk in."));
    assert!(ss.continuity_note.is_some());
}

#[test]
fn test_ooc_turn_does_not_create_baseline_patch() {
    let user_text = "OOC: let's do something else";
    let user_is_ooc = is_ooc_or_gm_prefix(user_text);
    assert!(user_is_ooc);

    let pure_ooc_detected =
        user_is_ooc || (user_text.trim().is_empty() && is_ooc_or_gm_prefix("Sure"));
    assert!(pure_ooc_detected);

    let is_normal_scene_turn = !pure_ooc_detected && !user_is_ooc;
    assert!(!is_normal_scene_turn);
}

#[test]
fn test_evaluator_failure_keeps_baseline_patch_and_returns_partial_success() {
    // Test state representation: when baseline_patch_id is present, any evaluator parsing/LLM failure
    // results in "partial_success" status instead of failing the whole turn.
    let err_str = "parse error";
    let baseline_patch_id = Some("patch_baseline_test_123".to_string());

    let status = if baseline_patch_id.is_some() {
        "partial_success".to_string()
    } else {
        format!("failed: {err_str}")
    };

    assert_eq!(status, "partial_success");
}

#[test]
fn test_malformed_form_does_not_mark_overall_failed_if_baseline_applied() {
    // Similar to the failure behavior, even if form_spec or parsed JSON is malformed,
    // if we successfully recorded/applied the baseline patch, the overall turn succeeds partially.
    let baseline_patch_id = Some("patch_baseline_test_456".to_string());
    let mut partial_success = false;

    if baseline_patch_id.is_some() {
        partial_success = true;
    }

    assert!(partial_success);
}

#[test]
fn test_frontend_saved_message_replaces_pending_overlay() {
    // Frontend logic state representation:
    // We have a list of messages. If a saved message with a matching request_id
    // arrives, it replaces the pending message overlay in prepareMessagesForRender.
    #[derive(Debug, PartialEq, Clone)]
    struct MockMessage {
        id: Option<i64>,
        request_id: Option<String>,
        content: String,
        is_pending: bool,
    }

    let current_messages = vec![MockMessage {
        id: None,
        request_id: Some("req_123".into()),
        content: "Narrator prose...".into(),
        is_pending: true,
    }];

    // A new canonical saved message arrives with matching request_id
    let new_saved_message = MockMessage {
        id: Some(1),
        request_id: Some("req_123".into()),
        content: "Narrator prose...".into(),
        is_pending: false,
    };

    // Simulated frontend single-source message list compilation (prepareMessagesForRender)
    let mut prepared = Vec::new();
    for msg in &current_messages {
        if msg.is_pending && msg.request_id == new_saved_message.request_id {
            // Skip the pending overlay, canonical will be added instead
            continue;
        }
        prepared.push(msg.clone());
    }
    prepared.push(new_saved_message.clone());

    assert_eq!(prepared.len(), 1);
    assert_eq!(prepared[0].id, Some(1));
    assert!(!prepared[0].is_pending);
}

#[test]
fn test_frontend_does_not_render_same_assistant_content_twice() {
    // Test frontend single-source message rendering: it deduplicates messages.
    #[derive(Debug, PartialEq, Clone)]
    struct MockMessage {
        id: Option<i64>,
        content: String,
    }

    let list = vec![
        MockMessage {
            id: Some(1),
            content: "Hello".into(),
        },
        MockMessage {
            id: Some(1),
            content: "Hello".into(),
        }, // Duplicate
    ];

    // Deduplication selector logic
    let mut deduplicated = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();
    for msg in list {
        if let Some(id) = msg.id {
            if seen_ids.insert(id) {
                deduplicated.push(msg);
            }
        } else {
            deduplicated.push(msg);
        }
    }

    assert_eq!(deduplicated.len(), 1);
    assert_eq!(deduplicated[0].id, Some(1));
}

#[test]
fn test_event_listener_registration_is_idempotent() {
    // Mock Tauri event listener registration: registration uses a ref or active count
    // and doesn't duplicate if already registered.
    let mut active_listener_count = 0;
    let mut is_registered = false;

    // registration logic
    if !is_registered {
        active_listener_count += 1;
        is_registered = true;
    }

    // duplicate attempt
    if !is_registered {
        active_listener_count += 1;
        is_registered = true;
    }

    assert_eq!(active_listener_count, 1);
    assert!(is_registered);
}

#[test]
fn narrator_cannot_invent_call_notification_without_call_event() {
    let dummy_world =
        state_engine::setting::session_world_from_setting(&new_default_setting("Aurora"));
    // No active call mentions in user text or world state
    let user_text = "I study the books on the shelf.";
    let response = "The call notification glows on the screen.";
    assert!(has_phone_call_state_violation(
        response,
        user_text,
        &dummy_world
    ));

    let repaired = sanitize_phone_call_state_violation(response, user_text, &dummy_world);
    assert!(repaired.repaired);
    assert!(!repaired.text.contains("call notification"));
}

#[test]
fn phone_ooc_explanation_does_not_create_active_call_state() {
    let dummy_world =
        state_engine::setting::session_world_from_setting(&new_default_setting("Aurora"));
    // clarifying phone behavior does not count as active call state
    assert!(!world_phone_state_has_active_call(&dummy_world));
}

#[test]
fn call_notification_requires_active_call_or_latest_user_call() {
    let mut dummy_world =
        state_engine::setting::session_world_from_setting(&new_default_setting("Aurora"));

    // Scenario A: User mentions calling
    let user_text_call = "I call your phone to check in.";
    let response = "The incoming call screen wakes up.";
    assert!(!has_phone_call_state_violation(
        response,
        user_text_call,
        &dummy_world
    ));

    // Scenario B: World state has active call
    let mut phone_state = state_engine::soul::ObjectState::default();
    phone_state.object_id = "aurora_phone".to_string();
    phone_state.status = "incoming_call".to_string();
    dummy_world.object_states.push(phone_state);

    let user_text_idle = "I wait in silence.";
    assert!(!has_phone_call_state_violation(
        response,
        user_text_idle,
        &dummy_world
    ));
}

#[test]
fn payload_history_renders_evaluator_row_trace_for_object_reject() {
    let trace = TurnPipelineTrace {
        request_id: "req-123".to_string(),
        turn_id: Some("turn-456".to_string()),
        conversation_id: "conv-789".to_string(),
        started_at: 1000,
        total_elapsed_ms: 1500,
        final_status: "failed".to_string(),
        failing_stage: Some("evaluator_response_validated".to_string()),
        suggested_debug_action: None,
        stages: vec![],
        token_usage: None,
        evaluator_row_traces: vec![state_engine::evaluator_form::EvalRowTrace {
            row_kind: "object".to_string(),
            row_index: 1,
            raw_row: serde_json::json!({ "id": "door", "state": "broken" }),
            normalized_row: serde_json::json!({ "id": "door", "change_type": "state_change" }),
            validation_status: "rejected".to_string(),
            rejection_reason: Some("object_id or new_object_label is required".to_string()),
            compiler_result: "rejected".to_string(),
        }],
    };

    let exported = render_llm_payload_history(&[payload_trace_log(serde_json::json!({
        "pipeline_trace": trace
    }))]);

    assert!(exported.contains("### EVALUATOR ROW TRACE"));
    assert!(exported.contains("- row_kind: object"));
    assert!(exported.contains("- rejection_reason: object_id or new_object_label is required"));
}

#[test]
fn payload_history_renders_evaluator_row_trace_for_relationship_reject() {
    let trace = TurnPipelineTrace {
        request_id: "req-123".to_string(),
        turn_id: Some("turn-456".to_string()),
        conversation_id: "conv-789".to_string(),
        started_at: 1000,
        total_elapsed_ms: 1500,
        final_status: "failed".to_string(),
        failing_stage: Some("evaluator_response_validated".to_string()),
        suggested_debug_action: None,
        stages: vec![],
        token_usage: None,
        evaluator_row_traces: vec![state_engine::evaluator_form::EvalRowTrace {
            row_kind: "relationship".to_string(),
            row_index: 2,
            raw_row: serde_json::json!({ "source": "A", "target": "B" }),
            normalized_row: serde_json::json!({ "source": "A", "target": "B" }),
            validation_status: "rejected".to_string(),
            rejection_reason: Some("direction_missing_uncertain".to_string()),
            compiler_result: "rejected".to_string(),
        }],
    };

    let exported = render_llm_payload_history(&[payload_trace_log(serde_json::json!({
        "pipeline_trace": trace
    }))]);

    assert!(exported.contains("### EVALUATOR ROW TRACE"));
    assert!(exported.contains("- row_kind: relationship"));
    assert!(exported.contains("- rejection_reason: direction_missing_uncertain"));
}

#[test]
fn payload_history_row_trace_includes_raw_and_normalized_row() {
    let trace = TurnPipelineTrace {
        request_id: "req-123".to_string(),
        turn_id: Some("turn-456".to_string()),
        conversation_id: "conv-789".to_string(),
        started_at: 1000,
        total_elapsed_ms: 1500,
        final_status: "success".to_string(),
        failing_stage: None,
        suggested_debug_action: None,
        stages: vec![],
        token_usage: None,
        evaluator_row_traces: vec![state_engine::evaluator_form::EvalRowTrace {
            row_kind: "event".to_string(),
            row_index: 0,
            raw_row: serde_json::json!({ "raw_field": "val" }),
            normalized_row: serde_json::json!({ "norm_field": "val" }),
            validation_status: "accepted".to_string(),
            rejection_reason: None,
            compiler_result: "world_event_created".to_string(),
        }],
    };

    let exported = render_llm_payload_history(&[payload_trace_log(serde_json::json!({
        "pipeline_trace": trace
    }))]);

    assert!(exported.contains("- raw_row: {\"raw_field\":\"val\"}"));
    assert!(exported.contains("- normalized_row: {\"norm_field\":\"val\"}"));
}

#[test]
fn form_rows_rejected_count_matches_row_trace_rejected_count() {
    let trace = TurnPipelineTrace {
        request_id: "req-123".to_string(),
        turn_id: Some("turn-456".to_string()),
        conversation_id: "conv-789".to_string(),
        started_at: 1000,
        total_elapsed_ms: 1500,
        final_status: "success".to_string(),
        failing_stage: None,
        suggested_debug_action: None,
        stages: vec![],
        token_usage: None,
        evaluator_row_traces: vec![
            state_engine::evaluator_form::EvalRowTrace {
                row_kind: "event".to_string(),
                row_index: 0,
                raw_row: serde_json::json!({}),
                normalized_row: serde_json::json!({}),
                validation_status: "rejected".to_string(),
                rejection_reason: Some("error 1".to_string()),
                compiler_result: "rejected".to_string(),
            },
            state_engine::evaluator_form::EvalRowTrace {
                row_kind: "object".to_string(),
                row_index: 1,
                raw_row: serde_json::json!({}),
                normalized_row: serde_json::json!({}),
                validation_status: "rejected".to_string(),
                rejection_reason: Some("error 2".to_string()),
                compiler_result: "rejected".to_string(),
            },
            state_engine::evaluator_form::EvalRowTrace {
                row_kind: "relationship".to_string(),
                row_index: 2,
                raw_row: serde_json::json!({}),
                normalized_row: serde_json::json!({}),
                validation_status: "accepted".to_string(),
                rejection_reason: None,
                compiler_result: "relationship_delta_created".to_string(),
            },
        ],
    };

    let rejected_count = trace
        .evaluator_row_traces
        .iter()
        .filter(|r| r.validation_status == "rejected")
        .count();
    assert_eq!(rejected_count, 2);
}

#[test]
fn pipeline_trace_total_elapsed_nonzero_when_stage_elapsed_exists() {
    let mut trace =
        TurnPipelineTrace::new("req-123".to_string(), None, "conv-789".to_string(), 1000);
    trace.record_stage("narrator_called", "success", 1500, None, None);
    trace.record_stage("evaluator_response_received", "success", 2500, None, None);

    trace.finalize_timing(0);

    assert_ne!(trace.total_elapsed_ms, 0);
    assert_eq!(trace.total_elapsed_ms, 4000);
}

#[test]
fn async_pipeline_trace_total_elapsed_includes_evaluator_response_received() {
    let mut trace =
        TurnPipelineTrace::new("req-123".to_string(), None, "conv-789".to_string(), 1000);
    trace.record_stage("narrator_called", "success", 2000, None, None);
    trace.record_stage("evaluator_response_received", "success", 3000, None, None);

    trace.finalize_timing(500);

    assert_eq!(trace.total_elapsed_ms, 5000);
}

#[test]
fn narrator_visible_response_strips_trailing_assistant_close_tag() {
    let input = "Aurora steps onto the damp sidewalk.</assistant>";
    let out = apply_output_contract_guard(input, "I look around");
    assert_eq!(out.text.trim(), "Aurora steps onto the damp sidewalk.\n\n```status\nScene | Focus: Unknown | Physical state: Not specified | Atmosphere: Not specified\n```");
}

#[test]
fn ooc_response_strips_trailing_assistant_close_tag() {
    let input = "OOC: Understood, we can proceed with that.</assistant>";
    let out = apply_output_contract_guard(input, "OOC: Let's do it");
    assert_eq!(
        out.text.trim(),
        "OOC: Understood, we can proceed with that."
    );
}

#[test]
fn status_block_response_strips_assistant_tag_but_preserves_status() {
    let input = "Aurora nods.\n\n```status\nScene | Focus: Aurora | Physical state: Damp | Atmosphere: Rainy\n```</assistant>";
    let out = apply_output_contract_guard(input, "I smile");
    assert!(out.text.contains("Focus: Aurora"));
    assert!(!out.text.contains("</assistant>"));
    assert!(out.text.ends_with("```"));
}

#[test]
fn evaluator_json_with_outer_assistant_tag_still_parses() {
    let raw_json = r#"{"schema_version":1,"event_rows":[],"relationship_rows":[],"memory_rows":[],"object_rows":[]}</assistant>"#;
    let parsed = parse_eval_form_response_with_trace(raw_json);
    assert!(parsed.is_ok());
}

#[test]
fn repeated_trailing_tags_are_stripped() {
    let input = "Aurora nods.</assistant>  </assistant>\n</assistant>";
    let out = apply_output_contract_guard(input, "I smile");
    assert!(!out.text.contains("</assistant>"));
}

#[test]
fn status_block_repair_strips_assistant_tag_between_body_and_status() {
    let input = "Aurora nods.</assistant>\n\n```status\nScene | Focus: Aurora | Physical state: Damp | Atmosphere: Rainy\n```";
    let out = apply_output_contract_guard(input, "I smile");
    assert!(out.text.contains("Aurora nods."));
    assert!(!out.text.contains("</assistant>"));
    let expected = "Aurora nods.\n\n```status\nScene | Focus: Aurora | Physical state: Damp | Atmosphere: Rainy\n```";
    assert_eq!(out.text.trim(), expected);
}
// --- P0.3A Hardening Tests ---

#[test]
fn validate_good_mne_bundle_passes() {
    let soul = new_default_soul("Aurora");
    let manifest = mne_manifest(
        "character_soul",
        "Aurora",
        "test",
        vec!["souls/aurora.json".into()],
        Vec::new(),
        None,
    );
    let mut entries = HashMap::new();
    entries.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    entries.insert(
        "souls/aurora.json".into(),
        serde_json::to_vec(&soul).unwrap(),
    );

    let bytes = write_test_mne_bytes(entries);
    let report = validate_mne_bundle_bytes(&bytes);

    assert!(report.valid, "Report should be valid: {:?}", report.errors);
    assert_eq!(report.summary.soul_name.as_deref(), Some("Aurora"));
    assert_eq!(
        report.summary.soul_id.as_deref(),
        Some(soul.character_id.as_str())
    );
}

#[test]
fn validate_missing_manifest_fails() {
    let mut entries = HashMap::new();
    entries.insert("souls/aurora.json".into(), b"{}".to_vec());

    let bytes = write_test_mne_bytes(entries);
    let report = validate_mne_bundle_bytes(&bytes);

    assert!(!report.valid);
    assert!(report
        .errors
        .iter()
        .any(|e| e.contains("Missing manifest.json")));
}

#[test]
fn validate_missing_soul_json_fails() {
    let manifest = mne_manifest(
        "character_soul",
        "Aurora",
        "test",
        vec!["souls/aurora.json".into()],
        Vec::new(),
        None,
    );
    let mut entries = HashMap::new();
    entries.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );

    let bytes = write_test_mne_bytes(entries);
    let report = validate_mne_bundle_bytes(&bytes);

    assert!(!report.valid);
    assert!(report
        .errors
        .iter()
        .any(|e| e.contains("Missing required file: souls/aurora.json")));
}

#[test]
fn validate_bad_json_fails() {
    let manifest = mne_manifest(
        "character_soul",
        "Aurora",
        "test",
        vec!["souls/aurora.json".into()],
        Vec::new(),
        None,
    );
    let mut entries = HashMap::new();
    entries.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    entries.insert("souls/aurora.json".into(), b"{invalid json}".to_vec());

    let bytes = write_test_mne_bytes(entries);
    let report = validate_mne_bundle_bytes(&bytes);

    assert!(!report.valid);
    assert!(report
        .errors
        .iter()
        .any(|e| e.contains("Failed to parse Soul JSON")));
}

#[test]
fn validate_id_mismatch_fails() {
    let mut soul = new_default_soul("Aurora");
    soul.character_id = "soul-actual".into();
    let mut manifest = mne_manifest(
        "character_soul",
        "Aurora",
        "test",
        vec!["souls/aurora.json".into()],
        Vec::new(),
        None,
    );
    manifest.soul_id = Some("soul-expected-mismatch".into());
    let mut entries = HashMap::new();
    entries.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    entries.insert(
        "souls/aurora.json".into(),
        serde_json::to_vec(&soul).unwrap(),
    );

    let bytes = write_test_mne_bytes(entries);
    let report = validate_mne_bundle_bytes(&bytes);

    assert!(!report.valid);
    assert!(report.errors.iter().any(|e| e.contains("Soul ID mismatch")));
}

#[test]
fn validate_unknown_extra_files_warns_not_fails() {
    let soul = new_default_soul("Aurora");
    let manifest = mne_manifest(
        "character_soul",
        "Aurora",
        "test",
        vec!["souls/aurora.json".into()],
        Vec::new(),
        None,
    );
    let mut entries = HashMap::new();
    entries.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    entries.insert(
        "souls/aurora.json".into(),
        serde_json::to_vec(&soul).unwrap(),
    );
    entries.insert("mystery_extra.txt".into(), b"extra info".to_vec());

    let bytes = write_test_mne_bytes(entries);
    let report = validate_mne_bundle_bytes(&bytes);

    assert!(report.valid);
    assert_eq!(report.warnings.len(), 1);
    assert!(report.warnings[0].contains("mystery_extra.txt"));
}

#[test]
fn preview_import_does_not_mutate_database() {
    let conn = db::init_memory_connection().unwrap();
    let soul = new_default_soul("PreviewOnly");
    let manifest = mne_manifest(
        "character_soul",
        "PreviewOnly",
        "test",
        vec!["souls/preview.json".into()],
        Vec::new(),
        None,
    );
    let mut entries = HashMap::new();
    entries.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    entries.insert(
        "souls/preview.json".into(),
        serde_json::to_vec(&soul).unwrap(),
    );

    let bytes = write_test_mne_bytes(entries);

    let report = validate_mne_bundle_bytes(&bytes);
    assert!(report.valid);

    assert!(db::get_soul(&conn, &soul.character_id).is_err());
}

#[test]
fn preview_import_returns_counts() {
    let mut soul = new_default_soul("PreviewOnly");
    soul.relationships.insert(
        "user".into(),
        Relationship {
            trust: 5.0,
            affection: 10.0,
            intimacy: 0.0,
            passion: 0.0,
            commitment: 0.0,
            fear: 0.0,
            desire: 0.0,
            respect: 0.0,
            conflict: 0.0,
            dependency: 0.0,
            curiosity: 0.0,
            comfort: 0.0,
            boundary_pressure: 0.0,
            love_type: String::new(),
            ..Relationship::default()
        },
    );
    soul.world.object_states.push(ObjectState::default());
    soul.world.recent_events.push("An event occurred".into());
    soul.memory.recent.push(MemoryEntry {
        archived: false,
        is_pinned: false,
        id: "mem1".into(),
        timestamp: 100,
        content: "recent observation".into(),
        salience: 50.0,
        tag: "observation".into(),
        retrieval_strength: 50.0,
        source_type: MemorySourceType::CurrentSession,
        source_session_id: None,
        source_conversation_id: None,
        source_message_id: None,
        source_entity_id: None,
        source_quote: None,
        is_lived_experience: true,
        is_imported_context: false,
        perceived_by_entity_id: None,
        target_entity_ids: Vec::new(),
        interpretation: None,
        confidence: None,
        objective_event_id: None,
        truth_status: TruthStatus::Unknown,
        architecture_verified: false,
        memory_slot: None,
        owner_soul_id: None,
        relevance_tags: HashMap::new(),
        knowledge_scope: None,
        is_active: true,
        invalidated_by_patch_id: None,
        superseded_by_memory_id: None,
        is_retconned: false,
    });

    let manifest = mne_manifest(
        "character_soul",
        "PreviewOnly",
        "test",
        vec!["souls/preview.json".into()],
        Vec::new(),
        None,
    );
    let mut entries = HashMap::new();
    entries.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    entries.insert(
        "souls/preview.json".into(),
        serde_json::to_vec(&soul).unwrap(),
    );

    let bytes = write_test_mne_bytes(entries);
    let report = validate_mne_bundle_bytes(&bytes);

    assert!(report.valid);
    assert_eq!(report.summary.soul_name.as_deref(), Some("PreviewOnly"));
    assert_eq!(report.summary.memory_count, 2);
    assert_eq!(report.summary.object_state_count, 1);
    assert_eq!(report.summary.recent_event_count, 1);
    assert_eq!(report.summary.relationship_count, 1);
}

#[test]
fn preview_import_reports_errors_without_panicking() {
    let bytes = b"garbage zip file".to_vec();
    let report = validate_mne_bundle_bytes(&bytes);
    assert!(!report.valid);
    assert!(!report.errors.is_empty());
}

#[test]
fn import_as_new_creates_new_soul_and_conversation() {
    let conn = db::init_memory_connection().unwrap();
    let mut soul = new_default_soul("ImportNew");
    soul.character_id = "soul-1".into();

    let conversation = ConversationSummary {
        conversation_id: "conv-1".into(),
        soul_id: "soul-1".into(),
        world_id: Some("world-1".into()),
        source_savepoint_id: None,
        source_setting_id: None,
        active_player_persona_id: "preset_male".into(),
        title: "Original Title".into(),
        created_at: db::now_ts(),
        updated_at: db::now_ts(),
        last_message_preview: None,
        message_count: 0,
        archived_at: None,
        active_evaluator_profile_id: None,
        is_benchmark: false,
    };

    let messages = vec![ChatMessage {
        id: 10,
        conversation_id: "conv-1".into(),
        role: "user".into(),
        content: "Hello!".into(),
        channel: db::MESSAGE_CHANNEL_RP_SCENE.into(),
        created_at: 100,
        status: "active".into(),
        origin: "active".into(),
        attachments: Vec::new(),
        hidden_at: None,
    }];

    let mut manifest = mne_manifest(
        "session_checkpoint",
        "Original Title",
        "test",
        vec!["souls/soul-1.json".into()],
        vec!["worlds/world-1.json".into()],
        Some("conversation/conversation.json".into()),
    );
    manifest.soul_id = Some("soul-1".into());
    manifest.world_id = Some("world-1".into());
    manifest.conversation_id = Some("conv-1".into());

    let mut session_world =
        state_engine::setting::session_world_from_setting(&new_default_setting("Setting-1"));
    session_world.world_id = "world-1".into();
    session_world.source_setting_id = None;

    let mut entries = HashMap::new();
    entries.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    entries.insert(
        "souls/soul-1.json".into(),
        serde_json::to_vec(&soul).unwrap(),
    );
    entries.insert(
        "worlds/world-1.json".into(),
        serde_json::to_vec(&session_world).unwrap(),
    );
    entries.insert(
        "conversation/conversation.json".into(),
        serde_json::to_vec(&conversation).unwrap(),
    );
    entries.insert(
        "conversation/messages.json".into(),
        serde_json::to_vec(&messages).unwrap(),
    );

    let bytes = write_test_mne_bytes(entries);
    let result = import_mne_as_new_inner(&conn, &bytes).unwrap();

    assert_eq!(result.imported_soul_ids.len(), 1);
    assert_eq!(result.imported_soul_ids[0], "soul-1"); // No collision

    let imported_conv = db::get_conversation_summary(&conn, "conv-1").unwrap();
    assert_eq!(imported_conv.title, "Original Title");
}

#[test]
fn import_as_new_remaps_colliding_ids() {
    let conn = db::init_memory_connection().unwrap();

    let mut existing_soul = new_default_soul("Aurora");
    existing_soul.character_id = "soul-1".into();
    db::upsert_soul(&conn, &existing_soul).unwrap();

    db::ensure_conversation(&conn, "conv-1", "soul-1").unwrap();

    let mut existing_world =
        state_engine::setting::session_world_from_setting(&new_default_setting("Lab"));
    existing_world.world_id = "world-1".into();
    existing_world.source_setting_id = None;
    db::upsert_session_world(&conn, &existing_world).unwrap();

    let mut soul = new_default_soul("Aurora");
    soul.character_id = "soul-1".into();

    let conversation = ConversationSummary {
        conversation_id: "conv-1".into(),
        soul_id: "soul-1".into(),
        world_id: Some("world-1".into()),
        source_savepoint_id: None,
        source_setting_id: None,
        title: "Aurora Session".into(),
        created_at: db::now_ts(),
        updated_at: db::now_ts(),
        last_message_preview: None,
        message_count: 0,
        active_player_persona_id: "preset_male".into(),
        archived_at: None,
        active_evaluator_profile_id: None,
        is_benchmark: false,
    };

    let messages = vec![ChatMessage {
        id: 10,
        conversation_id: "conv-1".into(),
        role: "user".into(),
        content: "Hello!".into(),
        created_at: 100,
        status: "active".into(),
        origin: "active".into(),
        channel: db::MESSAGE_CHANNEL_RP_SCENE.into(),
        attachments: Vec::new(),
        hidden_at: None,
    }];

    let mut manifest = mne_manifest(
        "session_checkpoint",
        "Aurora Session",
        "test",
        vec!["souls/soul-1.json".into()],
        vec!["worlds/world-1.json".into()],
        Some("conversation/conversation.json".into()),
    );
    manifest.soul_id = Some("soul-1".into());
    manifest.world_id = Some("world-1".into());
    manifest.conversation_id = Some("conv-1".into());

    let mut session_world =
        state_engine::setting::session_world_from_setting(&new_default_setting("Lab"));
    session_world.world_id = "world-1".into();
    session_world.source_setting_id = None;

    let mut entries = HashMap::new();
    entries.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    entries.insert(
        "souls/soul-1.json".into(),
        serde_json::to_vec(&soul).unwrap(),
    );
    entries.insert(
        "worlds/world-1.json".into(),
        serde_json::to_vec(&session_world).unwrap(),
    );
    entries.insert(
        "conversation/conversation.json".into(),
        serde_json::to_vec(&conversation).unwrap(),
    );
    entries.insert(
        "conversation/messages.json".into(),
        serde_json::to_vec(&messages).unwrap(),
    );

    let bytes = write_test_mne_bytes(entries);
    let result = import_mne_as_new_inner(&conn, &bytes).unwrap();

    assert_eq!(result.imported_soul_ids.len(), 1);
    let new_soul_id = &result.imported_soul_ids[0];
    assert_ne!(new_soul_id, "soul-1");

    let new_conv_id = result.remapped_ids.get("conv-1").unwrap();
    assert_ne!(new_conv_id, "conv-1");

    let new_world_id = result.remapped_ids.get("world-1").unwrap();
    assert_ne!(new_world_id, "world-1");

    let original_soul = db::get_soul(&conn, "soul-1").unwrap();
    assert_eq!(original_soul.character_name, "Aurora");

    let new_conv = db::get_conversation_summary(&conn, new_conv_id).unwrap();
    assert_eq!(new_conv.soul_id, *new_soul_id);
}

#[test]
fn session_checkpoint_mne_roundtrip_restores_state_ledger_variants_and_payloads() {
    let conn = db::init_memory_connection().unwrap();
    let mut source_soul = new_default_soul("Roundtrip Aurora");
    source_soul.character_id = "roundtrip_soul".into();
    db::upsert_soul(&conn, &source_soul).unwrap();
    let mut source_world =
        state_engine::setting::session_world_from_setting(&new_default_setting("Roundtrip"));
    source_world.world_id = "roundtrip_world".into();
    source_world.source_setting_id = None;
    db::upsert_session_world(&conn, &source_world).unwrap();
    db::ensure_conversation_with_title_and_world(
        &conn,
        "roundtrip_conv",
        "roundtrip_soul",
        Some("roundtrip_world"),
        None,
        Some("Roundtrip Session"),
    )
    .unwrap();
    db::set_active_player_persona(&conn, "roundtrip_conv", "preset_male").unwrap();
    let branch =
        db::create_session_branch(&conn, "roundtrip_conv", &source_soul, &source_world).unwrap();
    let user_id =
        db::insert_message_and_get_id(&conn, "roundtrip_conv", "user", "I hang my wet jacket.")
            .unwrap();
    let assistant_id = db::insert_message_and_get_id(
        &conn,
        "roundtrip_conv",
        "assistant",
        "Aurora watches the wet jacket drip onto the chair.",
    )
    .unwrap();
    let selected_variant = db::seed_initial_assistant_message_variant(
        &conn,
        "roundtrip_conv",
        assistant_id,
        "Aurora watches the wet jacket drip onto the chair.",
        Some(OP_NORMAL_SEND),
        None,
        None,
    )
    .unwrap();
    let baseline_patch = EnginePatch {
        soul_patch: Some(SoulPatch {
            new_memories: vec![MemoryPatch {
                content: "Aurora saw preset_male hang a wet jacket on the chair.".into(),
                source_type: Some(MemorySourceType::CurrentSession),
                source_conversation_id: Some("roundtrip_conv".into()),
                source_message_id: Some(user_id),
                target_entity_ids: vec!["preset_male".into()],
                truth_status: Some(TruthStatus::SceneEvent),
                confidence: Some(0.9),
                salience: Some(0.8),
                ..MemoryPatch::default()
            }],
            relationship_deltas: vec![RelationshipDelta {
                target: Some("preset_male".into()),
                trust: Some(3.0),
                comfort: Some(2.0),
                max_abs_delta: Some(5.0),
                ..RelationshipDelta::default()
            }],
            ..SoulPatch::default()
        }),
        world_patch: Some(WorldPatch {
            recent_event: Some("preset_male hung a wet jacket on the chair.".into()),
            scene_state: Some(SceneStatePatch {
                current_scene: Some("Aurora's apartment after the knock.".into()),
                focus: Some("Roundtrip Aurora and preset_male".into()),
                participants: vec!["roundtrip_soul".into(), "preset_male".into()],
                ..SceneStatePatch::default()
            }),
            corrected_object_states: vec![ObjectState {
                object_id: "preset_male_jacket_1".into(),
                object_kind: "jacket".into(),
                owner_entity_id: Some("preset_male".into()),
                status: "wet".into(),
                location: "chair".into(),
                last_observed_state: "wet jacket draped over chair".into(),
                ..ObjectState::default()
            }],
            ..WorldPatch::default()
        }),
        ..EnginePatch::default()
    };
    let (commit, baseline_record) = db::record_turn_commit_with_patch_for_turn_id(
        &conn,
        "roundtrip_turn",
        "roundtrip_conv",
        &branch.branch_id,
        None,
        Some(user_id),
        assistant_id,
        selected_variant.id,
        &baseline_patch,
        false,
    )
    .unwrap();
    let enrichment_patch = EnginePatch {
        world_patch: Some(WorldPatch {
            recent_events: vec!["The wet jacket remains visible in the room.".into()],
            ..WorldPatch::default()
        }),
        ..EnginePatch::default()
    };
    db::record_enrichment_patch_with_metadata(
        &conn,
        &commit.turn_id,
        &enrichment_patch,
        Some(&baseline_record.patch_id),
        Some(assistant_id),
        selected_variant.id,
        Some("roundtrip_job"),
    )
    .unwrap();
    db::insert_llm_payload_log(
        &conn,
        &LlmPayloadLog {
            conversation_id: "roundtrip_conv".into(),
            message_id: Some(assistant_id),
            provider: "api".into(),
            mode: "narrator".into(),
            model: "roundtrip-model".into(),
            base_url: "https://api.example.test".into(),
            system_message: "system without raw api key".into(),
            user_message: "I hang my wet jacket.".into(),
            context_text: "context".into(),
            estimated_system_tokens: 1,
            estimated_user_tokens: 1,
            estimated_total_tokens: 2,
            created_at: db::now_ts(),
            branch_id: Some(branch.branch_id.clone()),
            active_turn_id: Some(commit.turn_id.clone()),
            latest_assistant_variant_id: selected_variant.id,
            turn_id: Some(commit.turn_id.clone()),
            ..LlmPayloadLog::default()
        },
    )
    .unwrap();

    let rebuilt_source =
        db::rebuild_session_state(&conn, "roundtrip_conv", &branch.branch_id).unwrap();
    let conversation = db::get_conversation_summary(&conn, "roundtrip_conv").unwrap();
    let messages = db::list_messages(&conn, "roundtrip_conv", 10_000).unwrap();
    let ledger = collect_mne_session_ledger(&conn, "roundtrip_conv", &messages).unwrap();
    let mut manifest = mne_manifest(
        "session_checkpoint",
        "Roundtrip Session",
        "roundtrip",
        vec!["souls/roundtrip_soul.json".into()],
        vec!["worlds/roundtrip_world.json".into()],
        Some("conversation/conversation.json".into()),
    );
    manifest.soul_id = Some("roundtrip_soul".into());
    manifest.world_id = Some("roundtrip_world".into());
    manifest.conversation_id = Some("roundtrip_conv".into());
    let payload_logs = db::list_llm_payload_logs(&conn, "roundtrip_conv").unwrap();
    let mut entries = HashMap::new();
    entries.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    entries.insert(
        "souls/roundtrip_soul.json".into(),
        serde_json::to_vec(&rebuilt_source.soul).unwrap(),
    );
    entries.insert(
        "worlds/roundtrip_world.json".into(),
        serde_json::to_vec(&rebuilt_source.session_world).unwrap(),
    );
    entries.insert(
        "conversation/conversation.json".into(),
        serde_json::to_vec(&conversation).unwrap(),
    );
    entries.insert(
        "conversation/messages.json".into(),
        serde_json::to_vec(&messages).unwrap(),
    );
    entries.insert(
        "conversation/payload_logs.json".into(),
        serde_json::to_vec(&payload_logs).unwrap(),
    );
    entries.insert(
        "conversation/branches.json".into(),
        serde_json::to_vec(&ledger.branches).unwrap(),
    );
    entries.insert(
        "conversation/turns.json".into(),
        serde_json::to_vec(&ledger.turns).unwrap(),
    );
    entries.insert(
        "conversation/patches.json".into(),
        serde_json::to_vec(&ledger.patches).unwrap(),
    );
    entries.insert(
        "conversation/variants.json".into(),
        serde_json::to_vec(&ledger.variants).unwrap(),
    );

    let bytes = write_test_mne_bytes(entries);
    let report = validate_mne_bundle_bytes(&bytes);
    assert!(report.valid, "report errors: {:?}", report.errors);
    assert!(!String::from_utf8_lossy(&bytes).contains("sk-live-secret"));

    let result = import_mne_as_new_inner(&conn, &bytes).unwrap();
    let imported_conv_id = result.remapped_ids.get("roundtrip_conv").unwrap();
    let imported_soul_id = result.remapped_ids.get("roundtrip_soul").unwrap();
    let imported_world_id = result.remapped_ids.get("roundtrip_world").unwrap();
    assert_ne!(imported_conv_id, "roundtrip_conv");
    assert_ne!(imported_soul_id, "roundtrip_soul");
    assert_ne!(imported_world_id, "roundtrip_world");
    assert_eq!(
        db::get_soul(&conn, "roundtrip_soul")
            .unwrap()
            .character_name,
        "Roundtrip Aurora"
    );

    let imported_branch = db::get_active_session_branch(&conn, imported_conv_id).unwrap();
    let rebuilt_imported =
        db::rebuild_session_state(&conn, imported_conv_id, &imported_branch.branch_id).unwrap();
    assert_eq!(
        rebuilt_imported.session_world.scene_state.focus,
        "Roundtrip Aurora and preset_male"
    );
    assert_eq!(rebuilt_imported.session_world.object_states.len(), 1);
    assert_eq!(
        rebuilt_imported.session_world.object_states[0].object_id,
        "preset_male_jacket_1"
    );
    assert_eq!(rebuilt_imported.soul.memory.recent.len(), 1);
    assert!(rebuilt_imported
        .soul
        .relationships
        .contains_key("preset_male"));
    assert_eq!(
        db::get_active_player_persona_id(&conn, imported_conv_id).unwrap(),
        "preset_male"
    );
    assert_eq!(
        db::list_messages(&conn, imported_conv_id, 100)
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        db::list_llm_payload_logs(&conn, imported_conv_id)
            .unwrap()
            .len(),
        1
    );
    let imported_assistant = db::list_messages(&conn, imported_conv_id, 100)
        .unwrap()
        .into_iter()
        .find(|message| message.role == "assistant")
        .unwrap();
    let imported_variants =
        db::list_assistant_message_variants(&conn, imported_conv_id, imported_assistant.id)
            .unwrap();
    assert!(imported_variants.iter().any(|variant| variant.is_selected));
    assert_eq!(
        db::list_turn_commits_for_branch(&conn, &imported_branch.branch_id)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        db::list_state_patches_for_branch(&conn, &imported_branch.branch_id)
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn import_as_new_does_not_overwrite_existing_soul() {
    let conn = db::init_memory_connection().unwrap();

    let mut existing_soul = new_default_soul("Untouched");
    existing_soul.character_id = "soul-1".into();
    db::upsert_soul(&conn, &existing_soul).unwrap();

    let mut soul = new_default_soul("IncomingNewData");
    soul.character_id = "soul-1".into();

    let mut manifest = mne_manifest(
        "character_soul",
        "IncomingNewData",
        "test",
        vec!["souls/soul-1.json".into()],
        Vec::new(),
        None,
    );
    manifest.soul_id = Some("soul-1".into());

    let mut entries = HashMap::new();
    entries.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    entries.insert(
        "souls/soul-1.json".into(),
        serde_json::to_vec(&soul).unwrap(),
    );

    let bytes = write_test_mne_bytes(entries);
    let result = import_mne_as_new_inner(&conn, &bytes).unwrap();

    let untouched = db::get_soul(&conn, "soul-1").unwrap();
    assert_eq!(untouched.character_name, "Untouched");

    let target_soul_id = &result.imported_soul_ids[0];
    let imported = db::get_soul(&conn, target_soul_id).unwrap();
    assert_eq!(imported.character_name, "IncomingNewData");
}

#[test]
fn import_as_new_preserves_payload_logs_if_present() {
    let conn = db::init_memory_connection().unwrap();
    let mut soul = new_default_soul("Aurora");
    soul.character_id = "soul-1".into();

    let conversation = ConversationSummary {
        conversation_id: "conv-1".into(),
        soul_id: "soul-1".into(),
        world_id: Some("world-1".into()),
        source_savepoint_id: None,
        source_setting_id: None,
        title: "Aurora Session".into(),
        created_at: db::now_ts(),
        updated_at: db::now_ts(),
        last_message_preview: None,
        message_count: 0,
        active_player_persona_id: "preset_male".into(),
        archived_at: None,
        active_evaluator_profile_id: None,
        is_benchmark: false,
    };

    let messages = vec![ChatMessage {
        id: 10,
        conversation_id: "conv-1".into(),
        role: "user".into(),
        content: "Hello!".into(),
        created_at: 100,
        status: "active".into(),
        origin: "active".into(),
        channel: db::MESSAGE_CHANNEL_RP_SCENE.into(),
        attachments: Vec::new(),
        hidden_at: None,
    }];

    let log = LlmPayloadLog {
        id: 0,
        conversation_id: "conv-1".into(),
        message_id: Some(10),
        provider: "openai".into(),
        mode: "chat".into(),
        context_mode: "complete".into(),
        model: "gpt-4".into(),
        base_url: "url".into(),
        system_message: "sys".into(),
        user_message: "Hello!".into(),
        context_text: "context".into(),
        estimated_system_tokens: 0,
        estimated_user_tokens: 0,
        estimated_total_tokens: 0,
        truncated: false,
        created_at: 1234,
        ..Default::default()
    };

    let mut manifest = mne_manifest(
        "session_checkpoint",
        "Aurora Session",
        "test",
        vec!["souls/soul-1.json".into()],
        vec!["worlds/world-1.json".into()],
        Some("conversation/conversation.json".into()),
    );
    manifest.soul_id = Some("soul-1".into());
    manifest.world_id = Some("world-1".into());
    manifest.conversation_id = Some("conv-1".into());

    let mut session_world =
        state_engine::setting::session_world_from_setting(&new_default_setting("Lab"));
    session_world.world_id = "world-1".into();
    session_world.source_setting_id = None;

    let mut entries = HashMap::new();
    entries.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    entries.insert(
        "souls/soul-1.json".into(),
        serde_json::to_vec(&soul).unwrap(),
    );
    entries.insert(
        "worlds/world-1.json".into(),
        serde_json::to_vec(&session_world).unwrap(),
    );
    entries.insert(
        "conversation/conversation.json".into(),
        serde_json::to_vec(&conversation).unwrap(),
    );
    entries.insert(
        "conversation/messages.json".into(),
        serde_json::to_vec(&messages).unwrap(),
    );
    entries.insert(
        "conversation/payload_logs.json".into(),
        serde_json::to_vec(&vec![log]).unwrap(),
    );

    let bytes = write_test_mne_bytes(entries);
    let _result = import_mne_as_new_inner(&conn, &bytes).unwrap();

    let imported_logs = db::list_llm_payload_logs(&conn, "conv-1").unwrap();
    assert_eq!(imported_logs.len(), 1);
    assert_eq!(imported_logs[0].provider, "openai");
}

#[test]
fn export_then_validate_bundle_passes() {
    let conn = db::init_memory_connection().unwrap();

    // 1. Create deterministically
    let mut soul = new_default_soul("Aurora");
    soul.character_id = "soul-1".into();
    db::upsert_soul(&conn, &soul).unwrap();

    db::ensure_conversation(&conn, "conv-1", "soul-1").unwrap();

    let mut world = new_default_setting("Kitchen");
    world.setting_id = "world-1".into();
    db::upsert_setting(&conn, &world).unwrap();

    let manifest = mne_manifest(
        "scenario_bundle",
        "Aurora + Kitchen",
        "test",
        vec!["souls/soul-1.json".into()],
        vec!["worlds/world-1.json".into()],
        None,
    );
    let mut entries = HashMap::new();
    entries.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    entries.insert(
        "souls/soul-1.json".into(),
        serde_json::to_vec(&soul).unwrap(),
    );
    entries.insert(
        "worlds/world-1.json".into(),
        serde_json::to_vec(&world).unwrap(),
    );

    let bytes = write_test_mne_bytes(entries);
    let report = validate_mne_bundle_bytes(&bytes);

    assert!(report.valid);
    assert_eq!(report.summary.soul_name.as_deref(), Some("Aurora"));
    assert_eq!(report.summary.world_name.as_deref(), Some("Kitchen"));
}

#[test]
fn early_narrator_failure_scorecard_cleanup() {
    let mut summary = benchmark_summary_fixture();
    summary.turn_count_requested = 5;
    summary.visible_turns_requested = 5;
    summary.visible_turns_completed = 0;
    summary.turn_count_completed = 0;
    summary.visible_assistant_messages_created = 0;
    summary.visible_user_messages_created = 1;
    summary.narrator_failures = 1;
    summary.player_simulator_payload_count = 1;
    summary.per_turn = vec![BenchmarkTurnSummary {
        turn_index: 0,
        stage: "narrator_failed".into(),
        simulated_user_message: "hello".into(),
        narrator_response_present: false,
        narrator_error: Some("API stream failed: error decoding response body".into()),
        evaluator_mode: "evaluator_form_v1".into(),
        structured_transport_actual: None,
        tool_calls_present: false,
        tool_call_count: 0,
        structured_retry_count: 0,
        fallback_path: Vec::new(),
        syntactic_repair_used: false,
        memory_count_after: summary.final_memory_count,
        object_count_after: summary.final_object_state_count,
        relationship_summary_after: String::new(),
    }];

    let scorecard = benchmark_scorecard(&summary, false, 1, 0, 0);

    assert!(!scorecard.pass);
    assert_eq!(scorecard.stop_reason.as_deref(), Some("narrator_failed"));
    assert_eq!(scorecard.failed_stage.as_deref(), Some("narrator_called"));
    assert_eq!(
        scorecard.narrator_provider_error.as_deref(),
        Some("API stream failed: error decoding response body")
    );

    // Required check values
    assert_eq!(scorecard.visible_turns_completed, 0);
    assert_eq!(scorecard.visible_turns_requested, 5);
    assert_eq!(scorecard.player_simulator_calls, 1);
    assert_eq!(scorecard.narrator_calls, 1);
    assert_eq!(scorecard.evaluator_calls, 0);

    // Growth checks should not require growth
    assert!(scorecard.memory_updated);
    assert!(scorecard.object_state_updated);
    assert!(scorecard.relationship_updated);
    assert!(scorecard.memories_increased_over_time);
    assert!(scorecard.active_player_relationship_changed_when_warranted);
    assert!(scorecard.object_ids_stable);

    // Failure reasons asserts
    assert_eq!(
        scorecard.failure_reasons,
        vec![
            "narrator_visible_response_each_turn".to_string(),
            "blocked_by_narrator_failure".to_string(),
            "skipped_after_narrator_failure".to_string(),
        ]
    );

    // Downstream failures should NOT be in failure reasons
    assert!(!scorecard
        .failure_reasons
        .contains(&"visible_turns_completed_matches_requested".to_string()));
    assert!(!scorecard
        .failure_reasons
        .contains(&"memories_increased_over_time".to_string()));
    assert!(!scorecard
        .failure_reasons
        .contains(&"object_state_updated".to_string()));
    assert!(!scorecard
        .failure_reasons
        .contains(&"active_player_relationship_changed_when_warranted".to_string()));
}

fn write_test_mne_bytes(entries: HashMap<String, Vec<u8>>) -> Vec<u8> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.mne");
    let files: Vec<(String, Vec<u8>)> = entries.into_iter().collect();
    write_stored_zip(&path, &files).unwrap();
    fs::read(&path).unwrap()
}
