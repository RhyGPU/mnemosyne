use crate::{
    evaluator::RelationshipEvaluation,
    evaluator_form::{EvalFormSpec, FormRelationshipState, ValidatedRelationshipEventRow},
};

const FLAG_REAPPRAISAL_CANDIDATE: u64 = 32;
const FLAG_HIGH_STAKES: u64 = 64;
const FLAG_COSTLY_ACTION: u64 = 128;
const FLAG_REPEATED_PATTERN: u64 = 1024;
const FLAG_FIRST_IMPRESSION_EVENT: u64 = 2048;

pub fn relationship_from_numeric_event_row(
    row: &ValidatedRelationshipEventRow,
    spec: &EvalFormSpec,
) -> RelationshipEvaluation {
    let current = relationship_state_for_numeric_row(row, spec);
    let event_strength = relationship_event_strength(row);
    let bias_cap = relationship_bias_cap(row);
    let state_cap = relationship_state_cap(row);
    let max_abs_delta = bias_cap.max(state_cap);

    let evidence = RelationshipEvidence::from_row(row);
    let bias_deltas = BiasDeltas::from_evidence(&current, &evidence, event_strength, bias_cap);
    let next_biases = bias_deltas.apply_to(&current);
    let state_targets = StateTargets::from_biases(row, &current, &next_biases);
    let state_deltas = StateDeltas::from_targets(&current, &state_targets, state_cap);
    let (first_impression_strength_delta, first_impression_confidence_delta) =
        first_impression_deltas(row, &current, max_abs_delta);
    let (reappraisal_debt_delta, reappraisal_state_code) =
        reappraisal_update(row, &current, &evidence, event_strength, max_abs_delta);

    RelationshipEvaluation {
        source_soul_id: row.relationship_source_soul_id.clone(),
        target_entity_id: row.relationship_target_entity_id.clone(),
        trust: nonzero_delta(state_deltas.trust),
        intimacy: nonzero_delta(state_deltas.intimacy),
        respect: nonzero_delta(state_deltas.respect),
        conflict: nonzero_delta(state_deltas.conflict),
        comfort: nonzero_delta(state_deltas.comfort),
        boundary_pressure: nonzero_delta(state_deltas.boundary_pressure),
        trustable_bias: nonzero_delta(bias_deltas.trustable),
        untrustworthy_bias: nonzero_delta(bias_deltas.untrustworthy),
        asshole_bias: nonzero_delta(bias_deltas.asshole),
        care_bias: nonzero_delta(bias_deltas.care),
        danger_bias: nonzero_delta(bias_deltas.danger),
        competence_bias: nonzero_delta(bias_deltas.competence),
        autonomy_respect_bias: nonzero_delta(bias_deltas.autonomy_respect),
        first_impression_strength: nonzero_delta(first_impression_strength_delta),
        first_impression_confidence: nonzero_delta(first_impression_confidence_delta),
        reappraisal_debt: nonzero_delta(reappraisal_debt_delta),
        reappraisal_state_code,
        max_abs_delta: Some(max_abs_delta),
        evidence_quote: Some(row.evidence_quote.clone()),
        criterion_met: true,
        confidence: m(row.certainty).clamp(0.55, 1.0),
        evidence_validated_by_form: true,
        ..RelationshipEvaluation::default()
    }
}

#[derive(Debug, Clone, Copy)]
struct RelationshipEvidence {
    trustable: f32,
    untrustworthy: f32,
    asshole: f32,
    care: f32,
    danger: f32,
    autonomy_respect: f32,
    competence: f32,
}

impl RelationshipEvidence {
    fn from_row(row: &ValidatedRelationshipEventRow) -> Self {
        Self {
            trustable: 0.30 * pos(row.honesty)
                + 0.35 * pos(row.reliability)
                + 0.20 * pos(row.power_use)
                + 0.15 * pos(row.repair)
                - 1.60
                    * (0.35 * neg(row.honesty)
                        + 0.45 * neg(row.reliability)
                        + 0.20 * neg(row.boundary_treatment)),
            untrustworthy: 0.35 * neg(row.honesty)
                + 0.40 * neg(row.reliability)
                + 0.15 * neg(row.intent)
                + 0.10 * neg(row.predictability)
                - 0.20 * pos(row.honesty)
                - 0.15 * pos(row.reliability),
            asshole: 0.30 * neg(row.evaluation_tone)
                + 0.25 * neg(row.boundary_treatment)
                + 0.20 * neg(row.responsiveness)
                + 0.15 * neg(row.intent)
                + 0.10 * neg(row.power_use)
                - 0.10 * pos(row.evaluation_tone)
                - 0.05 * pos(row.repair),
            care: 0.35 * pos(row.intent)
                + 0.35 * pos(row.responsiveness)
                + 0.15 * pos(row.reciprocity)
                + 0.15 * pos(row.repair)
                - 0.30 * neg(row.intent)
                - 0.20 * neg(row.responsiveness),
            danger: 0.35 * neg(row.power_use)
                + 0.30 * neg(row.boundary_treatment)
                + 0.20 * neg(row.intent)
                + 0.15 * neg(row.predictability)
                - 0.15 * pos(row.boundary_treatment)
                - 0.10 * pos(row.power_use),
            autonomy_respect: 0.45 * pos(row.boundary_treatment)
                + 0.20 * pos(row.power_use)
                + 0.20 * pos(row.responsiveness)
                + 0.15 * pos(row.intent)
                - 1.40
                    * (0.50 * neg(row.boundary_treatment)
                        + 0.30 * neg(row.power_use)
                        + 0.20 * neg(row.intent)),
            competence: 0.75 * pos(row.competence)
                + 0.15 * pos(row.predictability)
                + 0.10 * pos(row.reliability)
                - 0.45 * neg(row.competence),
        }
    }

    fn positive_counter(self) -> f32 {
        self.trustable
            .max(self.care)
            .max(self.autonomy_respect)
            .max(0.0)
    }

    fn negative_counter(self) -> f32 {
        self.untrustworthy
            .max(self.asshole)
            .max(self.danger)
            .max(0.0)
    }
}

#[derive(Debug, Clone, Copy)]
struct BiasDeltas {
    trustable: f32,
    untrustworthy: f32,
    asshole: f32,
    care: f32,
    danger: f32,
    competence: f32,
    autonomy_respect: f32,
}

impl BiasDeltas {
    fn from_evidence(
        current: &FormRelationshipState,
        evidence: &RelationshipEvidence,
        event_strength: f32,
        cap: f32,
    ) -> Self {
        Self {
            trustable: bias_delta(
                current.trustable_bias,
                evidence.trustable,
                event_strength,
                cap,
            ),
            untrustworthy: bias_delta(
                current.untrustworthy_bias,
                evidence.untrustworthy,
                event_strength,
                cap,
            ),
            asshole: bias_delta(current.asshole_bias, evidence.asshole, event_strength, cap),
            care: bias_delta(current.care_bias, evidence.care, event_strength, cap),
            danger: bias_delta(current.danger_bias, evidence.danger, event_strength, cap),
            competence: bias_delta(
                current.competence_bias,
                evidence.competence,
                event_strength,
                cap,
            ),
            autonomy_respect: bias_delta(
                current.autonomy_respect_bias,
                evidence.autonomy_respect,
                event_strength,
                cap,
            ),
        }
    }

    fn apply_to(self, current: &FormRelationshipState) -> NextBiases {
        NextBiases {
            trustable: (current.trustable_bias + self.trustable).clamp(0.0, 100.0),
            untrustworthy: (current.untrustworthy_bias + self.untrustworthy).clamp(0.0, 100.0),
            asshole: (current.asshole_bias + self.asshole).clamp(0.0, 100.0),
            danger: (current.danger_bias + self.danger).clamp(0.0, 100.0),
            competence: (current.competence_bias + self.competence).clamp(0.0, 100.0),
            autonomy_respect: (current.autonomy_respect_bias + self.autonomy_respect)
                .clamp(0.0, 100.0),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct NextBiases {
    trustable: f32,
    untrustworthy: f32,
    asshole: f32,
    danger: f32,
    competence: f32,
    autonomy_respect: f32,
}

#[derive(Debug, Clone, Copy)]
struct StateTargets {
    trust: f32,
    comfort: f32,
    boundary_pressure: f32,
    conflict: f32,
    intimacy: f32,
    respect: f32,
}

impl StateTargets {
    fn from_biases(
        row: &ValidatedRelationshipEventRow,
        current: &FormRelationshipState,
        biases: &NextBiases,
    ) -> Self {
        Self {
            trust: (0.45 * biases.trustable
                - 0.25 * biases.untrustworthy
                - 0.15 * biases.danger
                - 0.08 * biases.asshole
                + 8.0 * pos(row.boundary_treatment)
                + 6.0 * pos(row.reliability))
            .clamp(0.0, 100.0),
            comfort: (0.35 * (100.0 - biases.danger)
                + 0.25 * biases.autonomy_respect
                + 12.0 * pos(row.responsiveness)
                + 8.0 * pos(row.predictability)
                - 0.20 * current.boundary_pressure)
                .clamp(0.0, 100.0),
            boundary_pressure: (100.0
                * (0.45 * neg(row.boundary_treatment)
                    + 0.25 * neg(row.power_use)
                    + 0.15 * neg(row.responsiveness)
                    + 0.15 * neg(row.intent)
                    - 0.25 * pos(row.boundary_treatment)))
            .clamp(0.0, 100.0),
            conflict: (100.0
                * (0.35 * neg(row.evaluation_tone)
                    + 0.25 * neg(row.intent)
                    + 0.20 * neg(row.boundary_treatment)
                    + 0.20 * neg(row.reciprocity)))
            .clamp(0.0, 100.0),
            intimacy: (0.30 * current.trust
                + 0.25 * current.comfort
                + 20.0 * pos(row.disclosure)
                + 15.0 * pos(row.reciprocity)
                + 0.10 * current.attachment_pull
                - 0.20 * current.boundary_pressure)
                .clamp(0.0, 100.0),
            respect: (0.35 * biases.competence
                + 0.25 * biases.autonomy_respect
                + 0.20 * biases.trustable
                + 10.0 * pos(row.evaluation_tone)
                - 0.15 * biases.asshole)
                .clamp(0.0, 100.0),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct StateDeltas {
    trust: f32,
    comfort: f32,
    boundary_pressure: f32,
    conflict: f32,
    intimacy: f32,
    respect: f32,
}

impl StateDeltas {
    fn from_targets(current: &FormRelationshipState, target: &StateTargets, cap: f32) -> Self {
        Self {
            trust: state_delta(current.trust, target.trust, cap),
            comfort: state_delta(current.comfort, target.comfort, cap),
            boundary_pressure: state_delta(
                current.boundary_pressure,
                target.boundary_pressure,
                cap,
            ),
            conflict: state_delta(current.conflict, target.conflict, cap),
            intimacy: state_delta(current.intimacy, target.intimacy, cap),
            respect: state_delta(current.respect, target.respect, cap),
        }
    }
}

pub fn relationship_evaluation_has_delta(relation: &RelationshipEvaluation) -> bool {
    relation.trust.is_some()
        || relation.intimacy.is_some()
        || relation.respect.is_some()
        || relation.conflict.is_some()
        || relation.comfort.is_some()
        || relation.boundary_pressure.is_some()
        || relation.trustable_bias.is_some()
        || relation.untrustworthy_bias.is_some()
        || relation.asshole_bias.is_some()
        || relation.care_bias.is_some()
        || relation.danger_bias.is_some()
        || relation.competence_bias.is_some()
        || relation.autonomy_respect_bias.is_some()
        || relation.first_impression_strength.is_some()
        || relation.first_impression_confidence.is_some()
        || relation.reappraisal_debt.is_some()
        || relation.reappraisal_state_code.is_some()
}

fn relationship_state_for_numeric_row(
    row: &ValidatedRelationshipEventRow,
    spec: &EvalFormSpec,
) -> FormRelationshipState {
    spec.active_relationship_states
        .iter()
        .find(|state| {
            state.source_soul_id == row.relationship_source_soul_id
                && state.target_entity_id == row.relationship_target_entity_id
        })
        .cloned()
        .unwrap_or_else(|| FormRelationshipState {
            source_soul_id: row.relationship_source_soul_id.clone(),
            target_entity_id: row.relationship_target_entity_id.clone(),
            ..FormRelationshipState::default()
        })
}

fn relationship_event_strength(row: &ValidatedRelationshipEventRow) -> f32 {
    (m(row.salience)
        * m(row.certainty)
        * m(row.directness)
        * (1.0 + 0.35 * m(row.costliness))
        * (0.5 + m(row.stakes))
        * (0.75 + m(row.repetition)))
    .clamp(0.0, 4.0)
}

fn relationship_bias_cap(row: &ValidatedRelationshipEventRow) -> f32 {
    if has_flag(row.event_flags_u64, FLAG_HIGH_STAKES)
        || has_flag(row.event_flags_u64, FLAG_COSTLY_ACTION)
    {
        30.0
    } else {
        3.0 + 17.0 * m(row.stakes) * m(row.salience)
    }
}

fn relationship_state_cap(row: &ValidatedRelationshipEventRow) -> f32 {
    if has_flag(row.event_flags_u64, FLAG_HIGH_STAKES)
        || has_flag(row.event_flags_u64, FLAG_COSTLY_ACTION)
    {
        30.0
    } else {
        2.0 + 18.0 * m(row.salience) * m(row.stakes)
    }
}

fn bias_delta(current: f32, evidence: f32, event_strength: f32, cap: f32) -> f32 {
    let raw = (0.08 * event_strength * evidence * 100.0).clamp(-cap, cap);
    (current + raw).clamp(0.0, 100.0) - current
}

fn state_delta(current: f32, target: f32, cap: f32) -> f32 {
    let raw = (0.25 * (target - current)).clamp(-cap, cap);
    (current + raw).clamp(0.0, 100.0) - current
}

fn first_impression_deltas(
    row: &ValidatedRelationshipEventRow,
    current: &FormRelationshipState,
    cap: f32,
) -> (f32, f32) {
    if !has_flag(row.event_flags_u64, FLAG_FIRST_IMPRESSION_EVENT) {
        return (0.0, 0.0);
    }
    let next_strength = current.first_impression_strength.max(row.salience as f32);
    let next_confidence = current
        .first_impression_confidence
        .max(row.certainty as f32);
    (
        (next_strength - current.first_impression_strength).clamp(-cap, cap),
        (next_confidence - current.first_impression_confidence).clamp(-cap, cap),
    )
}

fn reappraisal_update(
    row: &ValidatedRelationshipEventRow,
    current: &FormRelationshipState,
    evidence: &RelationshipEvidence,
    event_strength: f32,
    cap: f32,
) -> (f32, Option<u8>) {
    let max_bias_negative = current
        .untrustworthy_bias
        .max(current.asshole_bias)
        .max(current.danger_bias)
        .max(current.schema_threat);
    let max_bias_positive = current
        .trustable_bias
        .max(current.care_bias)
        .max(current.autonomy_respect_bias)
        .max(current.competence_bias);

    let contradiction = if has_flag(row.event_flags_u64, FLAG_REAPPRAISAL_CANDIDATE) {
        (max_bias_negative * evidence.positive_counter() * event_strength)
            .max(max_bias_positive * evidence.negative_counter() * event_strength)
    } else {
        0.0
    };
    let next_debt = (current.reappraisal_debt + contradiction).clamp(0.0, 100.0);
    let debt_delta = (next_debt - current.reappraisal_debt).clamp(-cap, cap);
    let effective_debt = (current.reappraisal_debt + debt_delta).clamp(0.0, 100.0);

    let mut code = current.reappraisal_state_code;
    if has_flag(row.event_flags_u64, FLAG_FIRST_IMPRESSION_EVENT) && code == 0 {
        code = 1;
    }
    if effective_debt >= 40.0 {
        code = 2;
    }
    if effective_debt >= 70.0 && event_strength >= 1.0 {
        code = 3;
    }
    if has_flag(row.event_flags_u64, FLAG_REPEATED_PATTERN) && effective_debt < 20.0 {
        code = 4;
    }

    let code_delta = (code != current.reappraisal_state_code).then_some(code);
    (debt_delta, code_delta)
}

fn has_flag(flags: u64, flag: u64) -> bool {
    flags & flag != 0
}

fn pos(x: i32) -> f32 {
    (x.max(0) as f32) / 5.0
}

fn neg(x: i32) -> f32 {
    ((-x).max(0) as f32) / 5.0
}

fn m(x: u32) -> f32 {
    (x as f32) / 100.0
}

fn nonzero_delta(delta: f32) -> Option<f32> {
    (delta.abs() >= 0.0001).then_some(delta)
}
