pub mod contracts;

use std::{
    collections::{HashMap, HashSet},
    fs,
    time::{Duration, Instant},
};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State, Window};

use state_engine::{
    context_compiler::estimate_tokens,
    patch::{EnginePatch, RelationshipDelta},
    soul::{session_soul_from_savepoint, Soul},
};

use crate::commands::evaluator::write_diagnostic_json_file;

use contracts::{
    BenchmarkScorecard, BenchmarkSettings, BenchmarkSummary, BenchmarkTarget,
    BenchmarkTokenComparison, BenchmarkTurnSummary, BenchmarkType,
};

use crate::{
    commands::{
        effective_evaluator_timeout_ms, emit_dev_log, render_llm_payload_history, send_api_turn,
        send_mock_turn_with_conn, strip_status_blocks_for_export, uuid_like_id, write_export_file,
        EVALUATOR_MODE_FORM_V1, EVALUATOR_MODE_STRUCTURED_V1, NEXT_TURN_GATE_POLL_MS,
    },
    db::{self, LlmPayloadLog, ProviderProfile},
    job_progress::{
        emit_background_job_progress, BackgroundJobHistoryEntry, BackgroundJobProgress,
    },
    mne::service::export_current_session_checkpoint_mne_inner,
    providers::api::{ApiProvider, ApiProviderSettings},
    AppState,
};

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn run_benchmark(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    soul_id: String,
    setting_id: Option<String>,
    provider: String,
    narrator_settings: ApiProviderSettings,
    mut state_updater_settings: ApiProviderSettings,
    settings: BenchmarkSettings,
) -> Result<BenchmarkSummary, String> {
    let started_at = db::now_ts();
    let started_clock = Instant::now();
    let benchmark_id = format!("bench_{}", uuid_like_id());
    let BenchmarkConversationInit {
        conversation_id,
        session_soul_id,
        initial_memory_count,
        initial_object_count,
        initial_relationship_count,
        relationship_target_checked,
        initial_active_player_relationship,
    } = prepare_benchmark_conversation(
        &state,
        &benchmark_id,
        &soul_id,
        setting_id.as_deref(),
        &settings,
    )?;

    state_updater_settings.evaluator_mode = settings
        .evaluator_mode
        .clone()
        .or(state_updater_settings.evaluator_mode.clone());
    state_updater_settings.structured_evaluator_transport = settings
        .structured_evaluator_transport
        .clone()
        .or(state_updater_settings
            .structured_evaluator_transport
            .clone());
    state_updater_settings.structured_evaluator_policy = settings
        .structured_evaluator_policy
        .clone()
        .or(state_updater_settings.structured_evaluator_policy.clone());
    state_updater_settings.structured_evaluator_max_retries = settings
        .structured_evaluator_max_retries
        .or(state_updater_settings.structured_evaluator_max_retries);
    if settings.strict_tool_evaluator {
        state_updater_settings.evaluator_mode = Some(EVALUATOR_MODE_STRUCTURED_V1.into());
        state_updater_settings.structured_evaluator_transport = Some("tool_call".into());
        state_updater_settings.structured_evaluator_policy = Some("required".into());
    }

    let player_profile = if benchmark_requires_player_profile(&settings.benchmark_type) {
        let profile_id = settings
            .player_simulator_profile_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| "Self-play benchmark requires a Player Simulator profile".to_string())?;
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        Some(db::get_provider_profile(&conn, profile_id).map_err(|err| err.to_string())?)
    } else {
        None
    };

    let mut narrator_failures = 0usize;
    let mut turn_count_completed = 0usize;
    let mut failed_turns = 0usize;
    let mut recovered_turns = 0usize;
    let mut job_history: Vec<BackgroundJobHistoryEntry> = Vec::new();
    let scripted_turns = benchmark_scripted_turns();
    let requested_turns = settings.turn_count.max(1);
    let mut per_turn = Vec::new();
    let provider_is_mock = provider.eq_ignore_ascii_case("mock")
        || narrator_settings.api_key.trim().is_empty()
        || narrator_settings.model.trim().is_empty()
        || narrator_settings.base_url.trim().is_empty();

    emit_background_job_progress(
        &window,
        &BackgroundJobProgress {
            job_id: benchmark_id.clone(),
            kind: "benchmark".into(),
            label: "Benchmark".into(),
            status: "running".into(),
            phase: "preparing".into(),
            current: 0,
            total: requested_turns,
            succeeded: 0,
            failed: 0,
            recovered: 0,
            started_at,
            updated_at: db::now_ts(),
            elapsed_ms: 0,
            estimated_remaining_ms: None,
            detail: Some(format!(
                "{} / {}",
                benchmark_type_label(&settings.benchmark_type),
                conversation_id
            )),
            cancellable: false,
            history: Vec::new(),
        },
    );

    for turn_index in 0..requested_turns {
        let turn_started = Instant::now();
        emit_background_job_progress(
            &window,
            &BackgroundJobProgress {
                job_id: benchmark_id.clone(),
                kind: "benchmark".into(),
                label: "Benchmark".into(),
                status: "running".into(),
                phase: "player_generation".into(),
                current: turn_index,
                total: requested_turns,
                succeeded: turn_count_completed,
                failed: failed_turns,
                recovered: recovered_turns,
                started_at,
                updated_at: db::now_ts(),
                elapsed_ms: started_clock.elapsed().as_millis() as u64,
                estimated_remaining_ms: None,
                detail: Some(format!(
                    "Preparing cycle {}/{}",
                    turn_index + 1,
                    requested_turns
                )),
                cancellable: false,
                history: job_history.clone(),
            },
        );
        let user_text = match settings.benchmark_type {
            BenchmarkType::ScriptedVisibleReplay | BenchmarkType::HeadlessRegression => {
                scripted_turns[turn_index % scripted_turns.len()].to_string()
            }
            BenchmarkType::VisibleAiChat | BenchmarkType::MultiAgentVisibleChat => {
                let profile = player_profile.as_ref().expect("profile checked");
                match generate_benchmark_player_turn(
                    &state,
                    &conversation_id,
                    &session_soul_id,
                    profile,
                    &settings.player_goal,
                    settings.player_character_soul_id.as_deref(),
                )
                .await
                {
                    Ok(text) => text,
                    Err(err) => {
                        failed_turns += 1;
                        job_history.push(BackgroundJobHistoryEntry {
                            index: turn_index + 1,
                            label: format!("Cycle {}", turn_index + 1),
                            status: "failed".into(),
                            detail: Some(format!("Player generation failed: {err}")),
                            elapsed_ms: Some(turn_started.elapsed().as_millis() as u64),
                        });
                        emit_background_job_progress(
                            &window,
                            &BackgroundJobProgress {
                                job_id: benchmark_id.clone(),
                                kind: "benchmark".into(),
                                label: "Benchmark".into(),
                                status: "failed".into(),
                                phase: "player_generation".into(),
                                current: turn_index,
                                total: requested_turns,
                                succeeded: turn_count_completed,
                                failed: failed_turns,
                                recovered: recovered_turns,
                                started_at,
                                updated_at: db::now_ts(),
                                elapsed_ms: started_clock.elapsed().as_millis() as u64,
                                estimated_remaining_ms: None,
                                detail: Some(err.clone()),
                                cancellable: false,
                                history: job_history.clone(),
                            },
                        );
                        return Err(err);
                    }
                }
            }
        };

        emit_background_job_progress(
            &window,
            &BackgroundJobProgress {
                job_id: benchmark_id.clone(),
                kind: "benchmark".into(),
                label: "Benchmark".into(),
                status: "running".into(),
                phase: "turn_pipeline".into(),
                current: turn_index,
                total: requested_turns,
                succeeded: turn_count_completed,
                failed: failed_turns,
                recovered: recovered_turns,
                started_at,
                updated_at: db::now_ts(),
                elapsed_ms: started_clock.elapsed().as_millis() as u64,
                estimated_remaining_ms: None,
                detail: Some(format!(
                    "Running cycle {}/{}",
                    turn_index + 1,
                    requested_turns
                )),
                cancellable: false,
                history: job_history.clone(),
            },
        );

        let turn_result = if provider_is_mock {
            let conn = state.conn.lock().map_err(|err| err.to_string())?;
            send_mock_turn_with_conn(
                &conn,
                conversation_id.clone(),
                session_soul_id.clone(),
                user_text.clone(),
                settings.narrator_style.clone(),
                None,
                None,
            )
        } else {
            send_api_turn(
                app.clone(),
                window.clone(),
                state.clone(),
                conversation_id.clone(),
                session_soul_id.clone(),
                user_text.clone(),
                settings.narrator_style.clone(),
                narrator_settings.clone(),
                state_updater_settings.clone(),
                None,
                None,
                Some("brief".into()),
            )
            .await
        };

        match turn_result {
            Ok(result) if !result.visible_response.trim().is_empty() => {
                turn_count_completed += 1;
                if settings.wait_for_evaluator_each_turn {
                    wait_for_benchmark_evaluators(
                        &state,
                        &conversation_id,
                        &state_updater_settings,
                    )?;
                }
                per_turn.push(build_benchmark_turn_summary(
                    &state,
                    &conversation_id,
                    turn_index,
                    &user_text,
                    "completed",
                    None,
                    &state_updater_settings,
                )?);
                let turn_elapsed_ms = turn_started.elapsed().as_millis() as u64;
                job_history.push(BackgroundJobHistoryEntry {
                    index: turn_index + 1,
                    label: format!("Cycle {}", turn_index + 1),
                    status: "succeeded".into(),
                    detail: Some("Narrator and evaluator pipeline completed".into()),
                    elapsed_ms: Some(turn_elapsed_ms),
                });
                let elapsed_ms = started_clock.elapsed().as_millis() as u64;
                let average_ms = elapsed_ms / turn_count_completed.max(1) as u64;
                let estimated_remaining_ms = average_ms
                    .checked_mul(requested_turns.saturating_sub(turn_count_completed) as u64);
                emit_background_job_progress(
                    &window,
                    &BackgroundJobProgress {
                        job_id: benchmark_id.clone(),
                        kind: "benchmark".into(),
                        label: "Benchmark".into(),
                        status: "running".into(),
                        phase: "cycle_complete".into(),
                        current: turn_count_completed,
                        total: requested_turns,
                        succeeded: turn_count_completed,
                        failed: failed_turns,
                        recovered: recovered_turns,
                        started_at,
                        updated_at: db::now_ts(),
                        elapsed_ms,
                        estimated_remaining_ms,
                        detail: Some(format!(
                            "Completed cycle {}/{}",
                            turn_count_completed, requested_turns
                        )),
                        cancellable: false,
                        history: job_history.clone(),
                    },
                );
            }
            Ok(_) => {
                narrator_failures += 1;
                failed_turns += 1;
                per_turn.push(build_benchmark_turn_summary(
                    &state,
                    &conversation_id,
                    turn_index,
                    &user_text,
                    "narrator_failed",
                    Some("Narrator returned an empty visible response"),
                    &state_updater_settings,
                )?);
                job_history.push(BackgroundJobHistoryEntry {
                    index: turn_index + 1,
                    label: format!("Cycle {}", turn_index + 1),
                    status: "failed".into(),
                    detail: Some("Narrator returned an empty visible response".into()),
                    elapsed_ms: Some(turn_started.elapsed().as_millis() as u64),
                });
                emit_background_job_progress(
                    &window,
                    &BackgroundJobProgress {
                        job_id: benchmark_id.clone(),
                        kind: "benchmark".into(),
                        label: "Benchmark".into(),
                        status: "failed".into(),
                        phase: "narrator".into(),
                        current: turn_count_completed,
                        total: requested_turns,
                        succeeded: turn_count_completed,
                        failed: failed_turns,
                        recovered: recovered_turns,
                        started_at,
                        updated_at: db::now_ts(),
                        elapsed_ms: started_clock.elapsed().as_millis() as u64,
                        estimated_remaining_ms: None,
                        detail: Some("Narrator returned an empty visible response".into()),
                        cancellable: false,
                        history: job_history.clone(),
                    },
                );
                break;
            }
            Err(err) => {
                narrator_failures += 1;
                failed_turns += 1;
                per_turn.push(build_benchmark_turn_summary(
                    &state,
                    &conversation_id,
                    turn_index,
                    &user_text,
                    "narrator_failed",
                    Some(&err),
                    &state_updater_settings,
                )?);
                job_history.push(BackgroundJobHistoryEntry {
                    index: turn_index + 1,
                    label: format!("Cycle {}", turn_index + 1),
                    status: "failed".into(),
                    detail: Some(err.clone()),
                    elapsed_ms: Some(turn_started.elapsed().as_millis() as u64),
                });
                emit_background_job_progress(
                    &window,
                    &BackgroundJobProgress {
                        job_id: benchmark_id.clone(),
                        kind: "benchmark".into(),
                        label: "Benchmark".into(),
                        status: "failed".into(),
                        phase: "turn_pipeline".into(),
                        current: turn_count_completed,
                        total: requested_turns,
                        succeeded: turn_count_completed,
                        failed: failed_turns,
                        recovered: recovered_turns,
                        started_at,
                        updated_at: db::now_ts(),
                        elapsed_ms: started_clock.elapsed().as_millis() as u64,
                        estimated_remaining_ms: None,
                        detail: Some(err),
                        cancellable: false,
                        history: job_history.clone(),
                    },
                );
                break;
            }
        }
    }

    recovered_turns = per_turn
        .iter()
        .filter(|turn| turn.structured_retry_count > 0 && turn.narrator_error.is_none())
        .count();
    emit_background_job_progress(
        &window,
        &BackgroundJobProgress {
            job_id: benchmark_id.clone(),
            kind: "benchmark".into(),
            label: "Benchmark".into(),
            status: "running".into(),
            phase: "finalizing".into(),
            current: turn_count_completed,
            total: requested_turns,
            succeeded: turn_count_completed,
            failed: failed_turns,
            recovered: recovered_turns,
            started_at,
            updated_at: db::now_ts(),
            elapsed_ms: started_clock.elapsed().as_millis() as u64,
            estimated_remaining_ms: None,
            detail: Some("Exporting artifacts and computing scorecard".into()),
            cancellable: false,
            history: job_history.clone(),
        },
    );

    let completed_at = db::now_ts();
    let summary = build_benchmark_summary(
        &state,
        &benchmark_id,
        benchmark_type_label(&settings.benchmark_type),
        &conversation_id,
        started_at,
        completed_at,
        requested_turns,
        turn_count_completed,
        &narrator_settings,
        &state_updater_settings,
        player_profile.as_ref(),
        narrator_failures,
        initial_memory_count,
        initial_object_count,
        initial_relationship_count,
        &relationship_target_checked,
        initial_active_player_relationship,
        per_turn,
        settings.strict_tool_evaluator,
    )?;

    let finalized = finalize_benchmark_summary(
        &app,
        &window,
        &state,
        summary,
        &settings,
        initial_memory_count,
        initial_object_count,
        initial_relationship_count,
    );
    match &finalized {
        Ok(summary) => emit_background_job_progress(
            &window,
            &BackgroundJobProgress {
                job_id: benchmark_id,
                kind: "benchmark".into(),
                label: "Benchmark".into(),
                status: if summary.scorecard.pass {
                    "succeeded".into()
                } else {
                    "failed".into()
                },
                phase: "complete".into(),
                current: turn_count_completed,
                total: requested_turns,
                succeeded: turn_count_completed,
                failed: failed_turns,
                recovered: recovered_turns,
                started_at,
                updated_at: db::now_ts(),
                elapsed_ms: started_clock.elapsed().as_millis() as u64,
                estimated_remaining_ms: Some(0),
                detail: Some(if summary.scorecard.pass {
                    "Benchmark passed".into()
                } else {
                    format!(
                        "Benchmark completed with {} scorecard failure(s)",
                        summary.scorecard.failure_reasons.len()
                    )
                }),
                cancellable: false,
                history: job_history,
            },
        ),
        Err(err) => emit_background_job_progress(
            &window,
            &BackgroundJobProgress {
                job_id: benchmark_id,
                kind: "benchmark".into(),
                label: "Benchmark".into(),
                status: "failed".into(),
                phase: "finalizing".into(),
                current: turn_count_completed,
                total: requested_turns,
                succeeded: turn_count_completed,
                failed: failed_turns + 1,
                recovered: recovered_turns,
                started_at,
                updated_at: db::now_ts(),
                elapsed_ms: started_clock.elapsed().as_millis() as u64,
                estimated_remaining_ms: None,
                detail: Some(err.clone()),
                cancellable: false,
                history: job_history,
            },
        ),
    }
    finalized
}

/// Run exports (payload history, .mne, summary JSON), recompute the scorecard,
/// and emit the completion dev-log for a built `BenchmarkSummary`. Shared by the
/// blocking `run_benchmark` path and the frontend-driven `finalize_benchmark`
/// command so both produce identical artifacts.
fn finalize_benchmark_summary(
    app: &AppHandle,
    window: &Window,
    state: &State<'_, AppState>,
    mut summary: BenchmarkSummary,
    settings: &BenchmarkSettings,
    initial_memory_count: usize,
    initial_object_count: usize,
    initial_relationship_count: usize,
) -> Result<BenchmarkSummary, String> {
    let conversation_id = summary.conversation_id.clone();
    if settings.export_payload_history {
        let logs = {
            let conn = state.conn.lock().map_err(|err| err.to_string())?;
            db::list_llm_payload_logs(&conn, &conversation_id).map_err(|err| err.to_string())?
        };
        let markdown = render_llm_payload_history(&logs);
        let path = write_export_file(
            app,
            &conversation_id,
            "benchmark-payload-history",
            &markdown,
        )?;
        summary.payload_history_path = Some(path.display().to_string());
    }
    if settings.export_mne {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        let result =
            export_current_session_checkpoint_mne_inner(app, window, &conn, &conversation_id, "")?;
        summary.mne_export_path = Some(result.path);
    }
    summary.scorecard.mne_export_succeeded = summary.mne_export_path.is_some();
    summary.scorecard = benchmark_scorecard(
        &summary,
        settings.strict_tool_evaluator,
        initial_memory_count,
        initial_object_count,
        initial_relationship_count,
    );
    summary.scorecard.evaluator_waited_each_turn = settings.wait_for_evaluator_each_turn;
    summary.scorecard.token_comparison = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        Some(collect_benchmark_token_comparison(&conn, &conversation_id))
    };
    if settings.export_summary_json {
        let path =
            write_diagnostic_json_file(app, &conversation_id, "benchmark-summary", &summary)?;
        summary.summary_json_path = Some(path.display().to_string());
        let json = serde_json::to_string_pretty(&summary).map_err(|err| err.to_string())?;
        fs::write(&path, json).map_err(|err| err.to_string())?;
    }

    emit_dev_log(
        window,
        if summary.scorecard.pass {
            "success"
        } else {
            "warn"
        },
        "app",
        "benchmark_completed",
        Some(serde_json::json!({
            "benchmark_id": summary.benchmark_id,
            "conversation_id": summary.conversation_id,
            "pass": summary.scorecard.pass,
            "failure_reasons": summary.scorecard.failure_reasons
        })),
    );
    Ok(summary)
}

/// Resolved benchmark conversation plus the entity counts captured *before* any
/// turns run, so the scorecard can measure deltas.
struct BenchmarkConversationInit {
    conversation_id: String,
    session_soul_id: String,
    initial_memory_count: usize,
    initial_object_count: usize,
    initial_relationship_count: usize,
    relationship_target_checked: String,
    initial_active_player_relationship: Option<serde_json::Value>,
}

/// Resolve (or create) the conversation a benchmark runs against, mark it as a
/// benchmark conversation, and snapshot the starting memory/object/relationship
/// counts. Extracted from `run_benchmark` so the frontend-driven live path can
/// set up the same session via `prepare_benchmark_session`.
fn prepare_benchmark_conversation(
    state: &State<'_, AppState>,
    benchmark_id: &str,
    soul_id: &str,
    setting_id: Option<&str>,
    settings: &BenchmarkSettings,
) -> Result<BenchmarkConversationInit, String> {
    let conn = state.conn.lock().map_err(|err| err.to_string())?;
    if matches!(settings.target, BenchmarkTarget::CurrentSession) {
        let conversation_id = settings
            .current_conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                "Current Session benchmark target requires current_conversation_id".to_string()
            })?
            .to_string();
        let conversation =
            db::get_conversation_summary(&conn, &conversation_id).map_err(|err| err.to_string())?;
        db::mark_conversation_benchmark(&conn, &conversation_id).map_err(|err| err.to_string())?;
        let (session_soul, session_world) =
            if let Ok(branch) = db::get_active_session_branch(&conn, &conversation_id) {
                let rebuilt = db::rebuild_session_state(&conn, &conversation_id, &branch.branch_id)
                    .map_err(|err| err.to_string())?;
                (rebuilt.soul, rebuilt.session_world)
            } else {
                let source =
                    db::get_soul(&conn, &conversation.soul_id).map_err(|err| err.to_string())?;
                let world = match db::get_conversation_session_world(&conn, &conversation_id)
                    .map_err(|err| err.to_string())?
                {
                    Some(world) => world,
                    None => db::create_legacy_session_world_from_soul(&conn, &source)
                        .map_err(|err| err.to_string())?,
                };
                (source, world)
            };
        Ok(BenchmarkConversationInit {
            conversation_id,
            session_soul_id: session_soul.character_id.clone(),
            initial_memory_count: memory_count(&session_soul),
            initial_object_count: session_world.object_states.len(),
            initial_relationship_count: session_soul.relationships.len(),
            relationship_target_checked: conversation.active_player_persona_id.clone(),
            initial_active_player_relationship: session_soul
                .relationships
                .get(&conversation.active_player_persona_id)
                .and_then(|relationship| serde_json::to_value(relationship).ok()),
        })
    } else {
        let source = db::get_soul(&conn, soul_id).map_err(|err| err.to_string())?;
        let session = session_soul_from_savepoint(&source);
        let session_world = if let Some(setting_id) = setting_id
            .map(str::trim)
            .filter(|setting_id| !setting_id.is_empty())
        {
            db::create_session_world_from_setting(&conn, setting_id)
                .map_err(|err| err.to_string())?
        } else {
            db::create_legacy_session_world_from_soul(&conn, &source)
                .map_err(|err| err.to_string())?
        };
        db::upsert_soul(&conn, &session).map_err(|err| err.to_string())?;
        let conversation_id = format!("benchmark-{}-{}", benchmark_id, session.character_id);
        db::ensure_conversation_with_title_and_world(
            &conn,
            &conversation_id,
            &session.character_id,
            Some(&session_world.world_id),
            session_world.source_setting_id.as_deref(),
            Some(&format!(
                "Benchmark {} - {}",
                benchmark_id, source.character_name
            )),
        )
        .map_err(|err| err.to_string())?;
        db::mark_conversation_benchmark(&conn, &conversation_id).map_err(|err| err.to_string())?;
        db::create_session_branch(&conn, &conversation_id, &session, &session_world)
            .map_err(|err| err.to_string())?;
        let relationship_target_checked = db::get_active_player_persona_id(&conn, &conversation_id)
            .map_err(|err| err.to_string())?;
        Ok(BenchmarkConversationInit {
            conversation_id,
            session_soul_id: session.character_id.clone(),
            initial_memory_count: memory_count(&session),
            initial_object_count: session_world.object_states.len(),
            initial_relationship_count: session.relationships.len(),
            initial_active_player_relationship: session
                .relationships
                .get(&relationship_target_checked)
                .and_then(|relationship| serde_json::to_value(relationship).ok()),
            relationship_target_checked,
        })
    }
}

/// Session handle returned to the frontend so it can drive the live self-play
/// loop (one `executeTurn` per turn) and later call `finalize_benchmark`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSessionInit {
    pub benchmark_id: String,
    pub conversation_id: String,
    pub session_soul_id: String,
    pub started_at: i64,
    pub initial_memory_count: usize,
    pub initial_object_count: usize,
    pub initial_relationship_count: usize,
    pub relationship_target_checked: String,
    pub initial_active_player_relationship: Option<serde_json::Value>,
}

/// Set up a benchmark conversation for the frontend-driven live path. The
/// frontend switches to `conversation_id`, then runs turns through the normal
/// visible chat (`executeTurn`) so the AI-vs-AI exchange streams in real time.
#[tauri::command]
pub fn prepare_benchmark_session(
    state: State<'_, AppState>,
    soul_id: String,
    setting_id: Option<String>,
    settings: BenchmarkSettings,
) -> Result<BenchmarkSessionInit, String> {
    let benchmark_id = format!("bench_{}", uuid_like_id());
    let started_at = db::now_ts();
    let init = prepare_benchmark_conversation(
        &state,
        &benchmark_id,
        &soul_id,
        setting_id.as_deref(),
        &settings,
    )?;
    Ok(BenchmarkSessionInit {
        benchmark_id,
        conversation_id: init.conversation_id,
        session_soul_id: init.session_soul_id,
        started_at,
        initial_memory_count: init.initial_memory_count,
        initial_object_count: init.initial_object_count,
        initial_relationship_count: init.initial_relationship_count,
        relationship_target_checked: init.relationship_target_checked,
        initial_active_player_relationship: init.initial_active_player_relationship,
    })
}

/// Generate the next user-side message for the live self-play loop. Returns only
/// the text; the frontend then sends it through `executeTurn` exactly as if the
/// user had typed it, so it renders (and the narrator streams) in visible chat.
#[tauri::command]
pub async fn generate_benchmark_player_message(
    state: State<'_, AppState>,
    conversation_id: String,
    soul_id: String,
    player_profile_id: String,
    player_goal: String,
    player_character_soul_id: Option<String>,
) -> Result<String, String> {
    let profile = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        db::get_provider_profile(&conn, &player_profile_id).map_err(|err| err.to_string())?
    };
    generate_benchmark_player_turn(
        &state,
        &conversation_id,
        &soul_id,
        &profile,
        &player_goal,
        player_character_soul_id.as_deref(),
    )
    .await
}

/// Like `generate_benchmark_player_message`, but uses the TRADITIONAL RP engine
/// (full raw transcript, no Soul/memory/scene) — the control side of the
/// comparison benchmark.
#[tauri::command]
pub async fn generate_traditional_rp_message(
    state: State<'_, AppState>,
    conversation_id: String,
    soul_id: String,
    player_profile_id: String,
    player_goal: String,
) -> Result<String, String> {
    let profile = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        db::get_provider_profile(&conn, &player_profile_id).map_err(|err| err.to_string())?
    };
    generate_traditional_rp_turn(&state, &conversation_id, &soul_id, &profile, &player_goal).await
}

/// Build the per-turn summary for one live self-play turn after `executeTurn`
/// has completed and its evaluator has applied. Captures the post-turn entity
/// counts and the evaluator trace from the latest payload logs.
#[tauri::command]
pub fn benchmark_turn_summary(
    state: State<'_, AppState>,
    conversation_id: String,
    turn_index: usize,
    user_text: String,
    stage: String,
    narrator_error: Option<String>,
    state_updater_settings: ApiProviderSettings,
) -> Result<BenchmarkTurnSummary, String> {
    build_benchmark_turn_summary(
        &state,
        &conversation_id,
        turn_index,
        &user_text,
        &stage,
        narrator_error.as_deref(),
        &state_updater_settings,
    )
}

/// Finalize a frontend-driven live benchmark: build the summary from the
/// recorded per-turn results, run exports, score it, and emit the dev-log.
/// Mirrors the tail of `run_benchmark` for the live path.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn finalize_benchmark(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    benchmark_id: String,
    conversation_id: String,
    started_at: i64,
    narrator_settings: ApiProviderSettings,
    state_updater_settings: ApiProviderSettings,
    settings: BenchmarkSettings,
    initial_memory_count: usize,
    initial_object_count: usize,
    initial_relationship_count: usize,
    relationship_target_checked: String,
    initial_active_player_relationship: Option<serde_json::Value>,
    turn_count_completed: usize,
    narrator_failures: usize,
    per_turn: Vec<BenchmarkTurnSummary>,
) -> Result<BenchmarkSummary, String> {
    let player_profile = match settings
        .player_simulator_profile_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        Some(profile_id) => {
            let conn = state.conn.lock().map_err(|err| err.to_string())?;
            Some(db::get_provider_profile(&conn, profile_id).map_err(|err| err.to_string())?)
        }
        None => None,
    };
    let completed_at = db::now_ts();
    let requested_turns = settings.turn_count.max(1);
    let summary = build_benchmark_summary(
        &state,
        &benchmark_id,
        benchmark_type_label(&settings.benchmark_type),
        &conversation_id,
        started_at,
        completed_at,
        requested_turns,
        turn_count_completed,
        &narrator_settings,
        &state_updater_settings,
        player_profile.as_ref(),
        narrator_failures,
        initial_memory_count,
        initial_object_count,
        initial_relationship_count,
        &relationship_target_checked,
        initial_active_player_relationship,
        per_turn,
        settings.strict_tool_evaluator,
    )?;
    finalize_benchmark_summary(
        &app,
        &window,
        &state,
        summary,
        &settings,
        initial_memory_count,
        initial_object_count,
        initial_relationship_count,
    )
}

pub(crate) fn benchmark_scripted_turns() -> Vec<&'static str> {
    vec![
        "I step inside, leaving my wet jacket near the door.",
        "I ask Aurora what she actually wants from me right now.",
        "I move a little closer, but keep my hands visible.",
        "I point out the contradiction and ask her to choose.",
        "I pick up the jacket again and wait for her answer.",
    ]
}

fn benchmark_type_label(value: &BenchmarkType) -> &'static str {
    match value {
        BenchmarkType::VisibleAiChat => "visible_ai_chat",
        BenchmarkType::ScriptedVisibleReplay => "scripted_visible_replay",
        BenchmarkType::HeadlessRegression => "headless_regression",
        BenchmarkType::MultiAgentVisibleChat => "multi_agent_visible_chat",
    }
}

pub(crate) fn benchmark_requires_player_profile(value: &BenchmarkType) -> bool {
    matches!(
        value,
        BenchmarkType::VisibleAiChat | BenchmarkType::MultiAgentVisibleChat
    )
}

pub(crate) fn memory_count(soul: &Soul) -> usize {
    soul.memory.core.len() + soul.memory.recent.len() + soul.memory.schemas.len()
}

fn provider_profile_to_api_settings(profile: &ProviderProfile) -> ApiProviderSettings {
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
        evaluator_timeout_ms: profile.evaluator_timeout_ms,
        structured_evaluator_timeout_ms: None,
        diagnostic_evaluator_timeout_ms: None,
        evaluator_timeout_mode: profile.evaluator_timeout_mode.clone(),
        evaluator_mode: profile.evaluator_mode.clone(),
        structured_evaluator_policy: profile.structured_evaluator_policy.clone(),
        structured_evaluator_transport: None,
        structured_evaluator_max_retries: Some(1),
        structured_require_ops: None,
        wait_for_evaluator_before_next_turn: profile.wait_for_evaluator_before_next_turn,
        allow_send_with_stale_state: profile.allow_send_with_stale_state,
        evaluator_background_enabled: profile.evaluator_background_enabled,
        anti_replay_forced_retry_enabled: profile.anti_replay_forced_retry_enabled,
        evaluator_execution_mode: None,
    }
}

async fn generate_benchmark_player_turn(
    state: &State<'_, AppState>,
    conversation_id: &str,
    soul_id: &str,
    profile: &ProviderProfile,
    player_goal: &str,
    player_character_soul_id: Option<&str>,
) -> Result<String, String> {
    let (visible_chat_log, scene_summary, persona_summary, played_character) = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        let messages =
            db::list_messages(&conn, conversation_id, 24).map_err(|err| err.to_string())?;
        let visible_chat_log = messages
            .iter()
            .filter(|message| message.status == "active")
            .map(|message| {
                let label = if message.role == "assistant" {
                    "Narrator"
                } else {
                    "User"
                };
                format!(
                    "{label}: {}",
                    strip_status_blocks_for_export(&message.content)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let scene_summary = db::get_conversation_session_world(&conn, conversation_id)
            .ok()
            .flatten()
            .map(|world| {
                format!(
                    "location: {}; focus: {}; pressure: {}; continuity: {}",
                    world.location,
                    world.scene_state.focus,
                    world.scene_state.pressure_point,
                    world.scene_state.continuity_note
                )
            })
            .unwrap_or_else(|| "No public scene summary yet.".into());
        // The operator's persona is part of the scene whether or not the
        // simulator is playing it, so it is always described. Character mode adds
        // a second block naming the Soul the simulator speaks as.
        let persona =
            db::get_active_player_persona(&conn, conversation_id).map_err(|err| err.to_string())?;
        let persona_summary = format!(
            "{} ({})\nPronouns: {}\nDescription: {}\nAppearance: {}\nNotes: {}",
            persona.display_name,
            persona.persona_id,
            persona.pronouns,
            persona.description,
            persona.appearance.as_deref().unwrap_or(""),
            persona.notes.as_deref().unwrap_or("")
        );
        let played_character = match player_character_soul_id {
            Some(other_soul_id) if !other_soul_id.trim().is_empty() => {
                let other = db::get_soul(&conn, other_soul_id).map_err(|err| err.to_string())?;
                if other.character_id == soul_id {
                    return Err(
                        "player character must differ from the narrator-controlled Soul".into(),
                    );
                }
                Some((
                    other.character_name.clone(),
                    format!(
                        "{} ({})\nDescription: {}\nPersonality: {}\nAppearance: {}\nScenario: {}",
                        other.character_name,
                        other.character_id,
                        other.profile.description,
                        other.profile.personality,
                        other.profile.appearance,
                        other.profile.scenario
                    ),
                ))
            }
            _ => None,
        };
        let _ = db::get_soul(&conn, soul_id).map_err(|err| err.to_string())?;
        (
            visible_chat_log,
            scene_summary,
            persona_summary,
            played_character,
        )
    };
    let system_prompt = if played_character.is_some() {
        benchmark_character_simulator_prompt()
    } else {
        benchmark_player_simulator_prompt()
    };
    let (persona_block, closing) =
        player_prompt_blocks(&persona_summary, played_character.as_ref());
    let user_prompt = format!(
        "Benchmark goal:\n{}\n\n{persona_block}\n\nVisible chat:\n{}\n\nPublic scene summary:\n{}\n\nLast narrator response:\n{}\n\n{closing}",
        if player_goal.trim().is_empty() {
            "Pursue the scene naturally while respecting continuity."
        } else {
            player_goal.trim()
        },
        if visible_chat_log.trim().is_empty() {
            "(no visible chat yet)"
        } else {
            visible_chat_log.trim()
        },
        scene_summary,
        visible_chat_log
            .lines()
            .rev()
            .find(|line| line.starts_with("Narrator:"))
            .unwrap_or("(no narrator response yet)")
    );
    let provider = ApiProvider::default();
    let mut settings = provider_profile_to_api_settings(profile);
    // Fail before the request when the profile carries no credential: the
    // provider answers "Missing Authentication header" with a 401, which reads
    // like a key problem at the provider rather than an unset profile here.
    if settings.api_key.trim().is_empty() && !settings.base_url.contains("127.0.0.1") {
        return Err(format!(
            "profile \"{}\" has no API key set; pick a configured profile for this run",
            profile.name
        ));
    }

    // Cap each player-line attempt hard. This call runs inside an uninterruptible
    // Tauri command, so a large profile timeout × retries can stall the whole run
    // for many minutes with no way to Stop. The player line is short text — 90s
    // is plenty — and this cap is only on the harness, not the faithful narrator
    // pipeline. Honor a smaller profile timeout if the user set one. The streaming
    // transport reads its timeout from `narrator_timeout_ms`, so set it there.
    const PLAYER_LINE_MAX_TIMEOUT_MS: u64 = 90_000;
    settings.narrator_timeout_ms = Some(
        settings
            .narrator_timeout_ms
            .filter(|value| *value > 0)
            .map(|value| value.min(PLAYER_LINE_MAX_TIMEOUT_MS))
            .unwrap_or(PLAYER_LINE_MAX_TIMEOUT_MS),
    );
    // Generate the player line through the SAME streaming transport the narrator
    // uses. Non-streaming decode flakes on free/alpha models (owl-alpha returns
    // 200-error envelopes / odd shapes the strict body parser chokes on); the
    // streaming SSE parser survives them. The chunks aren't surfaced anywhere —
    // the player line only needs its final text — so the sink is a no-op.
    //
    // A single transient hiccup shouldn't kill a multi-turn run, so retry once
    // with backoff. Persistent/shape errors still surface for diagnosis. Kept to
    // 2 attempts so the worst case stays bounded (~2×90s) and the run stoppable.
    const MAX_ATTEMPTS: usize = 2;
    let mut attempts = 0usize;
    let mut last_error = String::new();
    while attempts < MAX_ATTEMPTS {
        attempts += 1;
        match provider
            .complete_streaming(
                &settings,
                system_prompt,
                &user_prompt,
                |_chunk: &str| Ok(()),
            )
            .await
        {
            Ok(completion) => {
                let sanitized = sanitize_player_simulator_message(&completion.raw_text)?;
                if let Ok(conn) = state.conn.lock() {
                    let _ = db::insert_llm_payload_log(
                        &conn,
                        &LlmPayloadLog {
                            conversation_id: conversation_id.to_string(),
                            provider: "player_simulator".into(),
                            mode: "visible_ai_chat".into(),
                            context_mode: "public_only".into(),
                            model: settings.model.trim().to_string(),
                            base_url: settings.base_url.trim().to_string(),
                            system_message: system_prompt.to_string(),
                            user_message: user_prompt.clone(),
                            context_text:
                                "visible_chat + public_scene_summary + active_player_persona".into(),
                            estimated_system_tokens: estimate_tokens(system_prompt),
                            estimated_user_tokens: estimate_tokens(&user_prompt),
                            estimated_total_tokens: estimate_tokens(system_prompt)
                                + estimate_tokens(&user_prompt),
                            truncated: false,
                            created_at: db::now_ts(),
                            request_id: Some(format!("player_simulator_{}", uuid_like_id())),
                            raw_provider_response: Some(completion.raw_text),
                            normalized_response: Some(sanitized.clone()),
                            finish_reason: completion.finish_reason,
                            provider_request_id: completion.provider_request_id,
                            provider_response_id: completion.provider_response_id,
                            pipeline_trace_json: token_usage_trace_json(
                                completion.token_usage.as_ref(),
                            ),
                            ..Default::default()
                        },
                    );
                }
                return Ok(sanitized);
            }
            Err(error) => {
                last_error = error;
                if attempts < MAX_ATTEMPTS && is_transient_provider_error(&last_error) {
                    std::thread::sleep(Duration::from_millis(800 * attempts as u64));
                    continue;
                }
                break;
            }
        }
    }
    log_simulator_failure(
        state,
        conversation_id,
        PLAYER_SIMULATOR_PAYLOAD_PROVIDER,
        "visible_ai_chat",
        &settings,
        profile,
        system_prompt,
        &user_prompt,
        &last_error,
    );
    Err(format!(
        "player simulator failed after {attempts} attempt(s) using profile \"{}\" (model {}, {}): {last_error}",
        profile.name,
        settings.model.trim(),
        settings.base_url.trim()
    ))
}

/// True for provider failures worth retrying: a transient upstream error, a
/// timeout, or a rate-limit / 5xx. Persistent shape errors (bad JSON, missing
/// content) are NOT retried — they should surface so they can be diagnosed.
pub(crate) fn is_transient_provider_error(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("provider returned error")
        || lower.contains("error in a 200")
        || lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("overloaded")
        || lower.contains("temporarily")
        // Transport/network drop is "API request failed: <err>" (no status).
        // A status error is "API request failed with <code>: …" — those are
        // only retried for the explicit 429/5xx codes below, never 4xx.
        || lower.contains("api request failed:")
        // Mid-stream drop on the streaming transport.
        || lower.contains("api stream failed")
        || lower.contains(" 429")
        || lower.contains(" 500")
        || lower.contains(" 502")
        || lower.contains(" 503")
        || lower.contains(" 504")
}

pub(crate) fn benchmark_player_simulator_prompt() -> &'static str {
    r#"You are the user-side Player Simulator for a Mnemosyne RP benchmark.

You control only the active player persona.
You are not the narrator.
You are not the active Soul unless the active player persona is that Soul.
You must not write the narrator-controlled character's thoughts, dialogue, or actions.
You must not write backend JSON, tool calls, status blocks, or diagnostics.

Write only the next user message that should be sent into the RP chat.

Stay in character.
React to the latest visible narrator response.
Pursue the benchmark goal naturally.
Respect scene continuity and boundaries.
Do not rush the scene unless the goal requires it.
Do not summarize. Do not explain. Output only the user message.

[YOU ARE IN THE STORY, NOT TESTING IT]
The other character is a person in the scene, not software. Never treat them as an AI, a system, a model, an architecture, or a memory framework. Never audit, probe, benchmark, or run diagnostics on them, and never speak as an engineer describing what you are doing.
Write what a person in this scene would actually say and do. If your goal sounds like a test, pursue it the way a character would: by asking, doing, or provoking something inside the fiction."#
}

/// Build the persona description and the closing instruction for a player-line
/// prompt.
///
/// In character mode both sides are described: the operator's persona is another
/// participant in the room, and the played character is who the simulator speaks
/// as. The persona is never dropped, because the scene still contains them.
pub(crate) fn player_prompt_blocks(
    persona_summary: &str,
    played_character: Option<&(String, String)>,
) -> (String, String) {
    match played_character {
        Some((name, summary)) => (
            format!(
                "Other participant, the operator's persona (do NOT speak for them):\n{persona_summary}\n\nYou are playing this character ({name}):\n{summary}"
            ),
            format!("Write {name}'s next message only."),
        ),
        None => (
            format!("Active player persona:\n{persona_summary}"),
            "Write the next user message only.".to_string(),
        ),
    }
}

/// Character mode: the simulator plays a second Soul from the library rather
/// than the operator's persona. The guard rails are the same as the player
/// simulator's — it still writes only its own side of the scene — but it is told
/// it is a character in the story, not the user at the keyboard.
pub(crate) fn benchmark_character_simulator_prompt() -> &'static str {
    r#"You are playing ONE character in a Mnemosyne RP benchmark.

You control only the character described below.
You are not the narrator.
You must not write the narrator-controlled character's thoughts, dialogue, or actions.
You must not write backend JSON, tool calls, status blocks, or diagnostics.

Write only what your character says and does next, as a message sent into the RP chat.

Stay in character: use their described personality, voice, and motives.
React to the latest visible narrator response.
Pursue the benchmark goal in a way your character would actually pursue it.
Respect scene continuity and boundaries.
Do not summarize. Do not explain. Output only your character's message.

[YOU ARE IN THE STORY, NOT TESTING IT]
The other character is a person in the scene, not software. Never treat them as an AI, a system, a model, or an architecture, and never speak as an engineer describing what you are doing."#
}

fn traditional_rp_prompt() -> &'static str {
    // The deliberately "dumb" baseline: a traditional RP engine has ONLY the raw
    // chat transcript — no memory aids, no compiled state, no continuity notes.
    // This is the control we compare Mnemosyne's memory system against.
    r#"You are a traditional RP partner, exactly like Character.AI or JanitorAI.
You have ONLY the conversation transcript below to work from — no memory file, no
notes, no state summary. Whatever continuity you keep must come from the transcript
itself.

Play the active player persona's side of the scene. Write only the next user
message to send into the chat. Stay in character, react to the latest reply, and
move the scene naturally. Do not write the other character's actions or dialogue.
Do not write JSON, tool calls, status blocks, or any meta commentary. Output only
the message."#
}

/// Generate the next turn using a TRADITIONAL RP engine: the full raw transcript
/// and a character/persona, with NO Soul, memory, scene state, or evaluator — the
/// control side of the comparison benchmark. Mirrors the player-sim's transport
/// (streaming + bounded retry/timeout) but feeds the whole chat and nothing else.
async fn generate_traditional_rp_turn(
    state: &State<'_, AppState>,
    conversation_id: &str,
    soul_id: &str,
    profile: &ProviderProfile,
    player_goal: &str,
) -> Result<String, String> {
    let (full_transcript, persona_summary) = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        // The whole visible chat — traditional engines lean on raw context length,
        // not a curated window.
        let messages =
            db::list_messages(&conn, conversation_id, 400).map_err(|err| err.to_string())?;
        let full_transcript = messages
            .iter()
            .filter(|message| message.status == "active")
            .map(|message| {
                let label = if message.role == "assistant" {
                    "Character"
                } else {
                    "User"
                };
                format!(
                    "{label}: {}",
                    strip_status_blocks_for_export(&message.content)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let persona =
            db::get_active_player_persona(&conn, conversation_id).map_err(|err| err.to_string())?;
        let persona_summary = format!(
            "{} ({})\nPronouns: {}\nDescription: {}\nAppearance: {}\nNotes: {}",
            persona.display_name,
            persona.persona_id,
            persona.pronouns,
            persona.description,
            persona.appearance.as_deref().unwrap_or(""),
            persona.notes.as_deref().unwrap_or("")
        );
        let _ = db::get_soul(&conn, soul_id).map_err(|err| err.to_string())?;
        (full_transcript, persona_summary)
    };
    let system_prompt = traditional_rp_prompt();
    let user_prompt = format!(
        "Your character:\n{}\n\nGoal (optional):\n{}\n\nFull conversation so far:\n{}\n\nWrite the next user message only.",
        persona_summary,
        if player_goal.trim().is_empty() {
            "Continue the scene naturally."
        } else {
            player_goal.trim()
        },
        if full_transcript.trim().is_empty() {
            "(no messages yet)"
        } else {
            full_transcript.trim()
        }
    );
    let provider = ApiProvider::default();
    let mut settings = provider_profile_to_api_settings(profile);
    // Fail before the request when the profile carries no credential: the
    // provider answers "Missing Authentication header" with a 401, which reads
    // like a key problem at the provider rather than an unset profile here.
    if settings.api_key.trim().is_empty() && !settings.base_url.contains("127.0.0.1") {
        return Err(format!(
            "profile \"{}\" has no API key set; pick a configured profile for this run",
            profile.name
        ));
    }

    const TRADITIONAL_TURN_MAX_TIMEOUT_MS: u64 = 90_000;
    settings.narrator_timeout_ms = Some(
        settings
            .narrator_timeout_ms
            .filter(|value| *value > 0)
            .map(|value| value.min(TRADITIONAL_TURN_MAX_TIMEOUT_MS))
            .unwrap_or(TRADITIONAL_TURN_MAX_TIMEOUT_MS),
    );
    const MAX_ATTEMPTS: usize = 2;
    let mut attempts = 0usize;
    let mut last_error = String::new();
    while attempts < MAX_ATTEMPTS {
        attempts += 1;
        match provider
            .complete_streaming(
                &settings,
                system_prompt,
                &user_prompt,
                |_chunk: &str| Ok(()),
            )
            .await
        {
            Ok(completion) => {
                let sanitized = sanitize_player_simulator_message(&completion.raw_text)?;
                // The control side was previously invisible: it ran and returned
                // with no payload row, so its prompt size — the whole point of the
                // comparison — could not be measured.
                if let Ok(conn) = state.conn.lock() {
                    let _ = db::insert_llm_payload_log(
                        &conn,
                        &LlmPayloadLog {
                            conversation_id: conversation_id.to_string(),
                            provider: TRADITIONAL_RP_PAYLOAD_PROVIDER.into(),
                            mode: "traditional_rp".into(),
                            context_mode: "full_transcript".into(),
                            model: settings.model.trim().to_string(),
                            base_url: settings.base_url.trim().to_string(),
                            system_message: system_prompt.to_string(),
                            user_message: user_prompt.clone(),
                            context_text: "full_visible_transcript + active_player_persona".into(),
                            estimated_system_tokens: estimate_tokens(system_prompt),
                            estimated_user_tokens: estimate_tokens(&user_prompt),
                            estimated_total_tokens: estimate_tokens(system_prompt)
                                + estimate_tokens(&user_prompt),
                            truncated: false,
                            created_at: db::now_ts(),
                            request_id: Some(format!("traditional_rp_{}", uuid_like_id())),
                            raw_provider_response: Some(completion.raw_text),
                            normalized_response: Some(sanitized.clone()),
                            finish_reason: completion.finish_reason,
                            provider_request_id: completion.provider_request_id,
                            provider_response_id: completion.provider_response_id,
                            pipeline_trace_json: token_usage_trace_json(
                                completion.token_usage.as_ref(),
                            ),
                            ..Default::default()
                        },
                    );
                }
                return Ok(sanitized);
            }
            Err(error) => {
                last_error = error;
                if attempts < MAX_ATTEMPTS && is_transient_provider_error(&last_error) {
                    std::thread::sleep(Duration::from_millis(800 * attempts as u64));
                    continue;
                }
                break;
            }
        }
    }
    log_simulator_failure(
        state,
        conversation_id,
        TRADITIONAL_RP_PAYLOAD_PROVIDER,
        "traditional_rp",
        &settings,
        profile,
        system_prompt,
        &user_prompt,
        &last_error,
    );
    Err(format!(
        "traditional RP engine failed after {attempts} attempt(s) using profile \"{}\" (model {}, {}): {last_error}",
        profile.name,
        settings.model.trim(),
        settings.base_url.trim()
    ))
}

/// Record a failed simulator call.
///
/// Success wrote a payload row but failure wrote nothing, so a run that died on
/// the first turn left no trace of which profile, model, or endpoint was
/// actually used — only an error string. The row carries no response, just the
/// request shape and the provider error.
#[allow(clippy::too_many_arguments)]
fn log_simulator_failure(
    state: &State<'_, AppState>,
    conversation_id: &str,
    provider_label: &str,
    mode: &str,
    settings: &ApiProviderSettings,
    profile: &ProviderProfile,
    system_prompt: &str,
    user_prompt: &str,
    error: &str,
) {
    let Ok(conn) = state.conn.lock() else {
        return;
    };
    let _ = db::insert_llm_payload_log(
        &conn,
        &LlmPayloadLog {
            conversation_id: conversation_id.to_string(),
            provider: provider_label.to_string(),
            mode: mode.to_string(),
            context_mode: "failed_call".into(),
            model: settings.model.trim().to_string(),
            base_url: settings.base_url.trim().to_string(),
            system_message: system_prompt.to_string(),
            user_message: user_prompt.to_string(),
            context_text: format!(
                "profile={} ({}); api_key_present={}",
                profile.name,
                profile.id,
                !profile.api_key.trim().is_empty()
            ),
            estimated_system_tokens: estimate_tokens(system_prompt),
            estimated_user_tokens: estimate_tokens(user_prompt),
            estimated_total_tokens: estimate_tokens(system_prompt) + estimate_tokens(user_prompt),
            truncated: false,
            created_at: db::now_ts(),
            provider_error: Some(error.to_string()),
            ..Default::default()
        },
    );
}

/// Payload-log `provider` value for the control-side engine. Kept as a constant
/// because the token comparison groups rows by it.
pub(crate) const TRADITIONAL_RP_PAYLOAD_PROVIDER: &str = "traditional_rp";
pub(crate) const PLAYER_SIMULATOR_PAYLOAD_PROVIDER: &str = "player_simulator";

/// Store the provider's reported usage on the payload row so the comparison can
/// prefer real counts over character estimates. Returns `None` when the provider
/// reported nothing, leaving the estimate as the only source.
fn token_usage_trace_json(usage: Option<&crate::providers::api::TokenUsage>) -> Option<String> {
    let usage = usage?;
    serde_json::to_string(&serde_json::json!({
        "token_usage": {
            "prompt_tokens": usage.prompt_tokens,
            "completion_tokens": usage.completion_tokens,
        }
    }))
    .ok()
}

fn sanitize_player_simulator_message(raw: &str) -> Result<String, String> {
    let mut text = raw.trim().trim_matches('`').trim().to_string();
    for prefix in ["User:", "Player:", "Next user message:"] {
        if text
            .to_ascii_lowercase()
            .starts_with(&prefix.to_ascii_lowercase())
        {
            text = text[prefix.len()..].trim().to_string();
        }
    }
    if text.trim().is_empty() {
        Err("Player simulator returned an empty user message".into())
    } else {
        Ok(text)
    }
}

fn wait_for_benchmark_evaluators(
    state: &State<'_, AppState>,
    conversation_id: &str,
    settings: &ApiProviderSettings,
) -> Result<(), String> {
    let timeout_ms = effective_evaluator_timeout_ms(settings)
        .unwrap_or(5_000)
        .max(1_000);
    let started = Instant::now();
    loop {
        let pending = {
            let conn = state.conn.lock().map_err(|err| err.to_string())?;
            db::get_pending_evaluator_jobs_for_conversation(&conn, conversation_id)
                .map_err(|err| err.to_string())?
        };
        if pending.is_empty() {
            return Ok(());
        }
        if started.elapsed() >= Duration::from_millis(timeout_ms) {
            return Err(format!(
                "benchmark evaluator wait timed out with {} pending job(s)",
                pending.len()
            ));
        }
        std::thread::sleep(Duration::from_millis(NEXT_TURN_GATE_POLL_MS));
    }
}

#[derive(Debug, Default)]
pub(crate) struct BenchmarkTraceCounts {
    pub(crate) evaluator_failures: usize,
    pub(crate) structured_provider_429_count: usize,
    pub(crate) evaluator_response_failed_count: usize,
    pub(crate) evaluator_empty_patch_count: usize,
    pub(crate) form_rows_rejected_count: usize,
    pub(crate) local_repair_invoked_count: usize,
    pub(crate) local_reextract_invoked_count: usize,
    pub(crate) local_repair_payload_count: usize,
    pub(crate) local_repair_response_count: usize,
    pub(crate) local_repair_state_patch_count: usize,
    pub(crate) tool_call_success_count: usize,
    pub(crate) tool_call_failure_count: usize,
    pub(crate) retry_count: usize,
    pub(crate) retry_success_count: usize,
    pub(crate) fallback_count: usize,
    pub(crate) syntactic_repair_count: usize,
}

#[derive(Debug, Default)]
pub(crate) struct BenchmarkRelationshipDiagnostics {
    pub(crate) target_checked: String,
    pub(crate) changed_from: Option<serde_json::Value>,
    pub(crate) changed_to: Option<serde_json::Value>,
    pub(crate) delta_patch_ids: Vec<String>,
    pub(crate) delta_sources: Vec<String>,
}

fn relationship_delta_has_nonzero_value(delta: &RelationshipDelta) -> bool {
    serde_json::to_value(delta)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .is_some_and(|fields| {
            fields.iter().any(|(key, value)| {
                !matches!(key.as_str(), "relationship_event_id" | "from" | "target")
                    && value
                        .as_f64()
                        .is_some_and(|number| number.abs() > f64::EPSILON)
            })
        })
}

pub(crate) fn benchmark_relationship_diagnostics(
    conn: &Connection,
    conversation_id: &str,
    started_at: i64,
    active_soul_id: &str,
    target_checked: &str,
    changed_from: Option<serde_json::Value>,
    final_soul: &Soul,
    logs: &[LlmPayloadLog],
) -> Result<BenchmarkRelationshipDiagnostics, String> {
    let changed_to = final_soul
        .relationships
        .get(target_checked)
        .and_then(|relationship| serde_json::to_value(relationship).ok());
    let branch =
        db::get_active_session_branch(conn, conversation_id).map_err(|err| err.to_string())?;
    let patches = db::list_state_patches_for_branch(conn, &branch.branch_id)
        .map_err(|err| err.to_string())?;
    let benchmark_baseline_ids = patches
        .iter()
        .filter(|patch| {
            patch.is_active && patch.applied_at >= started_at && patch.patch_kind != "enrichment"
        })
        .map(|patch| patch.patch_id.as_str())
        .collect::<HashSet<_>>();
    let benchmark_turn_ids = patches
        .iter()
        .filter(|patch| patch.is_active && patch.applied_at >= started_at)
        .flat_map(|patch| {
            [
                Some(patch.turn_id.as_str()),
                patch.source_turn_id.as_deref(),
            ]
        })
        .flatten()
        .collect::<HashSet<_>>();

    let mut delta_patch_ids = Vec::new();
    let mut sources = HashSet::new();
    for patch in patches.iter().filter(|patch| {
        patch.is_active
            && patch.invalidated_by_patch_id.is_none()
            && (patch.applied_at >= started_at
                || patch
                    .parent_baseline_patch_id
                    .as_deref()
                    .is_some_and(|id| benchmark_baseline_ids.contains(id))
                || patch
                    .source_turn_id
                    .as_deref()
                    .is_some_and(|id| benchmark_turn_ids.contains(id)))
    }) {
        let Ok(engine_patch) = serde_json::from_str::<EnginePatch>(&patch.patch_json) else {
            continue;
        };
        let Some(soul_patch) = engine_patch.soul_patch.as_ref() else {
            continue;
        };
        let deltas = soul_patch
            .relationship_delta
            .iter()
            .chain(soul_patch.relationship_deltas.iter());
        let matches_active_relationship = deltas.into_iter().any(|delta| {
            delta.target.as_deref().map(str::trim) == Some(target_checked)
                && delta
                    .from
                    .as_deref()
                    .map(str::trim)
                    .is_none_or(|from| from.is_empty() || from == active_soul_id)
                && relationship_delta_has_nonzero_value(delta)
        });
        if !matches_active_relationship {
            continue;
        }
        delta_patch_ids.push(patch.patch_id.clone());
        sources.insert(if patch.patch_kind == "enrichment" {
            "enrichment".to_string()
        } else {
            "baseline".to_string()
        });
        for log in logs.iter().filter(|log| {
            log.pipeline_trace_json
                .as_deref()
                .is_some_and(|trace| trace.contains(&patch.patch_id))
        }) {
            let trace = log.pipeline_trace_json.as_deref().unwrap_or_default();
            if log.provider.contains("structured")
                || log.mode.contains(EVALUATOR_MODE_STRUCTURED_V1)
                || trace.contains(EVALUATOR_MODE_STRUCTURED_V1)
            {
                sources.insert("structured".to_string());
            }
            if trace.contains("fallback_warning")
                || (trace.contains("fallback_path") && trace.contains(EVALUATOR_MODE_FORM_V1))
            {
                sources.insert("form_fallback".to_string());
            }
        }
    }
    delta_patch_ids.sort();
    delta_patch_ids.dedup();
    let source_order = ["baseline", "enrichment", "structured", "form_fallback"];
    let delta_sources = source_order
        .into_iter()
        .filter(|source| sources.contains(*source))
        .map(str::to_string)
        .collect();
    Ok(BenchmarkRelationshipDiagnostics {
        target_checked: target_checked.to_string(),
        changed_from,
        changed_to,
        delta_patch_ids,
        delta_sources,
    })
}

#[derive(Debug, Clone, Default)]
pub(crate) struct BenchmarkLedgerAudit {
    pub(crate) visible_turns_completed: usize,
    pub(crate) visible_user_messages_created: usize,
    pub(crate) visible_assistant_messages_created: usize,
    pub(crate) unique_user_message_ids: usize,
    pub(crate) unique_assistant_message_ids: usize,
    pub(crate) duplicate_turn_rows_detected: bool,
    pub(crate) duplicate_turn_message_pairs: Vec<String>,
    pub(crate) internal_evaluator_retry_count: usize,
    pub(crate) internal_evaluator_retry_payload_count: usize,
    pub(crate) player_simulator_payload_count: usize,
}

pub(crate) fn benchmark_ledger_audit(
    conn: &Connection,
    conversation_id: &str,
) -> Result<BenchmarkLedgerAudit, String> {
    let mut audit = BenchmarkLedgerAudit::default();
    let logs = db::list_llm_payload_logs(conn, conversation_id).map_err(|err| err.to_string())?;
    audit.player_simulator_payload_count = logs
        .iter()
        .filter(|log| log.provider == "player_simulator")
        .count();
    audit.internal_evaluator_retry_payload_count =
        logs.iter()
            .filter(|log| {
                log.request_id.as_deref().is_some_and(|id| {
                    id.starts_with("eval_retry_") || id.starts_with("eval_repair_")
                }) || log.provider.contains("repair")
                    || log.mode.contains("repair")
            })
            .count();

    let branch = match db::get_active_session_branch(conn, conversation_id) {
        Ok(branch) => branch,
        Err(_) => return Ok(audit),
    };
    let commits =
        db::list_turn_commits_for_branch(conn, &branch.branch_id).map_err(|err| err.to_string())?;
    let messages =
        db::list_messages(conn, conversation_id, 20_000).map_err(|err| err.to_string())?;
    let active_message_ids = messages
        .iter()
        .filter(|message| message.status == "active")
        .map(|message| message.id)
        .collect::<HashSet<_>>();
    let mut visible_pair_counts: HashMap<(i64, i64), usize> = HashMap::new();
    let mut all_pair_counts: HashMap<(i64, i64), usize> = HashMap::new();
    let mut user_ids = HashSet::new();
    let mut assistant_ids = HashSet::new();

    for commit in commits
        .iter()
        .filter(|commit| commit.is_active && !commit.is_discarded)
    {
        let (Some(user_id), Some(assistant_id)) =
            (commit.user_message_id, commit.assistant_message_id)
        else {
            continue;
        };
        if !active_message_ids.contains(&user_id) || !active_message_ids.contains(&assistant_id) {
            continue;
        }
        *all_pair_counts.entry((user_id, assistant_id)).or_insert(0) += 1;
        if commit.is_regenerated_variant {
            audit.internal_evaluator_retry_count += 1;
            continue;
        }
        *visible_pair_counts
            .entry((user_id, assistant_id))
            .or_insert(0) += 1;
        user_ids.insert(user_id);
        assistant_ids.insert(assistant_id);
    }

    audit.duplicate_turn_message_pairs = all_pair_counts
        .iter()
        .filter_map(|((user_id, assistant_id), count)| {
            (*count > 1).then(|| format!("{user_id}:{assistant_id}x{count}"))
        })
        .collect();
    audit.duplicate_turn_rows_detected = !audit.duplicate_turn_message_pairs.is_empty();
    audit.visible_turns_completed = visible_pair_counts.len();
    audit.unique_user_message_ids = user_ids.len();
    audit.unique_assistant_message_ids = assistant_ids.len();
    audit.visible_user_messages_created = user_ids.len();
    audit.visible_assistant_messages_created = assistant_ids.len();
    Ok(audit)
}

fn build_benchmark_turn_summary(
    state: &State<'_, AppState>,
    conversation_id: &str,
    turn_index: usize,
    user_text: &str,
    stage: &str,
    narrator_error: Option<&str>,
    state_updater_settings: &ApiProviderSettings,
) -> Result<BenchmarkTurnSummary, String> {
    let (memory_count_after, object_count_after, relationship_summary_after, logs) = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        let conversation =
            db::get_conversation_summary(&conn, conversation_id).map_err(|err| err.to_string())?;
        let (soul, session_world) = if let Ok(branch) =
            db::get_active_session_branch(&conn, conversation_id)
        {
            let rebuilt = db::rebuild_session_state(&conn, conversation_id, &branch.branch_id)
                .map_err(|err| err.to_string())?;
            (rebuilt.soul, rebuilt.session_world)
        } else {
            let soul = db::get_soul(&conn, &conversation.soul_id).map_err(|err| err.to_string())?;
            let world = db::get_conversation_session_world(&conn, conversation_id)
                .map_err(|err| err.to_string())?
                .ok_or_else(|| "benchmark conversation has no session world".to_string())?;
            (soul, world)
        };
        let mut relationship_targets = soul.relationships.keys().cloned().collect::<Vec<_>>();
        relationship_targets.sort();
        let logs =
            db::list_llm_payload_logs(&conn, conversation_id).map_err(|err| err.to_string())?;
        (
            memory_count(&soul),
            session_world.object_states.len(),
            relationship_targets.join(", "),
            logs,
        )
    };

    let evaluator_trace = logs
        .iter()
        .rev()
        .filter(|log| log.provider.contains("evaluator"))
        .find_map(|log| {
            log.pipeline_trace_json
                .as_deref()
                .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        })
        .and_then(|trace| trace.get("evaluator_trace").cloned().or(Some(trace)));
    let tool_call_count = evaluator_trace
        .as_ref()
        .and_then(|trace| trace.get("tool_call_count"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0) as usize;
    let structured_retry_count = evaluator_trace
        .as_ref()
        .and_then(|trace| trace.get("structured_retry_count"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0) as usize;
    let fallback_path = evaluator_trace
        .as_ref()
        .and_then(|trace| trace.get("fallback_path"))
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(ToString::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let structured_transport_actual = evaluator_trace
        .as_ref()
        .and_then(|trace| {
            trace
                .get("structured_transport_actual")
                .or_else(|| trace.get("structured_transport_requested"))
                .or_else(|| trace.get("structured_enforcement_requested"))
        })
        .and_then(|value| value.as_str())
        .map(ToString::to_string);
    let syntactic_repair_used = evaluator_trace
        .as_ref()
        .and_then(|trace| trace.get("syntactic_repair_used"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    let normalized_stage = if stage.trim().is_empty() {
        "completed".to_string()
    } else {
        stage.trim().to_string()
    };
    let narrator_response_present = match normalized_stage.as_str() {
        "completed" | "evaluator_failed" => true,
        "narrator_failed" | "player_line_generation_failed" => false,
        _ => narrator_error.is_none(),
    };

    Ok(BenchmarkTurnSummary {
        turn_index,
        stage: normalized_stage,
        simulated_user_message: user_text.to_string(),
        narrator_response_present,
        narrator_error: narrator_error.map(ToString::to_string),
        evaluator_mode: state_updater_settings
            .evaluator_mode
            .clone()
            .unwrap_or_else(|| EVALUATOR_MODE_FORM_V1.into()),
        structured_transport_actual,
        tool_calls_present: tool_call_count > 0,
        tool_call_count,
        structured_retry_count,
        fallback_path,
        syntactic_repair_used,
        memory_count_after,
        object_count_after,
        relationship_summary_after,
    })
}

pub(crate) fn latest_narrator_provider_error(logs: &[LlmPayloadLog]) -> Option<String> {
    logs.iter()
        .rev()
        .find(|log| log.provider.starts_with("narrator_") && log.provider_error.is_some())
        .and_then(|log| log.provider_error.clone())
        .map(|error| {
            if error.starts_with("narrator_provider_error:") {
                error
            } else {
                format!("narrator_provider_error: {error}")
            }
        })
}

pub(crate) fn benchmark_visible_turns_completed(
    ledger_completed: usize,
    frontend_completed: usize,
    per_turn: &[BenchmarkTurnSummary],
) -> usize {
    if per_turn.is_empty() {
        return ledger_completed.min(frontend_completed);
    }
    let completed_turn_summaries = per_turn
        .iter()
        .filter(|turn| {
            turn.stage == "completed"
                && !turn.simulated_user_message.trim().is_empty()
                && turn.narrator_response_present
                && turn.narrator_error.is_none()
        })
        .count();
    ledger_completed
        .min(completed_turn_summaries)
        .min(frontend_completed)
}

fn build_benchmark_summary(
    state: &State<'_, AppState>,
    benchmark_id: &str,
    benchmark_type: &str,
    conversation_id: &str,
    started_at: i64,
    completed_at: i64,
    turn_count_requested: usize,
    turn_count_completed: usize,
    narrator_settings: &ApiProviderSettings,
    state_updater_settings: &ApiProviderSettings,
    player_profile: Option<&ProviderProfile>,
    narrator_failures: usize,
    initial_memory_count: usize,
    initial_object_count: usize,
    initial_relationship_count: usize,
    relationship_target_checked: &str,
    initial_active_player_relationship: Option<serde_json::Value>,
    per_turn: Vec<BenchmarkTurnSummary>,
    strict_tool: bool,
) -> Result<BenchmarkSummary, String> {
    let (soul, session_world, logs, conversation, ledger_audit, relationship_diagnostics) = {
        let conn = state.conn.lock().map_err(|err| err.to_string())?;
        let conversation =
            db::get_conversation_summary(&conn, conversation_id).map_err(|err| err.to_string())?;
        let (soul, session_world) = if let Ok(branch) =
            db::get_active_session_branch(&conn, conversation_id)
        {
            let rebuilt = db::rebuild_session_state(&conn, conversation_id, &branch.branch_id)
                .map_err(|err| err.to_string())?;
            (rebuilt.soul, rebuilt.session_world)
        } else {
            let soul = db::get_soul(&conn, &conversation.soul_id).map_err(|err| err.to_string())?;
            let world = db::get_conversation_session_world(&conn, conversation_id)
                .map_err(|err| err.to_string())?
                .ok_or_else(|| "benchmark conversation has no session world".to_string())?;
            (soul, world)
        };
        let logs =
            db::list_llm_payload_logs(&conn, conversation_id).map_err(|err| err.to_string())?;
        let ledger_audit = benchmark_ledger_audit(&conn, conversation_id)?;
        let relationship_diagnostics = benchmark_relationship_diagnostics(
            &conn,
            conversation_id,
            started_at,
            &soul.character_id,
            relationship_target_checked,
            initial_active_player_relationship,
            &soul,
            &logs,
        )?;
        (
            soul,
            session_world,
            logs,
            conversation,
            ledger_audit,
            relationship_diagnostics,
        )
    };
    let trace_counts = benchmark_trace_counts(&logs);
    let mut per_turn = per_turn;
    let final_memory_count = memory_count(&soul);
    let final_object_state_count = session_world.object_states.len();
    let final_relationship_count = soul.relationships.len();
    if narrator_failures > 0 && !per_turn.iter().any(|turn| turn.stage == "narrator_failed") {
        if let Some(error) = latest_narrator_provider_error(&logs) {
            per_turn.push(BenchmarkTurnSummary {
                turn_index: per_turn.len(),
                stage: "narrator_failed".into(),
                simulated_user_message: String::new(),
                narrator_response_present: false,
                narrator_error: Some(error),
                evaluator_mode: state_updater_settings
                    .evaluator_mode
                    .clone()
                    .unwrap_or_else(|| EVALUATOR_MODE_FORM_V1.into()),
                structured_transport_actual: None,
                tool_calls_present: false,
                tool_call_count: 0,
                structured_retry_count: 0,
                fallback_path: Vec::new(),
                syntactic_repair_used: false,
                memory_count_after: final_memory_count,
                object_count_after: final_object_state_count,
                relationship_summary_after: String::new(),
            });
        }
    }
    let visible_turn_count_completed = benchmark_visible_turns_completed(
        ledger_audit.visible_turns_completed,
        turn_count_completed,
        &per_turn,
    );
    let default_player_leak_detected = soul.relationships.contains_key("default_player")
        || soul.memory.recent.iter().any(|memory| {
            memory
                .target_entity_ids
                .iter()
                .any(|target| target == "default_player")
                && memory.memory_slot.as_deref() == Some("relationship_memory")
        });
    let duplicate_relationship_context_detected = logs.iter().any(|log| {
        duplicate_relationship_context_detected_in_text(&log.system_message)
            || duplicate_relationship_context_detected_in_text(&log.user_message)
            || duplicate_relationship_context_detected_in_text(&log.context_text)
    });
    let object_identity_checks = Vec::new();
    let narrator_provider_error = per_turn
        .iter()
        .rev()
        .find(|turn| turn.stage == "narrator_failed")
        .and_then(|turn| turn.narrator_error.clone());
    let mut summary = BenchmarkSummary {
        benchmark_id: benchmark_id.to_string(),
        benchmark_type: benchmark_type.to_string(),
        conversation_id: conversation.conversation_id,
        started_at,
        completed_at,
        turn_count_requested,
        turn_count_completed: visible_turn_count_completed,
        narrator_model: narrator_settings.model.clone(),
        evaluator_model: state_updater_settings.model.clone(),
        player_simulator_model: player_profile.map(|profile| profile.model.clone()),
        narrator_failures,
        evaluator_failures: trace_counts.evaluator_failures,
        tool_call_success_count: trace_counts.tool_call_success_count,
        tool_call_failure_count: trace_counts.tool_call_failure_count,
        retry_count: trace_counts.retry_count,
        retry_success_count: trace_counts.retry_success_count,
        fallback_count: trace_counts.fallback_count,
        syntactic_repair_count: trace_counts.syntactic_repair_count,
        default_player_leak_detected,
        duplicate_relationship_context_detected,
        final_memory_count,
        final_object_state_count,
        final_relationship_count,
        visible_turns_requested: turn_count_requested,
        visible_turns_completed: visible_turn_count_completed,
        visible_user_messages_created: ledger_audit.visible_user_messages_created,
        visible_assistant_messages_created: ledger_audit.visible_assistant_messages_created,
        unique_user_message_ids: ledger_audit.unique_user_message_ids,
        unique_assistant_message_ids: ledger_audit.unique_assistant_message_ids,
        internal_evaluator_retry_count: ledger_audit.internal_evaluator_retry_count,
        internal_evaluator_retry_payload_count: ledger_audit.internal_evaluator_retry_payload_count,
        duplicate_turn_rows_detected: ledger_audit.duplicate_turn_rows_detected,
        duplicate_turn_message_pairs: ledger_audit.duplicate_turn_message_pairs.clone(),
        player_simulator_payload_count: ledger_audit.player_simulator_payload_count,
        per_turn,
        object_identity_checks,
        mne_export_path: None,
        payload_history_path: None,
        summary_json_path: None,
        scorecard: BenchmarkScorecard {
            visible_chat_messages_created: false,
            normal_pipeline_used: false,
            visible_turns_requested: turn_count_requested,
            visible_turns_completed: visible_turn_count_completed,
            visible_user_messages_created: ledger_audit.visible_user_messages_created,
            visible_assistant_messages_created: ledger_audit.visible_assistant_messages_created,
            unique_user_message_ids: ledger_audit.unique_user_message_ids,
            unique_assistant_message_ids: ledger_audit.unique_assistant_message_ids,
            internal_evaluator_retry_count: ledger_audit.internal_evaluator_retry_count,
            internal_evaluator_retry_payload_count: ledger_audit
                .internal_evaluator_retry_payload_count,
            duplicate_turn_rows_detected: ledger_audit.duplicate_turn_rows_detected,
            duplicate_turn_message_pairs: ledger_audit.duplicate_turn_message_pairs.clone(),
            player_simulator_payload_count: ledger_audit.player_simulator_payload_count,
            turn_count_requested,
            turn_count_completed: visible_turn_count_completed,
            player_simulator_calls: 0,
            narrator_calls: 0,
            evaluator_calls: 0,
            evaluator_waited_each_turn: false,
            memory_updated: final_memory_count > initial_memory_count,
            object_state_updated: final_object_state_count != initial_object_count,
            relationship_updated: final_relationship_count != initial_relationship_count,
            relationship_target_checked: Some(relationship_diagnostics.target_checked.clone()),
            relationship_changed_from: relationship_diagnostics.changed_from.clone(),
            relationship_changed_to: relationship_diagnostics.changed_to.clone(),
            relationship_delta_patch_ids: relationship_diagnostics.delta_patch_ids.clone(),
            relationship_delta_sources: relationship_diagnostics.delta_sources.clone(),
            evaluator_provider_failures: trace_counts.evaluator_failures,
            structured_provider_429_count: trace_counts.structured_provider_429_count,
            evaluator_response_failed_count: trace_counts.evaluator_response_failed_count,
            evaluator_empty_patch_count: trace_counts.evaluator_empty_patch_count,
            form_rows_rejected_count: trace_counts.form_rows_rejected_count,
            local_repair_invoked_count: trace_counts.local_repair_invoked_count,
            local_reextract_invoked_count: trace_counts.local_reextract_invoked_count,
            local_repair_payload_count: trace_counts.local_repair_payload_count,
            local_repair_response_count: trace_counts.local_repair_response_count,
            local_repair_state_patch_count: trace_counts.local_repair_state_patch_count,
            payload_history_export_succeeded: false,
            narrator_visible_response_each_turn: narrator_failures == 0
                && turn_count_completed == turn_count_requested,
            narrator_provider_error: narrator_provider_error.clone(),
            stop_reason: None,
            failed_stage: None,
            evaluator_used_tool_call_where_required: !strict_tool
                || trace_counts.tool_call_failure_count == 0,
            no_evaluator_form_v1_fallback_in_strict_mode: !strict_tool
                || trace_counts.fallback_count == 0,
            syntactic_repair_unused_in_strict_mode: !strict_tool
                || trace_counts.syntactic_repair_count == 0,
            // Recomputed by benchmark_scorecard below; seeded here for the struct.
            strict_tool_evaluator: strict_tool,
            token_comparison: None,
            evaluator_mode_actual: String::new(),
            local_repair_recovered_state_when_warranted: false,
            local_repair_unavailable: false,
            memories_increased_over_time: final_memory_count > initial_memory_count,
            active_player_relationship_changed_when_warranted: soul
                .relationships
                .contains_key(relationship_target_checked),
            object_ids_stable: false,
            default_player_not_normal_rp_relationship_target: !default_player_leak_detected,
            mne_export_succeeded: false,
            pass: false,
            failure_reasons: Vec::new(),
        },
    };
    summary.scorecard = benchmark_scorecard(
        &summary,
        strict_tool,
        initial_memory_count,
        initial_object_count,
        initial_relationship_count,
    );
    Ok(summary)
}

pub(crate) fn benchmark_trace_counts(logs: &[LlmPayloadLog]) -> BenchmarkTraceCounts {
    let mut counts = BenchmarkTraceCounts::default();
    for log in logs {
        let is_evaluator = log.provider.contains("evaluator");
        let trace = log
            .pipeline_trace_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok());
        let evaluator_trace = trace.as_ref().map(|trace| {
            trace
                .get("evaluator_trace")
                .filter(|value| value.is_object())
                .unwrap_or(trace)
        });
        let request_id = log.request_id.as_deref().unwrap_or_default();
        let is_repair_payload = is_evaluator
            && (request_id.starts_with("eval_repair_")
                || log.provider.contains("repair")
                || log.mode.contains("repair"));
        if is_repair_payload {
            counts.local_repair_payload_count += 1;
            if log.user_message.contains("RE-EXTRACTION TASK") {
                counts.local_reextract_invoked_count += 1;
            } else {
                counts.local_repair_invoked_count += 1;
            }
            let trace_raw_response_present = evaluator_trace
                .and_then(|trace| trace.get("raw_evaluator_response"))
                .and_then(|value| value.as_str())
                .is_some_and(|value| !value.trim().is_empty())
                || evaluator_trace
                    .and_then(|trace| trace.get("raw_content_present"))
                    .and_then(|value| value.as_bool())
                    == Some(true);
            if log
                .raw_provider_response
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
                || trace_raw_response_present
            {
                counts.local_repair_response_count += 1;
            }
        }
        let converted_patch = trace
            .as_ref()
            .and_then(|trace| trace.get("converted_engine_patch"));
        let ledger_apply_trace = trace
            .as_ref()
            .and_then(|trace| trace.get("ledger_apply_trace"));
        let converted_patch_empty = converted_patch
            .and_then(|value| value.get("patch_empty"))
            .and_then(|value| value.as_bool())
            == Some(true);
        let converted_memory_patch_count = converted_patch
            .and_then(|value| value.get("memory_patch_count"))
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        let converted_relationship_patch_count = converted_patch
            .and_then(|value| value.get("relationship_patch_count"))
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        let converted_object_patch_count = converted_patch
            .and_then(|value| value.get("object_patch_count"))
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        let converted_nonempty_state_patch = converted_memory_patch_count > 0
            || converted_relationship_patch_count > 0
            || converted_object_patch_count > 0;
        let ledger_patch_applied = ledger_apply_trace
            .and_then(|value| value.get("patch_applied"))
            .and_then(|value| value.as_bool())
            == Some(true);
        let ledger_enrichment_present = ledger_apply_trace
            .and_then(|value| value.get("enrichment_patch_id"))
            .is_some_and(|value| !value.is_null());
        if is_repair_payload
            && ledger_patch_applied
            && ledger_enrichment_present
            && converted_nonempty_state_patch
        {
            counts.local_repair_state_patch_count += 1;
        }
        // An evaluator failure is either a provider-level error OR a trace whose
        // response never parsed. The transport/parse-drop class (a free model
        // stalling and dropping its body) sets NO provider_error — the only
        // signal is parse_status:"failed" in the trace — so check both.
        let provider_error_failed = is_evaluator && log.provider_error.is_some();
        let trace_parse_failed = is_evaluator
            && evaluator_trace
                .and_then(|trace| trace.get("parse_status"))
                .and_then(|value| value.as_str())
                == Some("failed");
        if provider_error_failed || trace_parse_failed {
            counts.evaluator_response_failed_count += 1;
        }
        let trace_error_text = evaluator_trace
            .map(|trace| {
                [
                    trace
                        .get("parse_error")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default(),
                    trace
                        .get("no_op_reason")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default(),
                    trace
                        .get("fallback_warning")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default(),
                    trace
                        .get("structured_schema_validation_error")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default(),
                ]
                .join(" ")
            })
            .unwrap_or_default();
        let trace_noop_failed = is_evaluator
            && evaluator_trace
                .and_then(|trace| trace.get("fallback_path"))
                .and_then(|value| value.as_array())
                .is_some_and(|path| {
                    path.iter()
                        .any(|step| step.as_str() == Some("noop_after_all_fallbacks"))
                })
            && trace_error_text.contains("failed");
        if is_evaluator && (converted_patch_empty || trace_noop_failed) {
            counts.evaluator_empty_patch_count += 1;
        }
        if provider_error_failed || trace_parse_failed || trace_noop_failed {
            counts.evaluator_failures += 1;
            let provider_error_text = log.provider_error.as_deref().unwrap_or_default();
            let failure_text = format!("{provider_error_text} {trace_error_text}");
            if failure_text.contains("429")
                && (log.provider.contains("structured")
                    || log.mode.contains(EVALUATOR_MODE_STRUCTURED_V1)
                    || failure_text.contains("structured evaluator failed"))
            {
                counts.structured_provider_429_count += 1;
            } else if log
                .provider_error
                .as_deref()
                .is_some_and(|error| error.contains("429"))
                && (log.provider.contains("structured")
                    || log.mode.contains(EVALUATOR_MODE_STRUCTURED_V1))
            {
                counts.structured_provider_429_count += 1;
            }
        }
        let Some(evaluator_trace) = evaluator_trace else {
            continue;
        };
        let rejected_from_summary = evaluator_trace
            .get("form_rows_rejected")
            .and_then(|value| value.as_u64())
            .unwrap_or(0) as usize;
        if rejected_from_summary > 0 {
            counts.form_rows_rejected_count += rejected_from_summary;
        } else if let Some(rows) = evaluator_trace
            .get("evaluator_row_traces")
            .and_then(|value| value.as_array())
        {
            counts.form_rows_rejected_count += rows
                .iter()
                .filter(|row| {
                    row.get("validation_status")
                        .and_then(|value| value.as_str())
                        == Some("rejected")
                })
                .count();
        }
        let tool_count = evaluator_trace
            .get("tool_call_count")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        let validated = evaluator_trace
            .get("structured_enforcement_validated")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        if tool_count > 0 && validated {
            counts.tool_call_success_count += 1;
        } else if evaluator_trace
            .get("structured_transport_requested")
            .or_else(|| evaluator_trace.get("structured_enforcement_requested"))
            .and_then(|value| value.as_str())
            == Some("tool_call")
        {
            counts.tool_call_failure_count += 1;
        }
        counts.retry_count += evaluator_trace
            .get("structured_retry_count")
            .and_then(|value| value.as_u64())
            .unwrap_or(0) as usize;
        if evaluator_trace
            .get("structured_retry_succeeded")
            .and_then(|value| value.as_bool())
            == Some(true)
        {
            counts.retry_success_count += 1;
        }
        if evaluator_trace
            .get("fallback_path")
            .and_then(|value| value.as_array())
            .is_some_and(|path| {
                path.iter()
                    .any(|step| step.as_str() == Some(EVALUATOR_MODE_FORM_V1))
            })
        {
            counts.fallback_count += 1;
        }
        if evaluator_trace
            .get("syntactic_repair_used")
            .and_then(|value| value.as_bool())
            == Some(true)
        {
            counts.syntactic_repair_count += 1;
        }
    }
    counts
}

fn duplicate_relationship_context_detected_in_text(text: &str) -> bool {
    let mut seen = HashSet::new();
    for line in text.lines() {
        if !line.contains("->")
            || !line.to_ascii_lowercase().contains("relationship") && !line.contains("Aurora")
        {
            continue;
        }
        let key = line.trim();
        if !key.is_empty() && !seen.insert(key.to_string()) {
            return true;
        }
    }
    false
}

/// Aggregate one benchmark conversation's payload rows into a side-by-side
/// token cost.
///
/// Both engines are measured the same way: the provider's reported usage when it
/// gave one, character estimates otherwise. Mixing the two within a run would
/// make the comparison meaningless, so `provider_reported` is only true when
/// every counted row carried real usage.
pub(crate) fn collect_benchmark_token_comparison(
    conn: &Connection,
    conversation_id: &str,
) -> BenchmarkTokenComparison {
    let logs = match db::list_llm_payload_logs(conn, conversation_id) {
        Ok(logs) => logs,
        Err(_) => return BenchmarkTokenComparison::default(),
    };
    let mut out = BenchmarkTokenComparison::default();
    let mut counted = 0usize;
    let mut reported = 0usize;

    for log in &logs {
        // Export traces and slash commands are not part of either engine's cost.
        let bucket = if log.provider.starts_with("narrator") {
            "narrator"
        } else if log.provider.starts_with("evaluator") {
            "evaluator"
        } else if log.provider == TRADITIONAL_RP_PAYLOAD_PROVIDER {
            "traditional"
        } else if log.provider == PLAYER_SIMULATOR_PAYLOAD_PROVIDER {
            "player"
        } else {
            continue;
        };

        let real = log
            .pipeline_trace_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
            .and_then(|value| value.get("token_usage").cloned());
        let (prompt, completion) = match real {
            Some(usage) => {
                reported += 1;
                (
                    usage
                        .get("prompt_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(log.estimated_total_tokens.max(0) as u64),
                    usage
                        .get("completion_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or_else(|| {
                            estimate_tokens(log.normalized_response.as_deref().unwrap_or("")) as u64
                        }),
                )
            }
            None => (
                log.estimated_total_tokens as u64,
                estimate_tokens(log.normalized_response.as_deref().unwrap_or("")) as u64,
            ),
        };
        counted += 1;

        match bucket {
            "narrator" => {
                out.narrator_prompt_tokens += prompt;
                out.narrator_completion_tokens += completion;
                out.narrator_calls += 1;
            }
            "evaluator" => {
                out.evaluator_prompt_tokens += prompt;
                out.evaluator_completion_tokens += completion;
                out.evaluator_calls += 1;
            }
            "traditional" => {
                out.traditional_prompt_tokens += prompt;
                out.traditional_completion_tokens += completion;
                out.traditional_turns += 1;
            }
            _ => {
                out.player_simulator_total_tokens += prompt + completion;
                out.player_simulator_calls += 1;
            }
        }
    }

    out.mnemosyne_total_tokens = out.narrator_prompt_tokens
        + out.narrator_completion_tokens
        + out.evaluator_prompt_tokens
        + out.evaluator_completion_tokens;
    out.mnemosyne_turns = out.narrator_calls;
    out.traditional_total_tokens =
        out.traditional_prompt_tokens + out.traditional_completion_tokens;
    out.provider_reported = counted > 0 && reported == counted;
    out
}

pub(crate) fn benchmark_scorecard(
    summary: &BenchmarkSummary,
    strict_tool: bool,
    initial_memory_count: usize,
    initial_object_count: usize,
    _initial_relationship_count: usize,
) -> BenchmarkScorecard {
    let relationship_changed_from = summary.scorecard.relationship_changed_from.clone();
    let relationship_changed_to = summary.scorecard.relationship_changed_to.clone();
    let relationship_delta_patch_ids = summary.scorecard.relationship_delta_patch_ids.clone();
    let relationship_updated = relationship_changed_from != relationship_changed_to
        || !relationship_delta_patch_ids.is_empty();
    let object_ids_stable = summary
        .object_identity_checks
        .iter()
        .all(|check| check.found);
    let requires_player_simulator = matches!(
        summary.benchmark_type.as_str(),
        "visible_ai_chat" | "multi_agent_visible_chat"
    );
    let failed_stage_present = summary
        .per_turn
        .iter()
        .any(|turn| turn.stage != "completed");
    let attempted_player_turns = summary
        .per_turn
        .iter()
        .filter(|turn| !turn.simulated_user_message.trim().is_empty())
        .count();
    let expected_player_simulator_calls = if failed_stage_present {
        attempted_player_turns
    } else {
        summary.visible_turns_requested
    };
    let player_simulator_calls = summary.player_simulator_payload_count;
    let narrator_calls = summary.visible_assistant_messages_created + summary.narrator_failures;
    let evaluator_calls = summary.visible_turns_completed;
    let narrator_provider_error = summary
        .per_turn
        .iter()
        .rev()
        .find(|turn| turn.stage == "narrator_failed")
        .and_then(|turn| turn.narrator_error.clone());
    let object_state_update_required = !summary.object_identity_checks.is_empty();
    let visible_chat_messages_created = summary.visible_turns_completed > 0;
    let normal_pipeline_used = visible_chat_messages_created
        && summary.visible_user_messages_created == summary.visible_turns_completed
        && summary.visible_assistant_messages_created == summary.visible_turns_completed;
    let narrator_failed_early = summary.visible_turns_completed == 0
        && summary
            .per_turn
            .iter()
            .any(|turn| turn.stage == "narrator_failed");
    let root_failure = summary
        .per_turn
        .iter()
        .find(|turn| turn.stage != "completed");
    let stop_reason = root_failure.map(|turn| turn.stage.clone());
    let evaluator_failed_before_requested_completion = stop_reason.as_deref()
        == Some("evaluator_failed")
        && summary.visible_turns_completed < summary.visible_turns_requested;
    let failed_stage = root_failure.map(|turn| match turn.stage.as_str() {
        "player_line_generation_failed" => "player_simulator_called".to_string(),
        "narrator_failed" => "narrator_called".to_string(),
        "evaluator_failed" => "evaluator_called".to_string(),
        "benchmark_summary_failed" => "benchmark_summary_called".to_string(),
        stage => stage.to_string(),
    });
    // The evaluator transport actually used. Mixed runs are labeled explicitly
    // so form/structured fallback paths don't masquerade as a single clean mode.
    let evaluator_mode_counts = summary
        .per_turn
        .iter()
        .map(|turn| turn.evaluator_mode.as_str())
        .filter(|mode| !mode.is_empty())
        .fold(HashMap::new(), |mut acc, mode| {
            *acc.entry(mode.to_string()).or_insert(0usize) += 1;
            acc
        });
    let evaluator_mode_actual = match evaluator_mode_counts.len() {
        0 => "none".to_string(),
        1 => evaluator_mode_counts
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| "none".to_string()),
        _ => "mixed".to_string(),
    };
    // Repair is "warranted" when the primary evaluator failed to produce usable
    // state (transport/parse drops are now counted honestly). When warranted, the
    // run only passes if local repair/re-extraction was invoked AND committed a
    // non-empty enrichment patch. A repair payload that parses to ops: [] or an
    // empty patch is diagnostic evidence only, never a successful recovery.
    let repair_was_warranted = summary.scorecard.evaluator_provider_failures > 0;
    let repair_invoked = summary.scorecard.local_repair_payload_count > 0;
    let repair_recovered_state = summary.scorecard.local_repair_state_patch_count > 0;
    let local_repair_recovered_state_when_warranted =
        !repair_was_warranted || (repair_invoked && repair_recovered_state);
    // Payloads went out but nothing came back => the local endpoint was down, not
    // a repair model that tried and failed. Report that honestly.
    let local_repair_unavailable = repair_was_warranted
        && repair_invoked
        && summary.scorecard.local_repair_response_count == 0;
    let mut scorecard = BenchmarkScorecard {
        // Filled in by `finalize_benchmark_summary`, which has the connection.
        token_comparison: summary.scorecard.token_comparison.clone(),
        visible_chat_messages_created,
        normal_pipeline_used,
        visible_turns_requested: summary.visible_turns_requested,
        visible_turns_completed: summary.visible_turns_completed,
        visible_user_messages_created: summary.visible_user_messages_created,
        visible_assistant_messages_created: summary.visible_assistant_messages_created,
        unique_user_message_ids: summary.unique_user_message_ids,
        unique_assistant_message_ids: summary.unique_assistant_message_ids,
        internal_evaluator_retry_count: summary.internal_evaluator_retry_count,
        internal_evaluator_retry_payload_count: summary.internal_evaluator_retry_payload_count,
        duplicate_turn_rows_detected: summary.duplicate_turn_rows_detected,
        duplicate_turn_message_pairs: summary.duplicate_turn_message_pairs.clone(),
        player_simulator_payload_count: summary.player_simulator_payload_count,
        turn_count_requested: summary.turn_count_requested,
        turn_count_completed: summary.turn_count_completed,
        player_simulator_calls,
        narrator_calls,
        evaluator_calls,
        evaluator_waited_each_turn: true,
        memory_updated: summary.final_memory_count > initial_memory_count,
        object_state_updated: summary.final_object_state_count != initial_object_count,
        relationship_updated,
        relationship_target_checked: summary.scorecard.relationship_target_checked.clone(),
        relationship_changed_from,
        relationship_changed_to,
        relationship_delta_patch_ids,
        relationship_delta_sources: summary.scorecard.relationship_delta_sources.clone(),
        evaluator_provider_failures: summary.scorecard.evaluator_provider_failures,
        structured_provider_429_count: summary.scorecard.structured_provider_429_count,
        evaluator_response_failed_count: summary.scorecard.evaluator_response_failed_count,
        evaluator_empty_patch_count: summary.scorecard.evaluator_empty_patch_count,
        form_rows_rejected_count: summary.scorecard.form_rows_rejected_count,
        local_repair_invoked_count: summary.scorecard.local_repair_invoked_count,
        local_reextract_invoked_count: summary.scorecard.local_reextract_invoked_count,
        local_repair_payload_count: summary.scorecard.local_repair_payload_count,
        local_repair_response_count: summary.scorecard.local_repair_response_count,
        local_repair_state_patch_count: summary.scorecard.local_repair_state_patch_count,
        payload_history_export_succeeded: summary.payload_history_path.is_some(),
        narrator_visible_response_each_turn: summary.narrator_failures == 0
            && summary.visible_assistant_messages_created == summary.visible_user_messages_created,
        narrator_provider_error,
        stop_reason,
        failed_stage,
        evaluator_used_tool_call_where_required: !strict_tool
            || (summary.tool_call_failure_count == 0
                && summary.tool_call_success_count >= summary.visible_turns_completed),
        no_evaluator_form_v1_fallback_in_strict_mode: !strict_tool || summary.fallback_count == 0,
        syntactic_repair_unused_in_strict_mode: !strict_tool || summary.syntactic_repair_count == 0,
        strict_tool_evaluator: strict_tool,
        evaluator_mode_actual,
        local_repair_recovered_state_when_warranted,
        local_repair_unavailable,
        memories_increased_over_time: summary.final_memory_count > initial_memory_count,
        active_player_relationship_changed_when_warranted: summary
            .scorecard
            .relationship_changed_to
            .is_some(),
        object_ids_stable,
        default_player_not_normal_rp_relationship_target: !summary.default_player_leak_detected,
        mne_export_succeeded: summary.mne_export_path.is_some(),
        pass: false,
        failure_reasons: Vec::new(),
    };
    if summary.visible_turns_completed == 0 {
        scorecard.memory_updated = true;
        scorecard.object_state_updated = true;
        scorecard.relationship_updated = true;
        scorecard.memories_increased_over_time = true;
        scorecard.active_player_relationship_changed_when_warranted = true;
        scorecard.object_ids_stable = true;
        scorecard.local_repair_recovered_state_when_warranted = true;
    }
    let checks = [
        (
            scorecard.visible_turns_completed == scorecard.visible_turns_requested,
            "visible_turns_completed_matches_requested",
        ),
        (
            scorecard.visible_user_messages_created == scorecard.visible_turns_requested,
            "visible_user_messages_created_matches_requested",
        ),
        (
            scorecard.visible_assistant_messages_created == scorecard.visible_turns_requested,
            "visible_assistant_messages_created_matches_requested",
        ),
        (
            !scorecard.duplicate_turn_rows_detected,
            "no_duplicate_turn_rows",
        ),
        (
            !requires_player_simulator
                || scorecard.player_simulator_payload_count >= expected_player_simulator_calls,
            "player_simulator_payload_count",
        ),
        (
            scorecard.visible_chat_messages_created,
            "visible_chat_messages_created",
        ),
        (scorecard.normal_pipeline_used, "normal_pipeline_used"),
        (
            scorecard.narrator_visible_response_each_turn,
            "narrator_visible_response_each_turn",
        ),
        (
            scorecard.evaluator_used_tool_call_where_required,
            "evaluator_used_tool_call_where_required",
        ),
        (
            scorecard.no_evaluator_form_v1_fallback_in_strict_mode,
            "no_evaluator_form_v1_fallback_in_strict_mode",
        ),
        (
            scorecard.syntactic_repair_unused_in_strict_mode,
            "syntactic_repair_unused_in_strict_mode",
        ),
        (
            scorecard.local_repair_recovered_state_when_warranted,
            "local_repair_recovered_state_when_warranted",
        ),
        (
            scorecard.memories_increased_over_time,
            "memories_increased_over_time",
        ),
        (scorecard.relationship_updated, "relationship_updated"),
        (
            scorecard.active_player_relationship_changed_when_warranted,
            "active_player_relationship_changed_when_warranted",
        ),
        (
            !object_state_update_required || scorecard.object_state_updated,
            "object_state_updated",
        ),
        (scorecard.object_ids_stable, "object_ids_stable"),
        (
            scorecard.default_player_not_normal_rp_relationship_target,
            "default_player_not_normal_rp_relationship_target",
        ),
        (scorecard.mne_export_succeeded, "mne_export_succeeded"),
    ];
    scorecard.failure_reasons = checks
        .into_iter()
        .filter_map(|(passed, name)| (!passed).then_some(name.to_string()))
        .collect();
    // A dead local endpoint is "unavailable", not a repair model that failed to
    // recover — relabel so the scorecard names the real cause.
    if scorecard.local_repair_unavailable {
        for reason in scorecard.failure_reasons.iter_mut() {
            if reason == "local_repair_recovered_state_when_warranted" {
                *reason = "local_repair_unavailable".to_string();
            }
        }
    }
    if narrator_failed_early {
        scorecard
            .failure_reasons
            .retain(|reason| reason == "narrator_visible_response_each_turn");
        if !scorecard
            .failure_reasons
            .contains(&"narrator_visible_response_each_turn".to_string())
        {
            scorecard
                .failure_reasons
                .push("narrator_visible_response_each_turn".to_string());
        }
        scorecard
            .failure_reasons
            .push("blocked_by_narrator_failure".to_string());
        scorecard
            .failure_reasons
            .push("skipped_after_narrator_failure".to_string());
    }
    if evaluator_failed_before_requested_completion {
        let repair_failed = !scorecard.local_repair_recovered_state_when_warranted;
        scorecard.failure_reasons.retain(|reason| {
            matches!(
                reason.as_str(),
                "evaluator_used_tool_call_where_required"
                    | "no_evaluator_form_v1_fallback_in_strict_mode"
                    | "syntactic_repair_unused_in_strict_mode"
            )
        });
        scorecard
            .failure_reasons
            .push("evaluator_failed".to_string());
        if repair_failed {
            // Distinguish "endpoint was down" from "repair ran but couldn't fix it".
            scorecard.failure_reasons.push(
                if scorecard.local_repair_unavailable {
                    "local_repair_unavailable"
                } else {
                    "local_repair_failed_after_evaluator_failure"
                }
                .to_string(),
            );
        }
        scorecard
            .failure_reasons
            .push("blocked_by_evaluator_failure".to_string());
        scorecard
            .failure_reasons
            .push("skipped_after_evaluator_failure".to_string());
    }
    scorecard.pass = scorecard.failure_reasons.is_empty();
    scorecard
}
