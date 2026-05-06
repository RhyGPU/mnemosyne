# Mnemosyne Program Design Document ("The Bible")

## 1. Project Identity
**Name:** Mnemosyne  
**Pronunciation:** "neh-MOSS-uh-nee"  
**Meaning:** Greek goddess of memory, mother of the Muses.  
**Tagline:** *Remember who you are.*  
**License:** AGPL-3.0-or-later  
**Repository:** https://github.com/RhyGPU/mnemosyne  

## 2. Vision & Unique Selling Points
Mnemosyne is a **narrator/GM engine** for AI-assisted roleplay, not a character impersonator. It solves the fundamental problem of AI amnesia and emotional shallowness through a persistent, scientifically-modeled "Soul" file that evolves over time.

Unlike Janitor AI, SillyTavern, or ChatGPT-based RP, Mnemosyne:
- Separates the **narrator AI** from the **character's mind**.
- Maintains a **persistent memory system** with realistic forgetting, consolidation, and schema formation.
- Tracks **psychological stats** (trust, trauma, needs) with speed‑gated progression.
- Supports **multi‑character and multi‑setting** play.
- Is **open source (AGPL)** and runs locally, with the user bringing their own API keys or subscription chat accounts.

## 3. Core Architecture

### 3.1 High‑Level Data Flow
```
User Message
    |
    v
State Engine (Rust) compiles context
    |  (Soul summary, World Log, recent messages)
    v
System Prompt (Narrator + Mode) + Context + User Message
    |
    v
LLM API (DeepSeek, OpenAI, etc.)
    |
    v
LLM Response: Third‑person narration + [HIDDEN STATE] block
    |
    v
Parser (Rust) extracts hidden state
    |
    v
Engine applies deltas (trust, affection, arousal)
    |
    v
Memory scorer assigns salience & novelty
    |
    v
If turn count % 10 == 0: run consolidation
    |
    v
Soul file updated, conversation stored in SQLite
    |
    v
Clean narration (no hidden block) shown to user
```

### 3.2 Dual‑System Memory (Character + Story)
- **System 1 (Soul):** Private character memory. Tracks relationships, trauma, arousal, core memories, recent memories, schemas. Evolves with every interaction.
- **System 2 (World Log + Context Compiler):** Story tracker. Location, active plots, key objects, shared recent events. Compressed over time.

The LLM never sees raw state JSON. Instead, the context compiler injects a curated summary.

### 3.3 Narrative Modes
The user selects one of four modes at session start, injected as a modifier to the base narrator prompt:
1. **Realistic** – External only (film camera)
2. **Reader** – Close third‑person, limited to character's knowledge
3. **God** – Full omniscience
4. **Custom** – User provides their own system prompt

## 4. The Soul File (`soul.mne` or `.soul.json`)

### 4.1 Identity (Static)
- `character_name`, `appearance`, `setting`, `personality`, `backstory`
- These do not auto‑update; they are the starting foundation.

### 4.2 Psychological State (Mutable)
| Category | Variables | Speed |
|----------|-----------|-------|
| **Global** | fear_baseline, resolve, shame, openness, dev_stage, attach_style | Slow |
| **Needs (Maslow)** | phys, safety, belong, esteem, actual | Medium |
| **SDT** | autonomy, competence, relatedness | Medium |
| **Trauma** | phase, hypervigilance, flashbacks, numbing, avoidance | Slow |
| **Relationships** | trust, affection, intimacy, passion, commitment, fear, desire per target | Fast |

### 4.3 Memory Store
- **Core (long‑term):** Up to 5 narrative summaries of high‑salience events (score >0.70). Never deleted unless displaced by higher‑score memories.
- **Recent (short‑term):** Up to 4 detailed entries with retrieval strengths that decay.
- **Schemas:** Merged routines (e.g., "prison_routine: days in cell, cold meals") to compress repetitive events.

### 4.4 Memory Scoring
*Deterministic Rust code (not the LLM)* calculates a memorability score:
```
score = (emotional_intensity × 0.4) + (novelty × 0.3) + (goal_relevance × 0.3)
        × repetition_discount
```
- emotional_intensity: from tag mapping (e.g., `betrayal` = 0.95, `routine` = 0.30)
- novelty: Jaccard distance from existing memories (MVP) → embeddings (future)
- goal_relevance: matches character's current need deficits
- repetition_discount: diminishes if same tag/content already frequently recorded

### 4.5 Consolidation (Every 10 Turns)
1. Re‑score recent memories with updated needs.
2. Promote >0.70 → core (write narrative summary).
3. Discard <0.30.
4. Merge 3+ same‑tag middle‑range memories into a schema.
5. Keep top 4 recent memories.

### 4.6 Arousal State Machine (Sexual Response Model)
- Tracks `arousal_level` (0–100) across phases: NEUTRAL → AWARE → WARM → READY → PLATEAU → PEAK → ORGASM.
- Stimulus intensity mapped to delta (+2 to +60) with repetition damping.
- Supports edging (cap at PEAK with frustration buildup, sensitivity modifier), denial, forced orgasms, and male/female refractory differences.
- Stored in the Soul, injected into context as a one‑line summary.

## 5. The Setting Soul (Multi‑Character)
Setting Souls are containers that hold:
- A shared **World Log** (location, plot threads, time, key objects)
- A shared **conversation history** (SQLite)
- Multiple **character Souls** (each independent)

Active character detection: scan last 5–10 messages for character names; the context compiler then injects memories and relationships for all present characters.

## 6. Provider Architecture
- **API Providers:** Direct HTTP calls (reqwest) to DeepSeek, OpenAI, OpenRouter, etc. Streaming via Tauri events.
- **Browser Automation (Phase 5):** Playwright‑based connector for subscription chat services (ChatGPT Plus, Claude Pro) to piggyback on existing subscriptions.
- **Mock Provider (built):** Tag‑based templates for testing the full pipeline without API costs.

## 7. UI Layout (Tauri + React + Tailwind)
- **Chat View:** Clean narration display, user input, mode selector, context token gauge, debug panel.
- **Library View:** Browse Souls and Settings, create new, import/export, presets.
- **Character Creator:** Identity fields + full psyche sliders with presets.

## 8. Data Persistence
- **SQLite** for conversations, messages, provider profiles, session metadata.
- **JSON files** (`.soul.json`, `.setting.json`) for portable character and setting packages.
- **Future:** `.mne` bundle (zip of card image + psyche + memories).

## 9. Key Design Decisions
- **Narrator, not character** — the AI describes in third‑person; this prevents immersion breaks and OOC contamination.
- **Speed‑gated psychology** — trust/affection changes max ±1‑3 per scene; personality traits drift over weeks (in‑game).
- **Memory is a living structure** — promotion, demotion, schema compression; never a static log.
- **Deterministic engine, creative LLM** — the Rust side handles rules, numbers, and consolidation; the LLM only narrates.

## 10. Roadmap
| Phase | Features |
|-------|----------|
| **MVP (current)** | Single‑character, mock & API providers, memory engine, psyche sliders, 3 narrative modes |
| **Phase 2** | Streaming, saved provider profiles, message regenerate/delete, session list |
| **Phase 3** | Gears/Equipment stats, character card importer, Tauri build/installer |
| **Phase 4** | Multi‑character settings, active character detection, multi‑Soul context compilation |
| **Phase 5** | Browser automation providers, narrator search function, mobile builds |
