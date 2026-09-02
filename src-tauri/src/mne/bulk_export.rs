//! One-off maintenance exporter: write a `.mne` checkpoint for every
//! conversation in a database file.
//!
//! This mirrors `export_current_session_checkpoint_mne_inner` but takes a plain
//! SQLite path instead of a Tauri `AppHandle`/`Window`, so a full archive can be
//! produced without driving the desktop UI 95 times. It deliberately reuses the
//! shipped manifest/bundle helpers rather than re-implementing the format, so an
//! archive written here imports through the normal path.
//!
//! Run it against a *copy* of the database — rebuilding session state writes
//! back the rebuilt soul/world and bumps `rebuild_generation`.
//!
//! ```text
//! MNE_BULK_DB=/path/to/mnemosyne.sqlite3 \
//! MNE_BULK_OUT=/path/to/out \
//! cargo test --manifest-path src-tauri/Cargo.toml bulk_export -- --ignored --nocapture
//! ```

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use std::path::PathBuf;

    use crate::db;
    use crate::mne::archive::write_stored_zip;
    use crate::mne::service::{
        collect_mne_session_ledger, json_bundle_file, mne_manifest, safe_bundle_name,
    };

    #[test]
    #[ignore = "maintenance utility; needs MNE_BULK_DB and MNE_BULK_OUT"]
    fn bulk_export_every_conversation() {
        let db_path = std::env::var("MNE_BULK_DB").expect("set MNE_BULK_DB");
        let out_dir = PathBuf::from(std::env::var("MNE_BULK_OUT").expect("set MNE_BULK_OUT"));
        std::fs::create_dir_all(&out_dir).expect("create output directory");

        let conn = Connection::open(&db_path).expect("open database");
        // Archived conversations are part of the archive too, so this reads the
        // table directly instead of the UI listing, which hides them.
        let conversations: Vec<(String, String)> = {
            let mut stmt = conn
                .prepare("SELECT id, COALESCE(title, '') FROM conversations ORDER BY created_at")
                .expect("prepare conversation listing");
            let rows = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .expect("query conversations");
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .expect("collect conversations")
        };
        println!("conversations: {}", conversations.len());

        let mut written = 0usize;
        let mut skipped: Vec<(String, String)> = Vec::new();

        for (conversation_id, title) in &conversations {
            match export_one(&conn, conversation_id, &out_dir) {
                Ok(path) => {
                    written += 1;
                    println!("  ok  {path}  <- {title}");
                }
                Err(reason) => {
                    skipped.push((conversation_id.to_string(), reason));
                }
            }
        }

        println!("\nwritten: {written}");
        println!("skipped: {}", skipped.len());
        for (id, reason) in &skipped {
            println!("  {id}: {reason}");
        }
    }

    /// Proof that the archive is usable: every bundle is run through the same
    /// validator the import path uses.
    #[test]
    #[ignore = "maintenance utility; needs MNE_BULK_OUT"]
    fn validate_every_exported_bundle() {
        let out_dir = PathBuf::from(std::env::var("MNE_BULK_OUT").expect("set MNE_BULK_OUT"));
        let mut checked = 0usize;
        let mut bad: Vec<String> = Vec::new();

        for entry in std::fs::read_dir(&out_dir).expect("read output directory") {
            let path = entry.expect("dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("mne") {
                continue;
            }
            let bytes = std::fs::read(&path).expect("read bundle");
            let report = crate::mne::service::validate_mne_bundle_bytes(&bytes);
            checked += 1;
            if !report.valid {
                bad.push(format!(
                    "{}: {:?}",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    report.errors
                ));
            }
        }

        println!("validated: {checked}");
        println!("invalid:   {}", bad.len());
        for line in bad.iter().take(10) {
            println!("  {line}");
        }
        assert!(bad.is_empty(), "{} bundles failed validation", bad.len());
    }

    fn export_one(
        conn: &Connection,
        conversation_id: &str,
        out_dir: &PathBuf,
    ) -> Result<String, String> {
        let conversation =
            db::get_conversation_summary(conn, conversation_id).map_err(|e| e.to_string())?;

        // Prefer the ledger-rebuilt state, exactly as the UI export does, so the
        // bundle carries the state the engine actually believes.
        let (soul, session_world) =
            if let Ok(branch) = db::get_active_session_branch(conn, conversation_id) {
                let rebuilt = db::rebuild_session_state(conn, conversation_id, &branch.branch_id)
                    .map_err(|e| e.to_string())?;
                (rebuilt.soul, rebuilt.session_world)
            } else {
                let soul = db::get_soul(conn, &conversation.soul_id).map_err(|e| e.to_string())?;
                let world = db::get_conversation_session_world(conn, conversation_id)
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| "no session world linked".to_string())?;
                (soul, world)
            };

        let messages =
            db::list_messages(conn, conversation_id, 10_000).map_err(|e| e.to_string())?;
        let ledger = collect_mne_session_ledger(conn, conversation_id, &messages)
            .map_err(|e| e.to_string())?;
        let payload_logs = db::list_llm_payload_logs(conn, conversation_id).unwrap_or_default();

        let soul_path = format!("souls/{}.json", safe_bundle_name(&soul.character_id));
        let world_path = format!("worlds/{}.json", safe_bundle_name(&session_world.world_id));
        let conversation_path = "conversation/conversation.json".to_string();

        let mut manifest = mne_manifest(
            "session_checkpoint",
            &conversation.title,
            "Mnemosyne session checkpoint bundle",
            vec![soul_path.clone()],
            vec![world_path.clone()],
            Some(conversation_path.clone()),
        );
        manifest.conversation_id = Some(conversation.conversation_id.clone());
        manifest.soul_id = Some(soul.character_id.clone());
        manifest.world_id = Some(session_world.world_id.clone());
        manifest.source_savepoint_id = soul.source_savepoint_id.clone();
        manifest.source_setting_id = session_world.source_setting_id.clone();

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
        if !ledger.branches.is_empty() {
            files.push(json_bundle_file(
                "conversation/branches.json",
                &ledger.branches,
            )?);
            files.push(json_bundle_file("conversation/turns.json", &ledger.turns)?);
            files.push(json_bundle_file(
                "conversation/patches.json",
                &ledger.patches,
            )?);
            if !ledger.variants.is_empty() {
                files.push(json_bundle_file(
                    "conversation/variants.json",
                    &ledger.variants,
                )?);
            }
        }

        // Conversation id in the name, so one file per session and no collisions
        // between sessions that share a title.
        let name = format!(
            "{}__{}.mne",
            safe_bundle_name(&conversation.title),
            safe_bundle_name(conversation_id)
        );
        let path = out_dir.join(&name);
        write_stored_zip(&path, &files)?;

        // A `.mne` has to be imported before anyone can read it. The point of a
        // full archive is that the record stays readable on its own, so each
        // session also gets a plain transcript beside its bundle.
        let transcript = format!(
            "<!-- conversation_id: {conversation_id} -->\n\
             <!-- messages: {} -->\n\n\
             # {}\n\n{}",
            messages.len(),
            conversation.title,
            crate::commands::render_visible_chat_log(&messages)
        );
        let md_path = path.with_extension("md");
        std::fs::write(&md_path, transcript).map_err(|e| e.to_string())?;

        Ok(path.to_string_lossy().to_string())
    }
}
