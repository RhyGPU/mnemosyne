# Mnemosyne State Map Roadmap

Mnemosyne should be understood as a universal RP state map: a local-first campaign brain for long-form roleplay.

It is not merely a better JanitorAI, SillyTavern, or chat frontend. Chat is the visible interaction layer. The durable product is the living state graph underneath it: characters, relationships, objects, locations, events, factions, secrets, unresolved tensions, and continuity facts that can survive long sessions, model swaps, imports, corrections, and branch changes.

## Core Thesis

Mnemosyne turns roleplay into inspectable state.

The same architecture should support:

- Character and relationship RP
- D&D-style party adventures
- Multi-user campaigns
- Political intrigue
- Survival and base-building stories
- Alternate-history war games

The goal is not to build a perfect simulator immediately. The near-term goal is State Map V1: a reliable, inspectable, correctable map of what the engine currently believes.

## Core Abstraction

### Entities

Entities are persistent things the story can refer to, track, and connect.

- Characters
- Player personas
- NPCs
- Factions
- Parties
- Armies
- Nations
- Objects
- Locations

### Edges

Edges describe durable relationships between entities.

- Relationship
- Ownership
- Alliance
- Hostility
- Proximity
- Command chain
- Knowledge
- Suspicion
- Promise
- Debt

### Events

Events are evidence-backed changes in the fiction.

Each event should be able to answer:

- Who acted
- Who perceived it
- Where it happened
- What changed
- What evidence supports it

### State

State is the current material truth of the active session.

- Scene position
- Current location
- Physical and object state
- Relationship state
- Faction state
- Resources
- Morale
- Danger
- Unresolved pressure points

### Secrets And Knowledge

Knowledge is scoped. A fact can be true without being known by every entity.

The state map should eventually track:

- Who knows a fact
- Who suspects a fact
- Who is wrong about a fact
- What remains hidden from whom

### Timeline

Timeline state keeps continuity from becoming a loose pile of memories.

- Recent major events
- Active plot threads
- Unresolved tensions
- Future timers
- Consequences waiting to trigger

## State Map V1

State Map V1 is the next product milestone. It should make the existing engine legible, not turn Mnemosyne into a full universal simulator.

The UI should expose:

- Scene State panel
- Characters panel
- Relationships panel
- Objects panel
- Timeline panel
- Open Tensions panel
- Debug/Memory Inspector panel

The user should be able to click a character, object, location, relationship, or event and see:

- What the engine believes is true
- Where that belief came from
- What changed this turn
- What memories were retrieved
- What relationships were updated
- What unresolved tensions remain

## Immediate Engineering Priorities

### 1. Model Compatibility Gate

Add role-specific compatibility tests.

- Narrator models can be creative.
- Evaluator models must be strict JSON/schema-compatible.
- Command models must be concise and instruction-following.
- A universal model must not silently take over all roles unless it passes each role contract test.

This should prevent a good prose model from being trusted with evaluator work it cannot reliably perform.

### 2. Evaluator Prompt Slimming

The evaluator prompt works, but it is too large and fragile for weaker models.

Convert evaluator instructions from a tutorial-style block into a compact schema card.

Requirements:

- Keep the hard fillable form.
- Remove contradictory or deprecated wording.
- Move relationship calibration references away from output schema instructions.
- Prefer machine-readable schema constraints over prose explanations where possible.

The evaluator should produce valid structured state evidence, not narrate its reasoning.

### 3. Running Scene Continuity Summary

Add a compact authoritative `scene_continuity_summary`.

This is current truth, not long-term memory.

It should include:

- Location
- Participants
- Positions
- Door/room state
- Current misunderstanding
- Active object
- Open question
- Last concrete action

This should reduce dependence on repeated memory snippets and make the narrator less likely to drift.

### 4. Memory Retrieval Cleanup

Do not remove prompt memory limits. Instead, separate stored memory capacity from prompt retrieval capacity.

Direction:

- Allow more stored memories.
- Retrieve only the best few.
- Prevent the same memory from appearing in multiple prompt sections.
- Use primary slot display.
- Hide secondary tags unless inspected.

The prompt should stay lean even when the local state map grows.

### 5. Relationship And Memory Dedupe

Relationship summaries should deduplicate by `source_soul_id + target_entity_id`.

Context should prefer latest materialized relationship state and remove baseline duplicate relationship facts from narrator/evaluator prompts.

Memory context should deduplicate across:

- `current_plot`
- `character_identity`
- `world_location`
- `unresolved_tension`
- `recent_emotional_state`

The same fact should not be repeated as five different prompt memories.

### 6. State Map V1 UI

The UI should show:

- Current scene truth
- Characters and their known facts
- Relationships and why they changed
- Objects with owner, location, and status
- Timeline events
- Unresolved tensions
- Retrieved memories and their evidence

Inspection and correction are the core loop. If the map is wrong, the user should be able to see why, correct it, and continue.

## Good-Enough Milestone

The milestone is not perfect simulation.

The milestone is:

> A user can run a 30-turn RP session, inspect what the engine believes, correct wrong memory/state, continue the RP, and export/import the session safely.

That milestone keeps the product honest. It proves Mnemosyne is becoming a local-first RP state map, not just a chat interface with nicer memory.
