# Mnemosyne Multi-Character Settings Architecture

This is a future-facing architecture note. It now sits under the broader State Map roadmap: Mnemosyne is moving toward a universal RP state map, while implementation should still ship in practical, inspectable milestones.

## Current Boundary

The original MVP stored one character Soul with one embedded `WorldLog`. That shape still matters for compatibility, but State Map V1 should treat world/session state as part of an inspectable graph rather than as a permanent single-character limitation.

The important compatibility rule is that world state must stay structurally separate from character-specific fields. In code, `WorldLog` remains its own serializable struct and `Soul.world` is defaultable during deserialization. That means future character-only Soul files can omit embedded world state while old Soul files with embedded world state still load.

## Target Architecture

Settings become containers for shared scene state:

```text
mnemosyne_data/
  settings/
    <setting_name>/
      setting.json
      world_log.json
      conversation.db
      souls/
        <character_1>.mne
        <character_2>.mne
        <character_3>.mne
  characters/
    <character_name>.mne
  presets/
    narrator_realistic.txt
    narrator_reader.txt
    narrator_god.txt
```

Each setting owns:

- Shared `WorldLog`: location, active plots, recent events, key objects, elapsed time.
- Shared conversation history.
- Multiple portable character Souls.

Each character Soul owns:

- Character identity and profile.
- Independent memory, schemas, relationships, trauma, and psyche.
- No required embedded world state in the future file shape.

## Active Character Detection

Future context compilation should detect active characters by scanning recent clean messages for character names in the current setting:

```rust
fn detect_active_characters(
    recent_messages: &[String],
    setting_characters: &[String],
) -> Vec<String> {
    let mut active = Vec::new();
    for message in recent_messages.iter().rev().take(10) {
        for character_name in setting_characters {
            if message.contains(character_name) && !active.contains(character_name) {
                active.push(character_name.clone());
            }
        }
    }
    active
}
```

Additional triggers:

- Hidden state field such as `characters_present`.
- Manual active/inactive toggles in the UI.
- Attributed dialogue.

## Multi-Soul Context Shape

When multiple characters are active, compile context as:

```text
[CURRENT STATE]
Location: <world_log.location>
Active Plot: <world_log.active_plots>

[PRESENT CHARACTERS]
- <name>: Fear <value>, Trust-><other> <value>, ...
- <name>: Fear <value>, Trust-><other> <value>, ...

[CHARACTER MEMORY - <name1>]
Core: <core memories>
Schema: <relevant schemas>
Recent: <recent events>

[CHARACTER MEMORY - <name2>]
Core: <core memories>
Schema: <relevant schemas>
Recent: <recent events>

[RELATIONSHIPS]
<name1> -> <name2>: Trust, Affection, notes
<name2> -> <name1>: Trust, Affection, notes

[RECENT EVENTS]
<shared recent events from world log>
```

## Schema Relevance

For MVP, do not implement schema relevance filtering. Later, filter by:

1. Recency within the last 50 turns.
2. Location match against the current `WorldLog.location`.
3. Character overlap with active characters.
4. Fallback to the most recent schemas if fewer than three survive.
5. Cap included schemas at three per character.

## Phase Gate

Do not chase the full multi-user campaign simulator before State Map V1 is legible and correctable. Implementation belongs after the core state map can show scene truth, entities, relationships, objects, timeline events, unresolved tensions, retrieved memories, and evidence:

1. Extract `WorldLog` into a standalone setting file.
2. Add `Setting` persistence and Setting Manager UI.
3. Update context compilation for multi-Soul injection.
4. Add character import/export between settings.

## Turn Structure for D&D / War Games (deferred — not the shipping path)

The sections above describe the *data* shape for multiple characters in a
setting. This section captures the *runtime turn structure* for a true
multi-character / multi-user simulator (D&D parties, war games). It is
**deferred**: the shipping product is single-narrator (the narrator reads the
scene and writes the story). The simulator is a separate mode that reuses the
same engine, not an increment on the narrator. Recorded here so the design
is not re-derived later.

### The core inversion

Today: narrator writes prose → evaluator extracts state from prose. Prose is
the source of truth; state is a lossy derivation.

Simulator mode flips this: **characters produce state/action → a referee
canonicalizes it → narration becomes an optional presentation layer.** For
multi-character and war-game play the authoritative record (who did what, who
knows what) must be structured state, not after-the-fact extraction. This
inversion is the whole reason the mode exists.

### The "Total War" activation model (starting idea)

Each character has a soul file, a **thought log** (its private belief stream),
and reads the shared action log. On its activation an LLM tool-calls edits to
its own soul + thought log and emits an action/dialogue to the shared log.
Then the next character activates. A narrator is optional and reads the action
log to dramatize it into prose, filling gaps.

This is a good core. Four changes make it work:

1. **Replace round-robin with a director/initiative scheduler.** Fixed turn
   order causes order bias (the last actor always reacts with full
   information; the first never reacts in the same beat) and stilted pacing
   (everyone speaks once per round). Pick the next actor by *pressure to act*
   (directly addressed, high stakes, holds initiative). This also yields a
   natural round-termination condition (no one above threshold → hand to
   user/narrator), which round-robin lacks. This is where the mode's quality
   concentrates.

2. **Referee is mandatory; narrator is optional.** Characters *propose*
   actions by editing their own soul/thought; a referee *disposes* — owning
   objective world truth and adjudicating conflicts (two characters grab the
   same object, two units enter the same hex, someone acts on a fact they
   should not know). The referee writes the canonical event log. Stack:
   per-character intents → referee resolves → canonical event log → optional
   narrator prose. Maps onto existing `SessionWorld` + event operations.

3. **Split belief from truth (this is fog of war).** The thought log is what
   a character *believes*; it is not ground truth. Ground truth lives with the
   referee. Each character's context = ground truth filtered through its
   perception, layered with its belief log. Get this right and hidden
   information falls out for free; get it wrong (every character reads the same
   omniscient world) and the simulator is theater where everyone secretly
   knows everything.

4. **Two activation modes by scene type.** Reactive sequential turns for
   dialogue (reacting to the last line is correct). Simultaneous-intent +
   deterministic resolution for tactical/war beats: all sides issue orders
   against the *same* world snapshot without seeing each other's move, then
   the referee resolves — restoring fog of war and removing order bias (real
   Total War resolves simultaneously, not per-unit turns). The scheduler
   chooses the mode.

### Discipline carried over from the narrator engine

Character self-edits to soul/thought must go through **validated patch ops on
the ledger** (the structured-evaluator + `curate_memory` machinery), never
free-form rewrites — otherwise characters drift into incoherence or game
themselves, and there is no replay/inspection. A character must not be able to
rewrite itself into a different person in one turn.

### Reuse map (most of this already exists)

- soul file → `Soul`
- world truth + events → `SessionWorld` + event operations
- per-character tool-call state edits → the structured evaluator, run
  per-character *pre-action* instead of once post-narration
- self-edit discipline → the patch ledger
- optional narrator → today's narrator, reading the event log instead of
  generating events

Genuinely new parts: the **thought log** (a per-character, belief-scoped
memory stream) and the **scheduler/referee** (new, and where the value is).

### Cost / latency caveat

Latency scales linearly with active cast — characters read prior outputs so
calls cannot be fully parallelized. Activation gating is therefore not
optional: only characters above the pressure threshold act each tick;
background characters idle.

### Prototype gate

This is a fork from the dual-pass product, not an increment. Prototype tiny
first — 2–3 characters, no narrator, a plain-text event log — and confirm the
belief-scoped turn-taking reads well before building the scheduler and
referee. If three characters taking belief-scoped turns already feels alive,
the thesis is proven cheaply. Player-controlled characters slot in as "skip
the LLM, wait for human input."
