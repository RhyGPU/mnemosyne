use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::{
    diagnostics::{CompilerContractError, CompilerDiagnostic},
    lower::{LoweringReport, StateEffect},
    source::SourceEnvelope,
    MEMORY_COMPILER_CONTRACT_VERSION,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProposedTransaction {
    pub compiler_version: u32,
    pub source_hash: String,
    pub parent_state_hash: Option<String>,
    pub effects: Vec<StateEffect>,
}

impl ProposedTransaction {
    pub fn try_from_lowering(
        source: &SourceEnvelope,
        report: LoweringReport,
    ) -> Result<Self, CompilerContractError> {
        source.validate()?;
        if report.source_hash != source.source_hash() {
            return Err(CompilerContractError::new(
                "lowering_source_mismatch",
                "lowering report source hash does not match the active source envelope",
            ));
        }
        if report.effects.iter().any(|effect| {
            effect.provenance.source_hash != source.source_hash()
                || effect.provenance.compiler_version != MEMORY_COMPILER_CONTRACT_VERSION
        }) {
            return Err(CompilerContractError::new(
                "effect_provenance_mismatch",
                "all effects must carry the active source hash and compiler version",
            ));
        }
        Ok(Self {
            compiler_version: MEMORY_COMPILER_CONTRACT_VERSION,
            source_hash: source.source_hash().to_string(),
            parent_state_hash: source.parent_state_hash().map(str::to_string),
            effects: report.effects,
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SimulationDecision {
    CommitReady,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SimulationReport {
    pub source_hash: String,
    pub decision: SimulationDecision,
    pub effects: Vec<StateEffect>,
    pub diagnostics: Vec<CompilerDiagnostic>,
}

pub trait TransactionSimulator {
    type State;

    fn simulate(
        &self,
        source: &SourceEnvelope,
        current_state: &Self::State,
        transaction: &ProposedTransaction,
    ) -> SimulationReport;
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SimulationSnapshot {
    pub state_hash: Option<String>,
    pub existing_effect_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DeterministicTransactionSimulator;

impl TransactionSimulator for DeterministicTransactionSimulator {
    type State = SimulationSnapshot;

    fn simulate(
        &self,
        source: &SourceEnvelope,
        current_state: &Self::State,
        transaction: &ProposedTransaction,
    ) -> SimulationReport {
        let mut diagnostics = Vec::new();
        let mut reject = |code: &str, message: String, candidate_id: Option<String>| {
            diagnostics.push(CompilerDiagnostic {
                stage: super::diagnostics::CompilerStage::Simulation,
                severity: super::diagnostics::DiagnosticSeverity::Error,
                code: code.into(),
                message,
                candidate_id,
                field_path: None,
            });
        };
        if source.validate().is_err() || transaction.source_hash != source.source_hash() {
            reject(
                "transaction_source_mismatch",
                "transaction does not belong to active source".into(),
                None,
            );
        }
        if transaction.compiler_version != MEMORY_COMPILER_CONTRACT_VERSION {
            reject(
                "transaction_compiler_version_mismatch",
                "transaction compiler version is not active".into(),
                None,
            );
        }
        if transaction.parent_state_hash != current_state.state_hash {
            reject(
                "parent_state_hash_mismatch",
                "transaction was planned against a different state".into(),
                None,
            );
        }
        if transaction.effects.is_empty() {
            reject(
                "empty_transaction",
                "a transaction must contain at least one effect".into(),
                None,
            );
        }

        let existing = current_state
            .existing_effect_ids
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        let mut seen = HashSet::new();
        for effect in &transaction.effects {
            let candidate_id = Some(effect.provenance.candidate_id.clone());
            if effect.provenance.source_hash != source.source_hash()
                || effect.provenance.compiler_version != MEMORY_COMPILER_CONTRACT_VERSION
            {
                reject(
                    "effect_provenance_mismatch",
                    "effect provenance is not valid for this transaction".into(),
                    candidate_id.clone(),
                );
            }
            if !seen.insert(effect.provenance.effect_id.clone()) {
                reject(
                    "duplicate_effect_id",
                    "transaction contains a duplicate effect id".into(),
                    candidate_id.clone(),
                );
            }
            if existing.contains(&effect.provenance.effect_id) {
                reject(
                    "effect_already_applied",
                    "effect id already exists in current state".into(),
                    candidate_id.clone(),
                );
            }
            if let super::lower::StateEffectKind::ApplyRelationshipEvidence { signal, .. } =
                &effect.effect
            {
                if !(-5..=5).contains(&signal.valence)
                    || signal.directness > 100
                    || signal.stakes > 100
                    || signal.costliness > 100
                    || signal.repetition > 100
                    || signal.behaviors.is_empty()
                {
                    reject(
                        "relationship_signal_out_of_bounds",
                        "relationship evidence signal is outside compiler policy bounds".into(),
                        candidate_id,
                    );
                }
            }
        }

        let decision = if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == super::diagnostics::DiagnosticSeverity::Error)
        {
            SimulationDecision::Rejected
        } else {
            SimulationDecision::CommitReady
        };
        SimulationReport {
            source_hash: source.source_hash().into(),
            decision,
            effects: if decision == SimulationDecision::CommitReady {
                transaction.effects.clone()
            } else {
                Vec::new()
            },
            diagnostics,
        }
    }
}
