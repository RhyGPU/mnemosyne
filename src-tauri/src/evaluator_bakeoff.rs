//! Measure candidate evaluator models on the same extraction task.
//!
//! The live harness showed the evaluator costing ~2.8x the narrator, almost all
//! of it completion tokens spent on internal reasoning rather than output. Price
//! per token does not capture that, and the provider catalogue's "reasoning"
//! flag is set on nearly every model, so the only way to choose is to run the
//! real ops schema against a real exchange and count what comes back.
//!
//! Every candidate gets a byte-identical prompt, so the numbers are comparable.
//!
//! ```text
//! MNE_LIVE_DB=~/.local/share/com.mnemosyne.app/mnemosyne.sqlite3 \
//! cargo test --manifest-path src-tauri/Cargo.toml evaluator_bakeoff -- --ignored --nocapture
//! ```

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use state_engine::evaluator_structured::{
        evaluator_ops_json_schema, EvaluatorStructuredOutputV1, EVALUATOR_OPS_SCHEMA_NAME,
    };
    use state_engine::soul::new_default_soul;
    use std::time::{Duration, Instant};

    use crate::providers::api::{
        build_structured_evaluator_prompt, ApiProvider, ApiProviderSettings,
    };

    /// Candidates that advertise tool calling. Kept small and explicit so a run
    /// stays cheap and the comparison is readable.
    const CANDIDATES: [&str; 6] = [
        "z-ai/glm-5.3-flash",
        "mistralai/mistral-nemo",
        "nvidia/nemotron-3-super-120b-a12b:free",
        "z-ai/glm-5.2:free",
        "minimax/minimax-m3:free",
        "inclusionai/ling-3.0-flash-fin:free",
    ];

    const USER_LINE: &str = "I set the crowbar down on the step and hold up both hands. \"I didn't force anything. Someone gave me this address — I don't know who.\"";
    const NARRATOR_LINE: &str = "Aurora keeps the chain latched, the door still open only three inches. Her eyes drop to the crowbar, then back to his face. \"Someone,\" she repeats, flat. She doesn't say she believes him. The rain keeps hammering the overhang behind him, and somewhere inside the apartment the record skips once and settles. She doesn't mention that the spare key is still in her pocket.";

    #[tokio::test]
    #[ignore = "makes real provider requests; needs MNE_LIVE_DB"]
    async fn evaluator_bakeoff() {
        let db_path = std::env::var("MNE_LIVE_DB").expect("set MNE_LIVE_DB");
        let conn = Connection::open(&db_path).expect("open database");
        let (api_key, base_url): (String, String) = conn
            .query_row(
                "SELECT api_key, base_url FROM provider_profiles
                 WHERE archived_at IS NULL AND base_url LIKE '%openrouter%' LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("an OpenRouter profile to borrow credentials from");

        let soul = new_default_soul("Aurora Schwarz");
        let system = build_structured_evaluator_prompt(&soul, None);
        let user =
            format!("[LATEST USER MESSAGE]\n{USER_LINE}\n\n[NARRATOR RESPONSE]\n{NARRATOR_LINE}");
        let schema = evaluator_ops_json_schema();
        let provider = ApiProvider::default();

        println!(
            "{:44} {:>6} {:>7} {:>7} {:>5} {:>6}  {}",
            "model", "sec", "prompt", "compl", "ops", "tools", "verdict"
        );

        for model in CANDIDATES {
            let settings = ApiProviderSettings {
                api_key: api_key.clone(),
                base_url: base_url.clone(),
                model: model.to_string(),
                evaluator_timeout_ms: Some(120_000),
                narrator_timeout_ms: Some(120_000),
                ..ApiProviderSettings::default()
            };
            let started = Instant::now();
            let mut result = provider
                .complete_structured_tool_call_prompt(
                    &settings,
                    &system,
                    &user,
                    0.2,
                    Some(Duration::from_secs(120)),
                    EVALUATOR_OPS_SCHEMA_NAME,
                    &schema,
                )
                .await;
            // Free pools rate-limit constantly; one retry keeps a candidate from
            // being judged on a transient 429.
            if result.as_ref().err().is_some_and(|e| e.contains("429")) {
                tokio::time::sleep(Duration::from_secs(6)).await;
                result = provider
                    .complete_structured_tool_call_prompt(
                        &settings,
                        &system,
                        &user,
                        0.2,
                        Some(Duration::from_secs(120)),
                        EVALUATOR_OPS_SCHEMA_NAME,
                        &schema,
                    )
                    .await;
            }
            let secs = started.elapsed().as_secs_f32();

            match result {
                Ok(completion) => {
                    let prompt = completion
                        .token_usage
                        .as_ref()
                        .and_then(|u| u.prompt_tokens)
                        .unwrap_or(0);
                    let compl = completion
                        .token_usage
                        .as_ref()
                        .and_then(|u| u.completion_tokens)
                        .unwrap_or(0);
                    let parsed: Result<EvaluatorStructuredOutputV1, _> =
                        serde_json::from_str(&completion.raw_text);
                    let (ops, verdict) = match parsed {
                        Ok(output) => {
                            let n = output.ops.len();
                            let verdict = if n == 0 {
                                format!(
                                    "no ops ({})",
                                    output.no_op_reason.as_deref().unwrap_or("no reason")
                                )
                            } else {
                                // What was extracted matters more than how much:
                                // a cheap model that only ever emits one kind of
                                // op is not cheaper, it is blinder.
                                let mut labels = output
                                    .ops
                                    .iter()
                                    .filter_map(|op| {
                                        serde_json::to_value(op).ok().and_then(|v| {
                                            v.get("op").and_then(|o| o.as_str()).map(str::to_string)
                                        })
                                    })
                                    .collect::<Vec<_>>();
                                labels.sort();
                                labels.dedup_by(|a, b| a == b);
                                labels.join(",")
                            };
                            (n, verdict)
                        }
                        Err(error) => (0, format!("PARSE FAIL: {error}")),
                    };
                    println!(
                        "{model:44} {secs:>6.1} {prompt:>7} {compl:>7} {ops:>5} {:>6}  {verdict}",
                        completion.trace.tool_call_count
                    );
                }
                Err(error) => {
                    let short = error.chars().take(70).collect::<String>();
                    println!(
                        "{model:44} {secs:>6.1} {:>7} {:>7} {:>5} {:>6}  FAIL: {short}",
                        "-", "-", "-", "-"
                    );
                }
            }
        }
    }
}
