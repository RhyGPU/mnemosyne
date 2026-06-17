# Dev Note — 2026-06-18 — Living memory lifecycle (decay / reinforce / core)

## Diagnosis (the recurring "something's wrong")
Junna's devlogs 000–002 spec a salience-scored memory where **meaningful memories
become permanent core and non-core decay over time unless reinforced** ("a repeated
meal is forgettable; repeated kindness can become trust").

The previous implementation built only the first half:
- `memory::MemoryScorer` scores a new memory once (emotional tag + novelty + goal),
- `patch.rs apply_memories` sorts `recent` by salience and caps the active pool to
  `MAX_RECENT_MEMORIES = 12` (overflow archived, not deleted), pinned exempt,
- near-duplicate **merge** bumped salience/retrieval_strength to ≥55.

**Missing:** any decay over time. `salience`/`retrieval_strength` were **frozen at
birth** and never moved. So retention was a *static ranking* + a hard cap — nothing
faded, "core" was just "currently top-12" (not an earned permanent tier), and
"unless reinforced" had no teeth (nothing decayed for reinforcement to save).

Root pattern (same as the relationship axes): **the engine computes snapshots, not
dynamics.** Relationship deltas are a one-shot linear nudge; memory salience is
frozen at creation. Neither evolves over time, which is the whole point of the vision.

## What changed (this note) — memory only
Slow-burn living lifecycle in `state_engine/src/patch.rs`, **no schema change**
(reuses `salience` = stable importance, `retrieval_strength` = living presence):

- Constants: `CORE_SALIENCE_THRESHOLD = 85`, `MEMORY_FADE_BASE = 12`,
  `MEMORY_FADE_FLOOR = 8`, `MEMORY_REINFORCE_CLIMB = 9`.
- `is_core_memory()` = salience ≥ 85 → permanent (never ages, never fades out).
- `age_memories(soul)` called once at the top of `apply_memories` (≈ once per turn;
  replaying a session reproduces the fade). Active, non-pinned, non-core memories
  lose `MEMORY_FADE_BASE * (1 - salience/85)` retrieval_strength per turn (higher
  salience fades slower); below `MEMORY_FADE_FLOOR` they archive (not delete).
- Reinforcement (merge path): a recurring near-duplicate now **climbs** salience by
  `+9` (capped 100) and refreshes retrieval_strength to ≥70 — so a small recurring
  moment can graduate into core over ~4 reinforcements ("repeated kindness → trust"),
  and un-fades if it just crossed into core.
- Feel chosen by Junna: **slow burn** (~10 turns for a middling salience-50 memory to
  fade) + **two roads to core** (high-impact at birth OR earned via reinforcement).

Tests (307 engine pass): `ordinary_memory_fades_out_of_active_pool_over_turns`,
`core_memory_never_fades`, `higher_salience_fades_slower`. The two existing cap tests
(`memory_overflow_is_archived_not_deleted`, `pinned_memory_is_exempt_from_archival_eviction`)
were converted from multi-turn loops to single-turn batch applies so they isolate
**cap archival** from the new per-turn fade (their actual intent) — assertions
unchanged in strength.

## Tunable knobs (plain-language → constant)
- "How fast ordinary memories fade" → `MEMORY_FADE_BASE` (higher = faster).
- "What counts as permanent core" → `CORE_SALIENCE_THRESHOLD` (lower = more becomes core).
- "How fast a recurring thing climbs to core" → `MEMORY_REINFORCE_CLIMB`.
- "When a faded memory drops from active" → `MEMORY_FADE_FLOOR`.

## Next steps / not done
1. **Relationship axes still have the same disease** — `relationship_delta_from_op`
   ([evaluator_structured.rs]) is a one-shot linear nudge that collapses 12 axes into
   6 scalars and discards `costliness`/`repetition`; it cannot hold "asshole but has
   your back". The faceted, non-linear, costly-signaling dynamical-system overhaul
   (per the earlier design discussion) is the next big piece. Memory was done first as
   the more self-contained place to land the first "dynamics over snapshots" change.
2. **Decay cadence**: aging runs per memory-apply (≈ per state-changing turn).
   Pure-dialogue turns the evaluator skips won't age memory. Fine for v1; revisit if
   long dialogue stretches should still age memory.
3. **Reinforcement reach**: only near-duplicate merge reinforces today. Thematic
   reinforcement (different events, same relational theme) isn't modeled yet.
4. Earlier-session benchmark work (live self-play, owl-alpha resilience, eval bg-job,
   perceived_by coercion) is in `2026-06-17-live-self-play-benchmark.md`.
5. **Local-eval + grammar** direction (bundled llama.cpp sidecar, Qwen2.5-3B,
   GBNF) still pending; prove the 3B's nuance before building the packaging.

## Gotcha
Cap tests now apply memories in a single batch on purpose — if you add a new
overflow/eviction test, decide whether you're testing cap (single batch) or fade
(multi-turn via repeated `age_memories`/applies), since both now archive.
