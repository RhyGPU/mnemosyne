# Mnemosyne UI Theme Direction

Status: decided 2026-06-27. Refines `UI-overhaul-plan.md` §6 ("Aesthetic: dual").

This pins the *visual language* only. UX/flow is governed by `UI-overhaul-plan.md`.

---

## The three registers (skin follows layer-of-reality)

The aesthetic is **layered to match what surface you're on**, so the texture itself
signals where you are. This supersedes the vaguer "warm and intimate" wording in
the overhaul plan — the human surface is **paper/e-ink, not dark-warm**.

| Surface | Register | Look |
|---|---|---|
| **Play / transcript** (the reading surface) | **Book** | Near-monochrome. Black ink on warm paper, one muted accent at most. Single column, generous margins, flat (no shadows/gradients). One reading serif. Mood: Kindle / reMarkable, distraction-free, "inside a novel." Calm on purpose — the story can't survive being busy. |
| **Documents** — Home/Library, State Map / Soul view, Soul file, sessions browse | **Editorial-lite** | Same paper, same ink, same serif — but more typographic *structure*: display headlines, rules/dividers, multi-column grids, section labels, pull quotes. Borrow editorial **hierarchy, not its color or loudness.** Info-dense screens that browse and inspect. Redaction bars read most "right" here (declassified-dossier feel). |
| **Dev / GM / god mode** (the machine room) | **Terminal** | Green-on-black, monospace, rigid rectangular frames, pipeline trace, command line. Matches the existing Dev Mode spec (overhaul plan §4). |

**Canonical base = Book; documents dial editorial structure up; play dials it to zero.**
One coherent paper family across the whole human surface; terminal only behind the
Dev/GM curtain.

## Why this fits Mnemosyne
- RP is *reading* → book serves immersion and long-session eye comfort.
- The killer feature is the **Soul** (memory/continuity) → the document register makes
  "this character remembers and has changed" legible without looking like a debug tool.
- **Redaction = the censorship mechanic.** Narrative modes (Realistic / Reader / God,
  already first-class in the Bible §17) decide what a state/Soul view shows. On paper,
  censored fields render as literal black redaction bars = a redacted dossier.
  - **Realistic → omit** the row entirely (no trace a secret exists).
  - **Reader → redact** with reveal-on-tap (the blackout is the feature).
  - **God → plaintext.**
  - Ownership flips the rule: your *own* character's knows/misbelieves stays hidden
    (spoiling your own blind spot kills roleplay); NPC misbeliefs are the dramatic
    irony reader/god mode is *for*.
- Texture = layer-of-reality: paper = inside the fiction, terminal = behind the curtain.
  Flipping to god/dev mode literally turns the page into the machine room.

## Rejected directions (and why)
- **Dark literary** (the handoff zip mock's skin): pretty, but user doesn't want dark; book/paper serves immersion + redaction better.
- **Cyberpunk neon** (pink/cyan): genre-locked (great for cyberpunk settings, fights fantasy/drama), glow tires eyes on long reads.
- **Brutalist primaries / hazard stripes**: marketing-loud, hostile to immersive reading.
- **Minimal dark SaaS**: safe but characterless; says nothing about "fiction" or "Soul." Possible fallback only.

## Open / deferred
- Exact type families (reading serif, label sans/mono), paper tint, the one accent.
- Whether a sci-fi/cyberpunk *per-setting* skin variant is worth it later (genre-themed paper).
