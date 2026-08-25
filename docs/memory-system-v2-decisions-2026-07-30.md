# Memory System V2 Architecture Decisions

- Date: 2026-07-30
- Status: Accepted
- Scope: Memory Compiler V2, Memory V2 projection, consolidation, recall

## Decision

Mnemosyne keeps the ledger and deterministic Rust compiler as the authority. Hindsight and
Cognee remain research references and optional benchmark opponents; neither is a runtime
dependency or source of truth.

The runtime pipeline is:

1. the LLM emits Perception IR, never state deltas;
2. Rust binds identities, validates evidence/authority/time, lowers effects, and simulates;
3. only a commit-ready `EnginePatch` enters the append-only ledger;
4. raw and derived Memory V2 records are rebuildable projections;
5. local recall combines FTS5, optional semantic scoring, temporal recency, filters, and
   one-hop evidence graph expansion;
6. the Context Compiler receives a compact evidence bundle with score traces, not an
   unlabelled memory dump.

## What We Adopted from Hindsight

- separate retain/recall/reflect responsibilities;
- evidence-backed derived observations;
- contradiction-aware consolidation;
- traceable recall scoring and long-horizon evaluation;
- idempotent processing as an explicit test target.

We did not adopt its service topology, database requirements, Python runtime, or data model.
Mnemosyne's branch-aware ledger remains authoritative and all projections are disposable.

## What We Adopted from Cognee

- graph relations are useful as a retrieval expansion signal;
- typed entities/evidence are more useful than a decorative neural map;
- ingestion, enrichment, and retrieval should be distinct stages.

We did not adopt an external graph database, a mandatory vector service, or a visual graph as
the system of record. SQLite edge rows are a rebuildable local projection.

## Consolidation Policies

- `Belief`: repeated testimony in the same topical cluster; explicit counterevidence lowers
  confidence and is stored separately.
- `Schema`: repeated episode/perception evidence in the same topical cluster.
- `RelationshipModel`: repeated evidence with the same owner/counterpart pair.
- `SelfModel`: repeated affect evidence owned by the same entity.
- `Reflection`: repeated intention evidence.

Every derived record keeps all source memory IDs and an evidence edge for every source.
Rebuild marks older derived rows stale before deterministic proposals are regenerated. New
evidence therefore never silently deletes the prior interpretation.

## Recall Policy

FTS5 is the always-available base. Optional embeddings implement a Rust trait and a disabled
adapter is the normal fallback. Truth status, memory type, owner/character, temporal range,
conversation, branch, and validity are applied before results enter context. Graph neighbors
must pass the same filters. Every hit records lexical, semantic, temporal, and graph scores
plus selection reasons.

PPR and an external graph database are rejected until a benchmark demonstrates a need.

## Benchmark Result and Limits

Deterministic tests verify:

- replay-equivalent projections and derived IDs;
- no cross-branch evidence;
- unrelated memories are not consolidated;
- explicit contradiction remains traceable;
- filtered recall excludes incompatible truth/type/owner/time records;
- the no-vector adapter keeps recall operational;
- a synthetic long-memory corpus retrieves both target memories with no irrelevant hit while
  using less text than the raw-memory context.

The configured narrator model currently returns an upstream 404, so provider latency/token
variance and a long live RP gate are not claimed here. Until that external gate passes,
`evaluator_form_v1` remains a rollback path and existing profiles retain their explicit mode.
New code should not expand the legacy evaluator.

## UI Boundary

State Map is a read-only evidence/projection inspector. A fix instruction appends a
`memory_correction_events` ledger-side event and creates a replacement turn; it does not edit a
projection row in place.
