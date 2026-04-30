# Mnemosyne Multi-Character Settings Architecture

This is a future-facing architecture note. The MVP remains a single-character desktop client.

## Current MVP Boundary

The MVP stores one character Soul with one embedded `WorldLog`. This supports 1-on-1 RP and keeps the first client shippable.

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

Do not build this in the MVP. Implementation belongs after the single-character client is stable:

1. Extract `WorldLog` into a standalone setting file.
2. Add `Setting` persistence and Setting Manager UI.
3. Update context compilation for multi-Soul injection.
4. Add character import/export between settings.
