use crate::soul::SpeechAct;
use std::collections::HashMap;

use super::*;
use crate::{
    evaluator::{
        evaluator_output_to_engine_patch, turn_flags, EvaluatorConversionContext, MemorySlot,
        RelationshipEvaluation,
    },
    setting::{session_world_from_legacy_world, SessionWorld},
    soul::{new_default_soul, MemoryEntry, MemorySourceType, ObjectState, Soul, TruthStatus},
};

fn soul_and_world() -> (Soul, SessionWorld) {
    let mut soul = new_default_soul("Aurora");
    soul.character_id = "aurora_soul".into();
    soul.memory.recent.push(MemoryEntry {
        archived: false,
        is_pinned: false,
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
        source_quote: None,
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
        speech_act: SpeechAct::Unspecified,
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
    let (spec, context) = spec_and_context(&soul, &world, "I walk in.", "The visitor walks in.");
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
    let (spec, context) = spec_and_context(&soul, &world, "I walk in.", "The visitor walks in.");
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
    let (spec, context) = spec_and_context(&soul, &world, "I walk in.", "The visitor walks in.");
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
    let (spec, context) = spec_and_context(&soul, &world, "I walk in.", "The visitor walks in.");
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
    let (spec, context) = spec_and_context(&soul, &world, "I walk in.", "The visitor walks in.");
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
    let (spec, context) = spec_and_context(&soul, &world, "I walk in.", "The visitor walks in.");
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
    let (_spec, _context) = spec_and_context(
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
    let (spec, context) =
        spec_and_context(&soul, &world, "Hey", "She smiles warm and lets you inside.");
    let response = parse_eval_form_response(LIVE_POOLSIDE_JSON).expect("parse response");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 2);

    let trust_delta = result
        .output
        .relationship_evaluations
        .iter()
        .find(|r| r.trust.is_some())
        .unwrap();
    assert!(trust_delta.trust.unwrap() > 0.0);

    let fear_delta = result
        .output
        .relationship_evaluations
        .iter()
        .find(|r| r.fear.is_some())
        .unwrap();
    assert!(fear_delta.fear.unwrap() < 0.0);
}

#[test]
fn live_poolside_memory_slot_alias_compiles() {
    let (soul, world) = soul_and_world();
    let (spec, context) =
        spec_and_context(&soul, &world, "Hey", "She smiles warm and lets you inside.");
    let response = parse_eval_form_response(LIVE_POOLSIDE_JSON).expect("parse response");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.memory_candidates.len(), 1);
    assert_eq!(
        result.output.memory_candidates[0].slot,
        MemorySlot::RelationshipMemory
    );
}

#[test]
fn live_poolside_candidate_summary_becomes_content() {
    let (soul, world) = soul_and_world();
    let (spec, context) =
        spec_and_context(&soul, &world, "Hey", "She smiles warm and lets you inside.");
    let response = parse_eval_form_response(LIVE_POOLSIDE_JSON).expect("parse response");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(
        result.output.memory_candidates[0].content,
        "Reunion with someone familiar who previously inspired Aurora's sketching"
    );
}

#[test]
fn live_poolside_missing_owner_defaults_to_active_soul() {
    let (soul, world) = soul_and_world();
    let (spec, context) =
        spec_and_context(&soul, &world, "Hey", "She smiles warm and lets you inside.");
    let response = parse_eval_form_response(LIVE_POOLSIDE_JSON).expect("parse response");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(
        result.output.memory_candidates[0].owner_soul_id,
        "aurora_soul"
    );
}

#[test]
fn live_poolside_review_without_evidence_is_advisory_not_fatal() {
    let (soul, world) = soul_and_world();
    let (spec, context) =
        spec_and_context(&soul, &world, "Hey", "She smiles warm and lets you inside.");
    let response = parse_eval_form_response(LIVE_POOLSIDE_JSON).expect("parse response");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert!(result.rejected_rows.is_empty());
}

#[test]
fn live_poolside_fixture_accepts_more_than_one_row() {
    let (soul, world) = soul_and_world();
    let (spec, context) =
        spec_and_context(&soul, &world, "Hey", "She smiles warm and lets you inside.");
    let response = parse_eval_form_response(LIVE_POOLSIDE_JSON).expect("parse response");
    let result = compile_eval_form_response(&spec, &response, &context);

    // 1 event row + 2 relationship rows + 1 memory row + 1 review row = 5 rows accepted
    assert_eq!(result.trace.form_rows_accepted, 5);
    assert_eq!(result.trace.form_rows_rejected, 0);
}

#[test]
fn live_poolside_fixture_writes_memory_and_relationship_delta() {
    let (soul, world) = soul_and_world();
    let (spec, context) =
        spec_and_context(&soul, &world, "Hey", "She smiles warm and lets you inside.");
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
    let response = parse_eval_form_response(raw_json)
        .expect("should repair unescaped quotes and parse successfully");
    assert_eq!(
        response.event_rows[0].evidence_quote,
        "You better not be late\" she calls back, voice low and rough with wine"
    );
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
        }"#,
    )
    .expect("parse should succeed");
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
        }"#,
    )
    .expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(
        result.normalized_response.relationship_rows[0].source_soul_id,
        "aurora_soul"
    );
}

#[test]
fn live_relationship_change_type_without_direction_infers_increase_when_dimension_boundary_pressure(
) {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "Hey",
        "increased tension, pressure, and conflict",
    );
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "source_entity_id": "aurora_soul",
            "target_entity_id": "default_player",
            "dimension": "boundary_pressure",
            "change_type": "shift",
            "evidence_quote": "increased tension, pressure, and conflict"
          }]
        }"#,
    )
    .expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert_eq!(
        result.normalized_response.relationship_rows[0].direction,
        Some(RelationshipDirection::Increase)
    );
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
        }"#,
    )
    .expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.output.object_changes.len(), 1);
    assert_eq!(
        result.output.object_changes[0].object_state.object_id,
        "unknown_jacket_1"
    );
    assert_eq!(
        result.output.object_changes[0].object_state.object_kind,
        "jacket"
    );
    assert_eq!(result.output.object_changes[0].object_state.status, "wet");
    assert_eq!(
        result.output.object_changes[0].object_state.location,
        "chair"
    );
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
        }"#,
    )
    .expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(
        result.normalized_response.object_rows[0].property_changed,
        "state"
    );
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
        }"#,
    )
    .expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(
        result.output.memory_candidates[0].content,
        "Reunion with someone familiar"
    );
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
        }"#,
    )
    .expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(
        result.output.memory_candidates[0].content,
        "Reunion with someone familiar who previously inspired Aurora"
    );
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
        }"#,
    )
    .expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(
        result.output.memory_candidates[0].content,
        "Reunion with someone familiar who previously inspired Aurora"
    );
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
        }"#,
    )
    .expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert_eq!(
        result.output.relationship_evaluations[0].source_soul_id,
        "aurora_soul"
    );
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
        }"#,
    )
    .expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert_eq!(
        result.output.relationship_evaluations[0].source_soul_id,
        "aurora_soul"
    );
}

#[test]
fn live_payload6_relationship_row_no_longer_rejected() {
    let (mut soul, world) = soul_and_world();
    soul.character_name = "Aurora Schwarz".into();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "Hey",
        "Didn't expect you to actually show up",
    );
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "source_entity_id": "aurora_schwarz",
            "target_entity_id": "default_player",
            "dimension": "boundary_pressure",
            "shift_direction": "increased",
            "evidence_quote": "Didn't expect you to actually show up"
          }]
        }"#,
    )
    .expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
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
fn object_status_alias_becomes_new_value() {
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [{
            "object_id": "wet_jacket",
            "new_object_label": "wet jacket",
            "status": "placed_on_chair",
            "evidence_quote": "and place a wet jacket over the chair."
          }]
        }"#,
    )
    .expect("parse should succeed");
    assert_eq!(response.object_rows[0].new_value, "placed_on_chair");
}

#[test]
fn object_status_defaults_property_changed_state() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "Hey",
        "and place a wet jacket over the chair.",
    );
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [{
            "object_id": "wet_jacket",
            "new_object_label": "wet jacket",
            "status": "placed_on_chair",
            "evidence_quote": "and place a wet jacket over the chair."
          }]
        }"#,
    )
    .expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.object_changes.len(), 1);
    assert_eq!(
        result.normalized_response.object_rows[0].property_changed,
        "state"
    );
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
        }"#,
    )
    .expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.object_changes.len(), 2);
    assert_eq!(
        result.output.object_changes[0].object_state.object_id,
        "unknown_jacket_1"
    );
    assert_eq!(result.output.object_changes[0].object_state.status, "wet");
    assert_eq!(
        result.output.object_changes[1].object_state.object_id,
        "unknown_chair_1"
    );
    assert_eq!(result.output.object_changes[1].object_state.status, "wet");
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
        }"#,
    )
    .expect("parse should succeed");
    assert_eq!(
        response.memory_rows[0].content,
        "Reunion with someone familiar"
    );
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
        },
    ];
    let status = format_honest_ui_status(true, true, true, &rejected);
    assert_eq!(status, "State updated; 2 evaluator rows skipped");
}

#[test]
fn live_payload11_chain_lock_change_type_state_change_compiles() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "Hey",
        "The chain lock still clicks softly...",
    );
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [{
            "object_id": "chain_lock",
            "change_type": "state_change",
            "importance_tier": "medium",
            "evidence_quote": "The chain lock still clicks softly against the doorframe..."
          }]
        }"#,
    )
    .expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.object_changes.len(), 1);
    assert_eq!(
        result.output.object_changes[0].object_state.object_id,
        "chain_lock"
    );
    assert_eq!(
        result.output.object_changes[0].object_state.status,
        "The chain lock still clicks softly against the doorframe..."
    );
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
        }"#,
    )
    .expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.object_changes.len(), 1);
    assert_eq!(
        result.output.object_changes[0].object_state.object_id,
        "default_player_jacket_1"
    );
    assert_eq!(
        result.output.object_changes[0].object_state.object_kind,
        "jacket"
    );
    assert_eq!(
        result.output.object_changes[0]
            .object_state
            .owner_entity_id
            .as_deref(),
        Some("default_player")
    );
    assert_eq!(result.output.object_changes[0].object_state.status, "wet");
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
        }"#,
    )
    .expect("parse should succeed");
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
        }"#,
    )
    .expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert_eq!(
        result.normalized_response.relationship_rows[0].direction,
        Some(RelationshipDirection::Increase)
    );
    assert_eq!(
        result.output.relationship_evaluations[0].intimacy.is_some(),
        true
    );
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
        }"#,
    )
    .expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert_eq!(
        result.normalized_response.relationship_rows[0].direction,
        Some(RelationshipDirection::Increase)
    );
    assert_eq!(
        result.output.relationship_evaluations[0].trust.is_some(),
        true
    );
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
        }"#,
    )
    .expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(
        result.normalized_response.relationship_rows[0].direction,
        Some(RelationshipDirection::Increase)
    );
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
        }"#,
    )
    .expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 1);
    assert_eq!(
        result.rejected_rows[0].reason,
        "direction_missing_uncertain"
    );
}

#[test]
fn live_exact_object_row_reaches_exported_object_state() {
    let (soul, mut world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "Hey",
        "draping it over the back of the chair",
    );
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
    report
        .patch
        .apply_to_session(&mut soul_mut, Some(&mut world))
        .expect("apply to session succeeds");

    let found = world
        .object_states
        .iter()
        .find(|obj| obj.object_id == "default_player_jacket_1")
        .expect("stable player jacket should exist in world");
    assert_eq!(found.object_kind, "jacket");
    assert_eq!(found.status, "wet");
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
            ..crate::soul::Relationship::default()
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
    assert_eq!(
        result.normalized_response.relationship_rows[0].direction,
        Some(RelationshipDirection::Increase)
    );

    let report = evaluator_output_to_engine_patch(&result.output, &context);
    assert!(!report.patch.is_empty());
    let soul_patch = report.patch.soul_patch.as_ref().unwrap();
    assert!(soul_patch.relationship_deltas.len() > 0);

    // Prove full path survival:
    // raw row -> normalized row -> validated row -> EvaluatorOutputV1 -> EnginePatch -> apply_to_session -> rebuilt/exported state
    report
        .patch
        .apply_to_session(&mut soul, Some(&mut world))
        .expect("apply to session succeeds");

    let updated_rel = soul
        .relationships
        .get("default_player")
        .expect("relationship should exist");
    assert!(
        updated_rel.comfort > 10.0,
        "comfort should have increased beyond 10.0, got {}",
        updated_rel.comfort
    );
}

#[test]
fn live_exact_memory_row_reaches_exported_memory_recent() {
    let (mut soul, mut world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "Hey",
        "Aurora's cigarette trembles slightly in her fingers",
    );
    let response = parse_eval_form_response(
        r#"{
          "memory_rows": [{
            "slot_type": "recent_emotional_state",
            "importance_tier": "medium",
            "evidence_quote": "Aurora's cigarette trembles slightly in her fingers",
            "content_summary": "Aurora is feeling highly anxious and emotional"
          }]
        }"#,
    )
    .expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.memory_candidates.len(), 1);

    let report = evaluator_output_to_engine_patch(&result.output, &context);
    assert!(!report.patch.is_empty());
    let soul_patch = report.patch.soul_patch.as_ref().unwrap();
    assert!(soul_patch.new_memories.len() > 0);

    // Prove full path survival:
    // raw row -> normalized row -> validated row -> EvaluatorOutputV1 -> EnginePatch -> apply_to_session -> rebuilt/exported state
    report
        .patch
        .apply_to_session(&mut soul, Some(&mut world))
        .expect("apply to session succeeds");

    let found_mem = soul
        .memory
        .recent
        .iter()
        .find(|mem| mem.memory_slot.as_deref() == Some("recent_emotional_state"))
        .expect("should have created a memory in the recent_emotional_state slot");
    assert!(found_mem
        .content
        .contains("Aurora is feeling highly anxious and emotional"));
}

#[test]
fn patch_applied_with_row_skips_does_not_show_hard_failure() {
    let rejected = vec![EvalFormRowRejection {
        row_kind: "relationship".into(),
        row_id: "event_latest_turn:aurora_soul:default_player".into(),
        reason: "direction_missing_uncertain".into(),
    }];
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
        }"#,
    )
    .expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(
        result.normalized_response.relationship_rows[0].source_soul_id,
        "aurora_soul"
    );
    assert_eq!(
        result.normalized_response.relationship_rows[0].target_entity_id,
        "default_player"
    );
    assert!(result.normalized_response.relationship_rows[0]
        .direction
        .is_none());
}

#[test]
fn live_payload8_boundary_pressure_arrow_direction_compiles_delta() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "Hey",
        "I'm studying the way you've positioned yourself just inside the doorway...",
    );
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
    assert_eq!(
        result.normalized_response.relationship_rows[0].direction,
        Some(RelationshipDirection::Increase)
    );

    let key = format!("event_latest_turn:aurora_soul:default_player:boundary_pressure");
    assert_eq!(
        result.trace.relationship_row_results.get(&key),
        Some(&"delta_created".to_string())
    );
    assert_eq!(result.trace.relationship_non_delta_count, 0);
}

#[test]
fn live_payload8_trust_arrow_direction_compiles_delta_or_explicit_uncertain() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "Hey",
        "God, you actually look vulnerable standing there...",
    );
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "relationship_dimension": "trust",
            "direction": "aurora_soul -> default_player",
            "evidence_quote": "God, you actually look vulnerable standing there..."
          }]
        }"#,
    )
    .expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert_eq!(
        result.normalized_response.relationship_rows[0].direction,
        Some(RelationshipDirection::Increase)
    );

    let key = format!("event_latest_turn:aurora_soul:default_player:trust");
    assert_eq!(
        result.trace.relationship_row_results.get(&key),
        Some(&"delta_created".to_string())
    );
    assert_eq!(result.trace.relationship_non_delta_count, 0);
}

#[test]
fn accepted_relationship_row_without_delta_is_reported_non_delta() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "Hey",
        "God, you actually look vulnerable standing there...",
    );
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "relationship_dimension": "trust",
            "direction": "no_change",
            "evidence_quote": "God, you actually look vulnerable standing there..."
          }]
        }"#,
    )
    .expect("parse should succeed");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 0);
    assert_eq!(result.trace.relationship_non_delta_count, 1);

    let key = format!("event_latest_turn:aurora_soul:default_player:trust");
    assert_eq!(
        result.trace.relationship_row_results.get(&key),
        Some(&"non_delta_no_change".to_string())
    );
}

// ---- Object-row defaulting tests (Gap 1 & Gap 2 fixes) ----

#[test]
fn object_row_empty_new_value_derives_from_evidence_quote() {
    // Gap 1: property_changed = "state" already set, new_value empty.
    // normalize_object_aliases (struct-level, called inside compile) should
    // fill new_value from evidence_quote before the validator runs.
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "I hang the jacket.",
        "The wet jacket drips onto the floor.",
    );
    let response = parse_eval_form_response(
        r#"{
          "event_rows": [{
            "event_id": "jacket_hang",
            "event_type": "scene_event",
            "summary": "The visitor hung up the wet jacket.",
            "participants": ["aurora_soul", "default_player"],
            "evidence_quote": "The wet jacket drips onto the floor."
          }],
          "object_rows": [{
            "linked_event_id": "jacket_hang",
            "object_id": "wet_jacket",
            "property_changed": "state",
            "evidence_quote": "The wet jacket drips onto the floor."
          }]
        }"#,
    )
    .expect("parse should succeed");

    let result = compile_eval_form_response(&spec, &response, &context);

    // Verify struct-level normalization derived new_value
    let norm_row = &result.normalized_response.object_rows[0];
    assert!(
        !norm_row.new_value.trim().is_empty(),
        "new_value should be derived from evidence_quote after normalization, got empty"
    );
    assert_eq!(norm_row.new_value, "The wet jacket drips onto the floor.");

    assert_eq!(
        result.trace.form_rows_rejected, 0,
        "row should pass validation after new_value is derived"
    );
    assert_eq!(result.output.object_changes.len(), 1);
    assert_eq!(
        result.output.object_changes[0].object_state.object_id,
        "unknown_jacket_1"
    );
    // compiler_result in row trace must be object_patch_created
    let obj_trace = result
        .trace
        .evaluator_row_traces
        .iter()
        .find(|t| t.row_kind == "object")
        .expect("object row trace should exist");
    assert_eq!(obj_trace.compiler_result, "object_patch_created");
}

#[test]
fn object_row_missing_id_infers_wet_jacket_from_evidence() {
    // Gap 2: object_id and new_object_label both missing.
    // Evidence contains "wet jacket" → after struct-level normalize_object_aliases
    // the normalized row should have object_id containing "jacket".
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "I hang my jacket.",
        "You hang your soaked jacket on the back of the kitchen chair.",
    );
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [{
            "change_type": "state_change",
            "evidence_quote": "You hang your soaked wet jacket on the back of the kitchen chair."
          }]
        }"#,
    )
    .expect("parse should succeed");

    let result = compile_eval_form_response(&spec, &response, &context);

    // Check normalized row for inferred id
    let norm_row = &result.normalized_response.object_rows[0];
    assert!(
        norm_row.object_id.as_deref().map(|id| id.contains("jacket")).unwrap_or(false)
            || norm_row.new_object_label.as_deref().map(|l| l.contains("jacket")).unwrap_or(false),
        "object_id or new_object_label should contain 'jacket' after normalization, got object_id={:?} label={:?}",
        norm_row.object_id,
        norm_row.new_object_label
    );

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.object_changes.len(), 1);
    let obj_id = &result.output.object_changes[0].object_state.object_id;
    assert!(
        obj_id.contains("jacket"),
        "compiled object_id should contain 'jacket', got {obj_id}"
    );
}

#[test]
fn object_row_missing_id_infers_chair_from_evidence() {
    // Gap 2 with a different noun: evidence mentions "chair".
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "I pull out the chair.",
        "The wooden chair scrapes against the floor.",
    );
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [{
            "change_type": "state_change",
            "evidence_quote": "The wooden chair scrapes against the floor."
          }]
        }"#,
    )
    .expect("parse should succeed");

    let result = compile_eval_form_response(&spec, &response, &context);

    let norm_row = &result.normalized_response.object_rows[0];
    assert!(
        norm_row
            .object_id
            .as_deref()
            .map(|id| id.contains("chair"))
            .unwrap_or(false)
            || norm_row
                .new_object_label
                .as_deref()
                .map(|l| l.contains("chair"))
                .unwrap_or(false),
        "should infer 'chair' after normalization, got object_id={:?} label={:?}",
        norm_row.object_id,
        norm_row.new_object_label
    );

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.object_changes.len(), 1);
    assert!(result.output.object_changes[0]
        .object_state
        .object_id
        .contains("chair"));
}

#[test]
fn object_row_without_physical_object_still_rejects() {
    // Abstract evidence with no known physical noun — must still be rejected.
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "The atmosphere shifts.",
        "Tension fills the room.",
    );
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [{
            "change_type": "state_change",
            "evidence_quote": "Tension fills the room."
          }]
        }"#,
    )
    .expect("parse should succeed");

    let result = compile_eval_form_response(&spec, &response, &context);
    // No physical noun found → object_id still missing → validator rejects
    assert!(
        result.trace.form_rows_rejected > 0 || result.output.object_changes.is_empty(),
        "abstract-only evidence should not produce an object change"
    );
    if !result.rejected_rows.is_empty() {
        assert!(
            result.rejected_rows.iter().any(|r| r.row_kind == "object"),
            "rejection should be for the object row"
        );
    }
}

#[test]
fn live_wet_jacket_chair_rows_create_object_patch() {
    // End-to-end: two minimal object rows (wet jacket + chair) from a real
    // live payload where the LLM omitted object_id and new_value.
    // Both should produce object_patch_created in the row trace.
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "I hang my jacket on the chair.",
        "You drape your soaked wet jacket over the back of the wooden chair.",
    );
    let response = parse_eval_form_response(
        r#"{
          "event_rows": [{
            "event_id": "jacket_placed",
            "event_type": "scene_event",
            "summary": "The visitor draped wet jacket over chair.",
            "participants": ["aurora_soul", "default_player"],
            "evidence_quote": "You drape your soaked wet jacket over the back of the wooden chair."
          }],
          "object_rows": [
            {
              "linked_event_id": "jacket_placed",
              "change_type": "state_change",
              "evidence_quote": "You drape your soaked wet jacket over the back of the wooden chair."
            },
            {
              "linked_event_id": "jacket_placed",
              "change_type": "state_change",
              "evidence_quote": "The wooden chair now has the visitor's wet jacket draped over it."
            }
          ]
        }"#,
    )
    .expect("parse should succeed");

    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(
        result.trace.form_rows_rejected, 0,
        "both rows should pass validation, rejected: {:?}",
        result.rejected_rows
    );
    assert_eq!(
        result.output.object_changes.len(),
        2,
        "expected 2 object patches"
    );

    let object_patch_count = result
        .trace
        .evaluator_row_traces
        .iter()
        .filter(|t| t.row_kind == "object" && t.compiler_result == "object_patch_created")
        .count();
    assert_eq!(
        object_patch_count, 2,
        "both object rows should show compiler_result = object_patch_created"
    );
}

#[test]
fn boundary_pressure_infers_increase_from_door_chain() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Knock", "Door chains");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "source_entity_id": "aurora_soul",
            "target_entity_id": "default_player",
            "dimension": "boundary_pressure",
            "evidence_quote": "keeps the door chained"
          }]
        }"#,
    )
    .expect("parse response");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert_eq!(
        result.normalized_response.relationship_rows[0].direction,
        Some(RelationshipDirection::Increase)
    );
}

#[test]
fn boundary_pressure_infers_increase_from_preparing_for_stranger() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Knock", "Door opens");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "source_entity_id": "aurora_soul",
            "target_entity_id": "default_player",
            "dimension": "boundary_pressure",
            "evidence_quote": "preparing for a stranger"
          }]
        }"#,
    )
    .expect("parse response");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert_eq!(
        result.normalized_response.relationship_rows[0].direction,
        Some(RelationshipDirection::Increase)
    );
}

#[test]
fn trust_infers_decrease_from_guarded_door_chain() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Knock", "Suspicious look");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "source_entity_id": "aurora_soul",
            "target_entity_id": "default_player",
            "dimension": "trust",
            "evidence_quote": "backs away suspiciously and keeps the chain on"
          }]
        }"#,
    )
    .expect("parse response");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert_eq!(
        result.normalized_response.relationship_rows[0].direction,
        Some(RelationshipDirection::Decrease)
    );
    assert!(result.output.relationship_evaluations[0].trust.unwrap() < 0.0);
}

#[test]
fn comfort_infers_decrease_from_guarded_uncertain_entry() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Knock", "Hesitates");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "source_entity_id": "aurora_soul",
            "target_entity_id": "default_player",
            "dimension": "comfort",
            "evidence_quote": "hesitates before opening and keeps distance"
          }]
        }"#,
    )
    .expect("parse response");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert_eq!(
        result.normalized_response.relationship_rows[0].direction,
        Some(RelationshipDirection::Decrease)
    );
    assert!(result.output.relationship_evaluations[0].comfort.unwrap() < 0.0);
}

#[test]
fn fear_infers_increase_from_stiffens_and_pulse() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Knock", "Startled");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "source_entity_id": "aurora_soul",
            "target_entity_id": "default_player",
            "dimension": "fear",
            "evidence_quote": "stiffens with her pulse thrumming"
          }]
        }"#,
    )
    .expect("parse response");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert_eq!(
        result.normalized_response.relationship_rows[0].direction,
        Some(RelationshipDirection::Increase)
    );
    assert!(result.output.relationship_evaluations[0].fear.unwrap() > 0.0);
}

#[test]
fn generic_watching_does_not_infer_direction() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Knock", "Watching");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "source_entity_id": "aurora_soul",
            "target_entity_id": "default_player",
            "dimension": "trust",
            "evidence_quote": "she watches him look at the wall"
          }]
        }"#,
    )
    .expect("parse response");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 1);
    assert_eq!(
        result.rejected_rows[0].reason,
        "direction_missing_uncertain"
    );
}

#[test]
fn ambiguous_relationship_row_still_rejects_direction_missing_uncertain() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Knock", "Smiles");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "source_entity_id": "aurora_soul",
            "target_entity_id": "default_player",
            "dimension": "comfort",
            "evidence_quote": "she smiles and she pauses and breathes"
          }]
        }"#,
    )
    .expect("parse response");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 1);
    assert_eq!(
        result.rejected_rows[0].reason,
        "direction_missing_uncertain"
    );
}

#[test]
fn live_knock_boundary_pressure_row_compiles() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Knock", "Chain is on the door");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "source_entity_id": "aurora_soul",
            "target_entity_id": "default_player",
            "dimension": "boundary_pressure",
            "evidence_quote": "The chain is still on the door—she hasn't decided if she's expecting someone or preparing for a stranger—but she's already reaching for the key."
          }]
        }"#
    ).expect("parse response");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert_eq!(
        result.normalized_response.relationship_rows[0].direction,
        Some(RelationshipDirection::Increase)
    );
    assert!(
        result.output.relationship_evaluations[0]
            .boundary_pressure
            .unwrap()
            > 0.0
    );
}

#[test]
fn row_trace_records_relationship_direction_inference() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Knock", "Chain lock");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "source_entity_id": "aurora_soul",
            "target_entity_id": "default_player",
            "dimension": "boundary_pressure",
            "evidence_quote": "keeps the door chained"
          }]
        }"#,
    )
    .expect("parse response");
    let result = compile_eval_form_response(&spec, &response, &context);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.trace.relationship_direction_inferred_from.len(), 1);
    assert_eq!(
        result.trace.relationship_direction_inferred_from[0],
        "evidence"
    );
    let rel_trace = result
        .trace
        .evaluator_row_traces
        .iter()
        .find(|t| t.row_kind == "relationship")
        .unwrap();
    assert_eq!(rel_trace.compiler_result, "relationship_delta_created");
}

#[test]
fn unwraps_evaluator_form_v1_top_level_envelope() {
    let (parsed, trace) = parse_eval_form_response_with_trace(
        r#"{
          "evaluator_form_v1": {
            "relationship_rows": [{
              "relationship_dimension": "trust",
              "evidence_quote": "Her shoulders ease as recognition settles in.",
              "tags": ["relationship"]
            }]
          }
        }"#,
    )
    .expect("wrapped evaluator_form_v1 should parse");

    assert_eq!(parsed.relationship_rows.len(), 1);
    assert_eq!(
        parsed.relationship_rows[0].dimension,
        Some(RelationshipDimension::Trust)
    );
    assert!(trace
        .raw_form_repair_warnings
        .iter()
        .any(|warning| warning == "top_level_evaluator_form_v1_envelope_unwrapped"));
}

#[test]
fn unwraps_form_top_level_envelope() {
    let parsed = parse_eval_form_response(
        r#"{
          "form": {
            "event_rows": [{
              "event_id": "knock",
              "event_type": "scene_event",
              "summary": "The visitor knocked.",
              "participants": ["aurora_soul", "default_player"],
              "evidence_quote": "A knock sounds."
            }]
          }
        }"#,
    )
    .expect("wrapped form should parse");

    assert_eq!(parsed.event_rows.len(), 1);
    assert_eq!(parsed.event_rows[0].event_id, "knock");
}

#[test]
fn unwraps_response_top_level_envelope_only_when_nested_object_has_rows() {
    let (parsed, trace) = parse_eval_form_response_with_trace(
        r#"{
          "response": {
            "memory_rows": [{
              "owner_soul_id": "aurora_soul",
              "slot_id": "current_plot_memory",
              "content": "Aurora noticed the familiar voice at the door.",
              "evidence_quote": "The voice is familiar.",
              "tags": ["current_plot"]
            }]
          }
        }"#,
    )
    .expect("response envelope with rows should parse");

    assert_eq!(parsed.memory_rows.len(), 1);
    assert!(trace
        .raw_form_repair_warnings
        .iter()
        .any(|warning| warning == "top_level_evaluator_form_v1_envelope_unwrapped"));

    let (unrelated, unrelated_trace) = parse_eval_form_response_with_trace(
        r#"{
          "response": {
            "message": "not a form"
          }
        }"#,
    )
    .expect("non-form response should remain an empty default form");

    assert_eq!(unrelated.memory_rows.len(), 0);
    assert!(!unrelated_trace
        .raw_form_repair_warnings
        .iter()
        .any(|warning| warning == "top_level_evaluator_form_v1_envelope_unwrapped"));
}

#[test]
fn does_not_unwrap_unrelated_nested_json() {
    let (parsed, trace) = parse_eval_form_response_with_trace(
        r#"{
          "metadata": {
            "relationship_rows": [{
              "relationship_dimension": "trust",
              "evidence_quote": "This is diagnostic metadata, not a submitted form."
            }]
          }
        }"#,
    )
    .expect("unrelated nested json should parse as default form");

    assert_eq!(parsed.relationship_rows.len(), 0);
    assert!(!trace
        .raw_form_repair_warnings
        .iter()
        .any(|warning| warning == "top_level_evaluator_form_v1_envelope_unwrapped"));
}

#[test]
fn live_wrapped_boundary_pressure_missing_direction_compiles() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "Knock",
        "The chain still holds the door an inch from fully open, but her grip on the deadbolt has unconsciously relaxed.",
    );
    let response = parse_eval_form_response(
        r#"{
          "evaluator_form_v1": {
            "event_rows": [],
            "object_rows": [],
            "relationship_rows": [
              {
                "relationship_dimension": "boundary_pressure",
                "evidence_quote": "The chain still holds the door an inch from fully open, but her grip on the deadbolt has unconsciously relaxed.",
                "tags": ["boundary"]
              },
              {
                "relationship_dimension": "trust",
                "evidence_quote": "It's not the confident drawl of a stranger, nor the clipped wariness of someone demanding entry--it's... softer. Familiar in a way that makes her chest tighten just a fraction.",
                "tags": ["relationship"]
              }
            ],
            "memory_rows": [],
            "review_rows": []
          }
        }"#,
    )
    .expect("live wrapped form should parse");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert!(result.trace.form_rows_submitted > 0);
    assert!(result.trace.form_rows_accepted > 0);
    assert!(!result.trace.relationship_direction_inferred_from.is_empty());
    assert!(!result.output.relationship_evaluations.is_empty());
    assert_eq!(
        result.normalized_response.relationship_rows[0].direction,
        Some(RelationshipDirection::Increase)
    );
    let boundary_key = "event_latest_turn:aurora_soul:default_player:boundary_pressure";
    assert_eq!(
        result.trace.relationship_row_results.get(boundary_key),
        Some(&"delta_created".to_string())
    );
    assert!(result.trace.evaluator_row_traces.iter().any(|row| {
        row.row_kind == "relationship"
            && row.compiler_result == "relationship_delta_created"
            && row
                .normalized_row
                .get("dimension")
                .and_then(serde_json::Value::as_str)
                == Some("boundary_pressure")
    }));
}

#[test]
fn wrapped_form_rows_are_counted_as_submitted() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "I knock.", "A knock sounds.");
    let response = parse_eval_form_response(
        r#"{
          "eval_form": {
            "event_rows": [{
              "event_id": "knock",
              "event_type": "scene_event",
              "summary": "The visitor knocked.",
              "participants": ["aurora_soul", "default_player"],
              "evidence_quote": "I knock."
            }],
            "memory_rows": [{
              "event_id": "knock",
              "owner_soul_id": "aurora_soul",
              "slot_id": "current_plot_memory",
              "content": "Aurora heard the visitor knock.",
              "evidence_quote": "I knock.",
              "tags": ["current_plot"]
            }]
          }
        }"#,
    )
    .expect("wrapped eval_form should parse");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_submitted, 2);
    assert_eq!(result.trace.form_rows_accepted, 2);
    assert_eq!(result.trace.form_rows_rejected, 0);
}

#[test]
fn wrapped_form_trace_records_unwrap_warning() {
    let (_, trace) = parse_eval_form_response_with_trace(
        r#"{
          "form": {
            "object_rows": [{
              "object_id": "apartment_door",
              "property_changed": "open_state",
              "new_value": "ajar",
              "evidence_quote": "The door is ajar."
            }]
          }
        }"#,
    )
    .expect("wrapped form should parse with trace");

    assert!(trace.raw_form_repair_applied);
    assert!(trace
        .raw_form_repair_warnings
        .iter()
        .any(|warning| warning == "top_level_evaluator_form_v1_envelope_unwrapped"));
}

#[test]
fn wrapped_form_memory_rows_still_compile() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "I knock.",
        "Aurora recognizes the visitor's familiar knock.",
    );
    let response = parse_eval_form_response(
        r#"{
          "evaluator_form_v1": {
            "memory_rows": [{
              "owner_soul_id": "aurora_soul",
              "slot_id": "current_plot_memory",
              "content": "Aurora recognizes the visitor's familiar knock.",
              "evidence_quote": "Aurora recognizes the visitor's familiar knock.",
              "tags": ["current_plot"]
            }]
          }
        }"#,
    )
    .expect("wrapped memory rows should parse");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.draft.memory_candidate_count, 1);
    assert_eq!(result.output.memory_candidates.len(), 1);
}

#[test]
fn hard_form_template_uses_active_player_persona_id() {
    let (soul, world) = soul_and_world();
    let spec = build_eval_form_spec_with_player_persona(
        &soul,
        Some(&world),
        "I wait outside.",
        "Aurora watches from behind the door chain.",
        8,
        "persona_jun",
        "Jun Persona",
    );
    let template = build_hard_eval_form_template(&spec);
    let template_text = serde_json::to_string(&template).expect("template json");

    assert!(spec.active_entities.iter().any(|entity| {
        entity.entity_id == "persona_jun"
            && entity.display_name == "Jun Persona"
            && entity.entity_type == "player_persona"
    }));
    assert!(template_text.contains("\"actor_entity_id\":\"persona_jun\""));
    assert!(template_text.contains("\"relationship_target_entity_id\":\"persona_jun\""));
    assert!(template_text.contains("\"object_rows\""));
    assert!(template_text.contains("\"row_enabled\":0"));
    assert!(template_text.contains("\"owner_entity_id\":\"persona_jun\""));
    assert!(template_text.contains("\"object_label\":\"\""));
    assert!(template_text.contains("\"object_type\":\"\""));
    assert!(template_text.contains("\"last_observed_state\":\"\""));
    assert!(!template_text.contains("\"actor_entity_id\":\"default_player\""));
    assert!(!template_text.contains("\"relationship_target_entity_id\":\"default_player\""));
}

#[test]
fn relationship_rows_default_to_active_player_persona() {
    let (soul, world) = soul_and_world();
    let spec = build_eval_form_spec_with_player_persona(
        &soul,
        Some(&world),
        "I stay outside the threshold.",
        "Aurora relaxes slightly behind the chain.",
        8,
        "persona_jun",
        "Jun Persona",
    );
    let context = EvaluatorConversionContext {
        active_soul_id: &soul.character_id,
        active_soul_ids: vec![soul.character_id.clone()],
        latest_user_message: "I stay outside the threshold.",
        latest_narrator_response: "Aurora relaxes slightly behind the chain.",
        session_world: Some(&world),
        baseline_recent_event_id: None,
    };
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "relationship_dimension": "trust",
            "direction": "increase",
            "evidence_quote": "Aurora relaxes slightly behind the chain.",
            "tags": ["relationship"]
          }]
        }"#,
    )
    .expect("relationship response");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(
        result.output.relationship_evaluations[0].target_entity_id,
        "persona_jun"
    );
    assert_eq!(
        result.normalized_response.relationship_rows[0].target_entity_id,
        "persona_jun"
    );
}

#[test]
fn normal_rp_i_maps_scene_participants_to_active_persona() {
    let (soul, world) = soul_and_world();
    let spec = build_eval_form_spec_with_player_persona(
        &soul,
        Some(&world),
        "I knock once.",
        "Aurora hears the knock.",
        8,
        "preset_male",
        "Male Persona",
    );
    let context = EvaluatorConversionContext {
        active_soul_id: &soul.character_id,
        active_soul_ids: vec![soul.character_id.clone()],
        latest_user_message: "I knock once.",
        latest_narrator_response: "Aurora hears the knock.",
        session_world: Some(&world),
        baseline_recent_event_id: None,
    };
    let response = EvalFormResponse::default();
    let result = compile_eval_form_response(&spec, &response, &context);

    let scene_state = result.output.world_changes[0]
        .scene_state
        .as_ref()
        .expect("scene state");
    assert!(scene_state
        .participants
        .contains(&"preset_male".to_string()));
    assert!(!scene_state
        .participants
        .contains(&"default_player".to_string()));
    assert!(scene_state
        .focus
        .as_deref()
        .unwrap_or("")
        .contains("preset_male"));
    assert!(!scene_state
        .focus
        .as_deref()
        .unwrap_or("")
        .contains("default_player"));
}

#[test]
fn normal_rp_memory_targets_active_persona_not_default_player() {
    let (soul, world) = soul_and_world();
    let spec = build_eval_form_spec_with_player_persona(
        &soul,
        Some(&world),
        "I knock once.",
        "Aurora remembers the visitor's knock.",
        8,
        "preset_male",
        "Male Persona",
    );
    let context = EvaluatorConversionContext {
        active_soul_id: &soul.character_id,
        active_soul_ids: vec![soul.character_id.clone()],
        latest_user_message: "I knock once.",
        latest_narrator_response: "Aurora remembers the visitor's knock.",
        session_world: Some(&world),
        baseline_recent_event_id: None,
    };
    let response = parse_eval_form_response(
        r#"{
          "memory_rows": [{
            "owner_soul_id": "aurora_soul",
            "slot_id": "current_plot_memory",
            "content": "Aurora remembers the visitor's knock.",
            "evidence_quote": "Aurora remembers the visitor's knock.",
            "tags": ["current_plot"]
          }]
        }"#,
    )
    .expect("memory response");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(
        result.output.memory_candidates[0].target_entity_ids,
        vec!["preset_male".to_string()]
    );
    assert_ne!(
        result.output.memory_candidates[0].target_entity_ids,
        vec!["default_player".to_string()]
    );
}

#[test]
fn wet_jacket_creates_stable_unknown_jacket_identity() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "A wet jacket is on the chair.",
        "A wet jacket is draped over the chair.",
    );
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [{
            "change_type": "new_object_observation",
            "new_object_label": "wet_jacket",
            "evidence_quote": "A wet jacket is draped over the chair."
          }]
        }"#,
    )
    .expect("object response");
    let result = compile_eval_form_response(&spec, &response, &context);
    let object = &result.output.object_changes[0].object_state;

    assert_eq!(object.object_id, "unknown_jacket_1");
    assert_ne!(object.object_id, "wet_jacket");
    assert_eq!(object.object_kind, "jacket");
    assert_eq!(object.owner_entity_id.as_deref(), Some("unknown"));
    assert_eq!(object.status, "wet");
    assert_eq!(object.location, "chair");
    assert!(object.last_observed_state.contains("wet jacket"));
}

#[test]
fn my_wet_jacket_uses_active_persona_owner() {
    let (soul, world) = soul_and_world();
    let spec = build_eval_form_spec_with_player_persona(
        &soul,
        Some(&world),
        "I hang my wet jacket on the chair.",
        "Aurora watches the jacket drip onto the chair.",
        8,
        "preset_male",
        "Male Persona",
    );
    let context = EvaluatorConversionContext {
        active_soul_id: &soul.character_id,
        active_soul_ids: vec![soul.character_id.clone()],
        latest_user_message: "I hang my wet jacket on the chair.",
        latest_narrator_response: "Aurora watches the jacket drip onto the chair.",
        session_world: Some(&world),
        baseline_recent_event_id: None,
    };
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [{
            "change_type": "new_object_observation",
            "new_object_label": "wet_jacket",
            "evidence_quote": "I hang my wet jacket on the chair."
          }]
        }"#,
    )
    .expect("object response");
    let result = compile_eval_form_response(&spec, &response, &context);
    let object = &result.output.object_changes[0].object_state;

    assert_eq!(object.object_id, "preset_male_jacket_1");
    assert_eq!(object.owner_entity_id.as_deref(), Some("preset_male"));
    assert_eq!(object.object_kind, "jacket");
    assert_eq!(object.status, "wet");
}

#[test]
fn hard_object_row_shape_compiles_my_wet_jacket_to_stable_identity() {
    let (soul, world) = soul_and_world();
    let spec = build_eval_form_spec_with_player_persona(
        &soul,
        Some(&world),
        "I drape my wet jacket over the chair.",
        "The wet jacket is draped over the chair.",
        8,
        "preset_male",
        "Male Persona",
    );
    let context = EvaluatorConversionContext {
        active_soul_id: &soul.character_id,
        active_soul_ids: vec![soul.character_id.clone()],
        latest_user_message: "I drape my wet jacket over the chair.",
        latest_narrator_response: "The wet jacket is draped over the chair.",
        session_world: Some(&world),
        baseline_recent_event_id: None,
    };
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [{
            "row_enabled": 1,
            "linked_event_id": "event_latest_turn",
            "object_id": "",
            "object_label": "my wet jacket",
            "object_type": "jacket",
            "owner_entity_id": "preset_male",
            "status": "wet",
            "location": "chair",
            "last_observed_state": "wet jacket draped over chair",
            "evidence_quote": "my wet jacket over the chair"
          }]
        }"#,
    )
    .expect("object response");
    let result = compile_eval_form_response(&spec, &response, &context);
    let object = &result.output.object_changes[0].object_state;

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(object.object_id, "preset_male_jacket_1");
    assert_eq!(object.owner_entity_id.as_deref(), Some("preset_male"));
    assert_eq!(object.object_kind, "jacket");
    assert_eq!(object.status, "wet");
    assert_eq!(object.location, "chair");
    assert_eq!(object.last_observed_state, "wet jacket draped over chair");
}

#[test]
fn generic_clothing_kind_uses_specific_object_label_type_and_reuses_identity() {
    let (soul, mut world) = soul_and_world();
    let spec = build_eval_form_spec_with_player_persona(
        &soul,
        Some(&world),
        "I drape my wet jacket over the chair.",
        "The wet jacket is draped over the chair.",
        8,
        "preset_male",
        "Male Persona",
    );
    let context = EvaluatorConversionContext {
        active_soul_id: &soul.character_id,
        active_soul_ids: vec![soul.character_id.clone()],
        latest_user_message: "I drape my wet jacket over the chair.",
        latest_narrator_response: "The wet jacket is draped over the chair.",
        session_world: Some(&world),
        baseline_recent_event_id: None,
    };
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [{
            "row_enabled": 1,
            "linked_event_id": "event_latest_turn",
            "object_label": "wet jacket",
            "object_type": "clothing",
            "owner_entity_id": "preset_male",
            "status": "wet",
            "location": "chair",
            "last_observed_state": "wet jacket draped over chair",
            "evidence_quote": "my wet jacket over the chair"
          }]
        }"#,
    )
    .expect("object response");
    let result = compile_eval_form_response(&spec, &response, &context);
    let object = &result.output.object_changes[0].object_state;
    assert_eq!(object.object_kind, "jacket");
    assert_eq!(object.object_id, "preset_male_jacket_1");
    assert_eq!(object.status, "wet");
    assert_eq!(object.location, "chair");

    world.object_states.push(object.clone());
    let spec = build_eval_form_spec_with_player_persona(
        &soul,
        Some(&world),
        "I move my wet jacket near the door.",
        "The wet jacket is near the door.",
        8,
        "preset_male",
        "Male Persona",
    );
    let context = EvaluatorConversionContext {
        active_soul_id: &soul.character_id,
        active_soul_ids: vec![soul.character_id.clone()],
        latest_user_message: "I move my wet jacket near the door.",
        latest_narrator_response: "The wet jacket is near the door.",
        session_world: Some(&world),
        baseline_recent_event_id: None,
    };
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [{
            "row_enabled": 1,
            "linked_event_id": "event_latest_turn",
            "object_label": "wet jacket",
            "object_type": "clothing",
            "owner_entity_id": "preset_male",
            "status": "wet",
            "location": "near door",
            "last_observed_state": "wet jacket moved near door",
            "evidence_quote": "my wet jacket near the door"
          }]
        }"#,
    )
    .expect("object response");
    let result = compile_eval_form_response(&spec, &response, &context);
    let object = &result.output.object_changes[0].object_state;
    assert_eq!(object.object_kind, "jacket");
    assert_eq!(object.object_id, "preset_male_jacket_1");
    assert_eq!(object.location, "near door");
}

#[test]
fn disabled_hard_object_row_is_ignored_without_rejection() {
    let (soul, world) = soul_and_world();
    let (spec, context) =
        spec_and_context(&soul, &world, "I wait.", "Aurora waits behind the door.");
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [{
            "row_enabled": 0,
            "linked_event_id": "event_latest_turn",
            "object_id": "",
            "object_label": "",
            "object_type": "",
            "owner_entity_id": "default_player",
            "status": "",
            "location": "",
            "last_observed_state": "",
            "evidence_quote": ""
          }]
        }"#,
    )
    .expect("disabled object row response");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert!(result.output.object_changes.is_empty());
    assert!(result.trace.evaluator_row_traces.iter().any(|row| {
        row.row_kind == "object"
            && row.validation_status == "accepted"
            && row.compiler_result == "disabled_row_ignored"
    }));
}

#[test]
fn auroras_jacket_uses_aurora_owner() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "I point at it.",
        "Aurora's jacket hangs from the chair.",
    );
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [{
            "change_type": "new_object_observation",
            "new_object_label": "jacket",
            "evidence_quote": "Aurora's jacket hangs from the chair."
          }]
        }"#,
    )
    .expect("object response");
    let result = compile_eval_form_response(&spec, &response, &context);
    let object = &result.output.object_changes[0].object_state;

    assert_eq!(object.object_id, "aurora_soul_jacket_1");
    assert_eq!(object.owner_entity_id.as_deref(), Some("aurora_soul"));
    assert_eq!(object.object_kind, "jacket");
}

#[test]
fn multiple_jackets_owned_by_same_entity_increment_ordinals() {
    let (soul, world) = soul_and_world();
    let spec = build_eval_form_spec_with_player_persona(
        &soul,
        Some(&world),
        "I set my wet jacket and my dry jacket on two chairs.",
        "Two jackets rest on separate chairs.",
        8,
        "preset_male",
        "Male Persona",
    );
    let context = EvaluatorConversionContext {
        active_soul_id: &soul.character_id,
        active_soul_ids: vec![soul.character_id.clone()],
        latest_user_message: "I set my wet jacket and my dry jacket on two chairs.",
        latest_narrator_response: "Two jackets rest on separate chairs.",
        session_world: Some(&world),
        baseline_recent_event_id: None,
    };
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [
            {
              "change_type": "new_object_observation",
              "new_object_label": "wet_jacket",
              "evidence_quote": "I set my wet jacket on the first chair."
            },
            {
              "change_type": "new_object_observation",
              "new_object_label": "dry_jacket",
              "evidence_quote": "I set my dry jacket on the second chair."
            }
          ]
        }"#,
    )
    .expect("object response");
    let result = compile_eval_form_response(&spec, &response, &context);
    let ids = result
        .output
        .object_changes
        .iter()
        .map(|change| change.object_state.object_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(ids, vec!["preset_male_jacket_1", "preset_male_jacket_2"]);
}

#[test]
fn object_state_update_reuses_existing_matching_object() {
    let (soul, mut world) = soul_and_world();
    world.object_states.push(ObjectState {
        object_id: "preset_male_jacket_1".into(),
        object_kind: "jacket".into(),
        owner_entity_id: Some("preset_male".into()),
        status: "wet".into(),
        location: "chair".into(),
        last_observed_state: "wet jacket on chair".into(),
        ..ObjectState::default()
    });
    let spec = build_eval_form_spec_with_player_persona(
        &soul,
        Some(&world),
        "My jacket is still wet on the chair.",
        "The jacket remains wet.",
        8,
        "preset_male",
        "Male Persona",
    );
    let context = EvaluatorConversionContext {
        active_soul_id: &soul.character_id,
        active_soul_ids: vec![soul.character_id.clone()],
        latest_user_message: "My jacket is still wet on the chair.",
        latest_narrator_response: "The jacket remains wet.",
        session_world: Some(&world),
        baseline_recent_event_id: None,
    };
    let response = parse_eval_form_response(
        r#"{
          "object_rows": [{
            "change_type": "state_change",
            "evidence_quote": "My jacket is still wet on the chair."
          }]
        }"#,
    )
    .expect("object response");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(
        result.output.object_changes[0].object_state.object_id,
        "preset_male_jacket_1"
    );
    assert_eq!(result.output.object_changes[0].object_state.status, "wet");

    let before_count = world.object_states.len();
    let report = evaluator_output_to_engine_patch(&result.output, &context);
    let mut soul_mut = soul;
    report
        .patch
        .apply_to_session(&mut soul_mut, Some(&mut world))
        .expect("object patch applies");
    assert_eq!(world.object_states.len(), before_count);
    assert_eq!(
        world
            .object_states
            .iter()
            .filter(|object| object.object_id == "preset_male_jacket_1")
            .count(),
        1
    );
}

#[test]
fn unknown_owned_jacket_merges_when_owner_becomes_known() {
    let (soul, mut world) = soul_and_world();
    let (unknown_spec, unknown_context) = spec_and_context(
        &soul,
        &world,
        "A wet jacket lies on the chair.",
        "A wet jacket lies on the chair.",
    );
    let unknown_response = parse_eval_form_response(
        r#"{
          "object_rows": [{
            "change_type": "new_object_observation",
            "new_object_label": "wet_jacket",
            "evidence_quote": "A wet jacket lies on the chair."
          }]
        }"#,
    )
    .expect("unknown object response");
    let unknown_result =
        compile_eval_form_response(&unknown_spec, &unknown_response, &unknown_context);
    let unknown_report = evaluator_output_to_engine_patch(&unknown_result.output, &unknown_context);
    let mut soul_mut = soul;
    unknown_report
        .patch
        .apply_to_session(&mut soul_mut, Some(&mut world))
        .expect("unknown object patch applies");
    assert!(world
        .object_states
        .iter()
        .any(|object| object.object_id == "unknown_jacket_1"));

    let known_spec = build_eval_form_spec_with_player_persona(
        &soul_mut,
        Some(&world),
        "I pick up my wet jacket.",
        "The jacket is clearly his now.",
        8,
        "preset_male",
        "Male Persona",
    );
    let known_context = EvaluatorConversionContext {
        active_soul_id: &soul_mut.character_id,
        active_soul_ids: vec![soul_mut.character_id.clone()],
        latest_user_message: "I pick up my wet jacket.",
        latest_narrator_response: "The jacket is clearly his now.",
        session_world: Some(&world),
        baseline_recent_event_id: None,
    };
    let known_response = parse_eval_form_response(
        r#"{
          "object_rows": [{
            "change_type": "state_change",
            "evidence_quote": "I pick up my wet jacket."
          }]
        }"#,
    )
    .expect("known object response");
    let known_result = compile_eval_form_response(&known_spec, &known_response, &known_context);
    assert_eq!(
        known_result.output.object_changes[0].object_state.object_id,
        "preset_male_jacket_1"
    );

    let known_report = evaluator_output_to_engine_patch(&known_result.output, &known_context);
    known_report
        .patch
        .apply_to_session(&mut soul_mut, Some(&mut world))
        .expect("known object patch applies");
    assert!(!world
        .object_states
        .iter()
        .any(|object| object.object_id == "unknown_jacket_1"));
    assert_eq!(
        world
            .object_states
            .iter()
            .filter(|object| object.object_id == "preset_male_jacket_1")
            .count(),
        1
    );
}

fn relationship_event_base_row() -> serde_json::Value {
    serde_json::json!({
        "row_enabled": 1,
        "event_id": "event_latest_turn",
        "actor_entity_id": "default_player",
        "relationship_source_soul_id": "aurora_soul",
        "relationship_target_entity_id": "default_player",
        "perceived_by_entity_id": "aurora_soul",
        "evidence_quote": "I stay outside the threshold with my hands visible. You don't have to open the door.",
        "intent": 3,
        "honesty": 1,
        "reliability": 1,
        "boundary_treatment": 5,
        "responsiveness": 3,
        "power_use": 1,
        "evaluation_tone": 1,
        "competence": 0,
        "disclosure": 0,
        "reciprocity": 1,
        "repair": 0,
        "predictability": 2,
        "salience": 65,
        "certainty": 95,
        "directness": 90,
        "costliness": 25,
        "stakes": 50,
        "repetition": 20,
        "event_flags_u64": 35
    })
}

fn relationship_event_disabled_template_row() -> serde_json::Value {
    serde_json::json!({
        "row_enabled": 0,
        "event_id": "event_latest_turn",
        "actor_entity_id": "default_player",
        "relationship_source_soul_id": "aurora_soul",
        "relationship_target_entity_id": "default_player",
        "perceived_by_entity_id": "aurora_soul",
        "evidence_quote": "",
        "intent": 0,
        "honesty": 0,
        "reliability": 0,
        "boundary_treatment": 0,
        "responsiveness": 0,
        "power_use": 0,
        "evaluation_tone": 0,
        "competence": 0,
        "disclosure": 0,
        "reciprocity": 0,
        "repair": 0,
        "predictability": 0,
        "salience": 0,
        "certainty": 0,
        "directness": 0,
        "costliness": 0,
        "stakes": 0,
        "repetition": 0,
        "event_flags_u64": 0
    })
}

fn compile_relationship_event_row(
    row: serde_json::Value,
    configure: impl FnOnce(&mut Soul),
) -> EvalFormCompileResult {
    let (mut soul, world) = soul_and_world();
    configure(&mut soul);
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "I stay outside the threshold.",
        "Aurora watches from behind the door chain.",
    );
    compile_eval_form_response(
        &spec,
        &EvalFormResponse {
            relationship_event_rows: vec![row],
            ..EvalFormResponse::default()
        },
        &context,
    )
}

fn first_relationship_delta(result: &EvalFormCompileResult) -> &RelationshipEvaluation {
    result
        .output
        .relationship_evaluations
        .first()
        .expect("relationship delta")
}

fn set_row_i64(row: &mut serde_json::Value, key: &str, value: i64) {
    row.as_object_mut()
        .expect("row object")
        .insert(key.into(), serde_json::Value::from(value));
}

fn calibrated_positive_relationship_row(
    axis_score: i64,
    salience: i64,
    stakes: i64,
) -> serde_json::Value {
    let mut row = relationship_event_base_row();
    for key in [
        "intent",
        "honesty",
        "reliability",
        "boundary_treatment",
        "responsiveness",
        "power_use",
        "evaluation_tone",
        "competence",
        "disclosure",
        "reciprocity",
        "repair",
        "predictability",
    ] {
        set_row_i64(&mut row, key, axis_score);
    }
    set_row_i64(&mut row, "salience", salience);
    set_row_i64(&mut row, "certainty", 90);
    set_row_i64(&mut row, "directness", 90);
    set_row_i64(&mut row, "costliness", 0);
    set_row_i64(&mut row, "stakes", stakes);
    set_row_i64(&mut row, "repetition", 0);
    row.as_object_mut()
        .expect("row object")
        .insert("event_flags_u64".into(), serde_json::Value::from(0_u64));
    row
}

fn calibrated_negative_relationship_row(
    axis_score: i64,
    salience: i64,
    stakes: i64,
) -> serde_json::Value {
    calibrated_positive_relationship_row(-axis_score.abs(), salience, stakes)
}

fn relationship_comfort_delta(result: &EvalFormCompileResult) -> f32 {
    first_relationship_delta(result)
        .comfort
        .expect("comfort delta")
}

fn apply_relationship_event_row_to_state(
    soul: &mut Soul,
    world: &mut SessionWorld,
    row: serde_json::Value,
) {
    let (spec, context) = spec_and_context(
        soul,
        world,
        "I stay outside the threshold.",
        "Aurora watches from behind the door chain.",
    );
    let result = compile_eval_form_response(
        &spec,
        &EvalFormResponse {
            relationship_event_rows: vec![row],
            ..EvalFormResponse::default()
        },
        &context,
    );
    assert_eq!(result.trace.form_rows_rejected, 0);
    let report = evaluator_output_to_engine_patch(&result.output, &context);
    report
        .patch
        .apply_to_session(soul, Some(world))
        .expect("relationship event patch applies");
}

#[test]
fn hard_relationship_event_template_accepts_exact_keys() {
    let result = compile_relationship_event_row(relationship_event_base_row(), |_| {});

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert!(result.trace.evaluator_row_traces.iter().any(|row| {
        row.row_kind == "relationship_event"
            && row.validation_status == "accepted"
            && row.compiler_result == "relationship_delta_created"
    }));
    assert_eq!(
        result.trace.relationship_event_template_version,
        RELATIONSHIP_EVENT_TEMPLATE_VERSION
    );
}

#[test]
fn hard_relationship_event_template_rejects_unknown_axis_trust() {
    let mut row = relationship_event_base_row();
    row.as_object_mut()
        .unwrap()
        .insert("axis_trust".into(), serde_json::Value::from(5));
    let result = compile_relationship_event_row(row, |_| {});

    assert_eq!(result.trace.form_rows_rejected, 1);
    assert_eq!(
        result.rejected_rows[0].reason,
        "relationship_event_unknown_key:axis_trust"
    );
}

#[test]
fn hard_relationship_event_template_rejects_unknown_modifier_trust() {
    let mut row = relationship_event_base_row();
    row.as_object_mut()
        .unwrap()
        .insert("modifier_trust".into(), serde_json::Value::from(80));
    let result = compile_relationship_event_row(row, |_| {});

    assert_eq!(result.trace.form_rows_rejected, 1);
    assert_eq!(
        result.rejected_rows[0].reason,
        "relationship_event_unknown_key:modifier_trust"
    );
}

#[test]
fn relationship_event_row_rejects_surface_fields_inside_numeric_event() {
    let mut row = relationship_event_base_row();
    let object = row.as_object_mut().unwrap();
    object.insert("trust".into(), serde_json::Value::from(75));
    object.insert("comfort".into(), serde_json::Value::from(60));
    object.insert("boundary_pressure".into(), serde_json::Value::from(20));
    let result = compile_relationship_event_row(row, |_| {});

    assert_eq!(result.trace.form_rows_rejected, 1);
    assert_eq!(
        result.rejected_rows[0].reason,
        "relationship_event_forbidden_surface_field:trust"
    );
    assert!(result.trace.evaluator_row_traces.iter().any(|row| {
        row.row_kind == "relationship_event"
            && row.validation_status == "rejected"
            && row
                .rejection_reason
                .as_deref()
                .is_some_and(|reason| reason == "relationship_event_forbidden_surface_field:trust")
    }));
}

#[test]
fn hard_relationship_event_template_rejects_missing_intent() {
    let mut row = relationship_event_base_row();
    row.as_object_mut().unwrap().remove("intent");
    let result = compile_relationship_event_row(row, |_| {});

    assert_eq!(result.trace.form_rows_rejected, 1);
    assert_eq!(
        result.rejected_rows[0].reason,
        "relationship_event_missing_key:intent"
    );
}

#[test]
fn hard_relationship_event_template_rejects_string_number() {
    let mut row = relationship_event_base_row();
    row.as_object_mut()
        .unwrap()
        .insert("intent".into(), serde_json::Value::String("5".into()));
    let result = compile_relationship_event_row(row, |_| {});

    assert_eq!(result.trace.form_rows_rejected, 1);
    assert_eq!(
        result.rejected_rows[0].reason,
        "numeric_field_non_numeric:intent"
    );
}

#[test]
fn hard_relationship_event_template_ignores_disabled_row() {
    let result = compile_relationship_event_row(relationship_event_disabled_template_row(), |_| {});

    assert_eq!(result.trace.form_rows_submitted, 2);
    assert_eq!(result.trace.form_rows_accepted, 1);
    assert_eq!(result.trace.form_rows_rejected, 0);
    assert!(result.output.relationship_evaluations.is_empty());
    assert!(result.trace.evaluator_row_traces.iter().any(|row| {
        row.row_kind == "relationship_event"
            && row.validation_status == "accepted"
            && row.compiler_result == "disabled_row_ignored"
    }));
}

#[test]
fn hard_relationship_event_template_compiles_enabled_row() {
    let result = compile_relationship_event_row(relationship_event_base_row(), |_| {});

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert!(result
        .trace
        .relationship_delta_source
        .values()
        .any(|source| source == "numeric_event_v2"));
}

#[test]
fn relationship_calibration_plus_one_low_salience_creates_small_delta() {
    let result =
        compile_relationship_event_row(calibrated_positive_relationship_row(1, 20, 20), |_| {});
    let comfort = relationship_comfort_delta(&result);

    assert!(comfort > 0.0);
    assert!(comfort <= 3.0, "low-salience +1 delta was {comfort}");
}

#[test]
fn relationship_calibration_plus_three_medium_salience_creates_moderate_delta() {
    let result =
        compile_relationship_event_row(calibrated_positive_relationship_row(3, 55, 55), |_| {});
    let comfort = relationship_comfort_delta(&result);

    assert!(comfort > 3.0, "medium-salience +3 delta was {comfort}");
    assert!(comfort <= 10.0, "medium-salience +3 delta was {comfort}");
}

#[test]
fn relationship_calibration_plus_five_high_salience_creates_large_bounded_delta() {
    let result =
        compile_relationship_event_row(calibrated_positive_relationship_row(5, 90, 90), |_| {});
    let comfort = relationship_comfort_delta(&result);

    assert!(comfort > 8.0, "high-salience +5 delta was {comfort}");
    assert!(comfort <= 20.0, "high-salience +5 delta was {comfort}");
    assert!(
        first_relationship_delta(&result)
            .max_abs_delta
            .unwrap_or_default()
            <= 20.0
    );
}

#[test]
fn relationship_calibration_repeated_minor_events_do_not_explode_values() {
    let (mut soul, mut world) = soul_and_world();
    let starting_comfort = soul.relationships["user"].comfort;

    for _ in 0..20 {
        apply_relationship_event_row_to_state(
            &mut soul,
            &mut world,
            calibrated_positive_relationship_row(1, 20, 20),
        );
    }

    let relationship = &soul.relationships["user"];
    assert!(relationship.comfort > starting_comfort);
    assert!(
        relationship.comfort < 45.0,
        "repeated low-salience +1 events raised comfort to {}",
        relationship.comfort
    );
    assert!(relationship.trust <= 100.0);
}

#[test]
fn relationship_calibration_clamps_values_to_valid_ranges() {
    let (mut soul, mut world) = soul_and_world();
    {
        let relationship = soul.relationships.get_mut("user").expect("user relation");
        relationship.comfort = 99.0;
        relationship.boundary_pressure = 1.0;
    }
    apply_relationship_event_row_to_state(
        &mut soul,
        &mut world,
        calibrated_positive_relationship_row(5, 100, 100),
    );
    let relationship = &soul.relationships["user"];
    assert!((0.0..=100.0).contains(&relationship.comfort));
    assert!((0.0..=100.0).contains(&relationship.boundary_pressure));

    let (mut soul, mut world) = soul_and_world();
    {
        let relationship = soul.relationships.get_mut("user").expect("user relation");
        relationship.conflict = 99.0;
        relationship.boundary_pressure = 99.0;
    }
    apply_relationship_event_row_to_state(
        &mut soul,
        &mut world,
        calibrated_negative_relationship_row(5, 100, 100),
    );
    let relationship = &soul.relationships["user"];
    assert!((0.0..=100.0).contains(&relationship.conflict));
    assert!((0.0..=100.0).contains(&relationship.boundary_pressure));
}

#[test]
fn hard_relationship_event_template_requires_row_enabled_zero_or_one() {
    let mut row = relationship_event_base_row();
    set_row_i64(&mut row, "row_enabled", 2);
    let result = compile_relationship_event_row(row, |_| {});

    assert_eq!(result.trace.form_rows_rejected, 1);
    assert_eq!(
        result.rejected_rows[0].reason,
        "relationship_event_row_enabled_invalid"
    );
}

#[test]
fn live_axis_trust_payload_rejected_with_unknown_key_reason() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "I wait outside.",
        "Aurora watches from behind the chain.",
    );
    let response = parse_eval_form_response(
        r#"{
          "evaluator_form_v1": {
            "relationship_event_rows": [{
              "row_enabled": 1,
              "event_id": "event_latest_turn",
              "actor_entity_id": "default_player",
              "relationship_source_soul_id": "aurora_soul",
              "relationship_target_entity_id": "default_player",
              "perceived_by_entity_id": "aurora_soul",
              "evidence_quote": "I wait outside.",
              "axis_trust": 5,
              "intent": 3,
              "honesty": 1,
              "reliability": 1,
              "boundary_treatment": 5,
              "responsiveness": 3,
              "power_use": 1,
              "evaluation_tone": 1,
              "competence": 0,
              "disclosure": 0,
              "reciprocity": 1,
              "repair": 0,
              "predictability": 2,
              "salience": 65,
              "certainty": 95,
              "directness": 90,
              "costliness": 25,
              "stakes": 50,
              "repetition": 20,
              "event_flags_u64": 35
            }]
          }
        }"#,
    )
    .expect("axis_trust payload should parse as raw row");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 1);
    assert_eq!(
        result.rejected_rows[0].reason,
        "relationship_event_unknown_key:axis_trust"
    );
    assert!(result.output.relationship_evaluations.is_empty());
}

#[test]
fn live_canonical_template_payload_compiles_numeric_event_v2() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(
        &soul,
        &world,
        "I wait outside.",
        "Aurora watches from behind the chain.",
    );
    let response = parse_eval_form_response(
        r#"{
          "evaluator_form_v1": {
            "relationship_event_rows": [{
              "row_enabled": 1,
              "event_id": "event_latest_turn",
              "actor_entity_id": "default_player",
              "relationship_source_soul_id": "aurora_soul",
              "relationship_target_entity_id": "default_player",
              "perceived_by_entity_id": "aurora_soul",
              "evidence_quote": "I wait outside the threshold with my hands visible.",
              "intent": 3,
              "honesty": 1,
              "reliability": 1,
              "boundary_treatment": 5,
              "responsiveness": 3,
              "power_use": 1,
              "evaluation_tone": 1,
              "competence": 0,
              "disclosure": 0,
              "reciprocity": 1,
              "repair": 0,
              "predictability": 2,
              "salience": 65,
              "certainty": 95,
              "directness": 90,
              "costliness": 25,
              "stakes": 50,
              "repetition": 20,
              "event_flags_u64": 35
            }]
          }
        }"#,
    )
    .expect("canonical payload should parse");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert!(result
        .trace
        .relationship_delta_source
        .values()
        .any(|source| source == "numeric_event_v2"));
}

#[test]
fn relationship_event_row_accepts_all_required_numbers() {
    let result = compile_relationship_event_row(relationship_event_base_row(), |_| {});

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert!(result.trace.form_rows_accepted >= 1);
    assert!(result.trace.evaluator_row_traces.iter().any(|row| {
        row.row_kind == "relationship_event" && row.validation_status == "accepted"
    }));
}

#[test]
fn relationship_event_row_rejects_missing_numeric_field() {
    let mut row = relationship_event_base_row();
    row.as_object_mut().unwrap().remove("intent");
    let result = compile_relationship_event_row(row, |_| {});

    assert_eq!(result.trace.form_rows_rejected, 1);
    assert_eq!(
        result.rejected_rows[0].reason,
        "relationship_event_missing_key:intent"
    );
}

#[test]
fn relationship_event_row_rejects_string_numeric_field() {
    let mut row = relationship_event_base_row();
    row.as_object_mut()
        .unwrap()
        .insert("intent".into(), serde_json::Value::String("5".into()));
    let result = compile_relationship_event_row(row, |_| {});

    assert_eq!(result.trace.form_rows_rejected, 1);
    assert_eq!(
        result.rejected_rows[0].reason,
        "numeric_field_non_numeric:intent"
    );
}

#[test]
fn relationship_event_row_rejects_axis_out_of_range() {
    let mut row = relationship_event_base_row();
    set_row_i64(&mut row, "intent", 6);
    let result = compile_relationship_event_row(row, |_| {});

    assert_eq!(result.trace.form_rows_rejected, 1);
    assert_eq!(result.rejected_rows[0].reason, "axis_out_of_range:intent");
}

#[test]
fn relationship_event_row_rejects_predictability_as_modifier_scale() {
    let mut row = relationship_event_base_row();
    set_row_i64(&mut row, "predictability", 40);
    let result = compile_relationship_event_row(row, |_| {});

    assert_eq!(result.trace.form_rows_rejected, 1);
    assert_eq!(
        result.rejected_rows[0].reason,
        "axis_out_of_range:predictability"
    );
}

#[test]
fn relationship_event_row_rejects_modifier_out_of_range() {
    let mut row = relationship_event_base_row();
    set_row_i64(&mut row, "salience", 101);
    let result = compile_relationship_event_row(row, |_| {});

    assert_eq!(result.trace.form_rows_rejected, 1);
    assert_eq!(
        result.rejected_rows[0].reason,
        "modifier_out_of_range:salience"
    );
}

#[test]
fn relationship_event_row_rejects_missing_evidence() {
    let mut row = relationship_event_base_row();
    row.as_object_mut().unwrap().insert(
        "evidence_quote".into(),
        serde_json::Value::String("".into()),
    );
    let result = compile_relationship_event_row(row, |_| {});

    assert_eq!(result.trace.form_rows_rejected, 1);
    assert_eq!(result.rejected_rows[0].reason, "evidence_quote_missing");
}

#[test]
fn relationship_event_row_rejects_bad_entity() {
    let mut row = relationship_event_base_row();
    row.as_object_mut().unwrap().insert(
        "actor_entity_id".into(),
        serde_json::Value::String("stranger".into()),
    );
    let result = compile_relationship_event_row(row, |_| {});

    assert_eq!(result.trace.form_rows_rejected, 1);
    assert_eq!(
        result.rejected_rows[0].reason,
        "actor_entity_resolution_failed"
    );
}

#[test]
fn boundary_respect_decreases_boundary_pressure() {
    let result = compile_relationship_event_row(relationship_event_base_row(), |soul| {
        soul.relationships
            .get_mut("user")
            .unwrap()
            .boundary_pressure = 40.0;
    });

    assert!(first_relationship_delta(&result).boundary_pressure.unwrap() < 0.0);
}

#[test]
fn pressure_after_refusal_increases_boundary_pressure() {
    let mut row = relationship_event_base_row();
    set_row_i64(&mut row, "intent", -3);
    set_row_i64(&mut row, "boundary_treatment", -5);
    set_row_i64(&mut row, "responsiveness", -3);
    set_row_i64(&mut row, "power_use", -2);
    set_row_i64(&mut row, "evaluation_tone", -2);
    let result = compile_relationship_event_row(row, |_| {});

    assert!(first_relationship_delta(&result).boundary_pressure.unwrap() > 0.0);
}

#[test]
fn honest_costly_support_increases_trustable_bias() {
    let mut row = relationship_event_base_row();
    set_row_i64(&mut row, "honesty", 5);
    set_row_i64(&mut row, "reliability", 5);
    set_row_i64(&mut row, "costliness", 90);
    row.as_object_mut()
        .unwrap()
        .insert("event_flags_u64".into(), serde_json::Value::from(128_u64));
    let result = compile_relationship_event_row(row, |_| {});

    assert!(first_relationship_delta(&result).trustable_bias.unwrap() > 0.0);
}

#[test]
fn betrayal_increases_untrustworthy_bias_more_than_minor_kindness_reduces_it() {
    let mut betrayal = relationship_event_base_row();
    set_row_i64(&mut betrayal, "honesty", -5);
    set_row_i64(&mut betrayal, "reliability", -5);
    set_row_i64(&mut betrayal, "intent", -4);
    let betrayal_result = compile_relationship_event_row(betrayal, |soul| {
        soul.relationships
            .get_mut("user")
            .unwrap()
            .untrustworthy_bias = 40.0;
    });

    let mut kindness = relationship_event_base_row();
    set_row_i64(&mut kindness, "honesty", 1);
    set_row_i64(&mut kindness, "reliability", 1);
    set_row_i64(&mut kindness, "salience", 20);
    set_row_i64(&mut kindness, "stakes", 10);
    let kindness_result = compile_relationship_event_row(kindness, |soul| {
        soul.relationships
            .get_mut("user")
            .unwrap()
            .untrustworthy_bias = 40.0;
    });

    let betrayal_delta = first_relationship_delta(&betrayal_result)
        .untrustworthy_bias
        .unwrap();
    let kindness_delta = first_relationship_delta(&kindness_result)
        .untrustworthy_bias
        .unwrap_or(0.0);
    assert!(betrayal_delta > kindness_delta.abs());
}

#[test]
fn asshole_and_trustable_bias_can_both_be_high() {
    let mut row = relationship_event_base_row();
    set_row_i64(&mut row, "honesty", 5);
    set_row_i64(&mut row, "reliability", 5);
    let result = compile_relationship_event_row(row, |soul| {
        let relationship = soul.relationships.get_mut("user").unwrap();
        relationship.asshole_bias = 70.0;
        relationship.trustable_bias = 60.0;
    });

    let delta = first_relationship_delta(&result);
    assert!(70.0 + delta.asshole_bias.unwrap_or(0.0) >= 65.0);
    assert!(60.0 + delta.trustable_bias.unwrap_or(0.0) >= 60.0);
}

#[test]
fn minor_nice_event_does_not_erase_asshole_bias() {
    let mut row = relationship_event_base_row();
    set_row_i64(&mut row, "salience", 10);
    set_row_i64(&mut row, "certainty", 50);
    set_row_i64(&mut row, "stakes", 10);
    let result = compile_relationship_event_row(row, |soul| {
        soul.relationships.get_mut("user").unwrap().asshole_bias = 80.0;
    });

    let next = 80.0
        + first_relationship_delta(&result)
            .asshole_bias
            .unwrap_or(0.0);
    assert!(next > 75.0);
}

#[test]
fn high_score_requires_cost_or_repetition() {
    let mut weak = relationship_event_base_row();
    set_row_i64(&mut weak, "honesty", 5);
    set_row_i64(&mut weak, "reliability", 5);
    set_row_i64(&mut weak, "costliness", 0);
    set_row_i64(&mut weak, "stakes", 10);
    set_row_i64(&mut weak, "repetition", 0);
    let weak_result = compile_relationship_event_row(weak, |_| {});

    let mut strong = relationship_event_base_row();
    set_row_i64(&mut strong, "honesty", 5);
    set_row_i64(&mut strong, "reliability", 5);
    set_row_i64(&mut strong, "costliness", 90);
    set_row_i64(&mut strong, "stakes", 90);
    set_row_i64(&mut strong, "repetition", 90);
    let strong_result = compile_relationship_event_row(strong, |_| {});

    let weak_delta = first_relationship_delta(&weak_result)
        .trustable_bias
        .unwrap_or(0.0);
    let strong_delta = first_relationship_delta(&strong_result)
        .trustable_bias
        .unwrap_or(0.0);
    assert!(strong_delta > weak_delta * 2.0);
}

#[test]
fn comfort_rises_from_responsiveness_and_boundary_respect() {
    let mut row = relationship_event_base_row();
    set_row_i64(&mut row, "responsiveness", 5);
    set_row_i64(&mut row, "boundary_treatment", 5);
    let result = compile_relationship_event_row(row, |_| {});

    assert!(first_relationship_delta(&result).comfort.unwrap() > 0.0);
}

#[test]
fn conflict_rises_from_negative_tone_and_boundary_violation() {
    let mut row = relationship_event_base_row();
    set_row_i64(&mut row, "evaluation_tone", -5);
    set_row_i64(&mut row, "boundary_treatment", -5);
    set_row_i64(&mut row, "intent", -4);
    set_row_i64(&mut row, "reciprocity", -4);
    let result = compile_relationship_event_row(row, |_| {});

    assert!(first_relationship_delta(&result).conflict.unwrap() > 0.0);
}

#[test]
fn contradictory_event_adds_reappraisal_debt() {
    let mut row = relationship_event_base_row();
    row.as_object_mut()
        .unwrap()
        .insert("event_flags_u64".into(), serde_json::Value::from(32_u64));
    let result = compile_relationship_event_row(row, |soul| {
        soul.relationships.get_mut("user").unwrap().asshole_bias = 90.0;
    });

    assert!(first_relationship_delta(&result).reappraisal_debt.unwrap() > 0.0);
}

#[test]
fn reappraisal_enters_under_review_at_threshold() {
    let mut row = relationship_event_base_row();
    row.as_object_mut()
        .unwrap()
        .insert("event_flags_u64".into(), serde_json::Value::from(32_u64));
    let result = compile_relationship_event_row(row, |soul| {
        let relationship = soul.relationships.get_mut("user").unwrap();
        relationship.asshole_bias = 100.0;
        relationship.reappraisal_debt = 35.0;
    });

    assert_eq!(
        first_relationship_delta(&result).reappraisal_state_code,
        Some(2)
    );
}

#[test]
fn reappraisal_revises_after_strong_contradiction() {
    let mut row = relationship_event_base_row();
    set_row_i64(&mut row, "salience", 100);
    set_row_i64(&mut row, "certainty", 100);
    set_row_i64(&mut row, "directness", 100);
    set_row_i64(&mut row, "costliness", 100);
    set_row_i64(&mut row, "stakes", 100);
    set_row_i64(&mut row, "repetition", 100);
    row.as_object_mut().unwrap().insert(
        "event_flags_u64".into(),
        serde_json::Value::from(32_u64 | 64_u64),
    );
    let result = compile_relationship_event_row(row, |soul| {
        let relationship = soul.relationships.get_mut("user").unwrap();
        relationship.asshole_bias = 100.0;
        relationship.reappraisal_debt = 65.0;
    });

    assert_eq!(
        first_relationship_delta(&result).reappraisal_state_code,
        Some(3)
    );
}

#[test]
fn first_impression_does_not_flip_from_one_minor_event() {
    let mut row = relationship_event_base_row();
    set_row_i64(&mut row, "salience", 10);
    set_row_i64(&mut row, "certainty", 20);
    row.as_object_mut()
        .unwrap()
        .insert("event_flags_u64".into(), serde_json::Value::from(2048_u64));
    let result = compile_relationship_event_row(row, |_| {});

    let delta = first_relationship_delta(&result);
    assert_eq!(delta.reappraisal_state_code, Some(1));
    assert!(delta.reappraisal_debt.is_none());
}

#[test]
fn legacy_relationship_rows_still_compile_when_no_numeric_rows() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hi", "Aurora relaxes.");
    let response = EvalFormResponse {
        relationship_rows: vec![RelationshipRow {
            source_soul_id: "aurora_soul".into(),
            target_entity_id: "default_player".into(),
            dimension: Some(RelationshipDimension::Comfort),
            direction: Some(RelationshipDirection::Increase),
            evidence_quote: "Aurora relaxes.".into(),
            ..RelationshipRow::default()
        }],
        ..EvalFormResponse::default()
    };
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert!(result.output.relationship_evaluations[0].comfort.unwrap() > 0.0);
}

#[test]
fn numeric_relationship_event_rows_take_priority_when_present() {
    let (mut soul, world) = soul_and_world();
    soul.relationships
        .get_mut("user")
        .unwrap()
        .boundary_pressure = 40.0;
    let (spec, context) = spec_and_context(&soul, &world, "Door", "Aurora waits.");
    let response = EvalFormResponse {
        relationship_event_rows: vec![relationship_event_base_row()],
        relationship_rows: vec![RelationshipRow {
            source_soul_id: "aurora_soul".into(),
            target_entity_id: "default_player".into(),
            dimension: Some(RelationshipDimension::BoundaryPressure),
            direction: Some(RelationshipDirection::Increase),
            evidence_quote: "Aurora waits.".into(),
            ..RelationshipRow::default()
        }],
        ..EvalFormResponse::default()
    };
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert_eq!(
        result
            .trace
            .relationship_delta_source
            .values()
            .next()
            .map(String::as_str),
        Some("numeric_event_v2")
    );
    assert!(result
        .trace
        .evaluator_row_traces
        .iter()
        .any(|row| { row.compiler_result == "deduped_numeric_event_v2_priority" }));
}

#[test]
fn bad_numeric_event_row_does_not_kill_legacy_relationship_rows() {
    let mut bad = relationship_event_base_row();
    bad.as_object_mut().unwrap().remove("intent");
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hi", "Aurora relaxes.");
    let response = EvalFormResponse {
        relationship_event_rows: vec![bad],
        relationship_rows: vec![RelationshipRow {
            source_soul_id: "aurora_soul".into(),
            target_entity_id: "default_player".into(),
            dimension: Some(RelationshipDimension::Comfort),
            direction: Some(RelationshipDirection::Increase),
            evidence_quote: "Aurora relaxes.".into(),
            ..RelationshipRow::default()
        }],
        ..EvalFormResponse::default()
    };
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 1);
    assert_eq!(
        result.rejected_rows[0].reason,
        "relationship_event_missing_key:intent"
    );
    assert_eq!(result.output.relationship_evaluations.len(), 1);
    assert!(result.output.relationship_evaluations[0].comfort.unwrap() > 0.0);
}

#[test]
fn door_chain_numeric_event_compiles_without_direction() {
    let result = compile_relationship_event_row(relationship_event_base_row(), |soul| {
        let relationship = soul.relationships.get_mut("user").unwrap();
        relationship.trust = 0.0;
        relationship.boundary_pressure = 30.0;
    });

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert!(!result
        .rejected_rows
        .iter()
        .any(|row| row.reason == "direction_missing_uncertain"));
    let delta = first_relationship_delta(&result);
    assert!(delta.boundary_pressure.unwrap() < 0.0);
    assert!(delta.comfort.unwrap() > 0.0);
    assert!(delta.trust.unwrap() > 0.0);
    assert!(delta.autonomy_respect_bias.unwrap() > 0.0);
    assert!(result
        .trace
        .relationship_delta_source
        .values()
        .any(|source| source == "numeric_event_v2"));
}

#[test]
fn relationship_event_row_rejects_intent_seventy() {
    let mut row = relationship_event_base_row();
    set_row_i64(&mut row, "intent", 70);
    let result = compile_relationship_event_row(row, |_| {});

    assert_eq!(result.trace.form_rows_rejected, 1);
    assert_eq!(result.rejected_rows[0].reason, "axis_out_of_range:intent");
}

#[test]
fn legacy_relationship_row_with_axis_affection_as_key_gets_rejected() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hi", "Aurora relaxes.");
    let response = parse_eval_form_response(
        r#"{
          "relationship_rows": [{
            "axis_affection": 3,
            "direction": "increase",
            "evidence_quote": "Aurora relaxes.",
            "tags": ["relationship"]
          }]
        }"#,
    )
    .expect("parsed");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 1);
    assert_eq!(
        result.rejected_rows[0].reason,
        "relationship dimension is not allowed"
    );
}

#[test]
fn legacy_relationship_row_rejects_missing_dimension() {
    let (soul, world) = soul_and_world();
    let (spec, context) = spec_and_context(&soul, &world, "Hi", "Aurora relaxes.");
    let response = EvalFormResponse {
        relationship_rows: vec![RelationshipRow {
            source_soul_id: "aurora_soul".into(),
            target_entity_id: "default_player".into(),
            dimension: None,
            direction: Some(RelationshipDirection::Increase),
            evidence_quote: "Aurora relaxes.".into(),
            ..RelationshipRow::default()
        }],
        ..EvalFormResponse::default()
    };
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 1);
    assert_eq!(
        result.rejected_rows[0].reason,
        "relationship dimension is not allowed"
    );
}

#[test]
fn test_regression_evaluator_form_fixes() {
    let (mut soul, world) = soul_and_world();
    soul.character_id = "aurora-uuid-12345".to_string();
    soul.character_name = "Aurora".to_string();

    soul.memory.recent.clear();

    let spec = build_eval_form_spec_with_player_persona(
        &soul,
        Some(&world),
        "I stay outside.",
        "Aurora watches.",
        8,
        "preset_male",
        "Jun",
    );

    let context = EvaluatorConversionContext {
        active_soul_id: &soul.character_id,
        active_soul_ids: vec![soul.character_id.clone()],
        latest_user_message: "I stay outside.",
        latest_narrator_response: "Aurora watches.",
        session_world: Some(&world),
        baseline_recent_event_id: Some("event_baseline_abc".to_string()),
    };

    // 1. Compile relationship_event source soul UUID + target preset_male
    // Also tests actor_entity_id preset_male compiles
    // And memory linked_event_id event_latest_turn compiles without event_rows
    // And object row linked to event_latest_turn compiles
    let raw_response = r#"{
      "relationship_event_rows": [{
        "row_enabled": 1,
        "event_id": "event_latest_turn",
        "actor_entity_id": "preset_male",
        "relationship_source_soul_id": "aurora-uuid-12345",
        "relationship_target_entity_id": "preset_male",
        "perceived_by_entity_id": "aurora-uuid-12345",
        "evidence_quote": "Aurora watches.",
        "intent": 3,
        "honesty": 1,
        "reliability": 1,
        "boundary_treatment": 5,
        "responsiveness": 3,
        "power_use": 1,
        "evaluation_tone": 1,
        "competence": 0,
        "disclosure": 0,
        "reciprocity": 1,
        "repair": 0,
        "predictability": 2,
        "salience": 65,
        "certainty": 95,
        "directness": 90,
        "costliness": 25,
        "stakes": 50,
        "repetition": 20,
        "event_flags_u64": 35
      }],
      "memory_rows": [{
        "row_enabled": 1,
        "owner_soul_id": "aurora-uuid-12345",
        "slot": "relationship_memory",
        "content": "Aurora remembers that Jun stayed outside.",
        "evidence_quote": "Aurora watches.",
        "linked_event_id": "event_latest_turn"
      }],
      "object_rows": [{
        "linked_event_id": "event_latest_turn",
        "object_id": "apartment_door",
        "property_changed": "status",
        "new_value": "closed",
        "evidence_quote": "Aurora watches."
      }]
    }"#;

    let response = parse_eval_form_response(raw_response).expect("parsed successfully");
    let result = compile_eval_form_response(&spec, &response, &context);

    assert_eq!(result.trace.form_rows_rejected, 0);
    assert_eq!(result.trace.form_rows_accepted, 3); // event, memory, object

    // 2. perceived_by_entity_id preset_male fails
    let raw_response_bad_perceiver = r#"{
      "relationship_event_rows": [{
        "row_enabled": 1,
        "event_id": "event_latest_turn",
        "actor_entity_id": "preset_male",
        "relationship_source_soul_id": "aurora-uuid-12345",
        "relationship_target_entity_id": "preset_male",
        "perceived_by_entity_id": "preset_male",
        "evidence_quote": "Aurora watches.",
        "intent": 3,
        "honesty": 1,
        "reliability": 1,
        "boundary_treatment": 5,
        "responsiveness": 3,
        "power_use": 1,
        "evaluation_tone": 1,
        "competence": 0,
        "disclosure": 0,
        "reciprocity": 1,
        "repair": 0,
        "predictability": 2,
        "salience": 65,
        "certainty": 95,
        "directness": 90,
        "costliness": 25,
        "stakes": 50,
        "repetition": 20,
        "event_flags_u64": 35
      }]
    }"#;
    let response_bad = parse_eval_form_response(raw_response_bad_perceiver).unwrap();
    let result_bad = compile_eval_form_response(&spec, &response_bad, &context);
    assert_eq!(result_bad.trace.form_rows_rejected, 1);
    assert_eq!(
        result_bad.rejected_rows[0].reason,
        "perceived_by_entity_resolution_failed"
    );

    // 3. Malformed evidence_quote with unescaped dialogue comma is repaired and parses successfully
    let raw_response_malformed_quote = r#"{
      "memory_rows": [{
        "row_enabled": 1,
        "owner_soul_id": "aurora-uuid-12345",
        "slot": "relationship_memory",
        "content": "Aurora remembers what was said.",
        "evidence_quote": "I said, "Hello," and walked away.",
        "linked_event_id": "event_latest_turn"
      }]
    }"#;
    let parsed_repair =
        parse_eval_form_response(raw_response_malformed_quote).expect("repaired successfully");
    assert_eq!(
        parsed_repair.memory_rows[0].evidence_quote,
        "I said, \"Hello,\" and walked away."
    );
}
