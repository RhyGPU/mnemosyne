# Mnemosyne Program Design Document, Bible V2

**Project name:** Mnemosyne  
**Pronunciation:** neh-MOSS-uh-nee  
**Meaning:** Greek goddess of memory, mother of the Muses  
**Tagline:** Remember who you are.  
**Repository:** https://github.com/RhyGPU/mnemosyne  
**License:** AGPL-3.0-or-later  
**Document status:** Working design bible, Git-ready draft  
**Version:** 2.0  

---

## 1. Executive Pitch

Mnemosyne is an open-source narrator engine for persistent AI roleplay and long-form story creation.

Most AI roleplay breaks because the model is forced to be everything at once: the character, the narrator, the memory bank, the world tracker, and the continuity manager. Mnemosyne separates those jobs.

The LLM acts as the **Narrator**. The Mnemosyne engine manages the character's persistent **Soul**.

A Soul is a portable, structured character state package that tracks identity, relationships, memories, emotional patterns, body and sensory state, boundaries, and long-term development. A separate World Log tracks the scene, location, active plots, key objects, time, and shared events. Before each turn, Mnemosyne compiles only the relevant state into a compact context brief, keeping token use lean while preserving continuity across long campaigns.

Characters remember what matters, forget what fades, and change at a believable pace. Stories can run for dozens or hundreds of sessions without collapsing into contradiction, emotional reset, or generic chatbot drift.

Mnemosyne runs locally, is AGPL open source, and lets users bring their own model provider or API key. It is built for roleplayers, worldbuilders, and writers who want AI stories with memory, consequence, continuity, and character growth.

---

## 2. One-Line Pitch Options

1. **The narrator writes. The Soul remembers.**
2. **AI roleplay with memory, continuity, and consequence.**
3. **Persistent souls for long-running AI stories.**
4. **Roleplay that remembers who the character is.**
5. **A local memory engine for AI-assisted storytelling.**

Recommended public tagline:

> **The narrator writes. The Soul remembers.**

Recommended project tagline:

> **Remember who you are.**

---

## 3. Project Identity

Mnemosyne is not a chatbot character impersonator. It is a local narrator and game-master engine for AI-assisted roleplay.

The design principle is:

> The LLM should not be the database. The LLM should be the writer.

Mnemosyne manages the database: character state, relationship change, world continuity, memory retrieval, consolidation, decay, and context compression. The LLM receives a curated briefing each turn and writes the next piece of narration.

This makes the system closer to a roleplay operating layer than a simple chat wrapper.

---

## 4. Core Problem

Most AI roleplay systems suffer from five recurring failures:

1. **Amnesia:** the model forgets important events, promises, trauma, relationships, injuries, objects, and plot threads.
2. **Emotional flatness:** characters react intensely for one turn, then reset.
3. **Context bloat:** long sessions become expensive and unstable because the entire chat history is repeatedly injected.
4. **Character drift:** the character gradually loses their starting personality and begins mirroring the model's default voice.
5. **World inconsistency:** locations, objects, timelines, and active plots contradict themselves over time.

Mnemosyne attacks these failures at the application layer through structured state, retrieval, compression, and deterministic rule application.

---

## 5. Core Solution

Mnemosyne splits AI roleplay into four cooperating systems:

| System | Responsibility | Updated |
|---|---|---:|
| **Narrator** | Generates prose, dialogue, scene flow, and descriptions | Every turn |
| **Soul** | Stores private character continuity, psychology, relationships, and memory | Every turn |
| **World Log** | Stores shared story continuity, location, plot, objects, and time | Scene shifts or scheduled updates |
| **Context Compiler** | Selects and compresses relevant state for the LLM | Every turn |

The LLM never receives raw state files. It receives a compact, curated context brief.

---

## 6. High-Level Data Flow

```text
User Message
    |
    v
Input Analyzer
    - detects entities, active characters, scene topic, emotional tone, body/status references
    |
    v
State Engine
    - loads Soul modules
    - loads World Log
    - retrieves relevant memories
    - selects recent chat turns
    - allocates token budget
    |
    v
Context Compiler
    - builds compact briefing
    - injects narrator mode
    - injects boundaries and rating constraints
    |
    v
LLM Provider
    - OpenAI-compatible API, local model, or supported provider connector
    |
    v
Narrator Response
    - visible narration
    - hidden structured patch proposal
    |
    v
Patch Parser
    - extracts hidden patch
    - validates JSON/schema/allowed fields
    |
    v
State Engine Applies Rules
    - clamps numeric deltas
    - updates relationships
    - stores new memories
    - updates world state
    - updates body/sensory state if relevant
    - scores salience and novelty
    |
    v
Consolidation
    - scheduled or event-driven
    - promote, merge, decay, discard
    |
    v
Persistence
    - SQLite for runtime state and conversations
    - .mne bundle export for portable Souls and Settings
    |
    v
Clean visible narration shown to user
```

---

## 7. Design Pillars

### 7.1 Narrator, Not Character

The AI does not directly become the character. It narrates the character in third person according to the selected mode.

Benefits:

- Reduces out-of-character contamination.
- Keeps player agency clearer.
- Makes multi-character scenes easier.
- Allows stronger world and scene management.
- Allows the engine to remain the authority on memory and state.

### 7.2 Deterministic Engine, Creative LLM

The Rust engine handles rules, numbers, memory scoring, patch validation, persistence, and consolidation.

The LLM handles prose, scene flow, tone, dialogue, and interpretation.

The LLM can propose state changes, but the engine decides what is accepted.

### 7.3 External Memory, Not Raw Chat History

The system does not rely on the LLM reading the entire session every turn.

Instead, Mnemosyne stores state externally and compiles a lean briefing:

```text
[SYSTEM PROMPT]
[NARRATOR MODE]
[CONTENT BOUNDARIES]
[CHARACTER SNAPSHOT]
[WORLD SNAPSHOT]
[RELEVANT MEMORIES]
[RECENT CHAT]
[USER MESSAGE]
```

This keeps token cost mostly flat over long campaigns.

### 7.4 Realistic Change, Not Instant Mutation

Characters should not emotionally transform from one line of dialogue unless the event truly justifies it.

Mnemosyne uses speed gates:

| State category | Change speed |
|---|---:|
| Immediate mood | Fast |
| Relationship trust | Medium-fast |
| Needs and stress | Medium |
| Trauma phase | Slow |
| Personality drift | Very slow |
| Core identity | Extremely slow |

### 7.5 Memory Is Living Structure

Memory is not a static transcript. It is a living system.

- Important memories are reinforced.
- Ordinary details fade.
- Repeated patterns compress into schemas.
- Highly relevant details are retrieved when needed.
- Contradictory or stale details can be superseded by newer state.

---

## 8. System Architecture

```text
Mnemosyne
├─ Desktop Client
│  ├─ React UI
│  ├─ Tauri shell
│  ├─ Library view
│  ├─ Chat view
│  ├─ Character creator
│  ├─ Setting creator
│  └─ Debug/state inspector
│
├─ Rust State Engine
│  ├─ Soul manager
│  ├─ World manager
│  ├─ Memory scorer
│  ├─ Consolidator
│  ├─ Context compiler
│  ├─ Patch validator
│  ├─ Retrieval layer
│  └─ Persistence layer
│
├─ Provider Layer
│  ├─ Mock provider
│  ├─ OpenAI-compatible API provider
│  ├─ Local model provider, planned
│  └─ Browser/subscription connector, experimental future
│
└─ Portable Data Layer
   ├─ .soul.json, legacy
   ├─ .setting.json, legacy
   └─ .mne bundle, planned
```

---

## 9. Runtime Data Model

During active play, Mnemosyne should prefer database-backed runtime state.

Recommended runtime source of truth:

```text
SQLite
├─ souls
├─ soul_modules
├─ settings
├─ conversations
├─ messages
├─ memories
├─ world_events
├─ provider_profiles
├─ sessions
└─ migrations
```

Recommended export format:

```text
.mne bundle
├─ manifest.json
├─ identity.json
├─ psyche.json
├─ relationships.json
├─ memory.json
├─ body_state.json
├─ sensory_state.json
├─ boundaries.json
├─ world_link.json
└─ metadata.json
```

Principle:

> Many modules internally, one portable package externally, one compiled context brief sent to the LLM.

---

## 10. Soul Package Design

A Soul should eventually be modular. One giant JSON file is acceptable for MVP, but long-term growth requires module separation.

### 10.1 Manifest

```json
{
  "package_type": "mnemosyne_soul",
  "package_version": "2.0",
  "character_id": "uuid",
  "character_name": "Aurora Schwarz",
  "created_with_engine_version": "0.2.0",
  "schema_versions": {
    "identity": 1,
    "psyche": 1,
    "relationships": 1,
    "memory": 1,
    "body_state": 1,
    "sensory_state": 1,
    "boundaries": 1
  },
  "modules": {
    "identity": "identity.json",
    "psyche": "psyche.json",
    "relationships": "relationships.json",
    "memory": "memory.json",
    "body_state": "body_state.json",
    "sensory_state": "sensory_state.json",
    "boundaries": "boundaries.json"
  }
}
```

### 10.2 Identity Module

Static or mostly static character foundation.

```json
{
  "character_name": "Aurora Schwarz",
  "description": "",
  "appearance": "",
  "personality": "",
  "backstory": "",
  "scenario_seed": "",
  "creator_notes": ""
}
```

Rules:

- The LLM should not automatically rewrite identity.
- Identity changes require explicit user approval or a major engine-level event.
- Identity is injected into context in compressed form.

### 10.3 Psyche Module

Mutable psychological state.

```json
{
  "global": {
    "dev_stage": 6,
    "attach_style": 2,
    "fear_baseline": 15,
    "resolve": 40,
    "shame": 45,
    "openness": 45
  },
  "needs": {
    "physiological": 60,
    "safety": 50,
    "belonging": 40,
    "esteem": 30,
    "actualization": 20
  },
  "self_determination": {
    "autonomy": 70,
    "competence": 40,
    "relatedness": 10
  },
  "trauma": {
    "phase": 2,
    "hypervigilance": 10,
    "flashbacks": 10,
    "numbing": 10,
    "avoidance": 10
  },
  "mood": {
    "valence": 0,
    "arousal": 0,
    "stability": 50
  }
}
```

Notes:

- Psyche values are not personality replacements.
- They are hidden state for continuity and response shaping.
- Changes should be speed-gated.

### 10.4 Relationships Module

Per-target relationship state.

```json
{
  "targets": {
    "user": {
      "trust": 10,
      "affection": 20,
      "intimacy": 10,
      "commitment": 10,
      "fear": 10,
      "respect": 10,
      "resentment": 0,
      "dependence": 0,
      "relationship_label": "stranger"
    }
  }
}
```

Rules:

- Relationship changes should be small unless the scene is major.
- The engine clamps values and rejects extreme jumps without a high-salience justification.
- Relationship summaries are injected, not raw tables unless debugging.

### 10.5 Memory Module

```json
{
  "core": [],
  "recent": [],
  "schemas": [],
  "retrieval_index": [],
  "forgotten_archive": []
}
```

Memory classes:

| Class | Purpose |
|---|---|
| **Core** | High-salience identity-shaping memories |
| **Recent** | Vivid short-term memories |
| **Schemas** | Compressed repeated patterns |
| **Retrieval index** | Search metadata, embeddings, tags, entities |
| **Forgotten archive** | Optional non-contextual archive for audit/debug |

### 10.6 Body State Module

A neutral physical-state system for fiction, survival, combat, illness, fatigue, recovery, sensory adaptation, and genre-specific extensions.

```json
{
  "global": {
    "health": 100,
    "fatigue": 0,
    "stress_load": 0,
    "pain_load": 0,
    "sleep_debt": 0
  },
  "regions": [],
  "conditions": []
}
```

Body regions should be dynamic and sparse. A region should be created only when it becomes narratively relevant.

```json
{
  "region_id": "right_fist",
  "label": "Right fist",
  "group": "hand",
  "side": "right",
  "traits": {
    "pain_sensitivity": 42,
    "touch_sensitivity": 50,
    "pressure_tolerance": 60,
    "impact_tolerance": 67,
    "temperature_sensitivity": 50,
    "fatigue": 30,
    "injury": 12,
    "numbness": 5,
    "control": 58,
    "comfort_association": 0,
    "aversion": 0
  },
  "sensitive_to": [
    {
      "stimulus": "blunt_impact",
      "response": "tolerance_gain",
      "strength": 0.35
    }
  ],
  "memory_links": [],
  "last_updated_turn": 183
}
```

Official body groups should stay broad:

```text
head, face, jaw, neck, shoulders, chest, back, abdomen, pelvis,
upper_arm, elbow, forearm, wrist, hand, palm, fingers, thumb,
hip, thigh, knee, shin, calf, ankle, foot, toes, internal, custom
```

Rules:

- Do not feed the full body map to the LLM.
- Feed only relevant regions.
- Use custom labels for local/private extensions.
- Keep the official schema neutral and genre-flexible.

### 10.7 Sensory State Module

Tracks associations between sensory cues and emotional or narrative responses.

```json
{
  "associations": [
    {
      "sense": "smell",
      "cue": "warm rice and broth",
      "association": "home_safety",
      "strength": 72,
      "valence": 60,
      "memory_links": ["mem_001"],
      "decay_rate": 0.02
    }
  ]
}
```

Supported senses:

```text
smell, taste, touch, sound, sight, balance, temperature, proprioception, custom
```

### 10.8 Boundaries Module

The boundaries module defines content scope, rating, user preferences, creator rules, and platform safety settings.

```json
{
  "content_rating": "mature",
  "allowed_themes": [],
  "blocked_themes": [],
  "soft_limits": [],
  "hard_limits": [],
  "commercial_export_allowed": true,
  "marketplace_allowed": true,
  "requires_adult_only_characters": true
}
```

Rules:

- The engine should support configurable boundaries.
- Mature or restricted content should be explicitly gated.
- Marketplace and export systems should be stricter than local private use.
- The public engine should avoid hardcoded explicit presets and instead allow abstract, user-defined local extensions.

---

## 11. World Log Design

The World Log tracks the shared external story state.

```json
{
  "setting_id": "uuid",
  "setting_name": "Carver City",
  "location": "Abandoned subway platform",
  "atmosphere": "cold, echoing, tense",
  "active_plots": [
    "Find a safe exit",
    "Avoid patrols"
  ],
  "recent_events": [],
  "key_objects": [],
  "present_characters": [],
  "time_elapsed": "Night 1, roughly 40 minutes since entry"
}
```

World Log update frequency:

| Trigger | Update? |
|---|---:|
| Major location change | Yes |
| New objective | Yes |
| New important object | Yes |
| New character enters | Yes |
| Every ordinary turn | No |
| Every 3 to 5 turns | Optional scheduled compression |

Compression rule:

```text
Recent events remain vivid.
Older events merge into summaries.
Repeated routines become schemas.
Resolved plot threads collapse into one-line history.
```

---

## 12. Context Compiler

The Context Compiler is the most important optimization layer.

It decides what the LLM sees.

### 12.1 Input Sources

```text
System prompt
Narrator mode
Content boundaries
Soul identity summary
Psyche summary
Relationship summary
Relevant core memories
Relevant recent memories
Relevant schemas
World Log summary
Relevant body/sensory state
Recent chat turns
Current user message
```

### 12.2 Token Budget Allocation

Default target:

```text
2,000 to 4,500 tokens per turn
```

Suggested allocation:

| Block | Target tokens |
|---|---:|
| System prompt and mode | 400 to 900 |
| Character identity | 150 to 400 |
| Psyche and relationship state | 200 to 500 |
| World Log | 250 to 600 |
| Retrieved memories | 300 to 900 |
| Body and sensory state | 0 to 500, only if relevant |
| Recent chat | 500 to 1,200 |
| User message | variable |

### 12.3 Dynamic Context Priorities

| Scene type | Prioritize |
|---|---|
| Combat | body state, injuries, environment, objects, recent action |
| Emotional scene | relationship state, recent dialogue, core memories |
| Investigation | world log, clues, objects, unresolved plot threads |
| Travel | location, time, weather, active objective |
| Recovery | body state, needs, sensory comfort cues, trust state |
| Multi-character | active characters, per-character relationship summaries |

---

## 13. Retrieval Layer

The retrieval layer selects old information relevant to the current turn.

Retrieval signals:

```text
entity overlap
location overlap
topic overlap
emotional tag overlap
relationship target
active plot relevance
recency
salience
retrieval strength
memory links
body/sensory relevance
```

Memory score for retrieval:

```text
retrieval_score =
  semantic_relevance
  + entity_relevance
  + plot_relevance
  + emotional_relevance
  + salience
  + recency_bonus
  + reinforcement_bonus
  - staleness_penalty
```

MVP retrieval can use lexical overlap and tags. Future retrieval should use embeddings.

---

## 14. Memory Scoring

Mnemonic salience should be deterministic and engine-owned.

Suggested formula:

```text
memorability =
  emotional_intensity * 0.35
  + novelty * 0.20
  + goal_relevance * 0.20
  + relationship_relevance * 0.15
  + plot_relevance * 0.10

final_score = memorability * repetition_discount * boundary_modifier
```

Where:

| Factor | Meaning |
|---|---|
| emotional_intensity | How emotionally charged the event was |
| novelty | How different the event is from existing memories |
| goal_relevance | Whether it affects needs or objectives |
| relationship_relevance | Whether it changes a social bond |
| plot_relevance | Whether it affects the active story |
| repetition_discount | Repeated low-value events are compressed |
| boundary_modifier | Some content may be stored only locally or excluded from export |

---

## 15. Memory Consolidation

Consolidation runs every 10 turns by default, and also after major events.

Process:

```text
1. Re-score recent memories.
2. Promote high-salience memories to core.
3. Merge repeated middle-salience memories into schemas.
4. Decay weak unrepeated memories.
5. Keep only top recent memories.
6. Update retrieval index.
7. Write audit event.
```

Default thresholds:

| Score | Action |
|---:|---|
| 0.80 and above | Promote to core |
| 0.50 to 0.79 | Keep recent or merge into schema |
| 0.30 to 0.49 | Decay unless reinforced |
| Below 0.30 | Drop or archive |

---

## 16. Hidden Patch Protocol

The LLM should not output a full Soul file. It should output a small patch proposal.

Visible response:

```text
Third-person narration shown to the user.
```

Hidden response:

```json
{
  "soul_patch": {
    "relationship_delta": {},
    "psyche_delta": {},
    "new_memories": []
  },
  "world_patch": {
    "location": null,
    "recent_event": null,
    "active_plot_add": [],
    "active_plot_resolve": [],
    "key_object_add": []
  },
  "body_patch": {
    "region_updates": [],
    "condition_updates": []
  },
  "sensory_patch": {
    "association_updates": []
  }
}
```

The engine must validate:

```text
valid JSON
known schema version
allowed fields only
numeric clamps
speed gates
content boundary rules
character age and rating rules
commercial export rules
module permissions
```

Rejected patches should not corrupt state.

---

## 17. Narrative Modes

### 17.1 Realistic Mode

External-only narration, like a film camera.

- No internal monologue.
- No private thoughts.
- Emotions shown through body language, tone, action, and dialogue.

### 17.2 Reader Mode

Close third-person fiction.

- Internal thoughts are allowed.
- Perspective is limited to the active character.
- No omniscient knowledge unless justified.

### 17.3 God Mode

Omniscient narrative mode.

- Can include hidden context, foreshadowing, off-screen events, and dramatic irony.
- Useful for GM-style narration and novel drafting.

### 17.4 Custom Mode

User-provided narrator prompt.

- Must still respect engine boundaries.
- Should not bypass patch validation.

---

## 18. Multi-Character Settings

A Setting Soul is a shared world container.

```text
Setting
├─ World Log
├─ Conversation history
├─ Present characters
├─ Active character list
├─ Shared objects
├─ Scene timeline
└─ Linked character Souls
```

Active character detection:

```text
1. Scan recent user and narrator turns.
2. Detect character names, aliases, pronouns, and direct address.
3. Include only present or relevant Souls.
4. Compile per-character summaries.
5. Prevent cross-character memory leakage unless shared in-scene.
```

Multi-character context must be aggressively compressed.

---

## 19. Provider Architecture

### 19.1 Mock Provider

Purpose:

- Test state pipeline without API cost.
- Validate hidden patch parsing.
- Debug UI and memory behavior.

### 19.2 OpenAI-Compatible API Provider

Supports providers that expose `/chat/completions`-style APIs.

Configuration:

```json
{
  "base_url": "https://api.example.com/v1",
  "api_key": "",
  "model": "",
  "system_prompt": ""
}
```

### 19.3 Local Model Provider, Planned

Possible backends:

```text
LM Studio
Ollama
llama.cpp server
OpenAI-compatible local endpoints
```

### 19.4 Browser/Subscription Connector, Experimental

Potential future phase for subscription-based services.

This should be treated carefully because provider terms, automation restrictions, and account policies vary.

---

## 20. UI Design

### 20.1 Library View

Purpose:

- Browse Souls.
- Browse Settings.
- Create new Soul.
- Create new Setting.
- Import/export packages.
- View schema version and module health.

### 20.2 Character Creator

Sections:

```text
Identity
Appearance
Personality
Backstory
Scenario seed
Psyche sliders
Relationship presets
Boundary settings
Advanced modules
```

### 20.3 Chat View

Sections:

```text
Narration panel
User input
Mode selector
Provider selector
Token gauge
State summary
Recent memory preview
World Log preview
Debug patch viewer
```

### 20.4 State Inspector

Advanced/debug view:

```text
Raw Soul module viewer
World Log viewer
Memory scoring inspector
Patch validation logs
Context preview
Token allocation report
```

---

## 21. Content Boundaries and Rating System

Mnemosyne is a local, user-controlled roleplay engine, but the official project still needs a clear content architecture.

Recommended rating levels:

```text
general
teen
mature
restricted_adult
```

Rules:

- The engine should support configurable content boundaries.
- Restricted content must be opt-in, adult-gated, and excluded from public marketplace defaults.
- The official schema should remain neutral and modular.
- Local user-defined extensions may exist, but the core project should avoid hardcoded explicit presets.
- Marketplace, hosted services, and commercial export require stricter rules than private local use.

This protects the project from becoming narrowly defined by one content category while still allowing genre flexibility.

---

## 22. Novel and Manuscript Export Vision

Long-term goal:

> Turn long-running roleplay campaigns into structured story material.

Pipeline:

```text
Chat session
    |
    v
Scene extractor
    |
    v
Timeline builder
    |
    v
Chapter planner
    |
    v
Continuity checker
    |
    v
Style pass
    |
    v
Editable manuscript
    |
    v
DOCX / Markdown / EPUB / PDF export
```

Export modes:

| Mode | Purpose |
|---|---|
| Raw transcript | Archive the session |
| Clean transcript | Remove hidden/debug data |
| Scene summary | Summarize session events |
| Novel draft | Convert RP to prose |
| Chapter outline | Planning and revision |
| Continuity report | Find contradictions |

Commercial export should check:

```text
user ownership
character ownership
setting ownership
provider terms
marketplace policy
content rating
boundary module
```

---

## 23. Monetization Strategy

The core engine should remain open source.

Monetization should come from optional services, not from locking local users.

Possible revenue layers:

| Layer | Revenue model |
|---|---|
| Mnemosyne Core | Free, AGPL open source |
| Mnemosyne Studio | Paid creator tools |
| Cloud sync | Subscription |
| Encrypted backup | Subscription |
| Creator marketplace | Commission |
| Premium export templates | One-time or subscription |
| Hosted multiplayer campaigns | Subscription |
| Professional writing tools | Paid tier |

Recommended principle:

> Charge for distribution, convenience, collaboration, export tooling, and marketplace infrastructure, not for local memory itself.

---

## 24. Licensing and Open Source Position

Mnemosyne is AGPL-3.0-or-later.

Implications:

- Users can run, study, modify, and redistribute the software under AGPL terms.
- Modified network-accessible versions have source disclosure obligations under AGPL.
- Commercial services are possible, but monetization should be designed around hosted value, support, marketplace infrastructure, or dual licensing if needed.

Possible future licensing strategy:

```text
Core engine: AGPL
Official assets/branding: trademark controlled
Marketplace terms: separate platform agreement
Hosted service: AGPL compliance plus commercial service terms
Optional enterprise/dual license: future consideration
```

---

## 25. Security and Privacy

Core privacy promises:

```text
local-first
user-owned data
portable Souls
no required central account for local use
API keys stored locally
clear provider configuration
```

Recommended safeguards:

```text
encrypted local key storage
redaction before export
module-level export controls
content boundary flags
marketplace validation
integrity hashes for .mne bundles
schema migration logs
```

---

## 26. Roadmap

### Phase 0, Current MVP

Already represented in the current project direction:

```text
Tauri desktop shell
React UI
Rust state engine
SQLite persistence
Soul schema
World Log basics
Mock provider
API provider
Hidden state parsing
Memory scoring
Consolidation
Import/export JSON
```

### Phase 1, Stabilize Engine Core

```text
Patch protocol v1
Patch validator
Schema migration system
Context compiler token allocator
Better debug state viewer
Provider profiles
Streaming responses
Error recovery
```

### Phase 2, Modular Soul Package

```text
manifest.json
identity module
psyche module
relationships module
memory module
body_state module
sensory_state module
boundaries module
.mne zipped bundle export/import
```

### Phase 3, Retrieval and Long-Term Memory

```text
lexical retrieval
entity index
location index
memory links
retrieval scoring
embedding retrieval, optional
memory contradiction detection
schema summarization improvements
```

### Phase 4, Multi-Character Settings

```text
Setting Soul
active character detection
multi-Soul context compilation
per-character relationship states
shared World Log
cross-character memory boundaries
```

### Phase 5, Creator and Novel Tools

```text
session timeline
scene extractor
chapter planner
manuscript export
continuity checker
Markdown export
DOCX export
EPUB export
```

### Phase 6, Distribution and Marketplace

```text
creator profiles
Soul marketplace
Setting marketplace
campaign templates
export templates
commission model
content rating enforcement
adult-gated sections where legally appropriate
```

---

## 27. Technical Inspirations and References

Mnemosyne's architecture is conceptually aligned with external memory and context-management approaches in LLM systems.

- **Attention Is All You Need**, Vaswani et al., 2017, introduced the Transformer architecture built around attention mechanisms.
- **Retrieval-Augmented Generation**, Lewis et al., 2020, combines parametric model knowledge with non-parametric retrieved memory.
- **MemGPT**, Packer et al., 2023, frames long-context LLM agents through virtual context management inspired by operating system memory hierarchies.
- **Tauri 2**, official project documentation, supports cross-platform apps using a web frontend and Rust application logic.
- **GNU AGPL-3.0**, official license text, defines terms for AGPL distribution and network-accessible modified versions.

External links:

```text
https://arxiv.org/abs/1706.03762
https://arxiv.org/abs/2005.11401
https://research.memgpt.ai/
https://v2.tauri.app/
https://www.gnu.org/licenses/agpl-3.0.en.html
```

---

## 28. Non-Goals

Mnemosyne should not become:

```text
a simple chatbot skin
a transcript-only memory tool
a prompt-only character card format
a platform that requires central hosting
a system that blindly trusts LLM-written state
a giant raw JSON dump injected into every prompt
a narrow single-genre adult-content tool
```

Mnemosyne should remain:

```text
local-first
state-driven
modular
open-source
provider-flexible
story-focused
memory-centered
creator-friendly
```

---

## 29. Final Product Thesis

Mnemosyne exists because long-form AI roleplay needs more than a bigger context window.

It needs memory architecture.

A bigger context window lets the model see more text. Mnemosyne gives the story a persistent structure: who the character is, what changed them, what they remember, where they are, what matters now, and what should fade.

The model writes the next scene.

Mnemosyne remembers why it matters.
