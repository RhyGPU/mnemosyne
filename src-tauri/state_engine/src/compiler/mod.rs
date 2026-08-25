//! Memory Compiler V2 contracts.
//!
//! This module deliberately contains no Tauri, database, provider transport, or
//! UI dependencies. The LLM-writable draft is separated from engine-owned
//! identity and effect types so authority cannot leak through JSON fields.

pub mod bind;
pub mod diagnostics;
pub mod engine_patch;
pub mod lower;
pub mod perception;
pub mod pipeline;
pub mod semantic;
pub mod simulate;
pub mod source;

pub use bind::{
    BindingReport, BindingStatus, BoundCandidate, DeterministicEntityBinder, EntityBinder,
    EntityBinding, EntityCatalog, EntityDescriptor, EntityRole,
};
pub use diagnostics::{
    CompilerContractError, CompilerDiagnostic, CompilerStage, DiagnosticSeverity,
};
pub use engine_patch::{lower_state_effects_to_engine_patch, EnginePatchLoweringReport};
pub use lower::{
    DeterministicEffectLowerer, EffectLowerer, EffectProvenance, LoweringReport,
    MemoryFormationKind, RelationshipEvidenceSignal, StateEffect, StateEffectKind,
};
pub use perception::{
    perception_ir_json_schema, seal_perception_batch, BehaviorEvidenceKind, ClaimValue,
    DurabilityHint, EpistemicMode, EvidenceSource, EvidenceSpan, ModelProvenance, PerceptionBatch,
    PerceptionBatchDraft, PerceptionCandidate, PerceptionCandidateDraft, PerceptionKind,
    RelationshipSignalDraft, TemporalAnchor, TemporalExpression, PERCEPTION_IR_SCHEMA_NAME,
    PERCEPTION_IR_SCHEMA_VERSION,
};
pub use pipeline::{compile_perception_pipeline, CompilerPipelineReport};
pub use semantic::{
    DeterministicSemanticAnalyzer, SemanticAnalyzer, SemanticDisposition, SemanticReport,
    ValidatedCandidate,
};
pub use simulate::{
    DeterministicTransactionSimulator, ProposedTransaction, SimulationDecision, SimulationReport,
    SimulationSnapshot, TransactionSimulator,
};
pub use source::{SourceEnvelope, SourceIdentity, SOURCE_ENVELOPE_SCHEMA_VERSION};

/// Version of the Rust-side compiler contract and artifact identity rules.
pub const MEMORY_COMPILER_CONTRACT_VERSION: u32 = 2;
