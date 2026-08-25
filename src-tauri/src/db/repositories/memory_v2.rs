use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension};
use state_engine::{
    compiler::MEMORY_COMPILER_CONTRACT_VERSION,
    memory_consolidation::propose_derived_memories,
    memory_v2::{
        project_legacy_memory, DerivedMemoryKind, EpisodicMemoryKind, MemoryLayerV2, MemoryV2Entry,
        MemoryValidity,
    },
    patch::{EnginePatch, MemoryPatch},
    soul::{Soul, TruthStatus},
};

use super::super::{
    now_ts, MemoryV2ConsolidationRun, MemoryV2ProjectionGeneration, MemoryV2ProjectionRecord,
    MemoryV2RecallFilter, MemoryV2RecallHit,
};

#[derive(Debug, Clone)]
struct MemoryPatchSource {
    patch_id: String,
    turn_id: String,
}

pub fn rebuild_memory_v2_projection(
    conn: &Connection,
    conversation_id: &str,
    branch_id: &str,
    soul: &Soul,
) -> rusqlite::Result<MemoryV2ProjectionGeneration> {
    let sources = memory_patch_sources(conn, branch_id)?;
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    for memory in &soul.memory.recent {
        if !seen.insert(memory.id.clone()) {
            continue;
        }
        let source = sources.get(&memory.id);
        let entry = project_legacy_memory(
            memory,
            conversation_id,
            branch_id,
            source.map(|value| value.patch_id.clone()),
            source.map(|value| value.turn_id.clone()),
            MEMORY_COMPILER_CONTRACT_VERSION,
        );
        entry.validate().map_err(|error| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                error,
            )))
        })?;
        entries.push(entry);
    }
    entries.sort_by(|left, right| left.memory_id.cmp(&right.memory_id));

    let previous_generation = conn
        .query_row(
            "SELECT generation FROM memory_v2_projection_generations
             WHERE conversation_id = ?1 AND branch_id = ?2",
            params![conversation_id, branch_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0);
    let generation = previous_generation.saturating_add(1);
    let rebuilt_at = now_ts();
    let transaction = conn.unchecked_transaction()?;
    transaction.execute(
        "UPDATE memory_v2_entries
         SET validity = 'stale'
         WHERE conversation_id = ?1 AND branch_id = ?2 AND layer = 'derived'",
        params![conversation_id, branch_id],
    )?;
    transaction.execute(
        "DELETE FROM memory_v2_entries
         WHERE conversation_id = ?1 AND branch_id = ?2 AND layer = 'raw'",
        params![conversation_id, branch_id],
    )?;
    for entry in &entries {
        insert_entry(&transaction, entry)?;
    }
    transaction.execute(
        "INSERT INTO memory_v2_projection_generations
            (conversation_id, branch_id, generation, entry_count, rebuilt_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(conversation_id, branch_id) DO UPDATE SET
            generation = excluded.generation,
            entry_count = excluded.entry_count,
            rebuilt_at = excluded.rebuilt_at",
        params![
            conversation_id,
            branch_id,
            generation,
            entries.len() as i64,
            rebuilt_at
        ],
    )?;
    transaction.commit()?;
    let _ = consolidate_memory_v2_projection(conn, conversation_id, branch_id)?;
    refresh_memory_v2_search_projection(conn, conversation_id, branch_id)?;
    Ok(MemoryV2ProjectionGeneration {
        conversation_id: conversation_id.to_string(),
        branch_id: branch_id.to_string(),
        generation,
        entry_count: entries.len(),
        rebuilt_at,
    })
}

pub fn recall_memory_v2(
    conn: &Connection,
    conversation_id: &str,
    branch_id: &str,
    query: &str,
    limit: usize,
) -> rusqlite::Result<Vec<MemoryV2RecallHit>> {
    recall_memory_v2_filtered(
        conn,
        conversation_id,
        branch_id,
        query,
        &MemoryV2RecallFilter::default(),
        limit,
    )
}

pub trait MemoryV2SemanticAdapter {
    fn score(&self, query: &str, memory: &MemoryV2ProjectionRecord) -> Option<f32>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DisabledMemoryV2SemanticAdapter;

impl MemoryV2SemanticAdapter for DisabledMemoryV2SemanticAdapter {
    fn score(&self, _query: &str, _memory: &MemoryV2ProjectionRecord) -> Option<f32> {
        None
    }
}

pub fn recall_memory_v2_filtered(
    conn: &Connection,
    conversation_id: &str,
    branch_id: &str,
    query: &str,
    filter: &MemoryV2RecallFilter,
    limit: usize,
) -> rusqlite::Result<Vec<MemoryV2RecallHit>> {
    recall_memory_v2_filtered_with_semantic(
        conn,
        conversation_id,
        branch_id,
        query,
        filter,
        &DisabledMemoryV2SemanticAdapter,
        limit,
    )
}

pub fn recall_memory_v2_filtered_with_semantic(
    conn: &Connection,
    conversation_id: &str,
    branch_id: &str,
    query: &str,
    filter: &MemoryV2RecallFilter,
    semantic: &dyn MemoryV2SemanticAdapter,
    limit: usize,
) -> rusqlite::Result<Vec<MemoryV2RecallHit>> {
    let limit = limit.clamp(1, 50);
    let match_query = fts_match_query(query);
    if match_query.is_empty() {
        return Ok(Vec::new());
    }
    let candidate_limit = (limit.saturating_mul(6)).clamp(limit, 300);
    let mut statement = conn.prepare(
        "SELECT e.conversation_id, e.branch_id, e.memory_id, e.layer, e.memory_kind,
                e.owner_entity_id, e.content, e.source_patch_id, e.source_turn_id,
                e.source_message_id, e.source_entity_id, e.source_quote,
                e.source_memory_ids_json, e.supporting_evidence_json,
                e.contradicting_evidence_json, e.confidence, e.truth_status, e.validity,
                e.schema_version, e.compiler_version, e.created_at_ms,
                bm25(memory_v2_fts, 0.0, 0.0, 0.0, 1.0, 0.6) AS rank
         FROM memory_v2_fts
         JOIN memory_v2_entries e
           ON e.conversation_id = memory_v2_fts.conversation_id
          AND e.branch_id = memory_v2_fts.branch_id
          AND e.memory_id = memory_v2_fts.memory_id
         WHERE memory_v2_fts MATCH ?1
           AND e.conversation_id = ?2
           AND e.branch_id = ?3
           AND e.validity = 'valid'
         ORDER BY rank, e.confidence DESC, e.memory_id
         LIMIT ?4",
    )?;
    let rows = statement.query_map(
        params![
            match_query,
            conversation_id,
            branch_id,
            candidate_limit as i64
        ],
        |row| {
            let record = record_from_row(row)?;
            let rank: f64 = row.get(21)?;
            Ok((record, rank))
        },
    )?;
    let mut hits = rows
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .filter(|(memory, _)| memory_matches_recall_filter(memory, filter))
        .map(|(memory, rank)| {
            let lexical_score = (1.0 / (1.0 + rank.abs() as f32)).clamp(0.0, 1.0);
            let semantic_score = semantic
                .score(query, &memory)
                .unwrap_or(0.0)
                .clamp(0.0, 1.0);
            let mut selection_reasons = vec!["fts5_bm25".into()];
            if semantic_score > 0.0 {
                selection_reasons.push("local_embedding".into());
            }
            append_filter_reasons(&mut selection_reasons, filter);
            MemoryV2RecallHit {
                memory,
                lexical_score,
                semantic_score,
                temporal_score: 0.0,
                graph_score: 0.0,
                final_score: (lexical_score * 0.8 + semantic_score * 0.2).min(1.0),
                selection_reasons,
            }
        })
        .collect::<Vec<_>>();
    expand_recall_neighbors(
        conn,
        conversation_id,
        branch_id,
        &mut hits,
        filter,
        semantic,
        query,
        limit,
    )?;
    collapse_redundant_derived_hits(&mut hits);
    apply_temporal_scores(&mut hits);
    hits.sort_by(|left, right| {
        right
            .final_score
            .total_cmp(&left.final_score)
            .then_with(|| left.memory.memory_id.cmp(&right.memory.memory_id))
    });
    hits.truncate(limit);
    Ok(hits)
}

fn collapse_redundant_derived_hits(hits: &mut Vec<MemoryV2RecallHit>) {
    let present_ids = hits
        .iter()
        .map(|hit| hit.memory.memory_id.clone())
        .collect::<HashSet<_>>();
    let redundant = hits
        .iter()
        .filter(|hit| hit.memory.layer == "derived")
        .filter_map(|hit| {
            let sources =
                serde_json::from_str::<Vec<String>>(&hit.memory.source_memory_ids_json).ok()?;
            (!sources.is_empty() && sources.iter().all(|source| present_ids.contains(source)))
                .then(|| (hit.memory.memory_id.clone(), sources, hit.graph_score))
        })
        .collect::<Vec<_>>();
    if redundant.is_empty() {
        return;
    }
    let redundant_ids = redundant
        .iter()
        .map(|(memory_id, _, _)| memory_id.as_str())
        .collect::<HashSet<_>>();
    hits.retain(|hit| !redundant_ids.contains(hit.memory.memory_id.as_str()));
    for (derived_id, sources, graph_score) in redundant {
        for hit in hits
            .iter_mut()
            .filter(|hit| sources.iter().any(|source| source == &hit.memory.memory_id))
        {
            hit.graph_score = hit.graph_score.max(graph_score * 0.5);
            hit.final_score = (hit.final_score + hit.graph_score).min(1.0);
            hit.selection_reasons
                .push(format!("graph_summary:{derived_id}"));
        }
    }
}

fn memory_matches_recall_filter(
    memory: &MemoryV2ProjectionRecord,
    filter: &MemoryV2RecallFilter,
) -> bool {
    (filter.truth_statuses.is_empty()
        || filter
            .truth_statuses
            .iter()
            .any(|value| value == &memory.truth_status))
        && (filter.memory_kinds.is_empty()
            || filter
                .memory_kinds
                .iter()
                .any(|value| value == &memory.memory_kind))
        && filter
            .owner_entity_id
            .as_ref()
            .is_none_or(|owner| memory.owner_entity_id.as_ref() == Some(owner))
        && filter
            .created_after_ms
            .is_none_or(|minimum| memory.created_at_ms >= minimum)
        && filter
            .created_before_ms
            .is_none_or(|maximum| memory.created_at_ms <= maximum)
}

fn append_filter_reasons(reasons: &mut Vec<String>, filter: &MemoryV2RecallFilter) {
    if !filter.truth_statuses.is_empty() {
        reasons.push("filter:truth_status".into());
    }
    if !filter.memory_kinds.is_empty() {
        reasons.push("filter:memory_kind".into());
    }
    if filter.owner_entity_id.is_some() {
        reasons.push("filter:character".into());
    }
    if filter.created_after_ms.is_some() || filter.created_before_ms.is_some() {
        reasons.push("filter:temporal_scope".into());
    }
}

fn apply_temporal_scores(hits: &mut [MemoryV2RecallHit]) {
    let Some(newest) = hits.iter().map(|hit| hit.memory.created_at_ms).max() else {
        return;
    };
    const THIRTY_DAYS_MS: f32 = 30.0 * 24.0 * 60.0 * 60.0 * 1000.0;
    for hit in hits {
        let age = newest.saturating_sub(hit.memory.created_at_ms) as f32;
        hit.temporal_score = (1.0 / (1.0 + age / THIRTY_DAYS_MS)).clamp(0.0, 1.0);
        hit.final_score = (hit.final_score * 0.9 + hit.temporal_score * 0.1).min(1.0);
        hit.selection_reasons.push("temporal_recency".into());
    }
}

pub fn consolidate_memory_v2_projection(
    conn: &Connection,
    conversation_id: &str,
    branch_id: &str,
) -> rusqlite::Result<MemoryV2ConsolidationRun> {
    let records = list_memory_v2_projection(conn, conversation_id, branch_id, false)?;
    let raw = records
        .iter()
        .filter(|record| record.layer == "raw")
        .map(raw_entry_from_record)
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let derived_at_ms = raw
        .iter()
        .map(|memory| memory.created_at_ms)
        .max()
        .unwrap_or_else(|| now_ts().saturating_mul(1000));
    let report = propose_derived_memories(&raw, derived_at_ms);
    let proposed = report.proposals.len();
    for proposal in &report.proposals {
        store_derived_memory_v2(conn, proposal)?;
    }
    let generation = conn.query_row(
        "SELECT generation FROM memory_v2_projection_generations
         WHERE conversation_id = ?1 AND branch_id = ?2",
        params![conversation_id, branch_id],
        |row| row.get::<_, i64>(0),
    )?;
    let has_contradiction = report
        .proposals
        .iter()
        .any(|proposal| !proposal.contradicting_evidence.is_empty());
    let distinct_turns = raw
        .iter()
        .filter_map(|memory| memory.source_turn_id.as_deref())
        .collect::<HashSet<_>>()
        .len();
    let accumulated_importance = raw.iter().map(|memory| memory.confidence).sum::<f32>();
    let trigger_reason = if has_contradiction {
        "contradiction_detected"
    } else if distinct_turns >= 4 {
        "turns_since_consolidation"
    } else if accumulated_importance >= 1.5 {
        "importance_accumulation"
    } else if raw.len() >= 2 {
        "raw_memory_threshold"
    } else {
        "below_threshold"
    };
    let status = if raw.len() < 2 || (report.proposals.is_empty() && report.rejected.is_empty()) {
        "skipped"
    } else if report.rejected.is_empty() {
        "completed"
    } else if report.proposals.is_empty() {
        "rejected"
    } else {
        "partial"
    };
    let artifact_json = serde_json::to_string(&report)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    conn.execute(
        "INSERT INTO memory_v2_consolidation_runs (
            run_id, conversation_id, branch_id, projection_generation,
            trigger_reason, raw_memory_count, proposed_count, stored_count,
            rejected_count, status, artifact_json, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(run_id) DO UPDATE SET
            trigger_reason = excluded.trigger_reason,
            raw_memory_count = excluded.raw_memory_count,
            proposed_count = excluded.proposed_count,
            stored_count = excluded.stored_count,
            rejected_count = excluded.rejected_count,
            status = excluded.status,
            artifact_json = excluded.artifact_json,
            created_at = excluded.created_at",
        params![
            format!("consolidation:{branch_id}:{generation}"),
            conversation_id,
            branch_id,
            generation,
            trigger_reason,
            raw.len() as i64,
            proposed as i64,
            report.proposals.len() as i64,
            report.rejected.len() as i64,
            status,
            artifact_json,
            now_ts(),
        ],
    )?;
    Ok(MemoryV2ConsolidationRun {
        proposed,
        stored: report.proposals.len(),
        rejected: report.rejected.len(),
    })
}

pub fn store_derived_memory_v2(conn: &Connection, entry: &MemoryV2Entry) -> rusqlite::Result<()> {
    if entry.layer != MemoryLayerV2::Derived {
        return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "store_derived_memory_v2 accepts only derived memories",
            ),
        )));
    }
    entry.validate().map_err(|error| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            error,
        )))
    })?;
    let transaction = conn.unchecked_transaction()?;
    transaction.execute(
        "DELETE FROM memory_v2_entries
         WHERE conversation_id = ?1 AND branch_id = ?2 AND memory_id = ?3",
        params![entry.conversation_id, entry.branch_id, entry.memory_id],
    )?;
    insert_entry(&transaction, entry)?;
    transaction.commit()
}

pub fn append_memory_correction_event(
    conn: &Connection,
    conversation_id: &str,
    branch_id: &str,
    turn_id: &str,
    target_assistant_message_id: Option<i64>,
    instruction: &str,
) -> rusqlite::Result<bool> {
    let instruction = instruction.trim();
    if instruction.is_empty() {
        return Ok(false);
    }
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO memory_correction_events (
            event_id, conversation_id, branch_id, turn_id,
            target_assistant_message_id, instruction, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            format!("correction:{branch_id}:{turn_id}"),
            conversation_id,
            branch_id,
            turn_id,
            target_assistant_message_id,
            instruction,
            now_ts(),
        ],
    )?;
    Ok(inserted > 0)
}

pub fn list_memory_v2_projection(
    conn: &Connection,
    conversation_id: &str,
    branch_id: &str,
    include_stale: bool,
) -> rusqlite::Result<Vec<MemoryV2ProjectionRecord>> {
    let sql = if include_stale {
        "SELECT conversation_id, branch_id, memory_id, layer, memory_kind,
                owner_entity_id, content, source_patch_id, source_turn_id,
                source_message_id, source_entity_id, source_quote,
                source_memory_ids_json, supporting_evidence_json,
                contradicting_evidence_json, confidence, truth_status, validity,
                schema_version, compiler_version, created_at_ms
         FROM memory_v2_entries
         WHERE conversation_id = ?1 AND branch_id = ?2
         ORDER BY layer, memory_kind, memory_id"
    } else {
        "SELECT conversation_id, branch_id, memory_id, layer, memory_kind,
                owner_entity_id, content, source_patch_id, source_turn_id,
                source_message_id, source_entity_id, source_quote,
                source_memory_ids_json, supporting_evidence_json,
                contradicting_evidence_json, confidence, truth_status, validity,
                schema_version, compiler_version, created_at_ms
         FROM memory_v2_entries
         WHERE conversation_id = ?1 AND branch_id = ?2 AND validity = 'valid'
         ORDER BY layer, memory_kind, memory_id"
    };
    let mut statement = conn.prepare(sql)?;
    let records = statement
        .query_map(params![conversation_id, branch_id], record_from_row)?
        .collect();
    records
}

fn memory_patch_sources(
    conn: &Connection,
    branch_id: &str,
) -> rusqlite::Result<HashMap<String, MemoryPatchSource>> {
    let mut statement = conn.prepare(
        "SELECT p.patch_id, p.turn_id, p.patch_json
         FROM state_patches p
         JOIN turn_commits t ON t.turn_id = p.turn_id
         WHERE t.branch_id = ?1
           AND t.is_active = 1
           AND t.is_discarded = 0
           AND p.is_active = 1
         ORDER BY t.created_at, p.applied_at, p.patch_id",
    )?;
    let rows = statement.query_map([branch_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut sources = HashMap::new();
    for row in rows {
        let (patch_id, turn_id, patch_json) = row?;
        let patch = serde_json::from_str::<EnginePatch>(&patch_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                2,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
        if let Some(soul_patch) = patch.soul_patch {
            for memory in soul_patch
                .new_memories
                .iter()
                .chain(soul_patch.memory_operations.iter())
            {
                if let Some(memory_id) = projected_memory_id(memory) {
                    sources.insert(
                        memory_id,
                        MemoryPatchSource {
                            patch_id: patch_id.clone(),
                            turn_id: turn_id.clone(),
                        },
                    );
                }
            }
        }
    }
    Ok(sources)
}

fn projected_memory_id(memory: &MemoryPatch) -> Option<String> {
    memory
        .memory_id
        .as_ref()
        .or(memory.target_memory_id.as_ref())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn insert_entry(conn: &Connection, entry: &MemoryV2Entry) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO memory_v2_entries (
            conversation_id, branch_id, memory_id, layer, memory_kind,
            owner_entity_id, content, source_patch_id, source_turn_id,
            source_message_id, source_entity_id, source_quote,
            source_memory_ids_json, supporting_evidence_json,
            contradicting_evidence_json, confidence, truth_status, validity,
            schema_version, compiler_version, created_at_ms
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
            ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21
         )",
        params![
            entry.conversation_id,
            entry.branch_id,
            entry.memory_id,
            layer_label(entry.layer),
            memory_kind_label(entry),
            entry.owner_entity_id,
            entry.content,
            entry.source_patch_id,
            entry.source_turn_id,
            entry.source_message_id,
            entry.source_entity_id,
            entry.source_quote,
            serde_json::to_string(&entry.source_memory_ids).unwrap_or_else(|_| "[]".into()),
            serde_json::to_string(&entry.supporting_evidence).unwrap_or_else(|_| "[]".into()),
            serde_json::to_string(&entry.contradicting_evidence).unwrap_or_else(|_| "[]".into()),
            entry.confidence,
            entry.truth_status.as_label(),
            validity_label(entry.validity),
            entry.schema_version,
            entry.compiler_version,
            entry.created_at_ms,
        ],
    )?;
    Ok(())
}

fn record_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryV2ProjectionRecord> {
    Ok(MemoryV2ProjectionRecord {
        conversation_id: row.get(0)?,
        branch_id: row.get(1)?,
        memory_id: row.get(2)?,
        layer: row.get(3)?,
        memory_kind: row.get(4)?,
        owner_entity_id: row.get(5)?,
        content: row.get(6)?,
        source_patch_id: row.get(7)?,
        source_turn_id: row.get(8)?,
        source_message_id: row.get(9)?,
        source_entity_id: row.get(10)?,
        source_quote: row.get(11)?,
        source_memory_ids_json: row.get(12)?,
        supporting_evidence_json: row.get(13)?,
        contradicting_evidence_json: row.get(14)?,
        confidence: row.get(15)?,
        truth_status: row.get(16)?,
        validity: row.get(17)?,
        schema_version: row.get::<_, i64>(18)?.max(0) as u32,
        compiler_version: row.get::<_, i64>(19)?.max(0) as u32,
        created_at_ms: row.get(20)?,
    })
}

fn layer_label(layer: MemoryLayerV2) -> &'static str {
    match layer {
        MemoryLayerV2::Raw => "raw",
        MemoryLayerV2::Derived => "derived",
    }
}

fn episodic_kind_label(kind: EpisodicMemoryKind) -> &'static str {
    match kind {
        EpisodicMemoryKind::Episode => "episode",
        EpisodicMemoryKind::Testimony => "testimony",
        EpisodicMemoryKind::Perception => "perception",
        EpisodicMemoryKind::Affect => "affect",
        EpisodicMemoryKind::Intention => "intention",
    }
}

fn derived_kind_label(kind: DerivedMemoryKind) -> &'static str {
    match kind {
        DerivedMemoryKind::Belief => "belief",
        DerivedMemoryKind::Schema => "schema",
        DerivedMemoryKind::RelationshipModel => "relationship_model",
        DerivedMemoryKind::SelfModel => "self_model",
        DerivedMemoryKind::Reflection => "reflection",
    }
}

fn memory_kind_label(entry: &MemoryV2Entry) -> &'static str {
    match entry.layer {
        MemoryLayerV2::Raw => {
            episodic_kind_label(entry.episodic_kind.expect("validated raw memory"))
        }
        MemoryLayerV2::Derived => {
            derived_kind_label(entry.derived_kind.expect("validated derived memory"))
        }
    }
}

fn validity_label(validity: MemoryValidity) -> &'static str {
    match validity {
        MemoryValidity::Valid => "valid",
        MemoryValidity::Stale => "stale",
        MemoryValidity::Superseded => "superseded",
        MemoryValidity::Invalidated => "invalidated",
    }
}

fn raw_entry_from_record(record: &MemoryV2ProjectionRecord) -> rusqlite::Result<MemoryV2Entry> {
    let episodic_kind = match record.memory_kind.as_str() {
        "episode" => EpisodicMemoryKind::Episode,
        "testimony" => EpisodicMemoryKind::Testimony,
        "perception" => EpisodicMemoryKind::Perception,
        "affect" => EpisodicMemoryKind::Affect,
        "intention" => EpisodicMemoryKind::Intention,
        other => return Err(invalid_projection(format!("unknown episodic kind {other}"))),
    };
    let truth_status = match record.truth_status.as_str() {
        "fiction" => TruthStatus::Fiction,
        "scene_event" => TruthStatus::SceneEvent,
        "character_belief" => TruthStatus::CharacterBelief,
        "narrator_claim" => TruthStatus::NarratorClaim,
        "user_claimed" => TruthStatus::UserClaimed,
        "verified_engine" => TruthStatus::VerifiedEngine,
        "actual_system_event" => TruthStatus::ActualSystemEvent,
        "unknown" => TruthStatus::Unknown,
        other => return Err(invalid_projection(format!("unknown truth status {other}"))),
    };
    Ok(MemoryV2Entry {
        schema_version: record.schema_version,
        memory_id: record.memory_id.clone(),
        conversation_id: record.conversation_id.clone(),
        branch_id: record.branch_id.clone(),
        owner_entity_id: record.owner_entity_id.clone(),
        layer: MemoryLayerV2::Raw,
        episodic_kind: Some(episodic_kind),
        derived_kind: None,
        content: record.content.clone(),
        source_patch_id: record.source_patch_id.clone(),
        source_turn_id: record.source_turn_id.clone(),
        source_message_id: record.source_message_id,
        source_entity_id: record.source_entity_id.clone(),
        source_quote: record.source_quote.clone(),
        source_memory_ids: Vec::new(),
        supporting_evidence: Vec::new(),
        contradicting_evidence: Vec::new(),
        confidence: record.confidence,
        truth_status,
        validity: MemoryValidity::Valid,
        compiler_version: record.compiler_version,
        created_at_ms: record.created_at_ms,
    })
}

fn invalid_projection(message: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message,
        )),
    )
}

fn refresh_memory_v2_search_projection(
    conn: &Connection,
    conversation_id: &str,
    branch_id: &str,
) -> rusqlite::Result<()> {
    let transaction = conn.unchecked_transaction()?;
    transaction.execute(
        "DELETE FROM memory_v2_fts WHERE conversation_id = ?1 AND branch_id = ?2",
        params![conversation_id, branch_id],
    )?;
    transaction.execute(
        "INSERT INTO memory_v2_fts
            (memory_id, conversation_id, branch_id, content, source_quote)
         SELECT memory_id, conversation_id, branch_id, content, COALESCE(source_quote, '')
         FROM memory_v2_entries
         WHERE conversation_id = ?1 AND branch_id = ?2 AND validity = 'valid'",
        params![conversation_id, branch_id],
    )?;
    transaction.execute(
        "DELETE FROM memory_v2_edges WHERE conversation_id = ?1 AND branch_id = ?2",
        params![conversation_id, branch_id],
    )?;
    let derived = {
        let mut statement = transaction.prepare(
            "SELECT memory_id, source_memory_ids_json
             FROM memory_v2_entries
             WHERE conversation_id = ?1 AND branch_id = ?2
               AND layer = 'derived' AND validity = 'valid'",
        )?;
        let rows = statement.query_map(params![conversation_id, branch_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (derived_id, sources_json) in derived {
        let sources = serde_json::from_str::<Vec<String>>(&sources_json).unwrap_or_default();
        for source_id in sources {
            transaction.execute(
                "INSERT OR REPLACE INTO memory_v2_edges
                    (conversation_id, branch_id, from_memory_id, to_memory_id, edge_kind, weight)
                 VALUES (?1, ?2, ?3, ?4, 'derived_from', 1.0)",
                params![conversation_id, branch_id, derived_id, source_id],
            )?;
            transaction.execute(
                "INSERT OR REPLACE INTO memory_v2_edges
                    (conversation_id, branch_id, from_memory_id, to_memory_id, edge_kind, weight)
                 VALUES (?1, ?2, ?3, ?4, 'supports_derived', 0.8)",
                params![conversation_id, branch_id, source_id, derived_id],
            )?;
        }
    }
    transaction.commit()
}

fn fts_match_query(query: &str) -> String {
    query
        .split(|character: char| !character.is_alphanumeric())
        .map(str::trim)
        .filter(|token| token.chars().count() >= 2)
        .take(12)
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn expand_recall_neighbors(
    conn: &Connection,
    conversation_id: &str,
    branch_id: &str,
    hits: &mut Vec<MemoryV2RecallHit>,
    filter: &MemoryV2RecallFilter,
    semantic: &dyn MemoryV2SemanticAdapter,
    query: &str,
    limit: usize,
) -> rusqlite::Result<()> {
    let seeds = hits
        .iter()
        .take(limit.min(8))
        .map(|hit| (hit.memory.memory_id.clone(), hit.lexical_score))
        .collect::<Vec<_>>();
    let mut by_id = hits
        .iter()
        .enumerate()
        .map(|(index, hit)| (hit.memory.memory_id.clone(), index))
        .collect::<HashMap<_, _>>();
    for (seed_id, seed_score) in seeds {
        let mut statement = conn.prepare(
            "SELECT to_memory_id, weight
             FROM memory_v2_edges
             WHERE conversation_id = ?1 AND branch_id = ?2 AND from_memory_id = ?3
             ORDER BY weight DESC, to_memory_id
             LIMIT 8",
        )?;
        let neighbors = statement
            .query_map(params![conversation_id, branch_id, seed_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f32>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (neighbor_id, weight) in neighbors {
            let graph_score = (seed_score * weight * 0.35).clamp(0.0, 0.35);
            if let Some(index) = by_id.get(&neighbor_id).copied() {
                let hit = &mut hits[index];
                if graph_score > hit.graph_score {
                    hit.graph_score = graph_score;
                    hit.final_score =
                        (hit.lexical_score * 0.8 + hit.semantic_score * 0.2 + graph_score).min(1.0);
                    hit.selection_reasons
                        .push(format!("graph_neighbor:{seed_id}"));
                }
                continue;
            }
            if let Some(memory) =
                get_memory_v2_record(conn, conversation_id, branch_id, &neighbor_id)?
            {
                if !memory_matches_recall_filter(&memory, filter) {
                    continue;
                }
                let semantic_score = semantic
                    .score(query, &memory)
                    .unwrap_or(0.0)
                    .clamp(0.0, 1.0);
                let mut selection_reasons = vec![format!("graph_neighbor:{seed_id}")];
                if semantic_score > 0.0 {
                    selection_reasons.push("local_embedding".into());
                }
                append_filter_reasons(&mut selection_reasons, filter);
                let index = hits.len();
                by_id.insert(neighbor_id, index);
                hits.push(MemoryV2RecallHit {
                    memory,
                    lexical_score: 0.0,
                    semantic_score,
                    temporal_score: 0.0,
                    graph_score,
                    final_score: (graph_score + semantic_score * 0.2).min(1.0),
                    selection_reasons,
                });
            }
        }
    }
    Ok(())
}

fn get_memory_v2_record(
    conn: &Connection,
    conversation_id: &str,
    branch_id: &str,
    memory_id: &str,
) -> rusqlite::Result<Option<MemoryV2ProjectionRecord>> {
    conn.query_row(
        "SELECT conversation_id, branch_id, memory_id, layer, memory_kind,
                owner_entity_id, content, source_patch_id, source_turn_id,
                source_message_id, source_entity_id, source_quote,
                source_memory_ids_json, supporting_evidence_json,
                contradicting_evidence_json, confidence, truth_status, validity,
                schema_version, compiler_version, created_at_ms
         FROM memory_v2_entries
         WHERE conversation_id = ?1 AND branch_id = ?2
           AND memory_id = ?3 AND validity = 'valid'",
        params![conversation_id, branch_id, memory_id],
        record_from_row,
    )
    .optional()
}
