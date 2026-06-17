# Archived Roadmap: State Map And Schema-Enforced Engine

Archived copy of `docs/state-map-roadmap.md` before the roadmap was cleaned into a single current plan focused on schema-enforced/tool-calling evaluator work.

---

# Mnemosyne Roadmap: The State Map And The Schema-Enforced Engine

Mnemosyne is a universal RP state map: a local-first campaign brain for long-form roleplay.

It is not merely a better JanitorAI, SillyTavern, or chat frontend. Chat is the visible interaction layer. The durable product is the living state graph underneath it: characters, relationships, objects, locations, events, factions, secrets, unresolved tensions, and continuity facts that can survive long sessions, model swaps, imports, corrections, and branch changes.

The project is open source. Its goals, in order: prove the Soul system (portable, inspectable character state) works for real long-form play; be genuinely useful for personal sessions, multi-character D&D-style campaigns, and war games; and stand as a portfolio-grade piece of engineering.

## Glossary (read this first if you are new to the codebase)

| Term | Meaning |
|---|---|
| **Narrator** | The LLM call that writes the visible RP prose the user reads. |
| **Evaluator** (state updater) | A second LLM call that reads the latest exchange and extracts structured state: new memories, relationship changes, object changes, world events. The user never sees its output directly. |
| **Dual-pass** | The narrator + evaluator pattern: one call for prose, one for state. |
| **Patch** | A structured diff describing state changes from one turn. Patches are stored in a ledger and applied to materialized state (`state_engine/src/patch.rs`). |
| **Soul** | A character's full persistent state: identity, memories, relationships, schemas. Portable across sessions and models. |
| **`.mne` bundle** | Exported checkpoint containing messages, branches, patches, memories, relationships, and object state. |
| **Structured outputs / tool calling** | Provider API features that force a model's output to match a JSON schema at the decoding level. The model *cannot* produce malformed JSON or unknown fields. Supported by Anthropic, OpenAI, Google, and llama.cpp (grammar-constrained sampling). |
| **Materialized state** | The current state rebuilt by replaying the patch ledger. |

## Where The Engine Stands (verified June 2026)

What works today, confirmed by code review and a passing test suite (596 tests):

- The full loop runs: narrator call → evaluator call → patch compile → patch apply → materialized state rebuild → `.mne` export/import.
- Evaluator model compatibility gates exist: `run_evaluator_contract_test` marks provider profiles untested/passed/failed, the UI blocks failed profiles, and `handle_evaluator_streak_and_fallback` auto-falls back to the last known good evaluator after repeated failures.
- Per-turn pipeline tracing exists (`TurnPipelineTrace`): per-stage timing, status, and error codes, visible in the dev UI, plus estimated input tokens per prompt section.
- Data safety is archive-first: archive columns instead of hard deletes, automatic DB backups, crash recovery on startup, and API keys are redacted from exports.

What is fragile, and why this roadmap exists:

- The evaluator contract is enforced by **prompt wording**, not by the API. Roughly 10,000 lines in `state_engine/src/evaluator_form/` exist to compile a bespoke text form, then parse, repair, normalize, and validate whatever the model sends back. Swapping the evaluator model broke state writing because every model interprets prompt conventions differently.
- The evaluator runs on every normal scene turn at full size, so a turn often costs two large model calls.
- The stored memory pool is destructively capped at 12 (`MAX_RECENT_MEMORIES`, `patch.rs`): after each patch apply, memories are sorted by salience and everything past 12 is hard-deleted.
- The same memory can be selected into multiple prompt sections, because slot retrieval has no cross-slot deduplication.
- Provenance fields (`source_message_id`, `confidence`, `truth_status`) exist in the schema but are mostly never populated, and there is no UI to inspect, pin, edit, or invalidate a memory.
- `commands.rs` is ~22,000 lines and contains the entire turn pipeline.

## The Core Architectural Finding

The dual-pass design is correct. Every serious memory system (MemGPT/Letta, ChatGPT memory, agent frameworks) separates "write prose" from "extract state," because one call cannot be maximally creative and maximally schema-compliant at the same time.

What was wrong is **how the schema contract was enforced**. The engine asked models to fill a custom text form described in prose, then repaired the output in Rust. That is hand-rolling, by prompt, what provider APIs now do natively: structured outputs and tool calling constrain the model at the decoding level so invalid JSON is impossible.

In plain terms: we were brute-forcing tool calling through prompting. The 10k-line repair layer is not a sign the idea is wrong — it is the cost of doing schema enforcement ourselves. Most of it becomes deletable once the API enforces the schema.

Everything below follows from that finding.

---

## Pillar 1: Schema-Enforced Evaluator (highest leverage, do first)

**Goal:** the evaluator returns provider-validated JSON matching a schema defined once in code. Model compatibility becomes "does it support tool calling?" instead of "did it learn our form dialect?"

**What changes:**

- `providers/api.rs` gains structured-output support: Anthropic tool use, OpenAI `response_format: json_schema`, and grammar-constrained sampling for local llama.cpp-style servers. Today this file uses none of these.
- The evaluator schema (memories, relationship events, object updates, world events) is defined once as a JSON Schema in `state_engine`, generated from the existing Rust types where possible.
- The evaluator proposes **patch operations against a compact current-state JSON** instead of filling one mega-form: "here is current state, here is the new exchange, output a list of ops." Smaller prompt, smaller output, and the ops map directly onto the existing patch ledger.
- **Keep** semantic validation (does this entity exist, is this delta sane, is this object a duplicate). **Delete** syntactic repair (JSON fixing, field-name canonicalization, form-line recovery) as the schema enforcement makes it unreachable. Expect to remove a large fraction of `raw_repair.rs` and `normalize.rs` over time.
- The existing compatibility gate and auto-fallback stay, but they become a safety net instead of a load-bearing wall.

**Why an entry-level dev should care:** after this change, adding a new evaluator-capable model means checking one box (supports tool calls) — not debugging why a model writes `Object_ID:` instead of `object_id`.

**Done when:** a model that was never tested against the old form prompt can be set as evaluator, pass the contract test, and write correct patches for 20 consecutive turns, with the syntactic repair paths reporting zero activations.

## Pillar 2: Cheap And Conditional Evaluator (kills the cost problem)

**Goal:** evaluator overhead drops from ~100% of turn cost to a small fraction, making the long-session token story honest.

Two independent levers:

**Lever A — cheap model.** Structured extraction does not need a frontier model. Once Pillar 1 enforces the schema, a small fast model (Haiku-class, or a local model with grammar constraints) can run the evaluator at roughly a tenth of narrator cost. The compatibility gate already exists to verify any candidate.

**Lever B — conditional execution.** The narrator already emits a ```status``` block every turn — a free significance signal currently unused for gating. Add a cheap pre-evaluator gate:

- Dialogue-only or OOC turns: skip the evaluator entirely.
- Scene-relevant turns: run it.
- Scene boundaries: run a batched catch-up evaluation covering the skipped turns.

**Exposed to users as three modes:**

| Mode | Behavior | For |
|---|---|---|
| **Fast** | Evaluator only on significant turns + scene-boundary catch-up | Casual sessions, cost-sensitive users |
| **Balanced** (default) | Evaluator every turn on a cheap model | Most sessions |
| **Long Context** | Evaluator every turn + richer extraction + larger memory budget | Deep campaigns, war games |

Add output-token tracking and a per-turn cost estimate to the existing pipeline trace panel so the modes are visibly different, not just labeled.

**Done when:** a 50-turn session in Fast mode runs the evaluator on ≤50% of turns with no continuity regressions in the state map, and the dev console shows per-turn estimated cost.

## Pillar 3: Memory Pool Overhaul (store more, inject less)

**Goal:** separate stored memory capacity from prompt memory budget. Storage can be large; the prompt stays lean.

- **Lift the destructive cap.** Replace the hard truncate-at-12 in `patch.rs` with archival: evicted memories become inactive but stay queryable, restorable, and exportable. Nothing the evaluator wrote is ever silently destroyed.
- **Keep the prompt budget capped.** The context compiler's slot budgets and token caps stay. More storage must not mean bigger prompts.
- **Cross-slot deduplication.** Thread a selected-IDs set through the slot retrieval loop in `context_compiler.rs` so one memory appears in at most one prompt section. (Small fix, large prompt-quality win.)
- **Decay and pinning.** Salience decays over time unless reinforced; users can pin memories that must never decay or be evicted. The `decay` placeholder in `consolidation.rs` becomes real.
- **Populate provenance.** Fill the existing `source_message_id`, `confidence`, and `truth_status` fields at memory creation, plus conversation/branch/turn IDs, so every memory can answer "which exchange created you?"

**Done when:** a 100-turn session retains its full memory history (active + archived), no memory appears twice in one prompt, and every memory created after the change carries a source message ID.

## Pillar 4: State Map V1 + Memory Inspector (the product centerpiece)

**Goal:** make what the engine believes visible and correctable. Inspection and correction are the core loop and the moat — history-replay frontends are wrong-and-baked-in; Mnemosyne is wrong-and-fixable.

The UI exposes:

- Scene State panel — current truth: location, participants, positions, door/room state, active objects, current misunderstanding, open question, last concrete action
- Characters panel — who exists, what each knows or misbelieves
- Relationships panel — current values, recent changes, and the events that caused them
- Objects panel — identity, owner, location, status (identity stays separate from condition)
- Timeline panel — recent events, active plot threads, unresolved tensions
- Memory Inspector — every memory with provenance: click through to the exact source turn; pin, edit, or invalidate from there; see why it entered (or didn't enter) the current prompt, powered by the existing `MemorySlotTrace` debug data

A compact `scene_continuity_summary` derived from this state becomes the narrator's primary continuity anchor, ahead of long-term memories. Current truth lives here; durable facts live in memory; what-just-happened lives in the timeline.

**Done when:** a user can run a session, open the state map, click a wrong fact, see which turn created it, correct or invalidate it, and continue playing with the correction respected.

## Pillar 5: Structural Decomposition (ongoing, ride along with Pillars 1–2)

`commands.rs` (~22k lines) holds the entire turn pipeline. Do not do a big-bang refactor; let the Pillar 1–2 rewrites carve out natural modules as they touch the code:

- turn orchestration (narrator path)
- evaluator job lifecycle (spawn, retry, fallback, streak handling)
- slash command handling
- import/export and `.mne` bundles

Each extraction must keep the test suite green. New code goes in the right module even if old code hasn't moved yet.

---

## Core Abstractions (unchanged direction)

### Entities

Persistent things the story can refer to, track, and connect: characters, player personas, NPCs, factions, parties, armies, nations, objects, locations.

### Edges

Durable connections between entities: relationship, ownership, alliance, hostility, proximity, command chain, knowledge, suspicion, promise, debt.

### Events

Evidence-backed changes in the fiction. Each event should answer: who acted, who perceived it, where it happened, what changed, what evidence supports it.

### State

The current material truth of the active session: scene position, location, physical and object state, relationship state, faction state, resources, morale, danger, unresolved pressure points.

### Secrets And Knowledge

Knowledge is scoped. A fact can be true without being known by every entity. The state map should eventually track: who knows a fact, who suspects it, who is wrong about it, what remains hidden from whom.

### Timeline

Keeps continuity from becoming a loose pile of memories: recent major events, active plot threads, unresolved tensions, future timers, consequences waiting to trigger.

### Entity Separation (standing requirement)

The engine must keep these identities distinct: real user/operator, player persona, narrator-controlled NPC, active Soul, imported-log speaker, previous-session persona, and the OOC/GM channel. Relationships attach to personas, not globally to the operator. Imported speakers never merge with the current user without confirmation. This matters most for multi-user campaigns, D&D parties, and war games — the exact scenarios this engine exists to serve.

## What We Are Explicitly Not Doing

- **Not selling "always cheaper tokens."** Short sessions may cost more than a single-call frontend. The honest claim: bounded context for long sessions, where history-replay frontends grow without limit and rot.
- **Not competing with SillyTavern on chat features.** The differentiators are the state engine, the Soul system, `.mne` portability, and the correction loop — not character cards or chat UI polish.
- **Not building a perfect simulator.** The state map tracks what the engine believes so a human can fix it; it does not simulate physics or economies.
- **Not hand-rolling what provider APIs do natively.** No more prompt-convention contracts where a schema-enforced API call exists.

## The Milestone

> A user can run a 50+ turn multi-character session (a D&D party or a war game qualifies), watch token cost stay bounded while a history-replay frontend's would keep growing, inspect what the engine believes in the State Map, trace any memory to the exact turn that created it, correct what's wrong, continue playing with the correction respected, and export/import the whole session as a `.mne` bundle without loss.

That milestone keeps the product honest. It proves Mnemosyne is a local-first RP state engine that makes long-form roleplay inspectable, correctable, portable, and stateful — not just a chat interface with nicer memory.
