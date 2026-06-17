# Mnemosyne Roadmap

Mnemosyne is a local-first universal RP state map: a campaign brain for long-form roleplay.

Chat is only the visible interaction layer. The product is the durable state graph underneath it: characters, relationships, objects, locations, events, factions, secrets, unresolved tensions, memories, and continuity facts that survive long sessions, model swaps, imports, corrections, and branches.

This roadmap is the current plan. Older roadmap text is archived under `docs/Old plans/`.

## Product Thesis

Mnemosyne should not compete with JanitorAI or SillyTavern as another chat frontend. Its differentiator is state.

The engine should answer:

- Who exists?
- Where is everyone?
- What happened?
- Who knows what?
- Who believes something false?
- What changed this turn?
- What object/faction/location/relationship was affected?
- Why did the AI remember this?
- Can the user correct it and continue?

The milestone is a 50+ turn session where the user can inspect the state map, trace memories to source turns, correct wrong facts, continue the RP with the correction respected, and export/import the session as a `.mne` bundle without loss.

## Current Architectural Finding

The dual-pass design is correct:

1. **Narrator** writes visible RP prose.
2. **Evaluator** extracts structured state changes.
3. **Patch ledger** stores state diffs.
4. **Materialized state** is rebuilt from the ledger.
5. **State Map UI** exposes what the engine believes.

The mistake was enforcing the evaluator schema through prompt wording. The old evaluator path asks a model to fill a large custom text form, then Rust repairs, normalizes, and validates whatever comes back. That is fragile because every model interprets prompt conventions differently.

The fix is provider-side schema enforcement: structured outputs, tool calling, or grammar-constrained JSON. In plain language: stop mimicking tool calling with prompts.

## Priority 1: Schema-Enforced Evaluator

**Goal:** evaluator output is constrained by a JSON schema or tool definition at the provider/decoder level.

The evaluator should emit patch operations against compact current-state JSON, not fill a mega-form.

Required work:

1. Define evaluator patch/operation schema once in code.
2. Add provider-side structured-output support:
   - OpenAI/OpenRouter-compatible `response_format: json_schema`
   - Anthropic tool use
   - Google structured output where available
   - local llama.cpp/grammar-constrained JSON where available
3. Route `evaluator_structured_v1` through schema-enforced completion.
4. Record enforcement level in traces:
   - `json_schema`
   - `json_object`
   - `none`
5. Keep semantic validation:
   - entity exists
   - relationship target is valid
   - object identity is stable
   - evidence quote is valid
   - deltas are sane
6. Gradually delete syntactic repair paths once schema enforcement proves stable:
   - malformed JSON repair
   - field-name canonicalization
   - form-line recovery
   - bespoke row-shape salvage

Done when:

- An evaluator model that never learned the old form prompt can pass the contract test.
- It writes correct patches for 20 consecutive normal RP turns.
- Syntactic repair paths report zero activations.
- State commits continue through memory, relationship, object, scene, and event patches.

## Priority 2: Cheap And Conditional Evaluator

**Goal:** evaluator overhead stops doubling every turn.

Two levers:

### Cheap evaluator model

Structured extraction does not need the same model as narration. Once schema enforcement works, a small strict model can be used for evaluator work.

### Conditional execution

Not every turn deserves full evaluation.

Modes:

| Mode | Behavior | Use case |
|---|---|---|
| Fast | Evaluator only on significant turns, with scene-boundary catch-up | Casual/cost-sensitive sessions |
| Balanced | Evaluator every turn with a cheap schema-compatible model | Default |
| Long Context | Evaluator every turn with richer extraction and larger memory budget | Deep campaigns, war games |

The dev console must show per-turn prompt tokens, output tokens, total estimated cost, enforcement mode, and whether the evaluator ran or skipped.

Done when:

- Fast mode evaluates at most 50% of turns in a 50-turn session.
- State map continuity does not regress.
- Cost differences are visible in the trace UI.

## Priority 3: Memory Pool Overhaul

**Goal:** store more, inject less.

Do not solve continuity by dumping more memories into prompts. Store memory generously, retrieve selectively.

Required work:

1. Remove destructive recent-memory truncation.
2. Archive inactive/evicted memories instead of deleting them.
3. Keep prompt memory budgets capped.
4. Add cross-slot deduplication so one memory appears in only one prompt section.
5. Add memory pinning.
6. Add real decay/reinforcement.
7. Populate provenance:
   - source message ID
   - conversation ID
   - branch ID
   - turn ID
   - confidence
   - truth status
   - evidence quote

Done when:

- A 100-turn session retains active + archived memory history.
- No selected memory appears twice in one prompt.
- Every new memory can be traced to the exact source turn.

## Priority 4: State Map V1 And Memory Inspector

**Goal:** make the engine's beliefs visible and correctable.

Panels:

- **Scene State:** location, participants, positions, door/room state, active objects, current misunderstanding, open question, last action.
- **Characters:** who exists, what each knows, what each misbelieves.
- **Relationships:** current values, recent changes, source events.
- **Objects:** identity, owner, location, status. Identity stays separate from condition.
- **Timeline:** recent events, active plot threads, unresolved tensions.
- **Memory Inspector:** all memories with provenance, pin/edit/invalidate controls, and retrieval trace.

Add `scene_continuity_summary` as the narrator's primary continuity anchor. Current truth belongs in scene state. Durable facts belong in memory. Immediate events belong in the timeline.

Done when:

- A user can inspect a wrong fact, see which turn created it, correct or invalidate it, and continue with the correction respected.

## Priority 5: Structural Decomposition

`commands.rs` remains too large. Do not do a big-bang refactor. Extract modules only while touching the relevant code.

Targets:

- narrator turn orchestration
- evaluator job lifecycle
- provider structured-output calls
- slash command routing
- import/export and `.mne` bundles
- state map query layer

Every extraction must keep tests green.

## Core Abstractions

### Entities

Characters, player personas, NPCs, factions, parties, armies, nations, objects, and locations.

### Edges

Relationship, ownership, alliance, hostility, proximity, command chain, knowledge, suspicion, promise, and debt.

### Events

Evidence-backed changes: who acted, who perceived it, where it happened, what changed, and what evidence supports it.

### State

Current material truth: scene position, location, object state, relationship state, faction state, resources, morale, danger, and unresolved pressure points.

### Secrets And Knowledge

Track who knows a fact, who suspects it, who is wrong, and what remains hidden.

### Timeline

Recent major events, active plot threads, unresolved tensions, future timers, and consequences waiting to trigger.

### Entity Separation

The engine must keep these identities distinct:

- real user/operator
- player persona
- narrator-controlled NPC
- active Soul
- imported-log speaker
- previous-session persona
- OOC/GM channel

Relationships attach to personas, not globally to the operator. Imported speakers never merge with the current user without confirmation.

## Explicit Non-Goals

- Not selling "always cheaper tokens." Short sessions may cost more than single-call frontends.
- Not competing on chat UI features alone.
- Not building a perfect physics/economy simulator.
- Not hand-rolling schema enforcement when provider APIs can enforce it.

## Current Build Target

Implement `evaluator_structured_v1` as the main experimental evaluator path.

The first implementation slice should be additive and safe:

1. Keep `evaluator_form_v1` as the stable fallback.
2. Add schema-enforced completion for structured evaluator calls.
3. Log enforcement level in payload history.
4. Parse schema-enforced EnginePatch JSON directly.
5. Fall back to `evaluator_form_v1` or last known-good evaluator if enforcement is unavailable or the patch is semantically invalid.
6. Prove it with a 20-turn diagnostic.

This keeps the project moving toward the real architecture without deleting the hard-won safety net too early.
