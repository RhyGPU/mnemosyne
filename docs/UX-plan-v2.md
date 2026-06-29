# Mnemosyne UX Plan v2 — Session-Purpose Driven

Status: draft, started 2026-06-27. **This is the new top-level UX plan.**
It supersedes the *information-architecture* portion of `UI-overhaul-plan.md`
(§3, §6) and reframes the whole flow around **what a session is for**.
The old plan stays valid as the **implementation backlog**: its Phase-0 monolith
refactor, the built-but-invisible feature inventory (§2/§2a), the Dev Mode spec
(§4), and the magnetic-scroll/chat fixes all still hold. Theme lives in
`UI-theme-direction.md`. Engine truth lives in `MNEMOSYNE_BIBLE.md`.

---

## 0. The core idea (read this first)

Every other RP app has **one** UI for **one** kind of play. Mnemosyne runs
solo immersion, GM-led adventures, wargames, multi-user ensembles, and novel
drafting — and those want *different things on screen.* So the master dial of
the whole UX is:

> **A session has a Purpose. The Purpose decides what every surface shows,
> which features turn on, and who is allowed to see what.**

Two layers stack:

- **Purpose** (set when the session is created) — the *frame*. Picks defaults,
  emphasis, and whether visibility is symmetric (everyone sees the same) or
  asymmetric (a GM sees more than players).
- **Narrative Mode** (Realistic / Reader / God / Custom — Bible §17) — the
  *lens*, switchable in-scene. Purpose sets its default and its ceiling.

**Storage principle (locked):** the backend is the single source of truth and
**always stores full, unredacted state.** Purpose and Mode only *filter
presentation* (omit / redact / show). Visibility is never enforced by withholding
storage — change the Mode and the hidden truth is simply revealed. This makes
Purpose **mutable mid-campaign**: flip it and every surface recomputes visibility
live from the same saved state.

Nothing below is a fixed screen. Every screen is a **template that the Purpose
configures.** That is the innovation the mock and the old plan both miss: they
designed one screen for one kind of player.

---

## 1. The five Session Purposes

| Purpose | Who's at the table | North star | Default mode | Visibility | The State Map leans on… |
|---|---|---|---|---|---|
| **Immersive** | 1 player + 1 Soul | Disappear into the story | Reader | Symmetric, heavily censored | Soul feelings + shared memories; NPC secrets hidden |
| **Director (GM)** | 1 host + player(s) | Run a living table | GM: God · Players: Reader | **Asymmetric** | Everything for GM; redacted dossier for players |
| **Tactical (Wargame)** | player(s), state-heavy | Consequence & position | Realistic | Symmetric | Body state, objects, world log, objectives |
| **Ensemble** | many Souls / users | Believable group dynamics | per-participant | Asymmetric, per-character | Relationship web; who-knows-what boundaries |
| **Author** | 1 writer | Draft a manuscript | God | Full open | Timeline = chapter spine; everything visible |

These map directly onto the stated product scope (AI RP, DnD, wargames, multi
AI/User RP) plus the Bible's novel-export vision (§22). A Purpose is stored on
the session; it's chosen at creation and rarely changes (changing it is a
deliberate act, like switching difficulty).

**Master rule:** Purpose sets (a) default narrative mode + its ceiling, (b)
which State Map panels are emphasized/hidden, (c) which innovations are on and
how loud, (d) symmetric vs asymmetric visibility. Mode is the per-view lens on
top, bounded by Purpose.

---

## 2. Information architecture — adaptive nav rail

Persistent left **nav rail** (decided 2026-06-27; supersedes the old "no tab
bar"). Meaningful destinations only; it **hides during full-immersion Play and
Dev Mode** so story and machine room stay full-bleed.

```
┌───────────┐
│ ▣ Home    │  campaigns / resume · recent + recommended         [global]
│ ▷ Play    │  active session — Book register                    [session]
│ ◈ State   │  Soul + World, purpose+mode censored               [session]
│ ◰ Library │  characters · worlds · personas — create/manage    [global]
│ ⚙ Settings│  providers · chat · data · about (drawer)          [overlay]
└───────────┘  Dev toggle (in session) → terminal layer; not a rail item
```

Each destination is **purpose-aware**, not a fixed page:

- **Home** — resume + recommended, with sessions badged by Purpose so you re-enter
  the right headspace. "New session" begins with **picking a Purpose** (this is the
  most important new step — it configures everything downstream).
- **Play** (Book) — the transcript. Sensory callbacks, the mode lens, and the
  consolidation beat appear here, tuned by Purpose (§4).
- **State Map** (Editorial) — panels reorder and censor by Purpose × Mode ×
  ownership (§3).
- **Library** (Editorial) — Souls, Worlds, Personas; create/manage; savepoints;
  import/export; "get more characters" → web. Absorbs the old Editor.
- **Settings** — drawer from anywhere.
- **Dev Mode** — in-session terminal re-skin (old plan §4): raw, unredacted,
  purpose-agnostic. The power surface.

Registers: Home / State Map / Library = Editorial paper · Play = Book paper ·
Dev = Terminal.

---

## 3. State Map — the killer-feature showcase

One page, Editorial register, panels: **Scene · Characters (knows / misbelieves)
· Relationships · Objects · Body · Timeline · Memory Inspector (with
provenance).** It is gated on three axes:

1. **Mode** (the lens):
   - **Realistic → omit** sensitive rows entirely (no trace a secret exists).
   - **Reader → redact** with reveal-on-tap (black bars are the feature).
   - **God → plaintext** (full dramatic irony).
2. **Ownership:** the player's *own* character's knows/misbelieves stays hidden
   (don't spoil your own blind spot); NPC misbeliefs are shown as the dramatic
   irony reader/god is *for*.
3. **Purpose** (emphasis + asymmetry): Immersive hides most; Director shows all
   to the GM and a redacted copy to players; Tactical floats Body/Objects/World
   up; Ensemble leads with the relationship web; Author opens everything.

The **raw** inspector (unredacted modules, salience scores, patch logs, context
preview) always lives in **Dev Mode** — same data, two faces: dossier vs machine.

---

## 4. The four innovations — specified per Purpose

Each is grounded in an existing engine capability, so none of this is UI writing
checks the backend can't cash.

### 4.1 Living memory surface — *memory that visibly lives and dies*
**Backend:** salience scoring (Bible §14), decay, consolidation every ~10 turns
(§15: promote → merge into schemas → decay → drop), classes core/recent/schemas/
forgotten-archive (§10.5).
**UX:** in the State Map Memory panel, render memories by **weight and age** —
core set in dark sharp ink, recent bright, fading ones greying toward the
forgotten archive (Book/Editorial ink = the perfect metaphor). Consolidation is a
visible **beat in Play**: every ~10 turns the character "settles" and you watch
memories merge into a schema or dim out — an invisible cron job made into an
emotional moment.
**Per Purpose:** subtle/ambient in *Immersive*; full and auditable (with scores
on hover) in *Director*/*Author*; *Tactical* mostly ignores it; *Ensemble* shows
it per-character with memory-boundary lines.
**Build first** — cheapest way to make the Soul *feel* real.

### 4.2 Mode as a lens you flip on the page
**Backend:** narrative modes already first-class (Bible §17).
**UX:** a prominent in-scene control (not a settings toggle). Switching visibly
transforms the surface — redaction bars lift, internal monologue fades in,
dramatic irony appears. Flipping to God lifts the veil over the State Map in real
time. The censorship mechanic becomes a *toy*, not a config.
**Per Purpose:** *Immersive* caps at Reader — going God is a deliberate "peek"
that's framed as stepping out of character; *Director* gives God to the GM seat
only; *Author* defaults God; *Tactical* defaults Realistic.

### 4.3 Body & sensory state — the underused gem
**Backend:** `body_state` with sparse regions + traits (Bible §10.6),
`sensory_state` cue→association links (§10.7). "Feed only relevant regions."
**UX:** (a) a sparse anatomical **ghost** on the State Map that lights only the
regions narratively active right now; (b) **sensory callbacks inline in Play** —
when a cue ("warm rice and broth") triggers an association (`home_safety`), a
quiet margin note shows the memory it stirred. The character *associating*, shown
in the prose.
**Per Purpose:** prominent in *Tactical* (injury/fatigue/position) and intimate
*Immersive* scenes; minimal in *Author*; off by default where no regions exist
(stays sparse, per the Bible rule).

### 4.4 The Soul's biography — the long arc
**Backend:** persistent Soul across hundreds of sessions; speed-gated change
(§7.4); trauma phases, relationship trajectories, very-slow identity drift.
**UX:** a per-Soul **longitudinal view** (reachable from State Map / Library):
trust with the player across N sessions, trauma phase stepping down over time,
identity drift. A returning player sees *growth*. Doubles as the on-ramp to novel
export — this timeline **is** the chapter outline (Bible §22).
**Per Purpose:** the pride of *Immersive* (proof she remembers) and the working
document of *Author*; a roster/standings view in *Ensemble*.

---

## 5. Build order (start here)

Sequenced so something *feels* magical as early as possible, on top of the old
plan's Phase-0 monolith refactor (prerequisite, unchanged).

- **A — Purpose model + adaptive router.** Add `purpose` + `mode` to the session
  data model; nav rail + screen router; "New session = pick a Purpose" flow.
- **B — State Map page.** The Purpose × Mode × Ownership censorship engine and the
  panel layout. Structural centerpiece everything else hangs on.
- **C — Living memory surface + consolidation beat (4.1).** First visible magic.
- **D — Mode-as-lens control (4.2).** Cheap once B exists; makes the demo sing.
- **E — Body/sensory ghost + sensory callbacks (4.3).**
- **F — Soul biography / longitudinal (4.4).**
- **G — Ensemble: relationship web + asymmetric GM visibility; novel-export
  on-ramp.** (Pairs with old-plan Phase 7 multi-character backend.)

Dev Mode (old plan §4) proceeds in parallel as the raw counterpart to B–F.

---

## 6. Data-mapping notes (verified against the Bible — don't assume these are free)
- **"knows / misbelieves" is not a literal schema field.** The Bible tracks
  memories (§10.5) and relationships, not a `knows`/`misbelieves` map. Define the
  mapping: *knows* = the character's retrievable memories/beliefs; *misbelieves* =
  a belief the character still holds that the current world-truth contradicts
  (a superseded/invalidated memory — the mock's "invalidated" state). If we want
  the State Map's Characters panel, the engine needs a cheap "belief vs current
  truth" diff, or we derive it from invalidated/superseded memories.
- **Memory provenance** (source turn) is plausibly available; the mock's exact
  **evidence quote** may not be stored today — confirm before promising it in the
  Memory Inspector, or add quote capture at extraction time.
- Everything in §4 (salience, decay, consolidation, modes, body/sensory, speed
  gates) **is** backed by the Bible (§14, §15, §17, §10.6, §10.7, §7.4).

## 7. Settled (2026-06-27)
- **Purpose = a set of toggles the user composes.** The five named Purposes are
  just **starting bundles** of toggles, not hard types. The user can recompose.
- **Purpose is mutable mid-campaign; visibility recomputes accordingly.** Backend
  stores everything always; Mode/Purpose only hide at the presentation layer
  (see Storage principle, §0).
- **Ensemble GM = whoever holds God mode** (for now). No separate role/seat yet.
- **Theme is fixed:** Book (Play) + Editorial-lite (documents) + Terminal (Dev),
  per `UI-theme-direction.md`. Not re-litigated.
- Consolidation beat: default to **ambient in the State Map** with an optional
  subtle Play marker; tune later with eyes on the running app.
