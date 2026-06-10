use std::collections::{HashMap, HashSet};

use crate::{
    evaluator::{
        evaluator_output_to_engine_patch, turn_flags, EvaluatorConversionContext,
        EvaluatorConversionReport, EvaluatorOutputV1, GlobalSceneEvaluation, MemoryCandidate,
        MemorySlot, ObjectChangeEvaluation, RelationshipEvaluation, RelevanceTags,
        TurnClassification, WorldChangeEvaluation, EVALUATOR_SCHEMA_VERSION,
    },
    evaluator_form::{
        active_player_entity_id, clean, normalize_eval_form_response, normalize_player_id,
        relationship_evaluation_has_delta, relationship_event_row_id,
        relationship_from_numeric_event_row, resolve_active_entity_id, slugify, validate_event_row,
        validate_memory_row, validate_object_row, validate_relationship_event_row,
        validate_relationship_row, validate_review_row, ConfidenceTier, EvalFormCompileResult,
        EvalFormResponse, EvalFormRowRejection, EvalFormSpec, EvalFormTrace, EvalRowTrace,
        EventRow, EventType, ExistingStateKind, ExistingStateRow, FormDedupeDecisionTrace,
        FormEntityOption, FormRelationshipState, ImportanceTier, MagnitudeTier, MemoryRow,
        ObjectRow, RelationshipDimension, RelationshipDirection, RelationshipEventValidation,
        RelationshipRow, ReviewDecision, ReviewRow, RELATIONSHIP_EVENT_TEMPLATE_VERSION,
    },
    evaluator_ingest::NormalizedEvaluationDraft,
    patch::{MemoryPatch, SceneStatePatch, PATCH_PROTOCOL_VERSION},
    setting::SessionWorld,
    soul::{MemorySourceType, ObjectState, Soul, TruthStatus},
};

pub fn build_eval_form_spec(
    soul: &Soul,
    session_world: Option<&SessionWorld>,
    latest_user_message: &str,
    latest_narrator_response: &str,
    top_k: usize,
) -> EvalFormSpec {
    build_eval_form_spec_with_player_persona(
        soul,
        session_world,
        latest_user_message,
        latest_narrator_response,
        top_k,
        "default_player",
        "User",
    )
}

pub fn build_eval_form_spec_with_player_persona(
    soul: &Soul,
    session_world: Option<&SessionWorld>,
    latest_user_message: &str,
    latest_narrator_response: &str,
    top_k: usize,
    player_persona_id: &str,
    player_persona_display_name: &str,
) -> EvalFormSpec {
    let player_persona_id = clean(player_persona_id).unwrap_or("default_player");
    let player_persona_display_name = clean(player_persona_display_name).unwrap_or("User");
    let world = session_world
        .map(SessionWorld::world_log)
        .unwrap_or_else(|| soul.world.clone());
    let mut known_object_ids = world
        .object_states
        .iter()
        .map(|object| object.object_id.clone())
        .filter(|id| !id.trim().is_empty())
        .collect::<Vec<_>>();
    known_object_ids.sort();
    known_object_ids.dedup();

    EvalFormSpec {
        form_version: crate::evaluator_form::EVALUATOR_FORM_VERSION.into(),
        active_entities: vec![
            FormEntityOption {
                entity_id: soul.character_id.clone(),
                display_name: soul.character_name.clone(),
                entity_type: "soul".into(),
            },
            FormEntityOption {
                entity_id: player_persona_id.into(),
                display_name: player_persona_display_name.into(),
                entity_type: if player_persona_id == "default_player" {
                    "user".into()
                } else {
                    "player_persona".into()
                },
            },
        ],
        active_soul_ids: vec![soul.character_id.clone()],
        active_relationship_states: relationship_states_for_spec(soul, player_persona_id),
        known_object_ids,
        allowed_memory_slots: vec![
            MemorySlot::RelationshipMemory,
            MemorySlot::CurrentPlotMemory,
            MemorySlot::CharacterIdentityMemory,
            MemorySlot::UnresolvedTension,
            MemorySlot::WorldLocationMemory,
            MemorySlot::RecentEmotionalState,
        ],
        allowed_relationship_dimensions: all_relationship_dimensions(),
        allowed_event_types: all_event_types(),
        allowed_importance_tiers: vec![
            ImportanceTier::Trivial,
            ImportanceTier::Low,
            ImportanceTier::Medium,
            ImportanceTier::High,
            ImportanceTier::Critical,
        ],
        allowed_tag_vocabularies: default_tag_vocabularies(),
        existing_memories: select_relevant_memories(
            soul,
            latest_user_message,
            latest_narrator_response,
            top_k,
        ),
        existing_events: select_relevant_events(
            &world.recent_events,
            &world.recent_event_records,
            top_k,
        ),
        existing_object_observations: world
            .object_states
            .iter()
            .take(top_k)
            .map(|object| ExistingStateRow {
                existing_id: object
                    .object_observation_id
                    .clone()
                    .unwrap_or_else(|| object.object_id.clone()),
                kind: ExistingStateKind::ObjectObservation,
                summary: format!(
                    "{}: {}",
                    object.object_id,
                    if object.last_observed_state.trim().is_empty() {
                        object.status.as_str()
                    } else {
                        object.last_observed_state.as_str()
                    }
                ),
            })
            .collect(),
        existing_relationship_facts: soul
            .relationships
            .iter()
            .take(top_k)
            .map(|(target, relation)| ExistingStateRow {
                existing_id: format!(
                    "rel:{}:{}",
                    soul.character_id,
                    relationship_target_for_spec(target, player_persona_id)
                ),
                kind: ExistingStateKind::RelationshipFact,
                summary: format!(
                    "{} -> {} trust {:.1}, affection {:.1}, comfort {:.1}, conflict {:.1}",
                    soul.character_name,
                    relationship_target_for_spec(target, player_persona_id),
                    relation.trust,
                    relation.affection,
                    relation.comfort,
                    relation.conflict
                ),
            })
            .collect(),
    }
}

fn relationship_states_for_spec(
    soul: &Soul,
    player_persona_id: &str,
) -> Vec<FormRelationshipState> {
    soul.relationships
        .iter()
        .map(|(target, relationship)| FormRelationshipState {
            source_soul_id: soul.character_id.clone(),
            target_entity_id: relationship_target_for_spec(target, player_persona_id),
            trust: relationship.trust,
            affection: relationship.affection,
            intimacy: relationship.intimacy,
            passion: relationship.passion,
            commitment: relationship.commitment,
            fear: relationship.fear,
            desire: relationship.desire,
            respect: relationship.respect,
            conflict: relationship.conflict,
            dependency: relationship.dependency,
            curiosity: relationship.curiosity,
            comfort: relationship.comfort,
            boundary_pressure: relationship.boundary_pressure,
            trustable_bias: relationship.trustable_bias,
            untrustworthy_bias: relationship.untrustworthy_bias,
            asshole_bias: relationship.asshole_bias,
            care_bias: relationship.care_bias,
            danger_bias: relationship.danger_bias,
            competence_bias: relationship.competence_bias,
            autonomy_respect_bias: relationship.autonomy_respect_bias,
            attachment_pull: relationship.attachment_pull,
            schema_threat: relationship.schema_threat,
            first_impression_strength: relationship.first_impression_strength,
            first_impression_confidence: relationship.first_impression_confidence,
            reappraisal_debt: relationship.reappraisal_debt,
            reappraisal_state_code: relationship.reappraisal_state_code,
        })
        .collect()
}

fn relationship_target_for_spec(target: &str, player_persona_id: &str) -> String {
    let normalized = normalize_player_id(target);
    if normalized == "default_player" || normalized == "user" {
        player_persona_id.to_string()
    } else {
        normalized
    }
}

pub fn compile_eval_form_response(
    spec: &EvalFormSpec,
    raw_response_struct: &EvalFormResponse,
    context: &EvaluatorConversionContext<'_>,
) -> EvalFormCompileResult {
    let response = normalize_eval_form_response(spec, raw_response_struct, context);
    let response = &response;
    let mut rejected_rows = Vec::new();
    let mut output = EvaluatorOutputV1 {
        schema_version: EVALUATOR_SCHEMA_VERSION,
        ..EvaluatorOutputV1::default()
    };

    let mut evaluator_row_traces = Vec::new();
    let mut object_row_results = HashMap::new();
    let mut memory_row_results = HashMap::new();
    let mut object_identity_registry = ObjectIdentityRegistry::new(spec);

    let mut trace = EvalFormTrace {
        form_spec_event_option_count: spec.allowed_event_types.len(),
        form_existing_memory_option_count: spec.existing_memories.len(),
        form_rows_submitted: response.event_rows.len()
            + response.object_rows.len()
            + response.relationship_rows.len()
            + response.relationship_event_rows.len()
            + response.memory_rows.len()
            + response.review_rows.len(),
        relationship_event_template_version: RELATIONSHIP_EVENT_TEMPLATE_VERSION.to_string(),
        ..EvalFormTrace::default()
    };
    trace.relationship_dimension_inferred_from = response
        .relationship_rows
        .iter()
        .filter(|row| row.dimension.is_some() && !row.selected_tags.is_empty())
        .map(|row| format!("tags:{}", row.selected_tags.join(",")))
        .collect();
    let mut direction_inferred = Vec::new();
    for (idx, row) in response.relationship_rows.iter().enumerate() {
        if row.direction.is_some() {
            let raw_has_dir = raw_response_struct
                .relationship_rows
                .get(idx)
                .and_then(|r| r.direction)
                .is_some();
            if !raw_has_dir {
                if row.summary.as_deref().and_then(clean).is_some() {
                    direction_inferred.push("summary".to_string());
                } else if row.shift.as_deref().and_then(clean).is_some() {
                    direction_inferred.push("shift".to_string());
                } else {
                    direction_inferred.push("evidence".to_string());
                }
            }
        }
    }
    trace.relationship_direction_inferred_from = direction_inferred;

    let allowed_entities = spec
        .active_entities
        .iter()
        .map(|entity| entity.entity_id.as_str())
        .collect::<HashSet<_>>();
    let mut event_ids = response
        .event_rows
        .iter()
        .filter_map(|row| clean(&row.event_id))
        .collect::<HashSet<_>>();
    if let Some(ref baseline_id) = context.baseline_recent_event_id {
        event_ids.insert(baseline_id.as_str());
    }
    event_ids.insert("event_latest_turn");
    let review_map = response
        .review_rows
        .iter()
        .filter_map(|row| {
            let decision = row.decision?;
            clean(&row.candidate_id).map(|id| (id.to_string(), (decision, row)))
        })
        .collect::<HashMap<_, _>>();

    for (idx, row) in response.event_rows.iter().enumerate() {
        let raw_row = raw_response_struct
            .event_rows
            .get(idx)
            .cloned()
            .unwrap_or_else(|| row.clone());
        let valid = validate_event_row(row, spec, &allowed_entities, &mut rejected_rows);

        let validation_status = if valid {
            "accepted".to_string()
        } else {
            "rejected".to_string()
        };
        let rejection_reason = if valid {
            None
        } else {
            rejected_rows.last().map(|r| r.reason.clone())
        };
        let compiler_result = if valid {
            "world_event_created".to_string()
        } else {
            "rejected".to_string()
        };

        evaluator_row_traces.push(EvalRowTrace {
            row_kind: "event".to_string(),
            row_index: idx,
            raw_row: serde_json::to_value(&raw_row).unwrap_or_default(),
            normalized_row: serde_json::to_value(row).unwrap_or_default(),
            validation_status,
            rejection_reason,
            compiler_result,
        });

        if valid {
            trace.form_rows_accepted += 1;
            output.world_changes.push(WorldChangeEvaluation {
                change_id: Some(row.event_id.clone()),
                location: row
                    .location
                    .as_ref()
                    .and_then(|value| clean(value).map(str::to_string)),
                event_summary: clean(&row.objective_summary).map(str::to_string),
                scene_state: Some(scene_state_from_event(row, context)),
                evidence_quote: clean(&row.evidence_quote).map(str::to_string),
                confidence: confidence_from_importance(
                    row.importance_tier.unwrap_or(ImportanceTier::Medium),
                ),
                relevance_tags: relevance_from_event(row),
                ..WorldChangeEvaluation::default()
            });
            apply_event_flags(&mut output, row);
        }
    }

    for (idx, row) in response.object_rows.iter().enumerate() {
        let raw_row = raw_response_struct
            .object_rows
            .get(idx)
            .cloned()
            .unwrap_or_else(|| row.clone());
        let disabled = object_row_disabled(row);
        let valid = disabled || validate_object_row(row, spec, &event_ids, &mut rejected_rows);

        let validation_status = if valid {
            "accepted".to_string()
        } else {
            "rejected".to_string()
        };
        let rejection_reason = if valid {
            None
        } else {
            rejected_rows.last().map(|r| r.reason.clone())
        };
        let compiler_result = if disabled {
            "disabled_row_ignored".to_string()
        } else if valid {
            "object_patch_created".to_string()
        } else {
            "rejected".to_string()
        };

        evaluator_row_traces.push(EvalRowTrace {
            row_kind: "object".to_string(),
            row_index: idx,
            raw_row: serde_json::to_value(&raw_row).unwrap_or_default(),
            normalized_row: serde_json::to_value(row).unwrap_or_default(),
            validation_status,
            rejection_reason,
            compiler_result: compiler_result.clone(),
        });

        if disabled {
            trace.form_rows_accepted += 1;
            let object_id = row
                .object_id
                .as_ref()
                .and_then(|id| clean(id).map(str::to_string))
                .or_else(|| {
                    row.new_object_label
                        .as_ref()
                        .and_then(|id| clean(id).map(slugify))
                })
                .unwrap_or_else(|| "disabled_object_row".into());
            object_row_results.insert(object_id, "disabled_row_ignored".to_string());
        } else if valid {
            let object_state =
                object_state_from_row(row, spec, context, &mut object_identity_registry);
            let object_id = object_state.object_id.clone();
            trace.form_rows_accepted += 1;
            object_row_results.insert(object_id.clone(), "patch_created".to_string());
            output.object_changes.push(ObjectChangeEvaluation {
                change_id: Some(stable_id(
                    "object_form",
                    &format!(
                        "{}:{}:{}",
                        row.linked_event_id, object_id, row.property_changed
                    ),
                )),
                object_state,
                evidence_quote: clean(&row.evidence_quote).map(str::to_string),
                confidence: confidence_from_confidence_tier(
                    row.confidence_tier.unwrap_or(ConfidenceTier::Medium),
                ),
                ..ObjectChangeEvaluation::default()
            });
            output.turn_flags_u64 |= turn_flags::OBJECT_CHANGE | turn_flags::WORLD_CHANGE;
        } else {
            let object_id = row
                .object_id
                .as_ref()
                .and_then(|id| clean(id).map(str::to_string))
                .or_else(|| {
                    row.new_object_label
                        .as_ref()
                        .and_then(|id| clean(id).map(slugify))
                })
                .unwrap_or_else(|| "unknown_object".into());
            object_row_results.insert(object_id, "rejected".to_string());
        }
    }

    let mut relationship_non_delta_count = 0;
    let mut relationship_row_results = std::collections::HashMap::new();
    let mut relationship_event_row_results = std::collections::HashMap::new();
    let mut relationship_delta_source = std::collections::HashMap::new();
    let mut numeric_relationship_pairs = HashSet::new();

    for (idx, row) in response.relationship_event_rows.iter().enumerate() {
        let row_id = relationship_event_row_id(row);
        let rejected_before = rejected_rows.len();
        let validated = validate_relationship_event_row(
            row,
            spec,
            &allowed_entities,
            &event_ids,
            &mut rejected_rows,
        );
        let rejection_reason = if validated.is_some() {
            None
        } else {
            rejected_rows
                .get(rejected_before)
                .map(|row| row.reason.clone())
        };

        let (validation_status, compiler_result, normalized_row) = match validated.as_ref() {
            Some(RelationshipEventValidation::Enabled(parsed_row)) => {
                let relation = relationship_from_numeric_event_row(parsed_row, spec);
                let has_delta = relationship_evaluation_has_delta(&relation);
                if has_delta {
                    numeric_relationship_pairs.insert((
                        relation.source_soul_id.clone(),
                        relation.target_entity_id.clone(),
                    ));
                    output.relationship_evaluations.push(relation);
                    output.turn_flags_u64 |= turn_flags::RELATIONSHIP_SHIFT;
                    relationship_event_row_results
                        .insert(row_id.clone(), "delta_created".to_string());
                    relationship_delta_source
                        .insert(row_id.clone(), "numeric_event_v2".to_string());
                    (
                        "accepted".to_string(),
                        "relationship_delta_created".to_string(),
                        serde_json::to_value(parsed_row).unwrap_or_else(|_| row.clone()),
                    )
                } else {
                    relationship_non_delta_count += 1;
                    relationship_event_row_results
                        .insert(row_id.clone(), "non_delta_no_change".to_string());
                    (
                        "accepted".to_string(),
                        "non_delta_no_change".to_string(),
                        serde_json::to_value(parsed_row).unwrap_or_else(|_| row.clone()),
                    )
                }
            }
            Some(RelationshipEventValidation::Disabled) => {
                relationship_event_row_results
                    .insert(row_id.clone(), "disabled_row_ignored".to_string());
                (
                    "accepted".to_string(),
                    "disabled_row_ignored".to_string(),
                    row.clone(),
                )
            }
            None => {
                relationship_event_row_results.insert(row_id.clone(), "rejected".to_string());
                ("rejected".to_string(), "rejected".to_string(), row.clone())
            }
        };

        evaluator_row_traces.push(EvalRowTrace {
            row_kind: "relationship_event".to_string(),
            row_index: idx,
            raw_row: raw_response_struct
                .relationship_event_rows
                .get(idx)
                .cloned()
                .unwrap_or_else(|| row.clone()),
            normalized_row,
            validation_status,
            rejection_reason,
            compiler_result,
        });

        if matches!(validated, Some(RelationshipEventValidation::Enabled(_))) {
            trace.form_rows_accepted += 1;
        }
    }

    for (idx, row) in response.relationship_rows.iter().enumerate() {
        let raw_row = raw_response_struct
            .relationship_rows
            .get(idx)
            .cloned()
            .unwrap_or_else(|| row.clone());
        let row_id = format!(
            "{}:{}:{}:{}",
            row.linked_event_id,
            row.source_soul_id,
            row.target_entity_id,
            row.dimension.map(|d| d.as_label()).unwrap_or("unknown")
        );

        if numeric_relationship_pairs
            .contains(&(row.source_soul_id.clone(), row.target_entity_id.clone()))
        {
            evaluator_row_traces.push(EvalRowTrace {
                row_kind: "relationship".to_string(),
                row_index: idx,
                raw_row: serde_json::to_value(&raw_row).unwrap_or_default(),
                normalized_row: serde_json::to_value(row).unwrap_or_default(),
                validation_status: "accepted".to_string(),
                rejection_reason: None,
                compiler_result: "deduped_numeric_event_v2_priority".to_string(),
            });
            trace.form_rows_accepted += 1;
            relationship_row_results
                .insert(row_id, "deduped_numeric_event_v2_priority".to_string());
            continue;
        }

        let valid =
            validate_relationship_row(row, spec, &allowed_entities, &event_ids, &mut rejected_rows);

        let validation_status = if valid {
            "accepted".to_string()
        } else {
            "rejected".to_string()
        };
        let rejection_reason = if valid {
            None
        } else {
            rejected_rows.last().map(|r| r.reason.clone())
        };
        let compiler_result = if valid {
            if row.direction != Some(RelationshipDirection::NoChange) {
                "relationship_delta_created".to_string()
            } else {
                "non_delta_no_change".to_string()
            }
        } else {
            "rejected".to_string()
        };

        evaluator_row_traces.push(EvalRowTrace {
            row_kind: "relationship".to_string(),
            row_index: idx,
            raw_row: serde_json::to_value(&raw_row).unwrap_or_default(),
            normalized_row: serde_json::to_value(row).unwrap_or_default(),
            validation_status,
            rejection_reason,
            compiler_result: compiler_result.clone(),
        });

        if valid {
            trace.form_rows_accepted += 1;
            if row.direction != Some(RelationshipDirection::NoChange) {
                output
                    .relationship_evaluations
                    .push(relationship_from_row(row));
                output.turn_flags_u64 |= turn_flags::RELATIONSHIP_SHIFT;
                relationship_row_results.insert(row_id.clone(), "delta_created".to_string());
                relationship_delta_source.insert(row_id, "legacy_relationship_row".to_string());
            } else {
                relationship_non_delta_count += 1;
                relationship_row_results.insert(row_id, "non_delta_no_change".to_string());
            }
        } else {
            relationship_row_results.insert(row_id, "rejected_uncertain".to_string());
        }
    }

    trace.relationship_non_delta_count = relationship_non_delta_count;
    trace.relationship_row_results = relationship_row_results;
    trace.relationship_event_row_results = relationship_event_row_results;
    trace.relationship_delta_source = relationship_delta_source;

    for (idx, row) in response.memory_rows.iter().enumerate() {
        let raw_row = raw_response_struct
            .memory_rows
            .get(idx)
            .cloned()
            .unwrap_or_else(|| row.clone());
        let candidate_id = memory_candidate_id(row);
        if memory_row_disabled(row) {
            memory_row_results.insert(candidate_id, "disabled_row_ignored".to_string());
            evaluator_row_traces.push(EvalRowTrace {
                row_kind: "memory".to_string(),
                row_index: idx,
                raw_row: serde_json::to_value(&raw_row).unwrap_or_default(),
                normalized_row: serde_json::to_value(row).unwrap_or_default(),
                validation_status: "accepted".to_string(),
                rejection_reason: None,
                compiler_result: "disabled_row_ignored".to_string(),
            });
            continue;
        }
        let review = review_map.get(&candidate_id).copied();

        let valid = validate_memory_row(row, spec, &event_ids, &mut rejected_rows);

        let validation_status = if valid {
            "accepted".to_string()
        } else {
            "rejected".to_string()
        };
        let rejection_reason = if valid {
            None
        } else {
            rejected_rows.last().map(|r| r.reason.clone())
        };

        let compiler_result = if valid {
            if matches!(
                review.map(|(decision, _)| decision),
                Some(
                    ReviewDecision::DuplicateOfExisting
                        | ReviewDecision::TooMinorNoOp
                        | ReviewDecision::NotSupportedByEvidence
                )
            ) {
                "advisory_only".to_string()
            } else {
                "memory_candidate_created".to_string()
            }
        } else {
            "rejected".to_string()
        };

        evaluator_row_traces.push(EvalRowTrace {
            row_kind: "memory".to_string(),
            row_index: idx,
            raw_row: serde_json::to_value(&raw_row).unwrap_or_default(),
            normalized_row: serde_json::to_value(row).unwrap_or_default(),
            validation_status,
            rejection_reason,
            compiler_result: compiler_result.clone(),
        });

        if let Some((decision, review_row)) = review {
            trace.form_dedupe_decisions.push(FormDedupeDecisionTrace {
                candidate_id: candidate_id.clone(),
                decision,
                existing_id: review_row.existing_id.clone(),
                reason: review_row.reason.clone(),
            });
            if !review_row.evidence_quote.trim().is_empty() {
                trace.form_rows_accepted += 1;
            }
        }
        if !valid {
            memory_row_results.insert(candidate_id, "rejected".to_string());
            continue;
        }
        if matches!(
            review.map(|(decision, _)| decision),
            Some(
                ReviewDecision::DuplicateOfExisting
                    | ReviewDecision::TooMinorNoOp
                    | ReviewDecision::NotSupportedByEvidence
            )
        ) {
            trace.form_rows_accepted += 1;
            memory_row_results.insert(candidate_id, "advisory_only".to_string());
            continue;
        }
        trace.form_rows_accepted += 1;
        memory_row_results.insert(candidate_id.clone(), "candidate_created".to_string());

        let candidate = memory_candidate_from_row(row, &candidate_id, spec);
        if row.owner_soul_id == "session_world" {
            output.world_changes.push(WorldChangeEvaluation {
                change_id: Some(candidate_id),
                event_summary: Some(row.content.clone()),
                evidence_quote: Some(row.evidence_quote.clone()),
                confidence: candidate.confidence,
                ..WorldChangeEvaluation::default()
            });
            output.turn_flags_u64 |= turn_flags::WORLD_CHANGE;
        } else {
            output.memory_candidates.push(candidate);
        }
    }

    for (idx, row) in response.review_rows.iter().enumerate() {
        let raw_row = raw_response_struct
            .review_rows
            .get(idx)
            .cloned()
            .unwrap_or_else(|| row.clone());
        let valid = validate_review_row(row, spec, &mut rejected_rows);

        let validation_status = if valid {
            "accepted".to_string()
        } else {
            "rejected".to_string()
        };
        let compiler_result = if valid {
            "advisory_only".to_string()
        } else {
            "rejected".to_string()
        };

        evaluator_row_traces.push(EvalRowTrace {
            row_kind: "review".to_string(),
            row_index: idx,
            raw_row: serde_json::to_value(&raw_row).unwrap_or_default(),
            normalized_row: serde_json::to_value(row).unwrap_or_default(),
            validation_status,
            rejection_reason: if valid {
                None
            } else {
                Some("missing_decision_or_candidate_id".to_string())
            },
            compiler_result,
        });

        if valid {
            trace.form_rows_accepted += 1;
        }
    }

    trace.form_rows_rejected = rejected_rows.len();
    trace.evaluator_row_traces = evaluator_row_traces;
    trace.object_row_results = object_row_results;
    trace.memory_row_results = memory_row_results;

    output.turn_flags_u64 = compute_turn_flags(&output);
    output.turn_classification = TurnClassification {
        is_pure_ooc: false,
        scene_event_occurred: output.turn_flags_u64 & turn_flags::SCENE_TURN != 0,
        is_retcon_or_correction: output.turn_flags_u64 & turn_flags::RETCON_OR_CORRECTION != 0,
        human_summary: response
            .event_rows
            .iter()
            .filter_map(|row| clean(&row.objective_summary))
            .next()
            .unwrap_or_default()
            .to_string(),
    };
    output.global_scene_evaluation = global_scene_from_output(&output);
    output.relevance_tags = aggregate_relevance_tags(&output);
    trace.compiled_turn_flags_u64 = output.turn_flags_u64;
    trace.code_assigned_decay_profile = response
        .memory_rows
        .iter()
        .filter(|row| !memory_row_disabled(row))
        .map(|row| {
            (
                memory_candidate_id(row),
                decay_profile(row.importance_tier.unwrap_or(ImportanceTier::Medium)).to_string(),
            )
        })
        .collect();
    trace.code_assigned_tag_weights = flatten_tag_weights(&output.relevance_tags);

    let draft = draft_from_output(&output, &rejected_rows, &trace);
    let mut conversion = evaluator_output_to_engine_patch(&output, context);
    apply_review_memory_operations(&mut conversion, response, &review_map);
    EvalFormCompileResult {
        output,
        draft,
        conversion,
        trace,
        rejected_rows,
        normalized_response: response.clone(),
    }
}

fn relationship_from_row(row: &RelationshipRow) -> RelationshipEvaluation {
    let magnitude = if let Some(ref shift_str) = row.shift {
        let clean_shift = shift_str.trim().trim_start_matches('+');
        clean_shift.parse::<f32>().unwrap_or_else(|_| {
            magnitude_value(
                row.direction.unwrap_or(RelationshipDirection::NoChange),
                row.magnitude_tier.unwrap_or(MagnitudeTier::Small),
            )
        })
    } else {
        magnitude_value(
            row.direction.unwrap_or(RelationshipDirection::NoChange),
            row.magnitude_tier.unwrap_or(MagnitudeTier::Small),
        )
    };
    let mut relation = RelationshipEvaluation {
        source_soul_id: row.source_soul_id.clone(),
        target_entity_id: row.target_entity_id.clone(),
        evidence_quote: Some(row.evidence_quote.clone()),
        criterion_met: row.direction != Some(RelationshipDirection::NoChange),
        confidence: 0.75,
        evidence_validated_by_form: true,
        ..RelationshipEvaluation::default()
    };
    match row.dimension.unwrap_or(RelationshipDimension::Trust) {
        RelationshipDimension::Trust => relation.trust = Some(magnitude),
        RelationshipDimension::Affection => relation.affection = Some(magnitude),
        RelationshipDimension::Intimacy => relation.intimacy = Some(magnitude),
        RelationshipDimension::Passion => relation.passion = Some(magnitude),
        RelationshipDimension::Commitment => relation.commitment = Some(magnitude),
        RelationshipDimension::Fear => relation.fear = Some(magnitude),
        RelationshipDimension::Desire => relation.desire = Some(magnitude),
        RelationshipDimension::Respect => relation.respect = Some(magnitude),
        RelationshipDimension::Conflict => relation.conflict = Some(magnitude),
        RelationshipDimension::Dependency => relation.dependency = Some(magnitude),
        RelationshipDimension::Curiosity => relation.curiosity = Some(magnitude),
        RelationshipDimension::Comfort => relation.comfort = Some(magnitude),
        RelationshipDimension::BoundaryPressure => relation.boundary_pressure = Some(magnitude),
    }
    relation
}

fn memory_candidate_from_row(
    row: &MemoryRow,
    candidate_id: &str,
    spec: &EvalFormSpec,
) -> MemoryCandidate {
    let importance = row.importance_tier.unwrap_or(ImportanceTier::Medium);
    let target_entity_ids = active_player_entity_id(spec)
        .map(|id| vec![id])
        .unwrap_or_else(|| vec!["default_player".into()]);
    MemoryCandidate {
        candidate_id: candidate_id.into(),
        owner_soul_id: row.owner_soul_id.clone(),
        slot: row.slot.unwrap_or(MemorySlot::Unknown),
        content: row.content.clone(),
        evidence_quote: row.evidence_quote.clone(),
        criterion_met: true,
        confidence: confidence_from_importance(importance),
        salience: Some(salience_from_importance(importance)),
        retrieval_strength: Some(retrieval_from_importance(importance)),
        perceived_by_entity_id: Some(row.owner_soul_id.clone()),
        target_entity_ids,
        source_type: MemorySourceType::CurrentSession,
        truth_status: TruthStatus::SceneEvent,
        relevance_tags: row.selected_tags.clone(),
        knowledge_scope: crate::evaluator::KnowledgeScope::DirectlyObserved,
    }
}

struct ObjectIdentityRegistry {
    used_ids: HashSet<String>,
}

impl ObjectIdentityRegistry {
    fn new(spec: &EvalFormSpec) -> Self {
        let mut used_ids = HashSet::new();
        for row in &spec.existing_object_observations {
            if let Some(id) = clean(&row.existing_id) {
                used_ids.insert(id.to_string());
            }
            if let Some((object_id, _)) = row.summary.split_once(':') {
                if let Some(id) = clean(object_id) {
                    used_ids.insert(id.to_string());
                }
            }
        }
        Self { used_ids }
    }

    fn assign(&mut self, owner_id: &str, object_type: &str, prefer_new_instance: bool) -> String {
        let owner = slugify(owner_id).trim().to_string();
        let object_type = slugify(object_type).trim().to_string();
        let owner = if owner.is_empty() {
            "unknown".into()
        } else {
            owner
        };
        let object_type = if object_type.is_empty() {
            "object".into()
        } else {
            object_type
        };
        let prefix = format!("{owner}_{object_type}_");
        if !prefer_new_instance {
            if let Some(existing) = self
                .used_ids
                .iter()
                .filter(|id| id.starts_with(&prefix))
                .min_by_key(|id| object_ordinal(id, &prefix).unwrap_or(u32::MAX))
                .cloned()
            {
                return existing;
            }
        }
        let mut ordinal = 1;
        loop {
            let candidate = format!("{prefix}{ordinal}");
            if self.used_ids.insert(candidate.clone()) {
                return candidate;
            }
            ordinal += 1;
        }
    }
}

fn object_ordinal(object_id: &str, prefix: &str) -> Option<u32> {
    object_id.strip_prefix(prefix)?.parse().ok()
}

fn object_state_from_row(
    row: &ObjectRow,
    spec: &EvalFormSpec,
    context: &EvaluatorConversionContext<'_>,
    registry: &mut ObjectIdentityRegistry,
) -> ObjectState {
    let identity_text = object_identity_text(row, context);
    let row_text = object_row_text(row);
    let object_type = infer_object_type(row, &identity_text);
    let owner_id = infer_object_owner_id(row, &identity_text, &object_type, spec);
    let prefer_new_instance = row
        .change_type
        .as_deref()
        .map(|change_type| change_type.eq_ignore_ascii_case("new_object_observation"))
        .unwrap_or(false);
    let object_id =
        stable_or_canonical_object_id(row, &owner_id, &object_type, registry, prefer_new_instance);
    let status = infer_object_status(row, &row_text);
    let location = row
        .location
        .as_deref()
        .and_then(clean)
        .map(str::to_string)
        .or_else(|| infer_object_location(&row_text))
        .or_else(|| infer_object_location(&identity_text))
        .unwrap_or_default();
    let last_observed_state = object_last_observed_state(row);
    ObjectState {
        object_id,
        object_kind: object_type,
        owner_entity_id: Some(owner_id),
        status,
        last_observed_state,
        confidence: confidence_from_confidence_tier(
            row.confidence_tier.unwrap_or(ConfidenceTier::Medium),
        ),
        location,
        ..ObjectState::default()
    }
}

fn object_identity_text(row: &ObjectRow, context: &EvaluatorConversionContext<'_>) -> String {
    [
        row.object_id.as_deref().unwrap_or_default(),
        row.new_object_label.as_deref().unwrap_or_default(),
        row.object_kind.as_deref().unwrap_or_default(),
        row.owner_entity_id.as_deref().unwrap_or_default(),
        row.last_observed_state.as_deref().unwrap_or_default(),
        row.property_changed.as_str(),
        row.new_value.as_str(),
        row.evidence_quote.as_str(),
        context.latest_user_message,
        context.latest_narrator_response,
    ]
    .join(" ")
}

fn object_row_text(row: &ObjectRow) -> String {
    [
        row.object_id.as_deref().unwrap_or_default(),
        row.new_object_label.as_deref().unwrap_or_default(),
        row.object_kind.as_deref().unwrap_or_default(),
        row.last_observed_state.as_deref().unwrap_or_default(),
        row.property_changed.as_str(),
        row.new_value.as_str(),
        row.evidence_quote.as_str(),
    ]
    .join(" ")
}

fn object_last_observed_state(row: &ObjectRow) -> String {
    if let Some(value) = row.last_observed_state.as_deref().and_then(clean) {
        return value.to_string();
    }
    if !matches!(row.property_changed.as_str(), "presence" | "state") {
        if let Some(value) = clean(&row.new_value) {
            return format!("{}: {}", row.property_changed, value);
        }
    }
    clean(&row.evidence_quote)
        .or_else(|| clean(&row.new_value))
        .map(str::to_string)
        .unwrap_or_else(|| format!("{}: {}", row.property_changed, row.new_value))
}

fn stable_or_canonical_object_id(
    row: &ObjectRow,
    owner_id: &str,
    object_type: &str,
    registry: &mut ObjectIdentityRegistry,
    prefer_new_instance: bool,
) -> String {
    if let Some(raw_id) = row.object_id.as_deref().and_then(clean) {
        let raw_id = slugify(raw_id);
        if !is_label_generated_object_id(&raw_id, row)
            && !is_temporary_state_object_id(&raw_id, object_type)
            && !is_generic_object_type_id(&raw_id, object_type)
        {
            registry.used_ids.insert(raw_id.clone());
            return raw_id;
        }
    }
    registry.assign(owner_id, object_type, prefer_new_instance)
}

fn is_label_generated_object_id(object_id: &str, row: &ObjectRow) -> bool {
    row.new_object_label
        .as_deref()
        .and_then(clean)
        .map(|label| slugify(label) == object_id)
        .unwrap_or(false)
}

fn is_temporary_state_object_id(object_id: &str, object_type: &str) -> bool {
    let normalized = slugify(object_id);
    let object_type = slugify(object_type);
    if normalized == object_type {
        return false;
    }
    let state_words = [
        "wet",
        "soaked",
        "damp",
        "dry",
        "broken",
        "shattered",
        "open",
        "closed",
        "locked",
        "unlocked",
        "dirty",
        "clean",
        "torn",
        "bloodied",
        "empty",
        "full",
    ];
    state_words.iter().any(|word| {
        normalized == format!("{word}_{object_type}")
            || normalized.starts_with(&format!("{word}_"))
            || normalized.ends_with(&format!("_{word}"))
    })
}

fn is_generic_object_type_id(object_id: &str, object_type: &str) -> bool {
    object_id == object_type
        && matches!(
            object_type,
            "jacket"
                | "coat"
                | "door"
                | "chair"
                | "table"
                | "glass"
                | "cup"
                | "mug"
                | "phone"
                | "window"
                | "lock"
                | "bag"
                | "book"
                | "candle"
                | "bottle"
                | "key"
                | "lamp"
        )
}

fn infer_object_type(row: &ObjectRow, text: &str) -> String {
    if let Some(kind) = row.object_kind.as_deref().and_then(clean) {
        let kind = strip_state_words_from_type(kind);
        if !is_generic_object_kind(&kind) {
            return kind;
        }
    }
    if let Some(label) = row.new_object_label.as_deref().and_then(clean) {
        return strip_state_words_from_type(label);
    }
    if let Some(object_id) = row.object_id.as_deref().and_then(clean) {
        let inferred = strip_state_words_from_type(object_id);
        if inferred != "object" {
            return inferred;
        }
    }
    infer_object_kind(text)
}

fn is_generic_object_kind(kind: &str) -> bool {
    matches!(
        kind,
        "clothing" | "clothes" | "item" | "object" | "thing" | "unknown"
    )
}

fn strip_state_words_from_type(value: &str) -> String {
    let slug = slugify(value);
    let lower = slug.replace('_', " ");
    if lower.contains("wine glass") {
        return "wine_glass".into();
    }
    for object_type in [
        "jacket", "hoodie", "bag", "glass", "chair", "door", "coat", "cup", "mug", "phone",
        "window", "lock", "book", "bottle", "key", "lamp",
    ] {
        if slug.split('_').any(|token| token == object_type) {
            return object_type.into();
        }
    }
    let tokens = slug
        .split('_')
        .filter(|token| {
            !matches!(
                *token,
                "wet"
                    | "soaked"
                    | "damp"
                    | "dry"
                    | "broken"
                    | "shattered"
                    | "open"
                    | "closed"
                    | "locked"
                    | "unlocked"
                    | "dirty"
                    | "clean"
                    | "torn"
                    | "bloodied"
                    | "old"
                    | "new"
                    | "leather"
                    | "wooden"
                    | "metal"
            )
        })
        .map(str::to_string)
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        "object".into()
    } else {
        tokens.join("_")
    }
}

fn infer_object_owner_id(
    row: &ObjectRow,
    text: &str,
    object_type: &str,
    spec: &EvalFormSpec,
) -> String {
    if let Some(owner_id) = row.owner_entity_id.as_deref().and_then(clean) {
        let resolved = resolve_active_entity_id(owner_id, spec);
        if spec
            .active_entities
            .iter()
            .any(|entity| entity.entity_id == resolved)
            || resolved.eq_ignore_ascii_case("unknown")
        {
            return resolved;
        }
    }
    let lower = text.to_ascii_lowercase();
    let object_word = object_type.replace('_', " ");
    let player_id = active_player_entity_id(spec).unwrap_or_else(|| "default_player".into());
    if contains_possessive_for_player(&lower, &object_word) {
        return player_id;
    }
    for entity in &spec.active_entities {
        if entity.entity_type != "soul" {
            continue;
        }
        let display = entity.display_name.to_ascii_lowercase();
        let first = display.split_whitespace().next().unwrap_or("").to_string();
        if (!display.is_empty() && lower.contains(&format!("{display}'s")))
            || (!first.is_empty() && lower.contains(&format!("{first}'s")))
        {
            return entity.entity_id.clone();
        }
    }
    "unknown".into()
}

fn contains_possessive_for_player(lower: &str, object_word: &str) -> bool {
    ["my", "your"].iter().any(|possessive| {
        lower.contains(&format!("{possessive} {object_word}"))
            || lower.contains(&format!("{possessive} wet {object_word}"))
            || lower.contains(&format!("{possessive} soaked {object_word}"))
            || lower.contains(&format!("{possessive} damp {object_word}"))
            || lower.contains(&format!("{possessive} broken {object_word}"))
    })
}

fn infer_object_status(row: &ObjectRow, text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let status = if lower.contains("soaked") || lower.contains("wet") || lower.contains("damp") {
        "wet"
    } else if lower.contains("dry") {
        "dry"
    } else if lower.contains("broken") || lower.contains("shattered") {
        "broken"
    } else if lower.contains("ajar") || lower.contains("open") {
        "open"
    } else if lower.contains("closed") {
        "closed"
    } else if lower.contains("unlocked") {
        "unlocked"
    } else if lower.contains("locked") {
        "locked"
    } else {
        clean(&row.new_value).unwrap_or("observed")
    };
    status.to_string()
}

fn infer_object_location(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    [
        "chair", "table", "couch", "sofa", "floor", "door", "counter", "bed", "desk",
    ]
    .iter()
    .find(|location| lower.contains(**location))
    .map(|location| (*location).to_string())
}

fn object_row_disabled(row: &ObjectRow) -> bool {
    row.row_enabled == Some(0)
}

fn memory_row_disabled(row: &MemoryRow) -> bool {
    row.row_enabled == Some(0)
}

fn apply_review_memory_operations(
    conversion: &mut EvaluatorConversionReport,
    response: &EvalFormResponse,
    review_map: &HashMap<String, (ReviewDecision, &ReviewRow)>,
) {
    let mut operations = Vec::new();
    for row in &response.memory_rows {
        if memory_row_disabled(row) {
            continue;
        }
        let candidate_id = memory_candidate_id(row);
        let Some((decision, review)) = review_map.get(&candidate_id).copied() else {
            continue;
        };
        let Some(existing_id) = review.existing_id.as_deref().and_then(clean) else {
            continue;
        };
        let operation = match decision {
            ReviewDecision::UpdateExisting => "update",
            ReviewDecision::SupersedeExisting => "supersede",
            ReviewDecision::ContradictsExisting => "invalidate",
            _ => continue,
        };
        operations.push(MemoryPatch {
            operation: Some(operation.into()),
            memory_id: Some(stable_id("memory_form", &candidate_id)),
            target_memory_id: Some(existing_id.to_string()),
            supersedes_memory_id: Some(existing_id.to_string()),
            content: row.content.clone(),
            tag: row.slot.map(|slot| slot.as_label().to_string()),
            ..MemoryPatch::default()
        });
    }
    if operations.is_empty() {
        return;
    }
    let mut patch = conversion.patch.clone();
    patch.schema_version = Some(PATCH_PROTOCOL_VERSION);
    let soul_patch = patch.soul_patch.get_or_insert_with(Default::default);
    soul_patch.memory_operations.extend(operations);
    conversion.patch = patch;
    conversion.no_op = false;
}

fn scene_state_from_event(
    row: &EventRow,
    context: &EvaluatorConversionContext<'_>,
) -> SceneStatePatch {
    SceneStatePatch {
        scene_state_id: Some(stable_id("scene_form", &row.event_id)),
        current_scene: clean(&row.objective_summary).map(str::to_string),
        focus: Some(row.participants.join(" and ")),
        participants: row.participants.clone(),
        last_user_action: clean(context.latest_user_message).map(str::to_string),
        continuity_note: clean(&row.objective_summary).map(str::to_string),
        ..SceneStatePatch::default()
    }
}

fn apply_event_flags(output: &mut EvaluatorOutputV1, row: &EventRow) {
    output.turn_flags_u64 |= turn_flags::SCENE_TURN | turn_flags::USER_ACTION_PRESENT;
    match row.event_type.unwrap_or(EventType::SceneEvent) {
        EventType::LocationChange => output.turn_flags_u64 |= turn_flags::WORLD_CHANGE,
        EventType::ObjectChange => {
            output.turn_flags_u64 |= turn_flags::OBJECT_CHANGE | turn_flags::WORLD_CHANGE
        }
        EventType::RelationshipShift => output.turn_flags_u64 |= turn_flags::RELATIONSHIP_SHIFT,
        EventType::CurrentPlotAdvanced => {
            output.turn_flags_u64 |= turn_flags::CURRENT_PLOT_ADVANCED
        }
        EventType::UnresolvedTension => output.turn_flags_u64 |= turn_flags::UNRESOLVED_TENSION,
        EventType::RecentEmotionalState => {
            output.turn_flags_u64 |= turn_flags::RECENT_EMOTIONAL_STATE
        }
        EventType::Correction => output.turn_flags_u64 |= turn_flags::RETCON_OR_CORRECTION,
        EventType::SceneEvent => {}
    }
}

fn compute_turn_flags(output: &EvaluatorOutputV1) -> u64 {
    let mut flags = output.turn_flags_u64;
    if !output.world_changes.is_empty() {
        flags |= turn_flags::SCENE_TURN | turn_flags::WORLD_CHANGE;
    }
    if !output.object_changes.is_empty() {
        flags |= turn_flags::OBJECT_CHANGE;
    }
    if !output.relationship_evaluations.is_empty() {
        flags |= turn_flags::RELATIONSHIP_SHIFT;
    }
    if output
        .memory_candidates
        .iter()
        .any(|candidate| candidate.slot == MemorySlot::UnresolvedTension)
    {
        flags |= turn_flags::UNRESOLVED_TENSION;
    }
    flags
}

fn global_scene_from_output(output: &EvaluatorOutputV1) -> GlobalSceneEvaluation {
    GlobalSceneEvaluation {
        scene_event_occurred: output.turn_flags_u64 & turn_flags::SCENE_TURN != 0,
        location_changed: output.world_changes.iter().any(|change| {
            change
                .location
                .as_ref()
                .is_some_and(|location| !location.trim().is_empty())
        }),
        object_state_changed: !output.object_changes.is_empty(),
        relationship_changed: !output.relationship_evaluations.is_empty(),
        unresolved_tension: output.turn_flags_u64 & turn_flags::UNRESOLVED_TENSION != 0,
        current_plot_advanced: output.turn_flags_u64 & turn_flags::CURRENT_PLOT_ADVANCED != 0,
        recent_emotional_state_changed: output.turn_flags_u64 & turn_flags::RECENT_EMOTIONAL_STATE
            != 0,
        evidence_quote: output
            .world_changes
            .first()
            .and_then(|change| change.evidence_quote.clone()),
        summary: output
            .world_changes
            .first()
            .and_then(|change| change.event_summary.clone())
            .unwrap_or_default(),
        ..GlobalSceneEvaluation::default()
    }
}

fn draft_from_output(
    output: &EvaluatorOutputV1,
    rejected_rows: &[EvalFormRowRejection],
    trace: &EvalFormTrace,
) -> NormalizedEvaluationDraft {
    NormalizedEvaluationDraft {
        scene_evaluation: output.global_scene_evaluation.clone(),
        memory_candidate_count: output.memory_candidates.len(),
        world_event_count: output.world_changes.len(),
        scene_state_present: output
            .world_changes
            .iter()
            .any(|change| change.scene_state.is_some()),
        relationship_delta_count: output.relationship_evaluations.len(),
        object_observation_count: output.object_changes.len(),
        warnings: rejected_rows
            .iter()
            .map(|row| format!("{} {} rejected: {}", row.row_kind, row.row_id, row.reason))
            .collect(),
        candidate_quality_decisions: rejected_rows.iter().map(|row| row.reason.clone()).collect(),
        candidate_routing_decisions: trace
            .form_dedupe_decisions
            .iter()
            .map(|decision| format!("{} {:?}", decision.candidate_id, decision.decision))
            .collect(),
        state_effect_guarantee_applied: false,
        state_effect_guarantee_reason: None,
        per_soul_interpretation_count: 0,
    }
}

fn aggregate_relevance_tags(output: &EvaluatorOutputV1) -> RelevanceTags {
    let mut tags = RelevanceTags::default();
    for change in &output.world_changes {
        tags.event_type_tags
            .extend(change.relevance_tags.event_type_tags.clone());
    }
    for candidate in &output.memory_candidates {
        for tag in &candidate.relevance_tags {
            tags.memory_slot_tags.insert(tag.clone(), 80);
        }
        tags.memory_slot_tags
            .insert(candidate.slot.as_label().into(), 80);
    }
    tags
}

fn relevance_from_event(row: &EventRow) -> RelevanceTags {
    let mut tags = RelevanceTags::default();
    tags.event_type_tags.insert(
        format!("{:?}", row.event_type.unwrap_or(EventType::SceneEvent)).to_ascii_lowercase(),
        80,
    );
    tags
}

fn flatten_tag_weights(tags: &RelevanceTags) -> HashMap<String, u8> {
    tags.setting_tags
        .iter()
        .chain(tags.location_tags.iter())
        .chain(tags.interacted_entities.iter())
        .chain(tags.event_type_tags.iter())
        .chain(tags.object_tags.iter())
        .chain(tags.emotional_tags.iter())
        .chain(tags.memory_slot_tags.iter())
        .map(|(key, value)| (key.clone(), *value))
        .collect()
}

fn select_relevant_memories(
    soul: &Soul,
    latest_user_message: &str,
    latest_narrator_response: &str,
    top_k: usize,
) -> Vec<ExistingStateRow> {
    let query = token_set(&format!("{latest_user_message} {latest_narrator_response}"));
    let mut rows = soul
        .memory
        .recent
        .iter()
        .map(|memory| {
            let overlap = token_set(&memory.content)
                .iter()
                .filter(|token| query.contains(token))
                .count();
            (overlap, memory)
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| right.0.cmp(&left.0));
    rows.into_iter()
        .take(top_k)
        .map(|(_, memory)| ExistingStateRow {
            existing_id: memory.id.clone(),
            kind: ExistingStateKind::Memory,
            summary: memory.content.clone(),
        })
        .collect()
}

fn select_relevant_events(
    recent_events: &[String],
    records: &[crate::soul::WorldEventRecord],
    top_k: usize,
) -> Vec<ExistingStateRow> {
    if !records.is_empty() {
        return records
            .iter()
            .rev()
            .take(top_k)
            .map(|record| ExistingStateRow {
                existing_id: record.recent_event_id.clone(),
                kind: ExistingStateKind::Event,
                summary: record.content.clone(),
            })
            .collect();
    }
    recent_events
        .iter()
        .rev()
        .take(top_k)
        .enumerate()
        .map(|(idx, event)| ExistingStateRow {
            existing_id: format!("recent_event_{idx}"),
            kind: ExistingStateKind::Event,
            summary: event.clone(),
        })
        .collect()
}

fn all_event_types() -> Vec<EventType> {
    vec![
        EventType::SceneEvent,
        EventType::LocationChange,
        EventType::ObjectChange,
        EventType::RelationshipShift,
        EventType::CurrentPlotAdvanced,
        EventType::UnresolvedTension,
        EventType::RecentEmotionalState,
        EventType::Correction,
    ]
}

fn all_relationship_dimensions() -> Vec<RelationshipDimension> {
    vec![
        RelationshipDimension::Trust,
        RelationshipDimension::Affection,
        RelationshipDimension::Intimacy,
        RelationshipDimension::Passion,
        RelationshipDimension::Commitment,
        RelationshipDimension::Fear,
        RelationshipDimension::Desire,
        RelationshipDimension::Respect,
        RelationshipDimension::Conflict,
        RelationshipDimension::Dependency,
        RelationshipDimension::Curiosity,
        RelationshipDimension::Comfort,
        RelationshipDimension::BoundaryPressure,
    ]
}

fn default_tag_vocabularies() -> Vec<String> {
    [
        "scene_event",
        "relationship",
        "current_plot",
        "location",
        "object",
        "emotional_state",
        "boundary",
        "doorway",
        "reunion",
    ]
    .iter()
    .map(|tag| (*tag).to_string())
    .collect()
}

fn magnitude_value(direction: RelationshipDirection, tier: MagnitudeTier) -> f32 {
    let amount = match tier {
        MagnitudeTier::Tiny => 0.5,
        MagnitudeTier::Small => 1.0,
        MagnitudeTier::Medium => 2.0,
        MagnitudeTier::Large => 4.0,
    };
    match direction {
        RelationshipDirection::Increase => amount,
        RelationshipDirection::Decrease => -amount,
        RelationshipDirection::NoChange => 0.0,
    }
}

fn confidence_from_importance(tier: ImportanceTier) -> f32 {
    match tier {
        ImportanceTier::Trivial => 0.45,
        ImportanceTier::Low => 0.6,
        ImportanceTier::Medium => 0.75,
        ImportanceTier::High => 0.88,
        ImportanceTier::Critical => 0.95,
    }
}

fn confidence_from_confidence_tier(tier: ConfidenceTier) -> f32 {
    match tier {
        ConfidenceTier::Low => 0.5,
        ConfidenceTier::Medium => 0.72,
        ConfidenceTier::High => 0.9,
    }
}

fn salience_from_importance(tier: ImportanceTier) -> f32 {
    match tier {
        ImportanceTier::Trivial => 20.0,
        ImportanceTier::Low => 40.0,
        ImportanceTier::Medium => 60.0,
        ImportanceTier::High => 82.0,
        ImportanceTier::Critical => 95.0,
    }
}

fn retrieval_from_importance(tier: ImportanceTier) -> f32 {
    match tier {
        ImportanceTier::Trivial => 15.0,
        ImportanceTier::Low => 35.0,
        ImportanceTier::Medium => 55.0,
        ImportanceTier::High => 78.0,
        ImportanceTier::Critical => 92.0,
    }
}

fn decay_profile(tier: ImportanceTier) -> &'static str {
    match tier {
        ImportanceTier::Trivial => "fast",
        ImportanceTier::Low => "normal",
        ImportanceTier::Medium => "normal",
        ImportanceTier::High => "slow",
        ImportanceTier::Critical => "pinned",
    }
}

fn infer_object_kind(object_id: &str) -> String {
    let lower = object_id.to_ascii_lowercase();
    if lower.contains("wine glass") || lower.contains("wine_glass") {
        "wine_glass".into()
    } else if lower.contains("chain lock") || lower.contains("chain_lock") {
        "chain_lock".into()
    } else if lower.contains("jacket") {
        "jacket".into()
    } else if lower.contains("coat") {
        "coat".into()
    } else if lower.contains("chair") {
        "chair".into()
    } else if lower.contains("cup") {
        "cup".into()
    } else if lower.contains("glass") {
        "glass".into()
    } else if lower.contains("door") {
        "door".into()
    } else if lower.contains("phone") {
        "phone".into()
    } else if lower.contains("lock") {
        "lock".into()
    } else {
        "object".into()
    }
}

fn token_set(text: &str) -> Vec<String> {
    let mut tokens = text
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| token.len() > 2)
        .map(|token| token.to_ascii_lowercase())
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.dedup();
    tokens
}

pub fn stable_id(prefix: &str, source: &str) -> String {
    let mut hash: u64 = 1469598103934665603;
    for byte in source.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    format!("{prefix}_{hash:016x}")
}

pub fn memory_candidate_id(row: &MemoryRow) -> String {
    stable_id(
        "form_memory",
        &format!(
            "{}|{}|{}|{}",
            row.linked_event_id,
            row.owner_soul_id,
            row.slot.map(|slot| slot.as_label()).unwrap_or("unknown"),
            row.content
        ),
    )
}
