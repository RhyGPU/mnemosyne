//! Headless multi-turn run against real models.
//!
//! `send_api_turn` takes Tauri's `AppHandle`/`Window`/`State`, which cannot be
//! constructed outside the desktop runtime, so a full benchmark normally needs a
//! human driving the UI. This harness replaces only that glue: the context
//! compiler, narrator prompt, evaluator prompt, provider-enforced ops schema,
//! op compilation, and patch application are all the shipped code paths.
//!
//! What it therefore *does* prove: whether real models populate the scene and
//! knowledge slots, what the prompt actually contains, and what a turn costs.
//! What it does *not* cover: the ledger commit path and background evaluator
//! jobs, which live in the Tauri command.
//!
//! ```text
//! MNE_LIVE_DB=~/.local/share/com.mnemosyne.app/mnemosyne.sqlite3 \
//! MNE_LIVE_TURNS=6 \
//! cargo test --manifest-path src-tauri/Cargo.toml live_multi_turn -- --ignored --nocapture
//! ```

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use state_engine::context_compiler::{compile_context_for_session, ContextMessage};
    use state_engine::evaluator::EvaluatorConversionContext;
    use state_engine::evaluator_structured::{
        compile_evaluator_ops_to_engine_patch, evaluator_ops_json_schema,
        EvaluatorStructuredOutputV1, EVALUATOR_OPS_SCHEMA_NAME,
    };
    use state_engine::setting::SessionWorld;
    use state_engine::soul::Soul;
    use std::time::Duration;

    use crate::providers::api::{
        build_structured_evaluator_prompt, build_system_prompt, ApiProvider, ApiProviderSettings,
    };

    struct Profile {
        settings: ApiProviderSettings,
    }

    fn load_profile(conn: &Connection, name: &str) -> Profile {
        let (api_key, model, base_url): (String, String, String) = conn
            .query_row(
                "SELECT api_key, model, base_url FROM provider_profiles WHERE name = ?1",
                [name],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap_or_else(|_| panic!("profile {name} not found"));
        Profile {
            settings: ApiProviderSettings {
                api_key,
                model,
                base_url,
                narrator_timeout_ms: Some(90_000),
                narrator_max_tokens: Some(700),
                ..ApiProviderSettings::default()
            },
        }
    }

    fn newest_aurora_session(conn: &Connection) -> (String, Soul, SessionWorld) {
        let conversation_id: String = conn
            .query_row(
                "SELECT c.id FROM conversations c
                 JOIN souls s ON s.character_id = c.soul_id
                 WHERE c.archived_at IS NULL AND s.character_name LIKE 'Aurora%'
                 ORDER BY c.updated_at DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .expect("an Aurora conversation");
        let soul_id: String = conn
            .query_row(
                "SELECT soul_id FROM conversations WHERE id = ?1",
                [&conversation_id],
                |row| row.get(0),
            )
            .expect("soul id");
        let soul_json: String = conn
            .query_row(
                "SELECT soul_json FROM souls WHERE character_id = ?1",
                [&soul_id],
                |row| row.get(0),
            )
            .expect("soul json");
        let world_json: String = conn
            .query_row(
                "SELECT w.world_json FROM session_worlds w
                 JOIN conversations c ON c.world_id = w.world_id WHERE c.id = ?1",
                [&conversation_id],
                |row| row.get(0),
            )
            .expect("world json");
        (
            conversation_id,
            serde_json::from_str(&soul_json).expect("soul"),
            serde_json::from_str(&world_json).expect("world"),
        )
    }

    fn section(text: &str, header: &str) -> Option<String> {
        let start = text.find(header)?;
        let rest = &text[start..];
        let end = rest[header.len()..]
            .find("\n\n[")
            .map(|i| i + header.len())
            .unwrap_or(rest.len());
        Some(rest[..end].to_string())
    }

    #[tokio::test]
    #[ignore = "makes real provider requests; needs MNE_LIVE_DB"]
    async fn live_multi_turn() {
        let db_path = std::env::var("MNE_LIVE_DB").expect("set MNE_LIVE_DB");
        let turns: usize = std::env::var("MNE_LIVE_TURNS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(6);
        let conn = Connection::open(&db_path).expect("open database");

        let narrator = load_profile(&conn, "nemotron-3-ultra");
        let evaluator = load_profile(&conn, "glm-5.3-flash");
        let player = load_profile(&conn, "nemotron-3-ultra");
        let (_conversation_id, mut soul, mut world) = newest_aurora_session(&conn);
        let provider = ApiProvider::default();

        println!("soul: {} | world: {}", soul.character_name, world.location);

        let mut messages: Vec<ContextMessage> = Vec::new();
        let mut narrator_prompt_tokens = 0u64;
        let mut narrator_completion_tokens = 0u64;
        let mut evaluator_prompt_tokens = 0u64;
        let mut evaluator_completion_tokens = 0u64;
        let mut ops_seen: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut scene_block_turns = 0usize;
        let mut knowledge_block_turns = 0usize;

        for turn in 1..=turns {
            println!("\n──────── turn {turn} ────────");

            // 1. player line
            let player_prompt = format!(
                "You are roleplaying as the visitor in a scene with {}. \
                 Move the scene forward with a concrete physical action and a direct question. \
                 Write one short message, no narration of {}'s actions.",
                soul.character_name, soul.character_name
            );
            let recent = messages
                .iter()
                .rev()
                .take(4)
                .map(|m| format!("{}: {}", m.role, m.content))
                .collect::<Vec<_>>()
                .join("\n");
            let player_line = match provider
                .complete_streaming(&player.settings, &player_prompt, &recent, |_c| Ok(()))
                .await
            {
                Ok(c) => c.raw_text.trim().to_string(),
                Err(e) => {
                    println!("  player FAILED: {e} — retrying once");
                    match provider
                        .complete_streaming(&player.settings, &player_prompt, &recent, |_c| Ok(()))
                        .await
                    {
                        Ok(c) => c.raw_text.trim().to_string(),
                        Err(e2) => {
                            println!("  player FAILED twice: {e2}");
                            break;
                        }
                    }
                }
            };
            println!(
                "  USER: {}",
                player_line.chars().take(110).collect::<String>()
            );
            messages.push(ContextMessage {
                role: "user".into(),
                content: player_line.clone(),
            });

            // 2. compile context with the real compiler
            let preview = compile_context_for_session(&soul, Some(&world), &messages);
            let has_scene = preview.text.contains("[SCENE CONTINUITY]");
            let has_knowledge = preview.text.contains("[WHO KNOWS WHAT]");
            if has_scene {
                scene_block_turns += 1;
            }
            if has_knowledge {
                knowledge_block_turns += 1;
            }
            println!(
                "  context: {} tokens | SCENE CONTINUITY={} | WHO KNOWS WHAT={}",
                preview.estimated_tokens, has_scene, has_knowledge
            );
            if turn == 1 || has_knowledge {
                if let Some(block) = section(&preview.text, "[SCENE CONTINUITY]") {
                    println!("  --- {}", block.replace('\n', "\n  --- "));
                }
                if let Some(block) = section(&preview.text, "[WHO KNOWS WHAT]") {
                    println!("  --- {}", block.replace('\n', "\n  --- "));
                }
            }

            // 3. narrator
            let system = build_system_prompt(&narrator.settings, &soul, &preview.text, "reader");
            let narration = match provider
                .complete_streaming(&narrator.settings, &system, &player_line, |_c| Ok(()))
                .await
            {
                Ok(c) => {
                    if let Some(u) = c.token_usage.as_ref() {
                        narrator_prompt_tokens += u.prompt_tokens.unwrap_or(0);
                        narrator_completion_tokens += u.completion_tokens.unwrap_or(0);
                    }
                    c.raw_text.trim().to_string()
                }
                Err(e) => {
                    println!("  narrator FAILED: {e}");
                    break;
                }
            };
            println!(
                "  NARRATOR: {}",
                narration.chars().take(110).collect::<String>()
            );
            messages.push(ContextMessage {
                role: "assistant".into(),
                content: narration.clone(),
            });

            // 4. evaluator through the provider-enforced ops schema
            let eval_system = build_structured_evaluator_prompt(&soul, Some(&world));
            let eval_user =
                format!("[LATEST USER MESSAGE]\n{player_line}\n\n[NARRATOR RESPONSE]\n{narration}");
            let completion = match provider
                .complete_structured_tool_call_prompt(
                    &evaluator.settings,
                    &eval_system,
                    &eval_user,
                    0.2,
                    Some(Duration::from_secs(90)),
                    EVALUATOR_OPS_SCHEMA_NAME,
                    &evaluator_ops_json_schema(),
                )
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    println!("  evaluator FAILED: {e}");
                    continue;
                }
            };
            if let Some(u) = completion.token_usage.as_ref() {
                evaluator_prompt_tokens += u.prompt_tokens.unwrap_or(0);
                evaluator_completion_tokens += u.completion_tokens.unwrap_or(0);
            }
            println!(
                "  evaluator tool_calls={} count={} retries={}",
                completion.trace.tool_calls_present,
                completion.trace.tool_call_count,
                completion.trace.structured_retry_count
            );

            let output: EvaluatorStructuredOutputV1 =
                match serde_json::from_str(&completion.raw_text) {
                    Ok(v) => v,
                    Err(e) => {
                        println!(
                            "  evaluator PARSE FAILED: {e} | raw: {}",
                            completion.raw_text.chars().take(200).collect::<String>()
                        );
                        continue;
                    }
                };
            for op in &output.ops {
                let label = serde_json::to_value(op)
                    .ok()
                    .and_then(|v| v.get("op").and_then(|o| o.as_str()).map(str::to_string))
                    .unwrap_or_else(|| "unknown".into());
                *ops_seen.entry(label).or_insert(0) += 1;
            }
            if output.ops.is_empty() {
                println!(
                    "  ops: NONE | no_op_reason={:?}",
                    output.no_op_reason.as_deref().unwrap_or("(none given)")
                );
                println!(
                    "  raw: {}",
                    completion.raw_text.chars().take(400).collect::<String>()
                );
            } else {
                println!("  ops this turn: {}", output.ops.len());
            }
            println!("  ops cumulative: {ops_seen:?}");

            let context = EvaluatorConversionContext {
                active_soul_id: &soul.character_id,
                active_soul_ids: vec![soul.character_id.clone()],
                latest_user_message: &player_line,
                latest_narrator_response: &narration,
                session_world: Some(&world),
                baseline_recent_event_id: None,
            };
            match compile_evaluator_ops_to_engine_patch(&output, &context, &soul) {
                Ok(report) => {
                    if !report.rejected_candidates.is_empty() {
                        for r in &report.rejected_candidates {
                            println!("  rejected {}: {}", r.candidate_id, r.reason);
                        }
                    }
                    match report.patch.apply_to_session(&mut soul, Some(&mut world)) {
                        Ok(applied) => println!(
                            "  applied: memories+{} world_updated={}",
                            applied.memories_added, applied.world_updated
                        ),
                        Err(e) => println!("  apply FAILED: {e:?}"),
                    }
                }
                Err(e) => println!("  compile FAILED: {e}"),
            }
        }

        println!("\n════════ summary ════════");
        println!("turns with [SCENE CONTINUITY]: {scene_block_turns}/{turns}");
        println!("turns with [WHO KNOWS WHAT]:   {knowledge_block_turns}/{turns}");
        println!("ops emitted: {ops_seen:?}");
        println!(
            "narrator tokens:  prompt {narrator_prompt_tokens}  completion {narrator_completion_tokens}"
        );
        println!(
            "evaluator tokens: prompt {evaluator_prompt_tokens}  completion {evaluator_completion_tokens}"
        );
        println!("\nfinal scene_state: {:#?}", world.scene_state);
        println!("knowledge entries: {}", world.knowledge.len());
        for entry in world.knowledge.iter().filter(|e| e.is_active) {
            println!(
                "  {} {} {}",
                entry.holder_entity_id,
                entry.status.as_label(),
                entry.proposition
            );
        }
    }
}
