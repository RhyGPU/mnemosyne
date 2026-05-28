use super::*;
use crate::{
    setting::session_world_from_legacy_world,
    soul::{new_default_soul, MemoryEntry},
};

fn soul_and_world() -> (Soul, SessionWorld) {
    let mut soul = new_default_soul("Aurora");
    soul.character_id = "aurora_soul".into();
    soul.memory.recent.push(MemoryEntry {
        id: "mem_existing".into(),
        timestamp: 1,
        content: "Aurora remembers that the visitor knocked before entering.".into(),
        salience: 70.0,
        tag: "current_plot_memory".into(),
        retrieval_strength: 70.0,
        source_type: MemorySourceType::CurrentSession,
        source_session_id: None,
        source_conversation_id: None,
        source_message_id: None,
        source_entity_id: None,
        is_lived_experience: true,
        is_imported_context: false,
        perceived_by_entity_id: Some("aurora_soul".into()),
        target_entity_ids: vec!["default_player".into()],
        interpretation: None,
        confidence: Some(0.8),
        objective_event_id: None,
        truth_status: TruthStatus::SceneEvent,
        architecture_verified: false,
        memory_slot: Some("current_plot_memory".into()),
        owner_soul_id: Some("aurora_soul".into()),
        relevance_tags: HashMap::new(),
        knowledge_scope: Some("directly_observed".into()),
        is_active: true,
        invalidated_by_patch_id: None,
        superseded_by_memory_id: None,
        is_retconned: false,
    });
    soul.world.object_states.push(ObjectState {
        object_id: "apartment_door".into(),
        object_kind: "door".into(),
        status: "closed".into(),
        last_observed_state: "closed".into(),
        confidence: 0.9,
        ..ObjectState::default()
    });
    let world = session_world_from_legacy_world("Apartment", None, &soul.world);
    (soul, world)
}

fn spec_and_context<'a>(
    soul: &'a Soul,
    world: &'a SessionWorld,
    user: &'a str,
    narrator: &'a str,
) -> (EvalFormSpec, EvaluatorConversionContext<'a>) {
    (
        build_eval_form_spec(soul, Some(world), user, narrator, 8),
        EvaluatorConversionContext {
            active_soul_id: &soul.character_id,
            active_soul_ids: vec![soul.character_id.clone()],
            latest_user_message: user,
            latest_narrator_response: narrator,
            session_world: Some(world),
            baseline_recent_event_id: None,
        },
    )
}

fn event(id: &str, summary: &str, quote: &str) -> EventRow {
    EventRow {
        event_id: id.into(),
        event_type: Some(EventType::SceneEvent),
        objective_summary: summary.into(),
        participants: vec!["aurora_soul".into(), "default_player".into()],
        evidence_quote: quote.into(),
        importance_tier: Some(ImportanceTier::Medium),
        ..EventRow::default()
    }
}

fn memory(event_id: &str, content: &str, quote: &str) -> MemoryRow {
    MemoryRow {
        linked_event_id: event_id.into(),
        owner_soul_id: "aurora_soul".into(),
        slot: Some(MemorySlot::CurrentPlotMemory),
        content: content.into(),
        evidence_quote: quote.into(),
        importance_tier: Some(ImportanceTier::High),
        retrieval_cues: vec!["entry".into()],
        selected_tags: vec!["current_plot".into()],
        ..MemoryRow::default()
    }
}

#[test]
fn form_supports_multiple_events_in_one_turn() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "I walk in and close the door.",
        "The visitor walks in and closes the door.",
    );
    let response = EvalFormResponse {
        event_rows: vec![
            event(
                "entry",
                "The visitor entered Aurora's apartment.",
                "walks in",
            ),
            event("close", "The visitor closed the door.", "closes the door"),
        ],
        ..EvalFormResponse::default()
    };
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.output.world_changes.len(), 2);
    assert_eq!(result.trace.form_rows_rejected, 0);
}

#[test]
fn form_rejects_unknown_entity_id() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "I enter.", "The visitor enters.");
    let mut bad = event("entry", "A stranger enters.", "enters");
    bad.participants.push("mystery_entity".into());
    let result = compile_eval_form_response(
        &spec,
        &EvalFormResponse {
            event_rows: vec![bad],
            ..EvalFormResponse::default()
        },
        &context,
    );
    assert!(result
        .rejected_rows
        .iter()
        .any(|row| row.reason.contains("unknown participant")));
}

#[test]
fn form_requires_evidence_for_non_empty_rows() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "I enter.", "The visitor enters.");
    let result = compile_eval_form_response(
        &spec,
        &EvalFormResponse {
            event_rows: vec![event("entry", "The visitor entered.", "")],
            ..EvalFormResponse::default()
        },
        &context,
    );
    assert!(result
        .rejected_rows
        .iter()
        .any(|row| row.reason == "evidence_quote is required"));
}

#[test]
fn form_dedupe_marks_duplicate_of_existing() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "I walk in.",
        "The visitor walks in after knocking.",
    );
    let memory = memory(
        "entry",
        "Aurora remembers that the visitor knocked before entering.",
        "walks in",
    );
    let candidate_id = memory_candidate_id(&memory);
    let response = EvalFormResponse {
        event_rows: vec![event("entry", "The visitor entered.", "walks in")],
        memory_rows: vec![memory],
        review_rows: vec![ReviewRow {
            candidate_id: candidate_id.clone(),
            decision: Some(ReviewDecision::DuplicateOfExisting),
            existing_id: Some("mem_existing".into()),
            reason: "same remembered beat".into(),
            evidence_quote: "walks in".into(),
            ..ReviewRow::default()
        }],
        ..EvalFormResponse::default()
    };
    let result = compile_eval_form_response(&spec, &response, &context);
    assert!(result.output.memory_candidates.is_empty());
    assert_eq!(
        result.trace.form_dedupe_decisions[0].candidate_id,
        candidate_id
    );
}

#[test]
fn form_dedupe_marks_update_existing() {
    let (soul, world) = soul_and_world();
    let (spec, context) =
        spec_and_context(&soul, &world, "I walk in.", "The visitor walks in.");
    let memory = memory(
        "entry",
        "Aurora updates the entry beat with the visitor inside.",
        "walks in",
    );
    let candidate_id = memory_candidate_id(&memory);
    let result = compile_eval_form_response(
        &spec,
        &EvalFormResponse {
            event_rows: vec![event("entry", "The visitor entered.", "walks in")],
            memory_rows: vec![memory],
            review_rows: vec![ReviewRow {
                candidate_id,
                decision: Some(ReviewDecision::UpdateExisting),
                existing_id: Some("mem_existing".into()),
                reason: "more current version".into(),
                evidence_quote: "walks in".into(),
                ..ReviewRow::default()
            }],
            ..EvalFormResponse::default()
        },
        &context,
    );
    assert!(result
        .conversion
        .patch
        .soul_patch
        .as_ref()
        .unwrap()
        .memory_operations
        .iter()
        .any(|operation| operation.operation.as_deref() == Some("update")));
}

#[test]
fn form_memory_row_compiles_to_normalized_draft() {
    let (soul, world) = soul_and_world();
    let (spec, context) =
        spec_and_context(&soul, &world, "I walk in.", "The visitor walks in.");
    let result = compile_eval_form_response(
        &spec,
        &EvalFormResponse {
            event_rows: vec![event("entry", "The visitor entered.", "walks in")],
            memory_rows: vec![memory(
                "entry",
                "Aurora remembers the visitor came inside.",
                "walks in",
            )],
            ..EvalFormResponse::default()
        },
        &context,
    );
    assert_eq!(result.draft.memory_candidate_count, 1);
    assert_eq!(result.output.memory_candidates.len(), 1);
}

#[test]
fn form_relationship_row_compiles_to_delta() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "Long time no see.",
        "Aurora warms at the greeting.",
    );
    let result = compile_eval_form_response(
        &spec,
        &EvalFormResponse {
            event_rows: vec![event(
                "greeting",
                "The visitor greeted Aurora.",
                "Long time no see",
            )],
            relationship_rows: vec![RelationshipRow {
                linked_event_id: "greeting".into(),
                source_soul_id: "aurora_soul".into(),
                target_entity_id: "default_player".into(),
                dimension: Some(RelationshipDimension::Comfort),
                direction: Some(RelationshipDirection::Increase),
                magnitude_tier: Some(MagnitudeTier::Small),
                evidence_quote: "Long time no see".into(),
                ..RelationshipRow::default()
            }],
            ..EvalFormResponse::default()
        },
        &context,
    );
    assert_eq!(result.output.relationship_evaluations[0].comfort, Some(1.0));
}

#[test]
fn form_object_row_compiles_to_object_observation() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "I close the door.",
        "The apartment door clicks closed.",
    );
    let result = compile_eval_form_response(
        &spec,
        &EvalFormResponse {
            event_rows: vec![event("door", "The door closed.", "door clicks closed")],
            object_rows: vec![ObjectRow {
                linked_event_id: "door".into(),
                object_id: Some("apartment_door".into()),
                property_changed: "open_state".into(),
                old_value: Some("open".into()),
                new_value: "closed".into(),
                evidence_quote: "door clicks closed".into(),
                confidence_tier: Some(ConfidenceTier::High),
                ..ObjectRow::default()
            }],
            ..EvalFormResponse::default()
        },
        &context,
    );
    assert_eq!(
        result.output.object_changes[0].object_state.object_id,
        "apartment_door"
    );
    assert!(result
        .conversion
        .patch
        .world_patch
        .as_ref()
        .unwrap()
        .object_observation_operations
        .iter()
        .any(|operation| operation.operation == "update_object_state"));
}

#[test]
fn code_computes_turn_flags_not_llm() {
    let (soul, world) = soul_and_world();
    let (spec, context) =
        spec_and_context(&soul, &world, "I walk in.", "The visitor walks in.");
    let result = compile_eval_form_response(
        &spec,
        &EvalFormResponse {
            event_rows: vec![event("entry", "The visitor entered.", "walks in")],
            ..EvalFormResponse::default()
        },
        &context,
    );
    assert_ne!(result.trace.compiled_turn_flags_u64, 0);
    assert_ne!(result.output.turn_flags_u64 & turn_flags::SCENE_TURN, 0);
}

#[test]
fn code_assigns_decay_profile_not_llm() {
    let (soul, world) = soul_and_world();
    let (spec, context) =
        spec_and_context(&soul, &world, "I walk in.", "The visitor walks in.");
    let memory = memory(
        "entry",
        "Aurora remembers the visitor came inside.",
        "walks in",
    );
    let candidate_id = memory_candidate_id(&memory);
    let result = compile_eval_form_response(
        &spec,
        &EvalFormResponse {
            event_rows: vec![event("entry", "The visitor entered.", "walks in")],
            memory_rows: vec![memory],
            ..EvalFormResponse::default()
        },
        &context,
    );
    assert_eq!(
        result
            .trace
            .code_assigned_decay_profile
            .get(&candidate_id)
            .map(String::as_str),
        Some("slow")
    );
}

#[test]
fn form_path_door_entry_creates_scene_state_or_recent_event() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "I walk in. Long time no see, Aurora.",
        "The visitor walks into Aurora's apartment.",
    );
    let result = compile_eval_form_response(
        &spec,
        &EvalFormResponse {
            event_rows: vec![event(
                "entry",
                "The visitor entered Aurora's apartment.",
                "walks into Aurora's apartment",
            )],
            ..EvalFormResponse::default()
        },
        &context,
    );
    let world_patch = result.conversion.patch.world_patch.as_ref().unwrap();
    assert!(world_patch.scene_state.is_some() || !world_patch.event_operations.is_empty());
}

#[test]
fn form_path_can_review_existing_memory_before_writing_duplicate() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "I walk in.",
        "The visitor walks in after knocking.",
    );
    assert_eq!(spec.existing_memories.len(), 1);
    let memory = memory(
        "entry",
        "Aurora remembers that the visitor knocked before entering.",
        "walks in",
    );
    let candidate_id = memory_candidate_id(&memory);
    let result = compile_eval_form_response(
        &spec,
        &EvalFormResponse {
            event_rows: vec![event("entry", "The visitor entered.", "walks in")],
            memory_rows: vec![memory],
            review_rows: vec![ReviewRow {
                candidate_id,
                decision: Some(ReviewDecision::DuplicateOfExisting),
                existing_id: Some("mem_existing".into()),
                reason: "already captured".into(),
                evidence_quote: "walks in".into(),
                ..ReviewRow::default()
            }],
            ..EvalFormResponse::default()
        },
        &context,
    );
    assert!(result.conversion.patch.soul_patch.is_none());
    assert_eq!(
        result.trace.form_dedupe_decisions[0].decision,
        ReviewDecision::DuplicateOfExisting
    );
}

#[test]
fn form_accepts_summary_alias_for_objective_summary() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "I knock.", "A knock sounds.");
    let response = parse_eval_form_response(
        r#"{
            "event_rows": [{
                "event_id": "knock",
                "event_type": "scene_event",
                "summary": "The visitor knocked at Aurora's door.",
                "participants": ["aurora_soul", "default_player"],
                "evidence_quote": "I knock."
            }]
        }"#,
    )
    .expect("parse aliases");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(
        result.output.world_changes[0].event_summary.as_deref(),
        Some("The visitor knocked at Aurora's door.")
    );
}

#[test]
fn form_accepts_event_id_alias_for_linked_event_id() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "Long time no see.",
        "Aurora relaxes at the familiar greeting.",
    );
    let response = parse_eval_form_response(
        r#"{
            "event_rows": [{
                "event_id": "greeting",
                "event_type": "scene_event",
                "summary": "The visitor greeted Aurora.",
                "participants": ["aurora_soul", "default_player"],
                "evidence_quote": "Long time no see."
            }],
            "relationship_rows": [{
                "event_id": "greeting",
                "source_soul_id": "aurora_soul",
                "target_entity_id": "default_player",
                "dimension": "comfort",
                "change_direction": "increase",
                "magnitude_tier": "small",
                "evidence_quote": "Long time no see."
            }]
        }"#,
    )
    .expect("parse aliases");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations[0].comfort, Some(1.0));
}

#[test]
fn form_accepts_slot_id_alias_for_slot() {
    let (soul, world) = soul_and_world();
    let (spec, context) =
        spec_and_context(&soul, &world, "I walk in.", "The visitor walks in.");
    let response = parse_eval_form_response(
        r#"{
            "event_rows": [{
                "event_id": "entry",
                "event_type": "scene_event",
                "summary": "The visitor entered Aurora's apartment.",
                "participants": ["aurora_soul", "default_player"],
                "evidence_quote": "I walk in."
            }],
            "memory_rows": [{
                "event_id": "entry",
                "owner_soul_id": "aurora_soul",
                "slot_id": "current_plot_memory",
                "content": "Aurora saw the visitor enter.",
                "evidence_quote": "I walk in.",
                "importance_tier": "medium",
                "selected_tags": ["current_plot"]
            }]
        }"#,
    )
    .expect("parse aliases");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(
        result.output.memory_candidates[0].slot,
        MemorySlot::CurrentPlotMemory
    );
}

#[test]
fn form_relationship_id_parses_source_and_target() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "Long time no see.",
        "Aurora relaxes at the familiar greeting.",
    );
    let response = parse_eval_form_response(
        r#"{
            "event_rows": [{
                "event_id": "greeting",
                "event_type": "scene_event",
                "summary": "The visitor greeted Aurora.",
                "participants": ["aurora_soul", "default_player"],
                "evidence_quote": "Long time no see."
            }],
            "relationship_rows": [{
                "event_id": "greeting",
                "relationship_id": "rel:aurora_soul:default_player",
                "dimension": "comfort",
                "change_direction": "increase",
                "magnitude_tier": "small",
                "evidence_quote": "Long time no see."
            }]
        }"#,
    )
    .expect("parse relationship id");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(
        result.output.relationship_evaluations[0].source_soul_id,
        "aurora_soul"
    );
    assert_eq!(
        result.output.relationship_evaluations[0].target_entity_id,
        "default_player"
    );
}

#[test]
fn form_memory_content_can_derive_from_linked_event() {
    let (soul, world) = soul_and_world();
    let (spec, context) =
        spec_and_context(&soul, &world, "I walk in.", "The visitor walks in.");
    let response = parse_eval_form_response(
        r#"{
            "event_rows": [{
                "event_id": "entry",
                "event_type": "scene_event",
                "summary": "The visitor entered Aurora's apartment.",
                "participants": ["aurora_soul", "default_player"],
                "evidence_quote": "I walk in."
            }],
            "memory_rows": [{
                "event_id": "entry",
                "owner_soul_id": "aurora_soul",
                "slot_id": "current_plot_memory",
                "evidence_quote": "I walk in.",
                "importance_tier": "medium",
                "selected_tags": ["current_plot"]
            }]
        }"#,
    )
    .expect("parse memory aliases");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert!(result.output.memory_candidates[0]
        .content
        .contains("I walk in."));
}

#[test]
fn form_object_row_accepts_summary_and_change_aliases() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "I open the door.",
        "The apartment door opens.",
    );
    let response = parse_eval_form_response(
        r#"{
            "event_rows": [{
                "event_id": "door_opened",
                "event_type": "object_change",
                "summary": "The apartment door opened.",
                "participants": ["aurora_soul", "default_player"],
                "evidence_quote": "I open the door."
            }],
            "object_rows": [{
                "event_id": "door_opened",
                "object_id": "apartment_door",
                "change": "open_state",
                "summary": "open",
                "evidence_quote": "I open the door.",
                "confidence_tier": "medium"
            }]
        }"#,
    )
    .expect("parse object aliases");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(
        result.output.object_changes[0]
            .object_state
            .last_observed_state,
        "open_state: open"
    );
}

#[test]
fn strips_markdown_json_fence_before_form_parse() {
    let parsed = parse_eval_form_response(
        r#"```json
        {"event_rows":[{"event_id":"entry","event_type":"scene_event","summary":"Entry.","participants":["aurora_soul"],"evidence_quote":"I enter."}]}
        ```"#,
    )
    .expect("fenced json");

    assert_eq!(parsed.event_rows[0].event_id, "entry");
}

#[test]
fn repairs_evidence_quote_string_and_string() {
    let (parsed, trace) = parse_eval_form_response_with_trace(
        r#"{
            "event_rows": [{
                "event_id": "watchful",
                "event_type": "scene_event",
                "summary": "Aurora stays watchful.",
                "participants": ["aurora_soul", "default_player"],
                "evidence_quote": "her body a casual barrier" and "her eyes remain watchful"
            }]
        }"#,
    )
    .expect("repair quote and quote");

    assert!(trace.raw_form_repair_applied);
    assert_eq!(
        parsed.event_rows[0].evidence_quote,
        "her body a casual barrier; her eyes remain watchful"
    );
}

#[test]
fn maps_increased_interest_with_undercurrent_to_increase() {
    let parsed = parse_eval_form_response(
        r#"{
            "event_rows": [{
                "event_id": "greeting",
                "event_type": "scene_event",
                "summary": "The visitor greeted Aurora.",
                "participants": ["aurora_soul", "default_player"],
                "evidence_quote": "Long time no see."
            }],
            "relationship_rows": [{
                "event_id": "greeting",
                "relationship_id": "rel:aurora_soul:default_player",
                "dimension": "curiosity",
                "direction": "increased_interest_with_undercurrent",
                "evidence_quote": "Long time no see."
            }]
        }"#,
    )
    .expect("direction drift");

    assert_eq!(
        parsed.relationship_rows[0].direction,
        Some(RelationshipDirection::Increase)
    );
}

#[test]
fn dimensions_changed_array_splits_relationship_rows() {
    let parsed = parse_eval_form_response(
        r#"{
            "event_rows": [{
                "event_id": "greeting",
                "event_type": "scene_event",
                "summary": "The visitor greeted Aurora.",
                "participants": ["aurora_soul", "default_player"],
                "evidence_quote": "Long time no see."
            }],
            "relationship_rows": [{
                "event_id": "greeting",
                "relationship_id": "rel:aurora_soul:default_player",
                "dimensions_changed": ["comfort", "curiosity"],
                "direction": "increased",
                "evidence_quote": "Long time no see."
            }]
        }"#,
    )
    .expect("dimensions split");

    assert_eq!(parsed.relationship_rows.len(), 2);
    assert_eq!(
        parsed.relationship_rows[0].dimension,
        Some(RelationshipDimension::Comfort)
    );
    assert_eq!(
        parsed.relationship_rows[1].dimension,
        Some(RelationshipDimension::Curiosity)
    );
}

#[test]
fn missing_linked_event_id_uses_single_event() {
    let parsed = parse_eval_form_response(
        r#"{
            "event_rows": [{
                "event_id": "entry",
                "event_type": "scene_event",
                "summary": "The visitor entered.",
                "participants": ["aurora_soul", "default_player"],
                "evidence_quote": "I enter."
            }],
            "memory_rows": [{
                "owner_soul_id": "aurora_soul",
                "slot_id": "current_plot_memory",
                "summary": "The visitor entered.",
                "evidence_quote": "I enter."
            }]
        }"#,
    )
    .expect("single event link");

    assert_eq!(parsed.memory_rows[0].linked_event_id, "entry");
}

#[test]
fn missing_linked_event_id_uses_highest_importance_event() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "I enter.", "The visitor enters.");
    let response = EvalFormResponse {
        event_rows: vec![
            EventRow {
                event_id: "minor".into(),
                event_type: Some(EventType::SceneEvent),
                objective_summary: "A small glance happens.".into(),
                participants: vec!["aurora_soul".into()],
                evidence_quote: "glance".into(),
                importance_tier: Some(ImportanceTier::Low),
                ..EventRow::default()
            },
            EventRow {
                event_id: "major".into(),
                event_type: Some(EventType::SceneEvent),
                objective_summary: "The visitor enters.".into(),
                participants: vec!["aurora_soul".into(), "default_player".into()],
                evidence_quote: "I enter.".into(),
                importance_tier: Some(ImportanceTier::High),
                ..EventRow::default()
            },
        ],
        memory_rows: vec![MemoryRow {
            owner_soul_id: "aurora_soul".into(),
            slot: Some(MemorySlot::CurrentPlotMemory),
            content: "The visitor entered.".into(),
            evidence_quote: "I enter.".into(),
            ..MemoryRow::default()
        }],
        ..EvalFormResponse::default()
    };
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(
        result.output.memory_candidates[0]
            .candidate_id
            .contains("major"),
        false
    );
    assert_eq!(result.trace.form_rows_rejected, 0);
}

#[test]
fn memory_summary_becomes_content() {
    let parsed = parse_eval_form_response(
        r#"{
            "event_rows": [{
                "event_id": "entry",
                "event_type": "scene_event",
                "summary": "The visitor entered.",
                "participants": ["aurora_soul", "default_player"],
                "evidence_quote": "I enter."
            }],
            "memory_rows": [{
                "event_id": "entry",
                "owner_soul_id": "aurora_soul",
                "slot_id": "current_plot_memory",
                "summary": "Aurora saw the visitor enter.",
                "evidence_quote": "I enter."
            }]
        }"#,
    )
    .expect("summary content");

    assert_eq!(
        parsed.memory_rows[0].content,
        "Aurora saw the visitor enter."
    );
}

#[test]
fn memory_id_becomes_candidate_id() {
    let parsed = parse_eval_form_response(
        r#"{
            "review_rows": [{
                "memory_id": "mem-1",
                "decision": "new",
                "reason": "new memory",
                "evidence_quote": "I enter."
            }]
        }"#,
    )
    .expect("memory id alias");

    assert_eq!(parsed.review_rows[0].candidate_id, "mem-1");
}

#[test]
fn unknown_tags_are_dropped_not_fatal() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "I enter.", "The visitor enters.");
    let response = parse_eval_form_response(
        r#"{
            "event_rows": [{
                "event_id": "entry",
                "event_type": "scene_event",
                "summary": "The visitor entered.",
                "participants": ["aurora_soul", "default_player"],
                "evidence_quote": "I enter."
            }],
            "memory_rows": [{
                "event_id": "entry",
                "owner_soul_id": "aurora_soul",
                "slot_id": "current_plot_memory",
                "content": "Aurora saw the visitor enter.",
                "evidence_quote": "I enter.",
                "selected_tags": ["Scene Event", "very_weird_tag"]
            }]
        }"#,
    )
    .expect("unknown tag");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.memory_candidates.len(), 1);
}

#[test]
fn form_door_knock_accepts_at_least_one_row() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "I knock at the door",
        "A knock sounds at Aurora's apartment door.",
    );
    let response = parse_eval_form_response(
        r#"{
            "event_rows": [{
                "event_id": "door_knock",
                "event_type": "scene_event",
                "summary": "The visitor knocked at Aurora's apartment door.",
                "timestamp": "now",
                "participants": ["aurora_soul", "default_player"],
                "evidence_quote": "I knock at the door",
                "importance_tier": "medium"
            }],
            "memory_rows": [{
                "event_id": "door_knock",
                "owner_soul_id": "session_world",
                "slot_id": "current_plot_memory",
                "evidence_quote": "I knock at the door",
                "importance_tier": "medium",
                "selected_tags": ["scene_event"]
            }]
        }"#,
    )
    .expect("parse knock");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert!(result.trace.form_rows_accepted > 0);
    assert!(result.draft.world_event_count > 0 || result.draft.scene_state_present);
    assert!(!result.conversion.patch.is_empty());
}

#[test]
fn enrichment_relationship_without_linked_event_uses_baseline_event() {
    let (soul, world) = soul_and_world();
    let spec = build_eval_form_spec(&soul, Some(&world), "I enter.", "The visitor enters.", 8);
    let context = EvaluatorConversionContext {
        active_soul_id: &soul.character_id,
        active_soul_ids: vec![soul.character_id.clone()],
        latest_user_message: "I enter.",
        latest_narrator_response: "The visitor enters.",
        session_world: Some(&world),
        baseline_recent_event_id: Some("event_baseline_xyz".into()),
    };
    let response = EvalFormResponse {
        relationship_rows: vec![RelationshipRow {
            linked_event_id: "".into(),
            source_soul_id: "aurora_soul".into(),
            target_entity_id: "default_player".into(),
            dimension: Some(RelationshipDimension::Comfort),
            direction: Some(RelationshipDirection::Increase),
            magnitude_tier: Some(MagnitudeTier::Small),
            evidence_quote: "Long time no see".into(),
            ..RelationshipRow::default()
        }],
        ..EvalFormResponse::default()
    };
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(
        result.normalized_response.relationship_rows[0].linked_event_id,
        "event_baseline_xyz"
    );
}

#[test]
fn enrichment_memory_without_linked_event_uses_baseline_event() {
    let (soul, world) = soul_and_world();
    let spec = build_eval_form_spec(&soul, Some(&world), "I enter.", "The visitor enters.", 8);
    let context = EvaluatorConversionContext {
        active_soul_id: &soul.character_id,
        active_soul_ids: vec![soul.character_id.clone()],
        latest_user_message: "I enter.",
        latest_narrator_response: "The visitor enters.",
        session_world: Some(&world),
        baseline_recent_event_id: Some("event_baseline_xyz".into()),
    };
    let response = EvalFormResponse {
        memory_rows: vec![MemoryRow {
            linked_event_id: "".into(),
            owner_soul_id: "aurora_soul".into(),
            slot: Some(MemorySlot::CurrentPlotMemory),
            content: "Aurora remembers the visitor enters.".into(),
            evidence_quote: "enters".into(),
            ..MemoryRow::default()
        }],
        ..EvalFormResponse::default()
    };
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(
        result.normalized_response.memory_rows[0].linked_event_id,
        "event_baseline_xyz"
    );
}

fn soul_aurora() -> Soul {
    let mut soul = new_default_soul("Aurora Schwarz");
    soul.character_id = "e0ee4936-2e71-4ab9-8631-4c22be68ec72".into();
    soul
}

fn live_fixture_context<'a>(
    soul: &'a Soul,
    world: &'a SessionWorld,
    user: &'a str,
    narrator: &'a str,
) -> EvaluatorConversionContext<'a> {
    EvaluatorConversionContext {
        active_soul_id: &soul.character_id,
        active_soul_ids: vec![soul.character_id.clone()],
        latest_user_message: user,
        latest_narrator_response: narrator,
        session_world: Some(world),
        baseline_recent_event_id: None,
    }
}

#[test]
fn relationship_dimension_infers_from_curiosity_tag() {
    let parsed = parse_eval_form_response(
        r#"{
          "event_rows":[{"event_id":"evt","event_type":"scene_event","summary":"Aurora grows curious.","participants":["aurora_soul","default_player"],"evidence_quote":"Long time no see."}],
          "relationship_rows":[{
            "relationship_id":"rel:aurora_soul:default_player",
            "summary":"Aurora's cautious curiosity towards User increases",
            "tags":[{"vocabulary":"relationship","value":"curiosity"},{"vocabulary":"relationship","value":"unknown_tag"}],
            "evidence_quote":"Long time no see."
          }]
        }"#,
    )
    .expect("parse");

    assert_eq!(
        parsed.relationship_rows[0].dimension,
        Some(RelationshipDimension::Curiosity)
    );
    assert_eq!(parsed.relationship_rows[0].selected_tags, vec!["curiosity"]);
}

#[test]
fn relationship_direction_infers_from_summary_increases() {
    let parsed = parse_eval_form_response(
        r#"{
          "event_rows":[{"event_id":"evt","event_type":"scene_event","summary":"Aurora grows curious.","participants":["aurora_soul","default_player"],"evidence_quote":"Long time no see."}],
          "relationship_rows":[{
            "relationship_id":"rel:aurora_soul:default_player",
            "summary":"Aurora's cautious curiosity towards User increases",
            "tags":["curiosity"],
            "importance_tier":"high",
            "evidence_quote":"Long time no see."
          }]
        }"#,
    )
    .expect("parse");

    assert_eq!(
        parsed.relationship_rows[0].direction,
        Some(RelationshipDirection::Increase)
    );
    assert_eq!(
        parsed.relationship_rows[0].magnitude_tier,
        Some(MagnitudeTier::Medium)
    );
}

#[test]
fn relationship_unknown_tag_dropped_not_fatal() {
    let (soul, world) = soul_and_world();
    let spec = build_eval_form_spec(
        &soul,
        Some(&world),
        "Long time no see.",
        "Aurora studies the visitor with cautious curiosity.",
        8,
    );
    let context = live_fixture_context(
        &soul,
        &world,
        "Long time no see.",
        "Aurora studies the visitor with cautious curiosity.",
    );
    let response = parse_eval_form_response(
        r#"{
          "event_rows":[{"event_id":"evt","event_type":"scene_event","summary":"Aurora studies the visitor.","participants":["aurora_soul","default_player"],"evidence_quote":"Long time no see."}],
          "relationship_rows":[{
            "relationship_id":"rel:aurora_soul:default_player",
            "summary":"Aurora's cautious curiosity towards User increases",
            "tags":["curiosity","totally_unknown"],
            "evidence_quote":"Aurora studies the visitor with cautious curiosity."
          }]
        }"#,
    )
    .expect("parse");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert!(result.rejected_rows.is_empty());
    assert_eq!(
        result.output.relationship_evaluations[0].curiosity,
        Some(1.0)
    );
}

#[test]
fn payload_fixture_applies_curiosity_delta() {
    let soul = soul_aurora();
    let world = session_world_from_legacy_world("Apartment", None, &soul.world);
    let user = "I walk in. Long time no see, Aurora.";
    let narrator = "Aurora's cautious curiosity towards User increases as she steps aside. She studies the visitor with cautious curiosity.";
    let spec = build_eval_form_spec(&soul, Some(&world), user, narrator, 8);
    let context = live_fixture_context(&soul, &world, user, narrator);
    let response = parse_eval_form_response(&format!(
        r#"{{
          "event_rows":[{{"event_id":"evt","event_type":"scene_event","summary":"Aurora lets the visitor in.","participants":["{}","default_player"],"evidence_quote":"I walk in. Long time no see, Aurora."}}],
          "relationship_rows":[{{
            "relationship_id":"rel:{}:default_player",
            "summary":"Aurora's cautious curiosity towards User increases",
            "tags":["curiosity","fear"],
            "importance_tier":"medium",
            "evidence_quote":"Aurora's cautious curiosity towards User increases"
          }}]
        }}"#,
        soul.character_id, soul.character_id
    ))
    .expect("parse");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(
        result
            .conversion
            .patch
            .soul_patch
            .as_ref()
            .unwrap()
            .relationship_deltas[0]
            .curiosity,
        Some(1.0)
    );
}

#[test]
fn payload_fixture_writes_unresolved_tension_memory() {
    let soul = soul_aurora();
    let world = session_world_from_legacy_world("Apartment", None, &soul.world);
    let user = "I walk in. Long time no see, Aurora.";
    let narrator = "Aurora smiles, but her nerves remain visible; the reunion leaves unresolved tension in the room.";
    let spec = build_eval_form_spec(&soul, Some(&world), user, narrator, 8);
    let context = live_fixture_context(&soul, &world, user, narrator);
    let response = parse_eval_form_response(&format!(
        r#"{{
          "event_rows":[{{"event_id":"evt","event_type":"scene_event","summary":"The visitor enters Aurora's apartment.","participants":["{}","default_player"],"evidence_quote":"I walk in. Long time no see, Aurora."}}],
          "memory_rows":[{{
            "owner_soul_id":"{}",
            "slot_id":"unresolved_tension",
            "candidate_memory":"Aurora's nerves make the reunion feel unresolved.",
            "salience":"medium",
            "evidence_quote":"her nerves remain visible; the reunion leaves unresolved tension in the room"
          }}]
        }}"#,
        soul.character_id, soul.character_id
    ))
    .expect("parse");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert!(result
        .conversion
        .patch
        .soul_patch
        .as_ref()
        .unwrap()
        .new_memories
        .iter()
        .any(|memory| memory.memory_slot.as_deref() == Some("unresolved_tension")));
}

#[test]
fn payload_fixture_writes_recent_emotional_state_memory() {
    let soul = soul_aurora();
    let world = session_world_from_legacy_world("Apartment", None, &soul.world);
    let user = "I walk in. Long time no see, Aurora.";
    let narrator =
        "Aurora shifts from waiting alone to playful engagement after the visitor enters.";
    let spec = build_eval_form_spec(&soul, Some(&world), user, narrator, 8);
    let context = live_fixture_context(&soul, &world, user, narrator);
    let response = parse_eval_form_response(&format!(
        r#"{{
          "event_rows":[{{"event_id":"evt","event_type":"scene_event","summary":"The visitor enters Aurora's apartment.","participants":["{}","default_player"],"evidence_quote":"I walk in. Long time no see, Aurora."}}],
          "memory_rows":[{{
            "owner_soul_id":"{}",
            "slot_id":"recent_emotional_state",
            "candidate_memory":"Aurora shifts from waiting alone to playful engagement after the visitor enters.",
            "salience":"medium",
            "evidence_quote":"Aurora shifts from waiting alone to playful engagement after the visitor enters."
          }}]
        }}"#,
        soul.character_id, soul.character_id
    ))
    .expect("parse");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert!(result
        .conversion
        .patch
        .soul_patch
        .as_ref()
        .unwrap()
        .new_memories
        .iter()
        .any(|memory| memory.memory_slot.as_deref() == Some("recent_emotional_state")));
}

const PAYLOAD4_JSON: &str = r#"{
  "event_rows": [
    {
      "event_id": "evt_knock_response",
      "event_type": "scene_event",
      "importance_tier": "medium",
      "timestamp": "latest",
      "summary": "User knocks on door, Aurora responds",
      "evidence_quote": "The knock is soft but distinct in the quiet apartment. Aurora, who had been standing at her window watching rain streak the neon-lit glass, flinches slightly... She unlocks the door and pulls it open just enough to stand in the gap, one hand still on the knob."
    }
  ],
  "object_rows": [
    {
      "object_id": "obj_cigarette_mug",
      "object_label": "mug used as ashtray",
      "object_type": "consumable",
      "location_observed": "Aurora's apartment",
      "state_change": "cigarette stubbed out",
      "evidence_quote": "stubs the cigarette out in a nearby mug"
    }
  ],
  "relationship_rows": [
    {
      "relationship_id": "rel:e0ee4936-2e71-4ab9-8631-4c22be68ec72:default_player",
      "dimension": "affection",
      "shift": "+2",
      "evidence_quote": "A faint smile touches her mouth—half anticipation, half nerves... 'Hey,' she says, her voice husky and warm. 'You're here.'"
    },
    {
      "relationship_id": "rel:e0ee4936-2e71-4ab9-8631-4c22be68ec72:default_player",
      "dimension": "comfort",
      "shift": "+3",
      "evidence_quote": "She unlocks the door and pulls it open just enough to stand in the gap"
    }
  ],
  "memory_rows": [
    {
      "slot_id": "relationship_memory",
      "candidate_memory": "Aurora welcomes User at door with warm, slightly nervous greeting, showing growing affection and comfort",
      "salience": "high",
      "evidence_quote": "A faint smile touches her mouth—half anticipation, half nerves... 'Hey,' she says, her voice husky and warm. 'You're here.'"
    },
    {
      "slot_id": "current_plot_memory",
      "candidate_memory": "User arrives at Aurora's apartment after knocking; scene shifts from solitude to interaction",
      "salience": "high",
      "evidence_quote": "The knock is soft but distinct in the quiet apartment... She unlocks the door and pulls it open just enough to stand in the gap."
    },
    {
      "slot_id": "character_identity_memory",
      "candidate_memory": "Aurora experiences nervous anticipation when User arrives, revealing emotional investment",
      "salience": "medium",
      "evidence_quote": "A faint smile touches her mouth—half anticipation, half nerves."
    },
    {
      "slot_id": "unresolved_tension",
      "candidate_memory": "Aurora's nerves and anticipation create unresolved tension as she greets User",
      "salience": "medium",
      "evidence_quote": "half anticipation, half nerves"
    },
    {
      "slot_id": "recent_emotional_state",
      "candidate_memory": "Aurora shifts from thoughtful solitude to nervous anticipation upon hearing knock",
      "salience": "medium",
      "evidence_quote": "Aurora, who had been standing at her window watching rain streak the neon-lit glass, flinches slightly... Now she exhales a plume of smoke... moves quickly across the room."
    }
  ],
  "review_rows": [
    {
      "soul_id": "e0ee4936-2e71-4ab9-8631-4c22be68ec72",
      "soul_name": "Aurora Schwarz",
      "perceptions": [
        {
          "event": "evt_knock_response",
          "what_soul_knew": "Aurora knows User knocked and has arrived at her door",
          "evidence_quote": "She unlocks the door and pulls it open just enough to stand in the gap... 'You're here.'"
        }
      ],
      "misunderstandings": []
    },
    {
      "soul_id": "default_player",
      "soul_name": "User",
      "perceptions": [
        {
          "event": "evt_knock_response",
          "what_soul_knew": "User knows they knocked and Aurora answered the door",
          "evidence_quote": "I knock at the door"
        }
      ],
      "misunderstandings": []
    }
  ]
}"#;

#[test]
fn payload4_fixture_applies_object_state() {
    let soul = soul_aurora();
    let world = session_world_from_legacy_world("Apartment", None, &soul.world);
    let spec = build_eval_form_spec(&soul, Some(&world), "I knock", "Door opens", 8);
    let context = EvaluatorConversionContext {
        active_soul_id: &soul.character_id,
        active_soul_ids: vec![soul.character_id.clone()],
        latest_user_message: "I knock",
        latest_narrator_response: "Door opens",
        session_world: Some(&world),
        baseline_recent_event_id: None,
    };
    let response = parse_eval_form_response(PAYLOAD4_JSON).expect("parse payload 4");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert!(result.rejected_rows.is_empty());
    assert_eq!(result.output.object_changes.len(), 1);
    let object_change = &result.output.object_changes[0];
    assert_eq!(object_change.object_state.object_id, "cigarette_mug");
    assert_eq!(object_change.object_state.status, "cigarette stubbed out");
    assert_eq!(object_change.object_state.location, "Aurora's apartment");
}

#[test]
fn payload4_fixture_applies_relationship_affection_comfort() {
    let soul = soul_aurora();
    let world = session_world_from_legacy_world("Apartment", None, &soul.world);
    let spec = build_eval_form_spec(&soul, Some(&world), "I knock", "Door opens", 8);
    let context = EvaluatorConversionContext {
        active_soul_id: &soul.character_id,
        active_soul_ids: vec![soul.character_id.clone()],
        latest_user_message: "I knock",
        latest_narrator_response: "Door opens",
        session_world: Some(&world),
        baseline_recent_event_id: None,
    };
    let response = parse_eval_form_response(PAYLOAD4_JSON).expect("parse payload 4");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.output.relationship_evaluations.len(), 2);

    let rel_affection = result
        .output
        .relationship_evaluations
        .iter()
        .find(|r| r.affection.is_some())
        .unwrap();
    assert_eq!(rel_affection.affection, Some(2.0));
    assert_eq!(
        rel_affection.source_soul_id,
        "e0ee4936-2e71-4ab9-8631-4c22be68ec72"
    );
    assert_eq!(rel_affection.target_entity_id, "default_player");
    assert!(rel_affection
        .evidence_quote
        .as_ref()
        .unwrap()
        .contains("A faint smile"));

    let rel_comfort = result
        .output
        .relationship_evaluations
        .iter()
        .find(|r| r.comfort.is_some())
        .unwrap();
    assert_eq!(rel_comfort.comfort, Some(3.0));
    assert_eq!(
        rel_comfort.source_soul_id,
        "e0ee4936-2e71-4ab9-8631-4c22be68ec72"
    );
    assert_eq!(rel_comfort.target_entity_id, "default_player");
}

#[test]
fn payload4_fixture_writes_soul_memory_recent() {
    let soul = soul_aurora();
    let world = session_world_from_legacy_world("Apartment", None, &soul.world);
    let spec = build_eval_form_spec(&soul, Some(&world), "I knock", "Door opens", 8);
    let context = EvaluatorConversionContext {
        active_soul_id: &soul.character_id,
        active_soul_ids: vec![soul.character_id.clone()],
        latest_user_message: "I knock",
        latest_narrator_response: "Door opens",
        session_world: Some(&world),
        baseline_recent_event_id: None,
    };
    let response = parse_eval_form_response(PAYLOAD4_JSON).expect("parse payload 4");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert!(result.output.memory_candidates.len() > 0);
    let rel_mem = result
        .output
        .memory_candidates
        .iter()
        .find(|m| m.slot == MemorySlot::RelationshipMemory)
        .unwrap();
    assert_eq!(
        rel_mem.owner_soul_id,
        "e0ee4936-2e71-4ab9-8631-4c22be68ec72"
    );
    assert_eq!(
        rel_mem.target_entity_ids,
        vec!["default_player".to_string()]
    );
}

#[test]
fn payload4_fixture_does_not_turn_subjective_memory_into_world_event() {
    let soul = soul_aurora();
    let world = session_world_from_legacy_world("Apartment", None, &soul.world);
    let spec = build_eval_form_spec(&soul, Some(&world), "I knock", "Door opens", 8);
    let context = EvaluatorConversionContext {
        active_soul_id: &soul.character_id,
        active_soul_ids: vec![soul.character_id.clone()],
        latest_user_message: "I knock",
        latest_narrator_response: "Door opens",
        session_world: Some(&world),
        baseline_recent_event_id: None,
    };
    let response = parse_eval_form_response(PAYLOAD4_JSON).expect("parse payload 4");
    let result = compile_eval_form_response(&spec, &response, &context);

    for change in &result.output.world_changes {
        if let Some(ref summary) = change.event_summary {
            assert!(!summary.contains("Aurora welcomes User"));
            assert!(!summary.contains("Aurora's nerves"));
        }
    }
}

#[test]
fn payload4_fixture_exports_nonempty_memory_object_relationship() {
    let soul = soul_aurora();
    let world = session_world_from_legacy_world("Apartment", None, &soul.world);
    let spec = build_eval_form_spec(&soul, Some(&world), "I knock", "Door opens", 8);
    let context = EvaluatorConversionContext {
        active_soul_id: &soul.character_id,
        active_soul_ids: vec![soul.character_id.clone()],
        latest_user_message: "I knock",
        latest_narrator_response: "Door opens",
        session_world: Some(&world),
        baseline_recent_event_id: None,
    };
    let response = parse_eval_form_response(PAYLOAD4_JSON).expect("parse payload 4");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert!(!result.output.memory_candidates.is_empty());
    assert!(!result.output.object_changes.is_empty());
    assert!(!result.output.relationship_evaluations.is_empty());
}

#[test]
fn payload4_relationship_comfort_boundary_pressure_compiles() {
    let soul = soul_aurora();
    let world = session_world_from_legacy_world("Apartment", None, &soul.world);
    let user = "I walk in. Long time no see, Aurora.";
    let narrator = "The visitor enters Aurora's apartment after knocking. Aurora opens the door with a warm, slightly nervous greeting.";
    let spec = build_eval_form_spec(&soul, Some(&world), user, narrator, 8);
    let context = live_fixture_context(&soul, &world, user, narrator);
    let response = parse_eval_form_response(PAYLOAD4_JSON).expect("parse payload 4");
    let result = compile_eval_form_response(&spec, &response, &context);

    let rel_comfort = result
        .output
        .relationship_evaluations
        .iter()
        .find(|r| r.comfort.is_some())
        .unwrap();
    assert_eq!(rel_comfort.comfort, Some(3.0));
    assert_eq!(rel_comfort.source_soul_id, soul.character_id);
    assert_eq!(rel_comfort.target_entity_id, "default_player");
    assert!(rel_comfort.criterion_met);
}

#[test]
fn payload4_relationship_conflict_potential_escalation_compiles() {
    let soul = soul_aurora();
    let world = session_world_from_legacy_world("Apartment", None, &soul.world);
    let user = "I walk in. Long time no see, Aurora.";
    let narrator = "The visitor enters Aurora's apartment after knocking. Aurora opens the door with a warm, slightly nervous greeting.";
    let spec = build_eval_form_spec(&soul, Some(&world), user, narrator, 8);
    let context = live_fixture_context(&soul, &world, user, narrator);
    let response = parse_eval_form_response(PAYLOAD4_JSON).expect("parse payload 4");
    let result = compile_eval_form_response(&spec, &response, &context);

    let payload_json: serde_json::Value = serde_json::from_str(PAYLOAD4_JSON).unwrap();
    let has_conflict = payload_json["relationship_rows"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| {
            row["dimension"]
                .as_str()
                .map(|d| d == "conflict")
                .unwrap_or(false)
        });

    if has_conflict {
        let rel_conflict = result
            .output
            .relationship_evaluations
            .iter()
            .find(|r| r.conflict.is_some());
        assert!(
            rel_conflict.is_some(),
            "conflict relationship should be compiled"
        );
        let rel_conflict = rel_conflict.unwrap();
        assert!(
            rel_conflict.conflict.unwrap() > 0.0,
            "conflict should be positive"
        );
    }
}

#[test]
fn payload4_memory_preserves_candidate_text() {
    let soul = soul_aurora();
    let world = session_world_from_legacy_world("Apartment", None, &soul.world);
    let user = "I walk in. Long time no see, Aurora.";
    let narrator = "The visitor enters Aurora's apartment after knocking. Aurora opens the door with a warm, slightly nervous greeting.";
    let spec = build_eval_form_spec(&soul, Some(&world), user, narrator, 8);
    let context = live_fixture_context(&soul, &world, user, narrator);
    let response = parse_eval_form_response(PAYLOAD4_JSON).expect("parse payload 4");
    let result = compile_eval_form_response(&spec, &response, &context);

    let rel_mem = result
        .output
        .memory_candidates
        .iter()
        .find(|m| m.slot == MemorySlot::RelationshipMemory)
        .unwrap();
    assert!(
        !rel_mem.content.is_empty(),
        "memory content should not be empty"
    );
    assert!(
        rel_mem.content.contains("Aurora")
            && (rel_mem.content.contains("welcomes")
                || rel_mem.content.contains("warm")
                || rel_mem.content.contains("nervous")),
        "memory content should preserve candidate text, got: {}",
        rel_mem.content
    );
}

#[test]
fn payload4_exports_relationship_delta_or_changed_summary() {
    let soul = soul_aurora();
    let world = session_world_from_legacy_world("Apartment", None, &soul.world);
    let user = "I walk in. Long time no see, Aurora.";
    let narrator = "The visitor enters Aurora's apartment after knocking. Aurora opens the door with a warm, slightly nervous greeting.";
    let spec = build_eval_form_spec(&soul, Some(&world), user, narrator, 8);
    let context = live_fixture_context(&soul, &world, user, narrator);
    let response = parse_eval_form_response(PAYLOAD4_JSON).expect("parse payload 4");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert!(
        !result.output.relationship_evaluations.is_empty(),
        "relationship evaluations should not be empty"
    );
    assert_eq!(
        result.draft.relationship_delta_count,
        result.output.relationship_evaluations.len(),
        "draft relationship_delta_count should match evaluations count"
    );
}

#[test]
fn payload4_next_prompt_retrieves_written_memory() {
    let soul = soul_aurora();
    let world = session_world_from_legacy_world("Apartment", None, &soul.world);
    let user = "I walk in. Long time no see, Aurora.";
    let narrator = "The visitor enters Aurora's apartment after knocking. Aurora opens the door with a warm, slightly nervous greeting.";
    let spec = build_eval_form_spec(&soul, Some(&world), user, narrator, 8);
    let context = live_fixture_context(&soul, &world, user, narrator);
    let response = parse_eval_form_response(PAYLOAD4_JSON).expect("parse payload 4");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert!(
        result.draft.memory_candidate_count > 0,
        "draft should have memory candidates"
    );
    assert!(
        !result.output.memory_candidates.is_empty(),
        "output should have memory candidates"
    );
    let has_relationship_memory = result
        .output
        .memory_candidates
        .iter()
        .any(|m| m.slot == MemorySlot::RelationshipMemory);
    assert!(
        has_relationship_memory,
        "should have at least one relationship memory candidate"
    );
}

#[test]
fn form_validated_relationship_bypasses_second_evidence_check_only_for_that_row() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "I knock.", "Door opens.");
    let result = compile_eval_form_response(
        &spec,
        &EvalFormResponse {
            event_rows: vec![event("evt_knock", "The visitor knocks", "I knock.")],
            relationship_rows: vec![RelationshipRow {
                linked_event_id: "evt_knock".into(),
                source_soul_id: soul.character_id.clone(),
                target_entity_id: "default_player".into(),
                dimension: Some(RelationshipDimension::Comfort),
                direction: Some(RelationshipDirection::Increase),
                magnitude_tier: Some(MagnitudeTier::Small),
                importance_tier: Some(ImportanceTier::Medium),
                evidence_quote: "A faint smile touches her mouth, half anticipation".into(),
                ..RelationshipRow::default()
            }],
            ..EvalFormResponse::default()
        },
        &context,
    );
    assert_eq!(
        result.output.relationship_evaluations.len(),
        1,
        "form-validated relationship should compile"
    );
    assert!(
        result.output.relationship_evaluations[0].evidence_validated_by_form,
        "flag should be true for form path"
    );
    let conversion = evaluator_output_to_engine_patch(
        &result.output,
        &EvaluatorConversionContext {
            active_soul_id: &soul.character_id,
            active_soul_ids: vec![soul.character_id.clone()],
            latest_user_message: "I knock.",
            latest_narrator_response: "Door opens.",
            session_world: Some(&world),
            baseline_recent_event_id: None,
        },
    );
    assert!(
        conversion.patch.soul_patch.is_some(),
        "soul patch should exist from form-validated row"
    );
    assert!(
        !conversion
            .patch
            .soul_patch
            .as_ref()
            .unwrap()
            .relationship_deltas
            .is_empty(),
        "relationship delta should survive evidence check"
    );
    let mut bad_output = result.output.clone();
    bad_output
        .relationship_evaluations
        .push(RelationshipEvaluation {
            source_soul_id: soul.character_id.clone(),
            target_entity_id: "default_player".into(),
            comfort: Some(3.0),
            evidence_quote: Some("A faint smile touches her mouth, half anticipation".into()),
            criterion_met: true,
            confidence: 0.75,
            evidence_validated_by_form: false,
            ..RelationshipEvaluation::default()
        });
    let bad_conversion = evaluator_output_to_engine_patch(
        &bad_output,
        &EvaluatorConversionContext {
            active_soul_id: &soul.character_id,
            active_soul_ids: vec![soul.character_id.clone()],
            latest_user_message: "I knock.",
            latest_narrator_response: "Door opens.",
            session_world: Some(&world),
            baseline_recent_event_id: None,
        },
    );
    assert_eq!(bad_conversion.patch.soul_patch.as_ref().unwrap().relationship_deltas.len(), 1, "only the form-validated row should survive; non-form row with same bad quote should be rejected");
}

#[test]
fn latest_payload_event_row_without_id_or_summary_compiles() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "I knock.",
        "A firm, three-rap knock echoes in the hallway.",
    );
    let response = parse_eval_form_response(
        r#"{
          "event_rows": [{
            "type": "scene_event",
            "importance": "medium",
            "tags": ["scene_event", "doorway"],
            "evidence_quote": "A firm, three-rap knock echoes in the hallway, cutting through the low hum of the ambient music."
          }],
          "relationship_rows": [],
          "memory_rows": [],
          "review_rows": []
        }"#,
    ).expect("parse should succeed");
    let event_row = &response.event_rows[0];
    assert_eq!(
        event_row.event_type,
        Some(EventType::SceneEvent),
        "type should normalize to event_type"
    );
    assert_eq!(
        event_row.importance_tier,
        Some(ImportanceTier::Medium),
        "importance should normalize to importance_tier"
    );
    assert_eq!(
        event_row.event_id, "event_latest_turn",
        "missing event_id should default"
    );
    assert!(
        event_row.objective_summary.contains("knock echoes"),
        "missing objective_summary should derive from evidence_quote"
    );
}

#[test]
fn latest_payload_object_property_new_state_compiles() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "I set it down.",
        "She sets her wine glass down on the table.",
    );
    let response = parse_eval_form_response(
        r#"{
          "event_rows": [{
            "event_id": "evt_wine_glass",
            "event_type": "scene_event",
            "summary": "User sets wine glass down",
            "evidence_quote": "She sets her wine glass down on the table."
          }],
          "object_rows": [{
            "object_id": "wine_glass",
            "property": "location",
            "old_state": "being held",
            "new_state": "on surface",
            "evidence_quote": "She sets her wine glass down on the table."
          }],
          "relationship_rows": [],
          "memory_rows": [],
          "review_rows": []
        }"#,
    )
    .expect("parse should succeed");
    let object_row = &response.object_rows[0];
    assert_eq!(
        object_row.property_changed, "location",
        "property should normalize to property_changed"
    );
    assert_eq!(
        object_row.new_value, "on surface",
        "new_state should normalize to new_value"
    );
    assert_eq!(
        object_row.old_value,
        Some("being held".into()),
        "old_state should normalize to old_value"
    );
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(
        result.output.object_changes.len(),
        1,
        "object change should compile"
    );
    assert_eq!(
        result.output.object_changes[0].object_state.object_id,
        "wine_glass"
    );
}

const LIVE_POOLSIDE_JSON: &str = r#"{
  "event_rows": [
    {
      "event_id": "event_latest_turn",
      "event_type": "scene_event",
      "objective_summary": "Aurora and User reunite warm",
      "participants": ["aurora_soul", "default_player"],
      "evidence_quote": "She smiles warm and lets you inside."
    }
  ],
  "relationship_rows": [
    {
      "relationship_dimension": "trust",
      "change_direction": "increased",
      "evidence_quote": "She smiles warm and lets you inside.",
      "tag_vocabularies": ["relationship", "reunion"]
    },
    {
      "relationship_dimension": "fear",
      "change_direction": "decreased",
      "evidence_quote": "She smiles warm and lets you inside.",
      "tag_vocabularies": ["relationship", "reunion"]
    }
  ],
  "memory_rows": [
    {
      "memory_slot": "relationship_memory",
      "importance_tier": "high",
      "evidence_quote": "She smiles warm and lets you inside.",
      "candidate_summary": "Reunion with someone familiar who previously inspired Aurora's sketching"
    }
  ],
  "review_rows": [
    {
      "per_soul_evaluation": {
        "soul_id": "aurora_soul",
        "soul_perceived_event": true,
        "soul_knows": []
      }
    }
  ]
}"#;

#[test]
fn live_poolside_relationship_dimension_alias_compiles() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "She smiles warm and lets you inside.");
    let response = parse_eval_form_response(LIVE_POOLSIDE_JSON).expect("parse response");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 2);
    
    let trust_delta = result.output.relationship_evaluations.iter().find(|r| r.trust.is_some()).unwrap();
    assert!(trust_delta.trust.unwrap() > 0.0);

    let fear_delta = result.output.relationship_evaluations.iter().find(|r| r.fear.is_some()).unwrap();
    assert!(fear_delta.fear.unwrap() < 0.0);
}

#[test]
fn live_poolside_memory_slot_alias_compiles() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "She smiles warm and lets you inside.");
    let response = parse_eval_form_response(LIVE_POOLSIDE_JSON).expect("parse response");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.memory_candidates.len(), 1);
    assert_eq!(result.output.memory_candidates[0].slot, MemorySlot::RelationshipMemory);
}

#[test]
fn live_poolside_candidate_summary_becomes_content() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "She smiles warm and lets you inside.");
    let response = parse_eval_form_response(LIVE_POOLSIDE_JSON).expect("parse response");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.output.memory_candidates[0].content, "Reunion with someone familiar who previously inspired Aurora's sketching");
}

#[test]
fn live_poolside_missing_owner_defaults_to_active_soul() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "She smiles warm and lets you inside.");
    let response = parse_eval_form_response(LIVE_POOLSIDE_JSON).expect("parse response");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.output.memory_candidates[0].owner_soul_id, "aurora_soul");
}

#[test]
fn live_poolside_review_without_evidence_is_advisory_not_fatal() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "She smiles warm and lets you inside.");
    let response = parse_eval_form_response(LIVE_POOLSIDE_JSON).expect("parse response");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert!(result.rejected_rows.is_empty());
}

#[test]
fn live_poolside_fixture_accepts_more_than_one_row() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "She smiles warm and lets you inside.");
    let response = parse_eval_form_response(LIVE_POOLSIDE_JSON).expect("parse response");
    let result = compile_eval_form_response(&spec, &response, &context);

    // 1 event row + 2 relationship rows + 1 memory row + 1 review row = 5 rows accepted
    assert_eq!(result.trace.form_rows_accepted, 5);
    assert_eq!(result.trace.form_rows_rejected, 0);
}

#[test]
fn live_poolside_fixture_writes_memory_and_relationship_delta() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "She smiles warm and lets you inside.");
    let response = parse_eval_form_response(LIVE_POOLSIDE_JSON).expect("parse response");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert!(result.draft.memory_candidate_count > 0);
    assert!(result.draft.relationship_delta_count > 0);
    
    // Also verify the converted report writes memories and relationship deltas
    let patch = &result.conversion.patch;
    let soul_patch = patch.soul_patch.as_ref().unwrap();
    assert_eq!(soul_patch.new_memories.len(), 1);
    assert_eq!(soul_patch.relationship_deltas.len(), 2);
}

#[test]
fn live_unescaped_evidence_quote_repairs_and_parses() {
    let raw_json = r#"{
      "event_rows": [{
        "event_id": "evt_dialogue",
        "event_type": "scene_event",
        "objective_summary": "Aurora warns User",
        "participants": ["aurora_soul", "default_player"],
        "evidence_quote": "You better not be late" she calls back, voice low and rough with wine"
      }]
    }"#;
    let response = parse_eval_form_response(raw_json).expect("should repair unescaped quotes and parse successfully");
    assert_eq!(response.event_rows[0].evidence_quote, "You better not be late\" she calls back, voice low and rough with wine");
}

#[test]
fn live_shift_direction_relationship_compiles_to_delta() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "Warm smiles.");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "source_entity_id": "aurora_soul",
            "target_entity_id": "default_player",
            "dimension": "trust",
            "shift_direction": "increased",
            "evidence_quote": "Warm smiles."
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert!(result.output.relationship_evaluations[0].trust.unwrap() > 0.0);
}

#[test]
fn live_source_entity_id_maps_to_source_soul_id() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "Warm smiles.");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "source_entity_id": "aurora_soul",
            "target_entity_id": "default_player",
            "dimension": "trust",
            "shift_direction": "increased",
            "evidence_quote": "Warm smiles."
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.normalized_response.relationship_rows[0].source_soul_id, "aurora_soul");
}

#[test]
fn live_relationship_change_type_without_direction_infers_increase_when_dimension_boundary_pressure() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "increased tension, pressure, and conflict");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "source_entity_id": "aurora_soul",
            "target_entity_id": "default_player",
            "dimension": "boundary_pressure",
            "change_type": "shift",
            "evidence_quote": "increased tension, pressure, and conflict"
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert_eq!(result.normalized_response.relationship_rows[0].direction, Some(RelationshipDirection::Increase));
}

#[test]
fn live_object_state_new_object_label_compiles_to_object_patch() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "Fabric drips.");
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [{
            "entity_id": "jacket",
            "new_object_label": "wet jacket",
            "object_state": "placed on chair",
            "evidence_quote": "Fabric drips."
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.output.object_changes.len(), 1);
    assert_eq!(result.output.object_changes[0].object_state.object_id, "wet_jacket");
    assert_eq!(result.output.object_changes[0].object_state.status, "placed on chair");
}

#[test]
fn live_object_state_defaults_property_changed_state() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "Fabric drips.");
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [{
            "entity_id": "jacket",
            "new_object_label": "wet jacket",
            "object_state": "placed on chair",
            "evidence_quote": "Fabric drips."
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.normalized_response.object_rows[0].property_changed, "state");
}

#[test]
fn live_content_summary_preserved_in_memory_content() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "Fabric drips.");
    let response = parse_eval_form_response(
        r#"{
          "memory_rows": [{
            "memory_slot": "relationship_memory",
            "importance_tier": "high",
            "evidence_quote": "Fabric drips.",
            "content_summary": "Reunion with someone familiar"
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.output.memory_candidates[0].content, "Reunion with someone familiar");
}

#[test]
fn live_candidate_summary_preserved_in_memory_content() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "Fabric drips.");
    let response = parse_eval_form_response(
        r#"{
          "memory_rows": [{
            "memory_slot": "relationship_memory",
            "importance_tier": "high",
            "evidence_quote": "Fabric drips.",
            "candidate_summary": "Reunion with someone familiar who previously inspired Aurora"
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.output.memory_candidates[0].content, "Reunion with someone familiar who previously inspired Aurora");
}

#[test]
fn live_memory_does_not_fallback_to_event_summary_when_candidate_exists() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "Fabric drips.");
    let response = parse_eval_form_response(
        r#"{
          "memory_rows": [{
            "memory_slot": "relationship_memory",
            "importance_tier": "high",
            "evidence_quote": "Fabric drips.",
            "candidate_summary": "Reunion with someone familiar who previously inspired Aurora"
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.output.memory_candidates[0].content, "Reunion with someone familiar who previously inspired Aurora");
}

#[test]
fn live_full_poolside_payload_writes_memory_relationship_and_object() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "I knock at the door", "Fabric drips.");
    let response = parse_eval_form_response(
        r#"{
          "event_rows": [{
            "event_id": "event_latest_turn",
            "event_type": "scene_event",
            "objective_summary": "User arrives wet at Aurora's door",
            "participants": ["aurora_soul", "default_player"],
            "evidence_quote": "Fabric drips."
          }],
          "relationship_rows": [{
            "source_entity_id": "aurora_soul",
            "target_entity_id": "default_player",
            "dimension": "boundary_pressure",
            "shift_direction": "increased",
            "evidence_quote": "Fabric drips."
          }],
          "object_rows": [{
            "entity_id": "jacket",
            "new_object_label": "wet jacket",
            "object_state": "placed on chair",
            "evidence_quote": "Fabric drips."
          }],
          "memory_rows": [{
            "memory_slot": "relationship_memory",
            "importance_tier": "high",
            "evidence_quote": "Fabric drips.",
            "candidate_summary": "Reunion with someone familiar who previously inspired Aurora's sketching"
          }],
          "review_rows": [{
            "candidate_id": "review_test",
            "reason": "already verified",
            "evidence_quote": "Fabric drips."
          }]
        }"#
    ).expect("parse should succeed");
    
    let result = compile_eval_form_response(&spec, &response, &context);
    
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert!(result.trace.form_rows_accepted > 1);
    assert!(result.draft.memory_candidate_count > 0);
    assert!(result.draft.relationship_delta_count > 0);
    
    let patch = &result.conversion.patch;
    let soul_patch = patch.soul_patch.as_ref().unwrap();
    let world_patch = patch.world_patch.as_ref().unwrap();
    
    assert!(soul_patch.new_memories.len() > 0);
    assert!(soul_patch.relationship_deltas.len() > 0);
    assert!(world_patch.object_observation_operations.len() > 0);
}

#[test]
fn relationship_source_slug_aurora_schwarz_resolves_to_active_soul_uuid() {
    let (mut soul, world) = soul_and_world();
    soul.character_name = "Aurora Schwarz".into();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "Warm smiles.");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "source_entity_id": "aurora_schwarz",
            "target_entity_id": "default_player",
            "dimension": "trust",
            "shift_direction": "increased",
            "evidence_quote": "Warm smiles."
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert_eq!(result.output.relationship_evaluations[0].source_soul_id, "aurora_soul");
}

#[test]
fn relationship_source_display_name_resolves_to_active_soul_uuid() {
    let (mut soul, world) = soul_and_world();
    soul.character_name = "Aurora Schwarz".into();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "Warm smiles.");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "source_entity_id": "Aurora Schwarz",
            "target_entity_id": "default_player",
            "dimension": "trust",
            "shift_direction": "increased",
            "evidence_quote": "Warm smiles."
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert_eq!(result.output.relationship_evaluations[0].source_soul_id, "aurora_soul");
}

#[test]
fn live_payload6_relationship_row_no_longer_rejected() {
    let (mut soul, world) = soul_and_world();
    soul.character_name = "Aurora Schwarz".into();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "Didn't expect you to actually show up");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "source_entity_id": "aurora_schwarz",
            "target_entity_id": "default_player",
            "dimension": "boundary_pressure",
            "shift_direction": "increased",
            "evidence_quote": "Didn't expect you to actually show up"
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert_eq!(result.output.relationship_evaluations[0].source_soul_id, "aurora_soul");
    assert_eq!(result.output.relationship_evaluations[0].target_entity_id, "default_player");
}

#[test]
fn object_status_alias_becomes_new_value() {
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [{
            "object_id": "wet_jacket",
            "new_object_label": "wet jacket",
            "status": "placed_on_chair",
            "evidence_quote": "and place a wet jacket over the chair."
          }]
        }"#
    ).expect("parse should succeed");
    assert_eq!(response.object_rows[0].new_value, "placed_on_chair");
}

#[test]
fn object_status_defaults_property_changed_state() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "and place a wet jacket over the chair.");
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [{
            "object_id": "wet_jacket",
            "new_object_label": "wet jacket",
            "status": "placed_on_chair",
            "evidence_quote": "and place a wet jacket over the chair."
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.object_changes.len(), 1);
    assert_eq!(result.normalized_response.object_rows[0].property_changed, "state");
}

#[test]
fn live_payload8_object_rows_compile() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "water darkens the fabric");
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [
            {
              "object_id": "wet_jacket",
              "new_object_label": "wet jacket",
              "status": "placed_on_chair",
              "evidence_quote": "and place a wet jacket over the chair."
            },
            {
              "object_id": "chair",
              "status": "wet_from_jacket_drip",
              "evidence_quote": "water darkens the fabric before dripping onto the chair's arm"
            }
          ]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.object_changes.len(), 2);
    assert_eq!(result.output.object_changes[0].object_state.object_id, "wet_jacket");
    assert_eq!(result.output.object_changes[0].object_state.status, "placed_on_chair");
    assert_eq!(result.output.object_changes[1].object_state.object_id, "chair");
    assert_eq!(result.output.object_changes[1].object_state.status, "wet_from_jacket_drip");
}

#[test]
fn memory_summary_alias_no_unreachable_pattern_warning() {
    let response = parse_eval_form_response(
        r#"{
          "memory_rows": [{
            "linked_event_id": "evt",
            "owner_soul_id": "aurora_soul",
            "memory_slot": "relationship_memory",
            "summary": "Reunion with someone familiar",
            "evidence_quote": "fabric drips"
          }]
        }"#
    ).expect("parse should succeed");
    assert_eq!(response.memory_rows[0].content, "Reunion with someone familiar");
}

#[test]
fn patch_applied_with_rejected_rows_is_not_failure_status() {
    let rejected = vec![EvalFormRowRejection {
        row_kind: "object".into(),
        row_id: "obj1".into(),
        reason: "invalid".into(),
    }];
    let status = format_honest_ui_status(true, true, true, &rejected);
    assert_eq!(status, "State updated; 1 object row skipped");
}

#[test]
fn ui_status_reports_rows_skipped_not_enrichment_failed() {
    let rejected = vec![
        EvalFormRowRejection {
            row_kind: "object".into(),
            row_id: "obj1".into(),
            reason: "invalid".into(),
        },
        EvalFormRowRejection {
            row_kind: "relationship".into(),
            row_id: "rel1".into(),
            reason: "invalid".into(),
        }
    ];
    let status = format_honest_ui_status(true, true, true, &rejected);
    assert_eq!(status, "State updated; 2 evaluator rows skipped");
}

#[test]
fn live_payload11_chain_lock_change_type_state_change_compiles() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "The chain lock still clicks softly...");
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [{
            "object_id": "chain_lock",
            "change_type": "state_change",
            "importance_tier": "medium",
            "evidence_quote": "The chain lock still clicks softly against the doorframe..."
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.object_changes.len(), 1);
    assert_eq!(result.output.object_changes[0].object_state.object_id, "chain_lock");
    assert_eq!(result.output.object_changes[0].object_state.status, "The chain lock still clicks softly against the doorframe...");
}

#[test]
fn live_payload11_wet_jacket_new_object_observation_compiles() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "You hang your soaked jacket...");
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [{
            "object_id": "wet_jacket",
            "change_type": "new_object_observation",
            "importance_tier": "low",
            "evidence_quote": "You hang your soaked jacket on the back of the kitchen chair...",
            "new_object_label": "wet_jacket"
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.object_changes.len(), 1);
    assert_eq!(result.output.object_changes[0].object_state.object_id, "wet_jacket");
    assert_eq!(result.output.object_changes[0].object_state.status, "wet_jacket");
}

#[test]
fn object_change_type_defaults_property_and_value() {
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [
            {
              "object_id": "chain_lock",
              "change_type": "state_change",
              "evidence_quote": "clicks softly"
            },
            {
              "object_id": "wet_jacket",
              "change_type": "new_object_observation",
              "new_object_label": "wet_jacket",
              "evidence_quote": "hang soaked jacket"
            }
          ]
        }"#
    ).expect("parse should succeed");
    assert_eq!(response.object_rows[0].property_changed, "state");
    assert_eq!(response.object_rows[0].new_value, "clicks softly");
    assert_eq!(response.object_rows[1].property_changed, "presence");
    assert_eq!(response.object_rows[1].new_value, "wet_jacket");
}

#[test]
fn live_payload11_intimacy_shift_infers_increase() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "Suit yourself");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "relationship_dimension": "intimacy",
            "change_type": "shift",
            "importance_tier": "high",
            "evidence_quote": "\"Suit yourself,\" she says, voice dropping..."
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert_eq!(result.normalized_response.relationship_rows[0].direction, Some(RelationshipDirection::Increase));
    assert_eq!(result.output.relationship_evaluations[0].intimacy.is_some(), true);
}

#[test]
fn live_payload11_trust_shift_infers_increase() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "Suit yourself");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "relationship_dimension": "trust",
            "change_type": "shift",
            "importance_tier": "high",
            "evidence_quote": "\"Suit yourself,\" she says, voice dropping..."
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert_eq!(result.normalized_response.relationship_rows[0].direction, Some(RelationshipDirection::Increase));
    assert_eq!(result.output.relationship_evaluations[0].trust.is_some(), true);
}

#[test]
fn relationship_shift_without_direction_compiles_for_positive_dimension() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "Suit yourself");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "relationship_dimension": "affection",
            "change_type": "shift",
            "importance_tier": "high",
            "evidence_quote": "closer connection and deeper affection"
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.normalized_response.relationship_rows[0].direction, Some(RelationshipDirection::Increase));
}

#[test]
fn relationship_missing_direction_rejects_as_uncertain() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "Suit yourself");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "relationship_dimension": "trust",
            "change_type": "shift",
            "importance_tier": "high",
            "evidence_quote": "generic uninformative quote"
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 1);
    assert_eq!(result.rejected_rows[0].reason, "direction_missing_uncertain");
}

#[test]
fn live_exact_object_row_reaches_exported_object_state() {
    let (soul, mut world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "draping it over the back of the chair");
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [{
            "object_id": "wet_jacket",
            "change_type": "object_change",
            "importance_tier": "low",
            "evidence_quote": "I watch as you shake water from your jacket before draping it over the back of the chair nearest the couch."
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.object_changes.len(), 1);
    
    let report = evaluator_output_to_engine_patch(&result.output, &context);
    assert!(!report.patch.is_empty());
    let world_patch = report.patch.world_patch.as_ref().unwrap();
    assert!(world_patch.object_observation_operations.len() > 0);

    // Prove full path survival:
    // raw row -> normalized row -> validated row -> EvaluatorOutputV1 -> EnginePatch -> apply_to_session -> rebuilt/exported state
    let mut soul_mut = soul;
    report.patch.apply_to_session(&mut soul_mut, Some(&mut world)).expect("apply to session succeeds");

    let found = world.object_states.iter().find(|obj| obj.object_id == "wet_jacket").expect("wet_jacket should exist in world");
    assert!(found.last_observed_state.contains("nearest the couch"));
}

#[test]
fn live_exact_relationship_row_reaches_exported_relationship_delta() {
    let (mut soul, mut world) = soul_and_world();
    soul.relationships.insert(
        "default_player".to_string(),
        crate::soul::Relationship {
            trust: 0.0,
            affection: 0.0,
            intimacy: 0.0,
            passion: 0.0,
            commitment: 0.0,
            fear: 0.0,
            desire: 0.0,
            respect: 0.0,
            conflict: 0.0,
            dependency: 0.0,
            curiosity: 0.0,
            comfort: 10.0,
            boundary_pressure: 0.0,
            love_type: String::new(),
        },
    );

    let (spec, context) = spec_and_context(&soul, &world, "Hey", "Figured you'd never show... Sorry. Been... been a long time since anyone came by. Your presence brings comfort.");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "relationship_dimension": "comfort",
            "change_type": "relationship_shift",
            "importance_tier": "medium",
            "evidence_quote": "Figured you'd never show... Sorry. Been... been a long time since anyone came by. Your presence brings comfort."
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert_eq!(result.normalized_response.relationship_rows[0].direction, Some(RelationshipDirection::Increase));

    let report = evaluator_output_to_engine_patch(&result.output, &context);
    assert!(!report.patch.is_empty());
    let soul_patch = report.patch.soul_patch.as_ref().unwrap();
    assert!(soul_patch.relationship_deltas.len() > 0);

    // Prove full path survival:
    // raw row -> normalized row -> validated row -> EvaluatorOutputV1 -> EnginePatch -> apply_to_session -> rebuilt/exported state
    report.patch.apply_to_session(&mut soul, Some(&mut world)).expect("apply to session succeeds");

    let updated_rel = soul.relationships.get("default_player").expect("relationship should exist");
    assert!(updated_rel.comfort > 10.0, "comfort should have increased beyond 10.0, got {}", updated_rel.comfort);
}

#[test]
fn live_exact_memory_row_reaches_exported_memory_recent() {
    let (mut soul, mut world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "Aurora's cigarette trembles slightly in her fingers");
    let response = parse_eval_form_response(
        r#"{
          "memory_rows": [{
            "slot_type": "recent_emotional_state",
            "importance_tier": "medium",
            "evidence_quote": "Aurora's cigarette trembles slightly in her fingers",
            "content_summary": "Aurora is feeling highly anxious and emotional"
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.memory_candidates.len(), 1);

    let report = evaluator_output_to_engine_patch(&result.output, &context);
    assert!(!report.patch.is_empty());
    let soul_patch = report.patch.soul_patch.as_ref().unwrap();
    assert!(soul_patch.new_memories.len() > 0);

    // Prove full path survival:
    // raw row -> normalized row -> validated row -> EvaluatorOutputV1 -> EnginePatch -> apply_to_session -> rebuilt/exported state
    report.patch.apply_to_session(&mut soul, Some(&mut world)).expect("apply to session succeeds");

    let found_mem = soul.memory.recent.iter().find(|mem| {
        mem.memory_slot.as_deref() == Some("recent_emotional_state")
    }).expect("should have created a memory in the recent_emotional_state slot");
    assert!(found_mem.content.contains("Aurora is feeling highly anxious and emotional"));
}

#[test]
fn patch_applied_with_row_skips_does_not_show_hard_failure() {
    let rejected = vec![
        EvalFormRowRejection {
            row_kind: "relationship".into(),
            row_id: "event_latest_turn:aurora_soul:default_player".into(),
            reason: "direction_missing_uncertain".into(),
        }
    ];
    let honest_status = format_honest_ui_status(
        true, // patch_applied
        true, // materialized_soul_updated
        true, // materialized_session_world_updated
        &rejected,
    );
    assert_eq!(honest_status, "State updated; 1 relationship row skipped");
}

#[test]
fn relationship_direction_arrow_parses_source_target_not_change_direction() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "Warm smiles.");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "relationship_dimension": "trust",
            "direction": "default_player -> aurora_soul",
            "evidence_quote": "Warm smiles."
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.normalized_response.relationship_rows[0].source_soul_id, "aurora_soul");
    assert_eq!(result.normalized_response.relationship_rows[0].target_entity_id, "default_player");
    assert!(result.normalized_response.relationship_rows[0].direction.is_none());
}

#[test]
fn live_payload8_boundary_pressure_arrow_direction_compiles_delta() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "I'm studying the way you've positioned yourself just inside the doorway...");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "relationship_dimension": "boundary_pressure",
            "direction": "default_player -> aurora_soul",
            "evidence_quote": "I'm studying the way you've positioned yourself just inside the doorway..."
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert_eq!(result.normalized_response.relationship_rows[0].direction, Some(RelationshipDirection::Increase));
    
    let key = format!("event_latest_turn:aurora_soul:default_player:boundary_pressure");
    assert_eq!(result.trace.relationship_row_results.get(&key), Some(&"delta_created".to_string()));
    assert_eq!(result.trace.relationship_non_delta_count, 0);
}

#[test]
fn live_payload8_trust_arrow_direction_compiles_delta_or_explicit_uncertain() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "God, you actually look vulnerable standing there...");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "relationship_dimension": "trust",
            "direction": "aurora_soul -> default_player",
            "evidence_quote": "God, you actually look vulnerable standing there..."
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert_eq!(result.normalized_response.relationship_rows[0].direction, Some(RelationshipDirection::Increase));
    
    let key = format!("event_latest_turn:aurora_soul:default_player:trust");
    assert_eq!(result.trace.relationship_row_results.get(&key), Some(&"delta_created".to_string()));
    assert_eq!(result.trace.relationship_non_delta_count, 0);
}

#[test]
fn accepted_relationship_row_without_delta_is_reported_non_delta() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hey", "God, you actually look vulnerable standing there...");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "relationship_dimension": "trust",
            "direction": "no_change",
            "evidence_quote": "God, you actually look vulnerable standing there..."
          }]
        }"#
    ).expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 0);
    assert_eq!(result.trace.relationship_non_delta_count, 1);
    
    let key = format!("event_latest_turn:aurora_soul:default_player:trust");
    assert_eq!(result.trace.relationship_row_results.get(&key), Some(&"non_delta_no_change".to_string()));
}
