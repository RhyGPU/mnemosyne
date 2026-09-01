use serde_json::{json, Value};
use state_engine::compiler::{
    compile_perception_pipeline, lower_state_effects_to_engine_patch, perception_ir_json_schema,
    seal_perception_batch, BehaviorEvidenceKind, ClaimValue, CompilerDiagnostic, DurabilityHint,
    EffectProvenance, EntityCatalog, EntityDescriptor, EntityRole, EpistemicMode, EvidenceSource,
    EvidenceSpan, LoweringReport, MemoryFormationKind, ModelProvenance, PerceptionBatchDraft,
    PerceptionCandidateDraft, PerceptionKind, ProposedTransaction, RelationshipSignalDraft,
    SimulationDecision, SimulationSnapshot, SourceEnvelope, SourceIdentity, StateEffect,
    StateEffectKind, TemporalAnchor, TemporalExpression, MEMORY_COMPILER_CONTRACT_VERSION,
    PERCEPTION_IR_SCHEMA_VERSION, SOURCE_ENVELOPE_SCHEMA_VERSION,
};

fn identity() -> SourceIdentity {
    SourceIdentity {
        conversation_id: "conversation-1".into(),
        branch_id: "branch-main".into(),
        turn_id: "turn-7".into(),
        parent_turn_id: Some("turn-6".into()),
        user_message_id: 41,
        assistant_message_id: 42,
        assistant_variant_id: Some(3),
    }
}

fn source(assistant_text: &str) -> SourceEnvelope {
    SourceEnvelope::new(
        identity(),
        vec!["soul-b".into(), "soul-a".into(), "soul-a".into()],
        "I return the brass key as promised.",
        assistant_text,
        Some("state:abc123".into()),
        1_785_340_800_000,
    )
    .expect("valid source envelope")
}

fn candidate() -> PerceptionCandidateDraft {
    PerceptionCandidateDraft {
        kind: PerceptionKind::RelationshipEvidence,
        subject_ref: "active_player".into(),
        predicate: "returned_as_promised".into(),
        object: Some(ClaimValue::EntityRef {
            entity_ref: "brass_key".into(),
        }),
        actor_ref: Some("active_player".into()),
        perceiver_ref: Some("active_soul".into()),
        target_refs: vec!["active_soul".into()],
        evidence: EvidenceSpan {
            source: EvidenceSource::AssistantMessage,
            quote: "returns the brass key exactly as promised".into(),
            start_char: None,
            end_char: None,
        },
        epistemic_mode: EpistemicMode::DirectlyObserved,
        extraction_confidence: 0.94,
        temporal: TemporalExpression {
            anchor: TemporalAnchor::CurrentTurn,
            expression: None,
        },
        durability_hint: DurabilityHint::LongTerm,
        relationship_signal: Some(RelationshipSignalDraft {
            behaviors: vec![BehaviorEvidenceKind::PromiseKept],
            valence: 4,
            directness: 100,
            stakes: 60,
            costliness: 20,
            repetition: 0,
        }),
    }
}

fn draft() -> PerceptionBatchDraft {
    PerceptionBatchDraft {
        schema_version: PERCEPTION_IR_SCHEMA_VERSION,
        candidates: vec![candidate()],
        no_op_reason: None,
    }
}

fn producer() -> ModelProvenance {
    ModelProvenance {
        provider: "test-provider".into(),
        model: "test-model".into(),
        prompt_version: "perception-v2.0".into(),
        schema_name: "perception_ir_v2".into(),
    }
}

fn catalog() -> EntityCatalog {
    EntityCatalog {
        entities: vec![
            EntityDescriptor {
                entity_id: "soul-a".into(),
                display_name: "Aurora".into(),
                aliases: vec!["active_soul".into()],
                role: EntityRole::Soul,
                active: true,
            },
            EntityDescriptor {
                entity_id: "player-1".into(),
                display_name: "Visitor".into(),
                aliases: vec!["active_player".into(), "latest_speaker".into()],
                role: EntityRole::ActivePlayer,
                active: true,
            },
            EntityDescriptor {
                entity_id: "key-1".into(),
                display_name: "Brass Key".into(),
                aliases: vec!["brass_key".into(), "key".into()],
                role: EntityRole::Object,
                active: true,
            },
        ],
    }
}

#[test]
fn source_envelope_is_canonical_deterministic_and_tamper_evident() {
    let first = source("Aurora watches as the visitor returns the brass key exactly as promised.");
    let second = source("Aurora watches as the visitor returns the brass key exactly as promised.");

    assert_eq!(first.schema_version(), SOURCE_ENVELOPE_SCHEMA_VERSION);
    assert_eq!(first, second);
    assert_eq!(first.active_soul_ids(), ["soul-a", "soul-b"]);
    assert!(first.source_hash().starts_with("fnv1a64:"));
    first.validate().expect("untampered envelope validates");

    let mut tampered = serde_json::to_value(&first).expect("serialize source");
    tampered["assistant_text"] = json!("The source text was changed after sealing.");
    let tampered: SourceEnvelope =
        serde_json::from_value(tampered).expect("serde alone can read archived artifacts");
    let error = tampered
        .validate()
        .expect_err("changed content must fail source authority validation");
    assert_eq!(error.code, "source_hash_mismatch");
}

#[test]
fn source_hash_changes_when_creating_exchange_changes() {
    let first = source("Aurora sees the visitor return the key.");
    let second = source("Aurora sees the visitor keep the key.");
    assert_ne!(first.source_hash(), second.source_hash());
}

#[test]
fn llm_draft_rejects_engine_owned_authority_fields() {
    let base = serde_json::to_value(draft()).expect("serialize draft");

    for field in [
        "source_hash",
        "conversation_id",
        "branch_id",
        "turn_id",
        "source_message_id",
        "compiler_version",
        "truth_status",
        "effect",
    ] {
        let mut value = base.clone();
        value
            .as_object_mut()
            .expect("draft object")
            .insert(field.into(), json!("forged"));
        let error = serde_json::from_value::<PerceptionBatchDraft>(value)
            .expect_err("top-level authority field must be rejected");
        assert!(
            error.to_string().contains("unknown field"),
            "{field}: {error}"
        );
    }

    for field in [
        "source_hash",
        "source_message_id",
        "truth_status",
        "state_delta",
        "effect",
    ] {
        let mut value = base.clone();
        value["candidates"][0]
            .as_object_mut()
            .expect("candidate object")
            .insert(field.into(), json!("forged"));
        let error = serde_json::from_value::<PerceptionBatchDraft>(value)
            .expect_err("candidate authority field must be rejected");
        assert!(
            error.to_string().contains("unknown field"),
            "{field}: {error}"
        );
    }
}

#[test]
fn rust_seals_draft_with_deterministic_source_and_candidate_identity() {
    let source = source("Aurora watches as the visitor returns the brass key exactly as promised.");
    let first = seal_perception_batch(&source, draft(), producer()).expect("first batch seals");
    let second = seal_perception_batch(&source, draft(), producer()).expect("replay batch seals");

    assert_eq!(first, second);
    assert_eq!(first.compiler_version, MEMORY_COMPILER_CONTRACT_VERSION);
    assert_eq!(first.source_hash, source.source_hash());
    assert_eq!(first.candidates.len(), 1);
    assert_eq!(first.candidates[0].source_hash, source.source_hash());
    assert!(first.candidates[0].candidate_id.starts_with("fnv1a64:"));
    assert_eq!(
        serde_json::to_vec(&first).expect("serialize first"),
        serde_json::to_vec(&second).expect("serialize replay"),
        "sealed artifact bytes must replay deterministically"
    );
}

#[test]
fn candidate_identity_is_bound_to_source_and_candidate_position() {
    let first_source =
        source("Aurora watches as the visitor returns the brass key exactly as promised.");
    let second_source = source("Aurora only hears that the visitor returned the key.");
    let first = seal_perception_batch(&first_source, draft(), producer()).expect("first");
    let second = seal_perception_batch(&second_source, draft(), producer()).expect("second");
    assert_ne!(
        first.candidates[0].candidate_id,
        second.candidates[0].candidate_id
    );

    let mut repeated_draft = draft();
    repeated_draft.candidates.push(candidate());
    let repeated =
        seal_perception_batch(&first_source, repeated_draft, producer()).expect("repeated");
    assert_ne!(
        repeated.candidates[0].candidate_id, repeated.candidates[1].candidate_id,
        "candidate position prevents duplicate artifact identity"
    );
}

#[test]
fn perception_schema_and_model_provenance_are_required() {
    let source = source("Aurora sees the visitor return the key.");
    let mut wrong_schema = draft();
    wrong_schema.schema_version = 999;
    assert_eq!(
        seal_perception_batch(&source, wrong_schema, producer())
            .expect_err("wrong schema")
            .code,
        "unsupported_perception_schema"
    );

    let mut missing_model = producer();
    missing_model.model.clear();
    assert_eq!(
        seal_perception_batch(&source, draft(), missing_model)
            .expect_err("missing model provenance")
            .code,
        "missing_model_provenance"
    );
}

#[test]
fn draft_sealing_rejects_invalid_candidate_primitives() {
    let source = source("Aurora sees the visitor return the key.");

    let mut invalid_confidence = draft();
    invalid_confidence.candidates[0].extraction_confidence = f32::NAN;
    assert_eq!(
        seal_perception_batch(&source, invalid_confidence, producer())
            .expect_err("non-finite confidence")
            .code,
        "invalid_extraction_confidence"
    );

    let mut invalid_span = draft();
    invalid_span.candidates[0].evidence.start_char = Some(20);
    invalid_span.candidates[0].evidence.end_char = Some(10);
    assert_eq!(
        seal_perception_batch(&source, invalid_span, producer())
            .expect_err("reversed evidence span")
            .code,
        "invalid_evidence_span"
    );
}

#[test]
fn effects_and_transactions_inherit_engine_owned_provenance() {
    let source = source("Aurora watches as the visitor returns the brass key exactly as promised.");
    let batch = seal_perception_batch(&source, draft(), producer()).expect("batch");
    let candidate = &batch.candidates[0];
    let first = EffectProvenance::from_candidate(&source, candidate, 0).expect("effect provenance");
    let replay =
        EffectProvenance::from_candidate(&source, candidate, 0).expect("replay provenance");
    assert_eq!(first, replay);

    let effect = StateEffect {
        provenance: first,
        effect: StateEffectKind::FormMemory {
            owner_soul_id: "soul-a".into(),
            memory_kind: MemoryFormationKind::Episode,
            content: "The visitor returned the brass key as promised.".into(),
            target_entity_ids: vec!["active_player".into()],
        },
    };
    let transaction = ProposedTransaction::try_from_lowering(
        &source,
        LoweringReport {
            source_hash: source.source_hash().into(),
            effects: vec![effect],
            diagnostics: Vec::<CompilerDiagnostic>::new(),
        },
    )
    .expect("trusted lowering becomes a transaction");

    assert_eq!(transaction.source_hash, source.source_hash());
    assert_eq!(
        transaction.parent_state_hash.as_deref(),
        Some("state:abc123")
    );
    assert_eq!(transaction.effects.len(), 1);
}

#[test]
fn transaction_rejects_cross_source_effect_injection() {
    let primary_source = source("Aurora sees the visitor return the key.");
    let other_source = source("Aurora sees the visitor keep the key.");
    let batch = seal_perception_batch(&other_source, draft(), producer()).expect("batch");
    let provenance = EffectProvenance::from_candidate(&other_source, &batch.candidates[0], 0)
        .expect("other provenance");
    let report = LoweringReport {
        source_hash: primary_source.source_hash().into(),
        effects: vec![StateEffect {
            provenance,
            effect: StateEffectKind::RecordIntention {
                owner_entity_id: "active_player".into(),
                content: "Keep the key.".into(),
                target_entity_ids: Vec::new(),
            },
        }],
        diagnostics: Vec::new(),
    };

    assert_eq!(
        ProposedTransaction::try_from_lowering(&primary_source, report)
            .expect_err("cross-source effect must not enter a transaction")
            .code,
        "effect_provenance_mismatch"
    );
}

#[test]
fn archived_contracts_round_trip_without_losing_type_information() {
    let source = source("Aurora watches as the visitor returns the brass key exactly as promised.");
    let batch = seal_perception_batch(&source, draft(), producer()).expect("batch");
    let encoded = serde_json::to_value(&batch).expect("encode");
    let decoded = serde_json::from_value(encoded).expect("decode");
    assert_eq!(batch, decoded);

    let value: Value = serde_json::to_value(candidate()).expect("candidate json");
    assert_eq!(value["kind"], "relationship_evidence");
    assert_eq!(value["epistemic_mode"], "directly_observed");
    assert_eq!(value["temporal"]["anchor"], "current_turn");
}

#[test]
fn provider_schema_is_strict_and_exposes_only_perception_fields() {
    let schema = perception_ir_json_schema();
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(
        schema["properties"]["schema_version"]["enum"][0],
        PERCEPTION_IR_SCHEMA_VERSION
    );
    let candidate = &schema["properties"]["candidates"]["items"];
    assert_eq!(candidate["additionalProperties"], false);

    let text = serde_json::to_string(&schema).expect("schema json");
    for required in [
        "relationship_evidence",
        "directly_observed",
        "evidence",
        "durability_hint",
    ] {
        assert!(text.contains(required), "schema missing {required}");
    }
    for forbidden in [
        "source_hash",
        "source_message_id",
        "conversation_id",
        "branch_id",
        "truth_status",
        "verified_engine",
        "state_delta",
        "engine_patch",
    ] {
        assert!(
            !text.contains(forbidden),
            "authority field {forbidden} leaked into provider schema"
        );
    }
}

#[test]
fn deterministic_pipeline_binds_validates_lowers_and_simulates_relationship_evidence() {
    let source = source("Aurora watches as the visitor returns the brass key exactly as promised.");
    let batch = seal_perception_batch(&source, draft(), producer()).expect("batch");
    let report = compile_perception_pipeline(
        &source,
        &batch,
        catalog(),
        &SimulationSnapshot {
            state_hash: Some("state:abc123".into()),
            existing_effect_ids: Vec::new(),
        },
    );

    assert!(report
        .binding
        .candidates
        .iter()
        .flat_map(|candidate| candidate.bindings.iter())
        .all(|binding| binding.resolved_entity_id.is_some()));
    assert!(report
        .semantic
        .candidates
        .iter()
        .all(|candidate| candidate.disposition
            == state_engine::compiler::SemanticDisposition::Accepted));
    assert_eq!(report.lowering.effects.len(), 1);
    let StateEffectKind::ApplyRelationshipEvidence {
        source_soul_id,
        target_entity_id,
        signal,
    } = &report.lowering.effects[0].effect
    else {
        panic!("relationship perception must lower to relationship evidence");
    };
    assert_eq!(source_soul_id, "soul-a");
    assert_eq!(target_entity_id, "player-1");
    assert_eq!(signal.behaviors, vec![BehaviorEvidenceKind::PromiseKept]);
    assert_eq!(report.simulation.decision, SimulationDecision::CommitReady);
    assert_eq!(report.simulation.effects.len(), 1);
}

fn scene_candidate(predicate: &str, value: Option<&str>) -> PerceptionCandidateDraft {
    PerceptionCandidateDraft {
        kind: PerceptionKind::SceneObservation,
        subject_ref: "active_soul".into(),
        predicate: predicate.into(),
        object: value.map(|text| ClaimValue::Text { text: text.into() }),
        actor_ref: None,
        perceiver_ref: Some("active_soul".into()),
        target_refs: vec!["active_player".into()],
        evidence: EvidenceSpan {
            source: EvidenceSource::AssistantMessage,
            quote: "the door stays on its chain".into(),
            start_char: None,
            end_char: None,
        },
        epistemic_mode: EpistemicMode::NarratorDescribed,
        extraction_confidence: 0.9,
        temporal: TemporalExpression {
            anchor: TemporalAnchor::CurrentTurn,
            expression: None,
        },
        durability_hint: DurabilityHint::Turn,
        relationship_signal: None,
    }
}

fn scene_source() -> SourceEnvelope {
    source("Aurora keeps the door on its chain while the visitor waits, and the door stays on its chain.")
}

#[test]
fn scene_perception_lowers_to_the_matching_continuity_slot() {
    let source = scene_source();
    let draft = PerceptionBatchDraft {
        schema_version: PERCEPTION_IR_SCHEMA_VERSION,
        candidates: vec![scene_candidate(
            "room_state",
            Some("Door open on its chain"),
        )],
        no_op_reason: None,
    };
    let batch = seal_perception_batch(&source, draft, producer()).expect("batch");
    let report = compile_perception_pipeline(
        &source,
        &batch,
        catalog(),
        &SimulationSnapshot {
            state_hash: Some("state:abc123".into()),
            existing_effect_ids: Vec::new(),
        },
    );

    assert_eq!(report.lowering.effects.len(), 1);
    let StateEffectKind::UpdateSceneProjection { slot, value, .. } =
        &report.lowering.effects[0].effect
    else {
        panic!("scene perception must lower to a scene projection");
    };
    assert_eq!(*slot, state_engine::compiler::SceneSlot::RoomState);
    assert_eq!(value.as_deref(), Some("Door open on its chain"));
    assert_eq!(report.simulation.decision, SimulationDecision::CommitReady);
}

#[test]
fn several_scene_claims_merge_instead_of_overwriting_each_other() {
    let source = scene_source();
    let draft = PerceptionBatchDraft {
        schema_version: PERCEPTION_IR_SCHEMA_VERSION,
        candidates: vec![
            scene_candidate("room_state", Some("Door open on its chain")),
            scene_candidate("open_question", Some("Who sent the visitor?")),
            scene_candidate("active_object", Some("brass key")),
        ],
        no_op_reason: None,
    };
    let batch = seal_perception_batch(&source, draft, producer()).expect("batch");
    let report = compile_perception_pipeline(
        &source,
        &batch,
        catalog(),
        &SimulationSnapshot {
            state_hash: Some("state:abc123".into()),
            existing_effect_ids: Vec::new(),
        },
    );
    let lowered = lower_state_effects_to_engine_patch(&source, &report.lowering.effects);
    let scene = lowered
        .patch
        .world_patch
        .as_ref()
        .and_then(|world| world.scene_state.as_ref())
        .expect("scene state patch");

    assert_eq!(scene.room_state.as_deref(), Some("Door open on its chain"));
    assert_eq!(
        scene.open_question.as_deref(),
        Some("Who sent the visitor?")
    );
    assert_eq!(scene.active_object.as_deref(), Some("brass key"));
    assert!(scene.participants.contains(&"player-1".to_string()));
}

#[test]
fn absent_scene_value_clears_resolved_truth() {
    let source = scene_source();
    let draft = PerceptionBatchDraft {
        schema_version: PERCEPTION_IR_SCHEMA_VERSION,
        candidates: vec![scene_candidate("misunderstanding", None)],
        no_op_reason: None,
    };
    let batch = seal_perception_batch(&source, draft, producer()).expect("batch");
    let report = compile_perception_pipeline(
        &source,
        &batch,
        catalog(),
        &SimulationSnapshot {
            state_hash: Some("state:abc123".into()),
            existing_effect_ids: Vec::new(),
        },
    );
    let lowered = lower_state_effects_to_engine_patch(&source, &report.lowering.effects);
    let scene = lowered
        .patch
        .world_patch
        .as_ref()
        .and_then(|world| world.scene_state.as_ref())
        .expect("scene state patch");

    assert_eq!(scene.current_misunderstanding.as_deref(), Some(""));
}

#[test]
fn hearsay_scene_claim_becomes_memory_not_current_truth() {
    let source = scene_source();
    let mut candidate = scene_candidate("room_state", Some("Door wide open"));
    candidate.epistemic_mode = EpistemicMode::StatedBy;
    // stated_by requires a bound speaker, which is a pre-existing semantic rule.
    candidate.actor_ref = Some("active_player".into());
    let draft = PerceptionBatchDraft {
        schema_version: PERCEPTION_IR_SCHEMA_VERSION,
        candidates: vec![candidate],
        no_op_reason: None,
    };
    let batch = seal_perception_batch(&source, draft, producer()).expect("batch");
    let report = compile_perception_pipeline(
        &source,
        &batch,
        catalog(),
        &SimulationSnapshot {
            state_hash: Some("state:abc123".into()),
            existing_effect_ids: Vec::new(),
        },
    );

    assert!(matches!(
        report.lowering.effects[0].effect,
        StateEffectKind::FormMemory { .. }
    ));
}

#[test]
fn past_anchored_scene_claim_is_rejected_as_stale() {
    let source = scene_source();
    let mut candidate = scene_candidate("room_state", Some("Door was open"));
    candidate.temporal.anchor = TemporalAnchor::BeforeCurrentTurn;
    let draft = PerceptionBatchDraft {
        schema_version: PERCEPTION_IR_SCHEMA_VERSION,
        candidates: vec![candidate],
        no_op_reason: None,
    };
    let batch = seal_perception_batch(&source, draft, producer()).expect("batch");
    let report = compile_perception_pipeline(
        &source,
        &batch,
        catalog(),
        &SimulationSnapshot {
            state_hash: Some("state:abc123".into()),
            existing_effect_ids: Vec::new(),
        },
    );

    assert!(report
        .semantic
        .candidates
        .iter()
        .any(|candidate| candidate.disposition
            != state_engine::compiler::SemanticDisposition::Accepted));
    assert!(report.lowering.effects.is_empty());
}

#[test]
fn unknown_scene_predicate_commits_nothing() {
    let source = scene_source();
    let draft = PerceptionBatchDraft {
        schema_version: PERCEPTION_IR_SCHEMA_VERSION,
        candidates: vec![scene_candidate("vibes", Some("tense"))],
        no_op_reason: None,
    };
    let batch = seal_perception_batch(&source, draft, producer()).expect("batch");
    let report = compile_perception_pipeline(
        &source,
        &batch,
        catalog(),
        &SimulationSnapshot {
            state_hash: Some("state:abc123".into()),
            existing_effect_ids: Vec::new(),
        },
    );

    assert!(
        report.lowering.effects.is_empty(),
        "an unrecognized slot must not be guessed into a real field"
    );
}

#[test]
fn knowledge_claim_lowers_to_a_recorded_epistemic_position() {
    let source = scene_source();
    let mut candidate = scene_candidate("hiding", Some("she kept the spare key"));
    candidate.kind = PerceptionKind::KnowledgeClaim;
    candidate.subject_ref = "active_soul".into();
    candidate.target_refs = vec!["active_player".into()];
    let draft = PerceptionBatchDraft {
        schema_version: PERCEPTION_IR_SCHEMA_VERSION,
        candidates: vec![candidate],
        no_op_reason: None,
    };
    let batch = seal_perception_batch(&source, draft, producer()).expect("batch");
    let report = compile_perception_pipeline(
        &source,
        &batch,
        catalog(),
        &SimulationSnapshot {
            state_hash: Some("state:abc123".into()),
            existing_effect_ids: Vec::new(),
        },
    );

    let StateEffectKind::RecordKnowledge {
        holder_entity_id,
        proposition,
        status,
        counterpart_entity_id,
    } = &report.lowering.effects[0].effect
    else {
        panic!("knowledge perception must lower to a knowledge effect");
    };
    assert_eq!(holder_entity_id, "soul-a");
    assert_eq!(proposition, "she kept the spare key");
    assert_eq!(*status, state_engine::soul::KnowledgeStatus::Hiding);
    // Concealment is always from someone; the counterpart is not optional detail.
    assert_eq!(counterpart_entity_id.as_deref(), Some("player-1"));
}

#[test]
fn unknown_epistemic_predicate_commits_nothing() {
    let source = scene_source();
    let mut candidate = scene_candidate("vaguely_senses", Some("something is off"));
    candidate.kind = PerceptionKind::KnowledgeClaim;
    let draft = PerceptionBatchDraft {
        schema_version: PERCEPTION_IR_SCHEMA_VERSION,
        candidates: vec![candidate],
        no_op_reason: None,
    };
    let batch = seal_perception_batch(&source, draft, producer()).expect("batch");
    let report = compile_perception_pipeline(
        &source,
        &batch,
        catalog(),
        &SimulationSnapshot {
            state_hash: Some("state:abc123".into()),
            existing_effect_ids: Vec::new(),
        },
    );

    assert!(report.lowering.effects.is_empty());
}

#[test]
fn hearsay_event_forms_testimony_instead_of_world_truth() {
    let source = source("Aurora watches as the visitor returns the brass key exactly as promised.");
    let mut hearsay = candidate();
    hearsay.kind = PerceptionKind::Event;
    hearsay.epistemic_mode = EpistemicMode::StatedBy;
    hearsay.evidence.source = EvidenceSource::UserMessage;
    hearsay.evidence.quote = "I return the brass key as promised".into();
    hearsay.relationship_signal = None;
    let batch = seal_perception_batch(
        &source,
        PerceptionBatchDraft {
            schema_version: PERCEPTION_IR_SCHEMA_VERSION,
            candidates: vec![hearsay],
            no_op_reason: None,
        },
        producer(),
    )
    .expect("hearsay batch");
    let report = compile_perception_pipeline(
        &source,
        &batch,
        catalog(),
        &SimulationSnapshot {
            state_hash: Some("state:abc123".into()),
            existing_effect_ids: Vec::new(),
        },
    );
    assert!(matches!(
        report.lowering.effects[0].effect,
        StateEffectKind::FormMemory {
            memory_kind: MemoryFormationKind::Testimony,
            ..
        }
    ));
    assert!(!report
        .lowering
        .effects
        .iter()
        .any(|effect| matches!(effect.effect, StateEffectKind::AppendWorldEvent { .. })));
}

#[test]
fn unsupported_evidence_never_reaches_a_commit_ready_effect() {
    let source = source("Aurora watches as the visitor returns the brass key exactly as promised.");
    let mut fabricated = candidate();
    fabricated.evidence.quote = "A dragon landed on the roof.".into();
    let batch = seal_perception_batch(
        &source,
        PerceptionBatchDraft {
            schema_version: PERCEPTION_IR_SCHEMA_VERSION,
            candidates: vec![fabricated],
            no_op_reason: None,
        },
        producer(),
    )
    .expect("syntactically valid batch");
    let report = compile_perception_pipeline(
        &source,
        &batch,
        catalog(),
        &SimulationSnapshot {
            state_hash: Some("state:abc123".into()),
            existing_effect_ids: Vec::new(),
        },
    );
    assert!(report.lowering.effects.is_empty());
    assert_eq!(report.simulation.decision, SimulationDecision::Rejected);
    assert!(report.simulation.effects.is_empty());
    assert!(report
        .semantic
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "evidence_quote_not_found"));
}

#[test]
fn validated_relationship_effect_lowers_to_bounded_v1_patch() {
    let source = source("Aurora watches as the visitor returns the brass key exactly as promised.");
    let batch = seal_perception_batch(&source, draft(), producer()).expect("batch");
    let pipeline = compile_perception_pipeline(
        &source,
        &batch,
        catalog(),
        &SimulationSnapshot {
            state_hash: Some("state:abc123".into()),
            existing_effect_ids: Vec::new(),
        },
    );
    let report = lower_state_effects_to_engine_patch(&source, &pipeline.simulation.effects);
    assert!(report.unsupported_effect_ids.is_empty());
    let delta = &report
        .patch
        .soul_patch
        .expect("soul patch")
        .relationship_deltas[0];
    assert_eq!(delta.from.as_deref(), Some("soul-a"));
    assert_eq!(delta.target.as_deref(), Some("player-1"));
    assert!(delta.trust.is_some_and(|value| value > 0.0 && value <= 5.0));
    assert_eq!(delta.max_abs_delta, Some(5.0));
}

#[test]
fn testimony_patch_uses_trusted_source_and_never_engine_truth() {
    let source = source("Aurora watches as the visitor returns the brass key exactly as promised.");
    let mut hearsay = candidate();
    hearsay.kind = PerceptionKind::Event;
    hearsay.epistemic_mode = EpistemicMode::StatedBy;
    hearsay.evidence.source = EvidenceSource::UserMessage;
    hearsay.evidence.quote = "I return the brass key as promised".into();
    hearsay.relationship_signal = None;
    let batch = seal_perception_batch(
        &source,
        PerceptionBatchDraft {
            schema_version: PERCEPTION_IR_SCHEMA_VERSION,
            candidates: vec![hearsay],
            no_op_reason: None,
        },
        producer(),
    )
    .expect("batch");
    let pipeline = compile_perception_pipeline(
        &source,
        &batch,
        catalog(),
        &SimulationSnapshot {
            state_hash: Some("state:abc123".into()),
            existing_effect_ids: Vec::new(),
        },
    );
    let report = lower_state_effects_to_engine_patch(&source, &pipeline.simulation.effects);
    let memory = &report.patch.soul_patch.expect("soul patch").new_memories[0];
    assert_eq!(
        memory.source_conversation_id.as_deref(),
        Some("conversation-1")
    );
    assert_eq!(memory.source_session_id.as_deref(), Some("branch-main"));
    assert_eq!(memory.source_message_id, Some(41));
    assert_eq!(
        memory.truth_status,
        Some(state_engine::soul::TruthStatus::CharacterBelief)
    );
    assert_eq!(memory.architecture_verified, Some(false));
}

#[test]
fn metamorphic_actor_perceiver_negation_and_paraphrase_change_only_owned_semantics() {
    let source = source(
        "The visitor intends to keep the brass key. Aurora intends to keep the brass key. \
         Aurora sees the brass key on the table, and the visitor sees the brass key on the table. \
         The visitor returned the brass key. The brass key was returned by the visitor. \
         The visitor did not return the brass key.",
    );
    let compile = |candidate: PerceptionCandidateDraft| {
        let batch = seal_perception_batch(
            &source,
            PerceptionBatchDraft {
                schema_version: PERCEPTION_IR_SCHEMA_VERSION,
                candidates: vec![candidate],
                no_op_reason: None,
            },
            producer(),
        )
        .expect("metamorphic batch");
        compile_perception_pipeline(
            &source,
            &batch,
            catalog(),
            &SimulationSnapshot {
                state_hash: Some("state:abc123".into()),
                existing_effect_ids: Vec::new(),
            },
        )
        .simulation
        .effects
        .into_iter()
        .map(|effect| effect.effect)
        .collect::<Vec<_>>()
    };

    let mut visitor_intention = candidate();
    visitor_intention.kind = PerceptionKind::Intention;
    visitor_intention.subject_ref = "active_player".into();
    visitor_intention.actor_ref = Some("active_player".into());
    visitor_intention.target_refs = vec!["brass_key".into()];
    visitor_intention.relationship_signal = None;
    visitor_intention.epistemic_mode = EpistemicMode::NarratorDescribed;
    visitor_intention.evidence.quote = "visitor intends to keep the brass key".into();
    visitor_intention.predicate = "intends_to_keep".into();
    let mut aurora_intention = visitor_intention.clone();
    aurora_intention.subject_ref = "active_soul".into();
    aurora_intention.actor_ref = Some("active_soul".into());
    aurora_intention.evidence.quote = "Aurora intends to keep the brass key".into();
    let visitor_effect = compile(visitor_intention);
    let aurora_effect = compile(aurora_intention);
    assert!(matches!(
        &visitor_effect[0],
        StateEffectKind::RecordIntention {
            owner_entity_id,
            ..
        } if owner_entity_id == "player-1"
    ));
    assert!(matches!(
        &aurora_effect[0],
        StateEffectKind::RecordIntention {
            owner_entity_id,
            ..
        } if owner_entity_id == "soul-a"
    ));

    let mut observed = candidate();
    observed.kind = PerceptionKind::ObjectObservation;
    observed.subject_ref = "brass_key".into();
    observed.actor_ref = None;
    observed.target_refs.clear();
    observed.relationship_signal = None;
    observed.epistemic_mode = EpistemicMode::DirectlyObserved;
    observed.evidence.quote = "Aurora sees the brass key on the table".into();
    observed.predicate = "rests_on_table".into();
    let mut player_observed = observed.clone();
    player_observed.perceiver_ref = Some("active_player".into());
    player_observed.evidence.quote = "visitor sees the brass key on the table".into();
    let soul_observation = compile(observed);
    let player_observation = compile(player_observed);
    assert!(matches!(
        &soul_observation[0],
        StateEffectKind::RecordObjectObservation {
            observer_entity_id,
            ..
        } if observer_entity_id == "soul-a"
    ));
    assert!(matches!(
        &player_observation[0],
        StateEffectKind::RecordObjectObservation {
            observer_entity_id,
            ..
        } if observer_entity_id == "player-1"
    ));

    let mut positive = candidate();
    positive.kind = PerceptionKind::Event;
    positive.relationship_signal = None;
    positive.epistemic_mode = EpistemicMode::NarratorDescribed;
    positive.evidence.quote = "visitor returned the brass key".into();
    positive.predicate = "returned".into();
    let mut paraphrase = positive.clone();
    paraphrase.evidence.quote = "brass key was returned by the visitor".into();
    let mut negated = positive.clone();
    negated.evidence.quote = "visitor did not return the brass key".into();
    negated.predicate = "did_not_return".into();
    assert_eq!(compile(positive.clone()), compile(paraphrase));
    assert_ne!(compile(positive), compile(negated));
}
