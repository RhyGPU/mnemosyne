use serde::{Deserialize, Serialize};

use super::{BindingReport, CompilerDiagnostic, CompilerStage, DiagnosticSeverity};
use super::{
    DeterministicEffectLowerer, DeterministicEntityBinder, DeterministicSemanticAnalyzer,
    DeterministicTransactionSimulator, EffectLowerer, EntityBinder, EntityCatalog, LoweringReport,
    PerceptionBatch, ProposedTransaction, SemanticAnalyzer, SemanticReport, SimulationDecision,
    SimulationReport, SimulationSnapshot, SourceEnvelope, TransactionSimulator,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CompilerPipelineReport {
    pub source_hash: String,
    pub binding: BindingReport,
    pub semantic: SemanticReport,
    pub lowering: LoweringReport,
    pub transaction: Option<ProposedTransaction>,
    pub simulation: SimulationReport,
}

pub fn compile_perception_pipeline(
    source: &SourceEnvelope,
    batch: &PerceptionBatch,
    catalog: EntityCatalog,
    snapshot: &SimulationSnapshot,
) -> CompilerPipelineReport {
    let binding = DeterministicEntityBinder::new(catalog).bind(source, batch);
    let semantic = DeterministicSemanticAnalyzer.analyze(source, &binding);
    let lowering = DeterministicEffectLowerer.lower(source, &semantic);
    let transaction = ProposedTransaction::try_from_lowering(source, lowering.clone()).ok();
    let simulation = if let Some(transaction) = transaction.as_ref() {
        DeterministicTransactionSimulator.simulate(source, snapshot, transaction)
    } else {
        SimulationReport {
            source_hash: source.source_hash().into(),
            decision: SimulationDecision::Rejected,
            effects: Vec::new(),
            diagnostics: vec![CompilerDiagnostic {
                stage: CompilerStage::Simulation,
                severity: DiagnosticSeverity::Error,
                code: "transaction_construction_failed".into(),
                message: "lowered effects could not form a source-bound transaction".into(),
                candidate_id: None,
                field_path: None,
            }],
        }
    };
    CompilerPipelineReport {
        source_hash: source.source_hash().into(),
        binding,
        semantic,
        lowering,
        transaction,
        simulation,
    }
}
