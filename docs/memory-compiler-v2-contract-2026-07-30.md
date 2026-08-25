# ADR: Memory Compiler V2 Contract Boundary

- Date: 2026-07-30
- Status: Accepted and implemented
- Scope: M2 contracts only; no production evaluator cutover

## Context

The V1 structured evaluator lets one output type carry interpretation and
effect-like instructions. M1 blocks known authority leaks, but field-by-field
validation alone is not a durable architecture: adding a future field can reopen
the boundary.

M2 needs a type boundary in which the LLM cannot represent engine-owned source
identity, truth authority, state deltas, or ledger effects.

## Decision

The compiler uses two distinct perception artifacts.

```text
LLM JSON
  PerceptionBatchDraft
      |
      | Rust seal(source envelope + model provenance)
      v
  PerceptionBatch
      |
      v
  BindingReport -> SemanticReport -> LoweringReport
      |
      v
  ProposedTransaction -> SimulationReport
```

`PerceptionBatchDraft` is the complete model-writable surface. It contains:

- perception kind
- subject/predicate/object claim
- actor/perceiver/targets
- evidence span
- epistemic mode
- extraction confidence
- temporal expression
- durability hint

It contains no conversation, branch, turn, message, source hash, compiler
version, truth status, state delta, or effect field. Unknown fields are rejected.

Application code creates a `SourceEnvelope` from trusted turn identity and exact
texts. Rust then seals a valid draft into `PerceptionBatch`, adding:

- source hash
- deterministic candidate IDs
- compiler contract version
- provider/model/prompt/schema provenance

Effect provenance is created from a sealed candidate. Transactions reject
effects whose source hash or compiler version does not match the active source.

## Module Boundary

```text
state_engine/src/compiler/
|-- source.rs        trusted source identity and tamper-evident envelope
|-- perception.rs    LLM draft and Rust-sealed perception artifacts
|-- bind.rs          entity binding contracts
|-- semantic.rs      semantic acceptance/rejection contracts
|-- lower.rs         evidence-backed StateEffect contracts
|-- simulate.rs      proposed transaction and simulation contracts
`-- diagnostics.rs   stage-specific machine-readable diagnostics
```

The module has no Tauri, SQLite, provider transport, or UI dependency.

## Identity Rule

Artifact IDs currently use deterministic FNV-1a 64-bit digests with
length-prefixed components and a namespace. They are replay identities and
tamper signals, not cryptographic signatures. If hostile storage becomes part of
the threat model, the digest implementation can be upgraded behind the contract.

Source hashes cover:

- source schema version
- conversation/branch/turn/parent identity
- user/assistant message and variant identity
- observation time and parent state hash
- exact user/assistant text
- canonical active soul IDs

## Deliberate Non-Implementation

M2 does not yet:

- call a V2 model
- resolve entity aliases
- decide epistemic authority
- lower perceptions into real `EnginePatch`
- write compiler artifacts to SQLite
- mutate the ledger

These belong to M3/M4. Keeping them out of M2 makes the authority contract
testable without changing product behavior.

## Rejected Alternatives

- Extend `EvaluatorStructuredOutputV1`: preserves the interpretation/effect
  conflation.
- Let the LLM emit source IDs and validate them afterward: creates avoidable
  spoofing surface.
- Put provider and DB types in `state_engine`: destroys the reusable,
  deterministic domain boundary.
- Write directly to a graph/vector store: makes a derived projection the source
  of truth.

## Verification

- 10 compiler contract tests pass.
- Draft authority-field injection is rejected.
- Source text mutation after sealing is detected.
- Candidate and effect IDs replay deterministically.
- Cross-source effect injection is rejected.
- Invalid confidence and malformed evidence offsets are rejected before sealing.
- Archived artifacts round-trip without type loss.
- Full `state_engine` test suite passes.
- Tauri library suite: 416 passed, 1 ignored.

## Rollback

The compiler module is not connected to the production V1 evaluator or ledger.
Removing the module and its `lib.rs` export fully rolls back M2 without data
migration.
