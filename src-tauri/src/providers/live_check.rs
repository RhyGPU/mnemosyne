//! Live credential check: send one trivial completion through the app's own
//! provider code using a stored profile.
//!
//! This exists because a bad key surfaces as a misleading provider message —
//! OpenRouter answers a malformed bearer token with "Missing Authentication
//! header", which reads like the header was never sent. One real round trip
//! settles it in seconds instead of another failed benchmark run.
//!
//! It never prints the key.
//!
//! ```text
//! MNE_LIVE_DB=~/.local/share/com.mnemosyne.app/mnemosyne.sqlite3 \
//! cargo test --manifest-path src-tauri/Cargo.toml live_profile_check -- --ignored --nocapture
//! ```

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use crate::providers::api::{ApiProvider, ApiProviderSettings};

    #[tokio::test]
    #[ignore = "makes a real provider request; needs MNE_LIVE_DB"]
    async fn live_profile_check() {
        let db_path = std::env::var("MNE_LIVE_DB").expect("set MNE_LIVE_DB");
        let conn = Connection::open(&db_path).expect("open database");

        let profiles: Vec<(String, String, String, String)> = {
            let mut stmt = conn
                .prepare(
                    "SELECT name, api_key, model, base_url FROM provider_profiles
                     WHERE archived_at IS NULL ORDER BY name",
                )
                .expect("prepare");
            stmt.query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .expect("query")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect")
        };

        let provider = ApiProvider::default();
        let mut failures = 0usize;

        for (name, api_key, model, base_url) in profiles {
            let settings = ApiProviderSettings {
                api_key,
                model: model.clone(),
                base_url: base_url.clone(),
                narrator_timeout_ms: Some(30_000),
                // Reasoning-style models spend their budget before emitting any
                // visible content, so a tiny cap looks like an empty stream.
                narrator_max_tokens: Some(256),
                ..ApiProviderSettings::default()
            };
            let result = provider
                .complete_streaming(
                    &settings,
                    "You are a test endpoint. Answer in one short sentence.",
                    "Say hello.",
                    |_chunk: &str| Ok(()),
                )
                .await;
            match result {
                Ok(completion) => {
                    let reply = completion.raw_text.trim().replace('\n', " ");
                    let shown = reply.chars().take(40).collect::<String>();
                    println!("  OK    {name:20} {model}  -> \"{shown}\"");
                }
                Err(error) => {
                    failures += 1;
                    println!("  FAIL  {name:20} {model}  -> {error}");
                }
            }
        }

        println!("\nfailures: {failures}");
    }
}
