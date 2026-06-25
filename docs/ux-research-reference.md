# UX Reference — Great Apps & Patterns to Inform the Mnemosyne UI Overhaul

Status: research compiled 2026-06-25. Companion to
[`UI-overhaul-plan.md`](./UI-overhaul-plan.md) and [`UI fixes to do.txt`](./UI%20fixes%20to%20do.txt).

## Purpose & how to use this

This is a **reference library of proven UX patterns**, drawn from apps known for
excellent experience, organized so each finding maps onto a concrete decision in
the Mnemosyne overhaul (persistent left nav, selection-first PLAY page,
first-run provider onboarding, magnetic chat scroll, calm pipeline indicator,
matrix-themed Dev Mode). Every section ends with a **→ Mnemosyne** note tying the
pattern back to a specific phase or open problem from the overhaul plan.

It is *not* a redesign spec — it's the evidence base the spec can cite.

---

## At-a-glance: the examples and why they're here

| App | Studied for | Most relevant to |
| --- | --- | --- |
| **Linear** | Keyboard-first nav, command palette, low-color density, action consistency | Nav shell, Dev Mode command line, visual restraint |
| **Discord** | Persistent multi-level left rail, server/channel IA, dark high-contrast theme | PLAY/LIBRARY nav rail, character/world picker |
| **Slack** | Message composer as a command line, threads, deep accessibility, ⌘K switcher | Chat composer, `/chat` command model, a11y baseline |
| **Figma** | "Drop you inside a real project" onboarding, contextual toolbars | First-run path, contextual editor actions |
| **Notion** | Checklist onboarding, warm empty states, progressive disclosure | First-run, empty states for no-character/no-session |
| **Raycast / Warp** | Terminal-native dark aesthetic, block-based command UI, keyboard-driven | Dev Mode matrix aesthetic, command stream |
| **SillyTavern / JanitorAI** | Character-card model, BYO-character grids, community card sources | PLAY character grid, "Get more characters" link |
| **AG-UI / agent UIs** | Streaming run/step lifecycle events, real-time agent status | Calm pipeline indicator, Dev Mode pipeline rail |
| **`use-stick-to-bottom`** | Reference implementation of magnetic chat scroll | Chat scroll fix (the #1 listed issue) |

---

## 1. Navigation & Information Architecture

**Linear — keyboard-first, one consistent action model.**
Every action is reachable multiple ways — button, keyboard shortcut, contextual
menu, or the ⌘K command palette — but always through the *same* pattern, so users
build muscle memory and can discover "how do I do X" by reflex. Power users can
drive the entire app without a mouse. The 2025 redesign also **cut color
aggressively** (monochrome black/white, very few bold accents) to let content and
hierarchy lead.
- [How we redesigned the Linear UI](https://linear.app/now/how-we-redesigned-the-linear-ui)
- [Command Palette pattern — UX Patterns for Developers](https://uxpatterns.dev/patterns/advanced/command-palette)
- [Linear design trend — LogRocket](https://blog.logrocket.com/ux-design/linear-design/)

**Discord — a persistent, multi-level left rail.**
Far-left rail = servers (top-level destinations); the adjacent sidebar = channels
within the active server; a top header carries current-context + global actions
(inbox, help). Navigation *never moves*, so users always know where they are. The
modern redesign leans into higher contrast, darker surfaces, and rounded corners.
- [Discord UI structure (ResearchGate figure)](https://www.researchgate.net/figure/The-Discord-user-interface-The-far-left-sidebar-lists-all-the-Discord-servers-the-user_fig1_337131371)
- [New Discord desktop design](https://whop.com/blog/discord-new-design/)

**Slack — sidebar + main + composer + universal search.**
Four stable regions (nav sidebar, conversation view, composer, ⌘K search) let
users move between broad topics and specific threads without losing place.
- [Slack UI/UX deep dive](https://createbytes.com/insights/is-ui-ux-for-slack-application-on-point-review)

> **→ Mnemosyne.** Validates the **persistent left nav rail** (Phase 1) with
> PLAY / LIBRARY / SETTINGS / DEV / CHAT as fixed destinations — directly fixing
> "no sense of place" and the mega-scroll. Adopt Linear's *one action, many
> entry points* rule so the same command works as a button in normal mode and a
> typed command in Dev Mode. Borrow Linear's **color restraint** to de-clutter
> the current 9-card surface. A ⌘K-style switcher is a natural later addition
> once the rail exists.

---

## 2. Onboarding & First-Run

**Figma — drop the user inside a real project.** Onboarding starts in an actual
working file rather than a slideshow, so the first action is real work.

**Notion — a simple checklist, one feature per step.** Each step teaches one
in-app skill and uses soft animations + inviting empty states to turn a blank
canvas into a starting point (progressive disclosure: just enough to make the
next choice).
- [Figma & Notion onboarding compared](https://help.figma.com/hc/en-us/articles/360046037373-Notion-and-Figma)
- [16 SaaS onboarding screen examples](https://www.appcues.com/blog/saas-onboarding-screens)

> **→ Mnemosyne.** Confirms the plan to **show AI Provider Settings on first
> launch** (Phase 4) — nothing works until a provider is configured, so that
> *is* the real first project (Figma model). Frame it as a 2–3 step checklist
> (Notion model): ① connect a provider → ② import or create a character → ③
> start a session. This also resolves the to-do's "AI api settings should show
> up at first launch."

---

## 3. Chat / Messaging

**Magnetic-bottom scroll — the single most-cited Mnemosyne issue.** The agreed
correct behavior, confirmed across the community and reference implementations:
- Use an **invisible bottom anchor + IntersectionObserver** to know if the user
  is pinned to the bottom.
- **Only auto-follow while pinned.** If the user scrolls up, *disengage* — never
  yank them back. Let them cancel stickiness at any time.
- Provide a **"jump to latest" pill** that re-engages stickiness on click.
- Handle **scroll anchoring on resize** so streaming/expanding messages above the
  viewport don't make the visible content jump. (Don't rely on CSS
  `overflow-anchor` alone — Safari lacks support.)
- `use-stick-to-bottom` (the StackBlitz reference React hook) adds the nuances
  worth copying: distinguishes user scroll from animated scroll *without*
  debouncing; handles content **shrinking** without losing stickiness; uses
  `ResizeObserver`; exposes a `scrollToBottom(): Promise<boolean>` and a context
  hook so a child "jump to latest" button respects cancellation; velocity-based
  spring animation suits variable-size streaming output.
- [use-stick-to-bottom (reference implementation)](https://github.com/stackblitz-labs/use-stick-to-bottom)
- [Intuitive scrolling for chatbot streaming](https://tuffstuff9.hashnode.dev/intuitive-scrolling-for-chatbot-message-streaming)
- [Pin scrolling to bottom — CSS-Tricks](https://css-tricks.com/books/greatest-css-tricks/pin-scrolling-to-bottom/)

**Slack — composer as a command line.** The message box isn't just text: it
carries formatting, attachments, emoji, code snippets, and **slash commands**
that trigger actions — turning a chat input into a command surface.
- [Slack app design guidelines](https://api.slack.com/start/designing/guidelines)
- [Chat UI design 2026 — UXPin](https://www.uxpin.com/studio/blog/chat-user-interface-design/)

> **→ Mnemosyne.** Adopt the magnetic-scroll checklist verbatim for **Phase 5**
> (CHAT) — `use-stick-to-bottom` is essentially a drop-in for the React/Tauri
> frontend and resolves the top to-do item. Slack's slash-command composer is
> the precedent for Dev Mode's **`/chat` injects an in-character turn while bare
> input is a terminal** model (§4 of the overhaul) — a known, learnable pattern,
> not a novelty.

---

## 4. Agent / Pipeline Feedback (micro-interactions)

**AG-UI & modern agent UIs — make multi-step work legible.** Emerging agent UIs
model a run as a **lifecycle of events**: `RunStarted` → optional
`StepStarted`/`StepFinished` pairs → `RunFinished` *or* `RunError`, plus
`ToolCallStart` for tool invocations. Frontends turn these into transparent,
real-time progress: an activity indicator on start, per-step state, and explicit
success/error terminal states. AgentsRoom shows each agent as **color-coded
states** (thinking / done / needs-input / idle) with timestamps so it's obvious
what's happening and who needs attention.
- [AG-UI event lifecycle](https://docs.ag-ui.com/concepts/events)
- [Agent status monitoring pattern](https://www.aiuxdesign.guide/patterns/agent-status-monitoring)
- [Master the 17 AG-UI event types](https://www.copilotkit.ai/blog/master-the-17-ag-ui-event-types-for-building-agents-the-right-way)

> **→ Mnemosyne.** This is the design vocabulary for both the **calm pipeline
> indicator** in normal Chat (§3.4: thinking → narrating → remembering → checking
> → done) and the **Dev Mode pipeline rail** (§4: per-cycle bars, stage states
> ✓/◐/▲/paused). Mnemosyne's turn pipeline (send → narrator → state-updater →
> evaluator → repair → done) maps cleanly onto Run/Step/ToolCall events — model
> it as that lifecycle and the green-check / yellow-loading / red-triangle states
> from the to-do fall out naturally. The "black box" problem is solved by
> *surfacing the events that already exist*.

---

## 5. Dev Mode / Power-User & Terminal Aesthetic

**Raycast & Warp — terminal-native, keyboard-driven, IDE-grade.** Warp "combines
the raw power of a terminal with the UX of a modern IDE" via a **block-based
command UI** and a fully custom, themeable dark surface; Raycast pairs a
developer-friendly dark chrome with a command-first, low-friction, keyboard-driven
flow. The throughline: **functionality over decoration**, rigid structure, dark
high-contrast.
- [How we designed themes for the terminal — Warp](https://www.warp.dev/blog/how-we-designed-themes-for-the-terminal-a-peek-into-our-process)
- [Warp on the Raycast Store](https://www.raycast.com/warpdotdev/warp)

**Linear (again) — the command palette as the power surface.** Combines CLI
efficiency with GUI discoverability; ideal for IDE-style action palettes and
in-app command menus with autocomplete.

> **→ Mnemosyne.** Direct support for the **matrix-aesthetic Dev Mode** (§4): a
> distinct full-screen *mode* (not a side log), monospace, green-on-black, rigid
> rectangular frames — that's the Warp/Raycast lineage made explicit. The
> **combined terminal + chatlog stream** is Warp's block model; the **typed
> command line with autocomplete over whitelisted ops** is Linear's command
> palette applied to Mnemosyne's 9 dev commands. The whole "closer to a Claude
> CLI where I can watch you work *and* run commands" goal in the to-do is exactly
> this family of tools.

---

## 6. Character / World Selection (domain-specific)

**SillyTavern & JanitorAI — the BYO-character model.** SillyTavern is a
**local-first, ships-no-characters** frontend built around **character cards**
(name, persona, dialogue style, first message, scenario/lore), commonly the
self-contained `.png` format so card art + data travel as one file. **JanitorAI**
is a primary *community source* people browse to download cards. Selection UIs
are card grids; the value is configuration depth, not a curated roster.
- [What is SillyTavern?](https://docs.sillytavern.app/)
- [SillyTavern character design](https://docs.sillytavern.app/usage/core-concepts/characterdesign/)

> **→ Mnemosyne.** Strong external validation of the product philosophy (§0): a
> **BYO-character engine/playground**, not a content catalog. The **PLAY**
> character picker (Phase 2) should be a scannable **card grid** — portrait +
> name + short description as "a handle to grab" — matching the mental model RP
> users already hold from SillyTavern. The **"Get more characters" outbound web
> link** mirrors the SillyTavern→community-card-source flow exactly, and the
> `.mne` bundle is Mnemosyne's analogue to the self-contained `.png` card. Keep
> tiles recognizable, *not* storefront-glossy (per the §0 caution).

---

## 7. Forms & Settings

**Slack / SillyTavern — a dedicated, structured settings surface.** Mature apps
give configuration its **own page with clear sections** rather than scattering
controls; SillyTavern exposes deep prompt/provider control as organized panels;
Slack documents adjustable zoom, themes, and high-contrast options as
first-class settings.
- [Slack app design guidelines](https://api.slack.com/start/designing/guidelines)

**Progressive disclosure (Figma/Notion).** Show the right amount at each step;
hide advanced controls until needed so beginners aren't overwhelmed and power
users can still reach depth.

> **→ Mnemosyne.** Backs replacing the **settings drawer with a full SETTINGS
> page** and **de-duplicating the provider controls** that currently live in both
> the drawer and the launcher (Phase 4). Group as the plan specifies — Providers
> · Chat · Memory · Data · About — and use progressive disclosure so the dense
> psyche/evaluator/benchmark controls don't leak onto simpler surfaces.

---

## 8. Accessibility (baseline to hold to)

**Slack — a strong, documented a11y bar worth matching:**
- **Keyboard everything.** A shortcut for nearly every action (channel switch
  ⌘/Ctrl+K, edit-message `E`, etc.) — critical for motor-impairment and
  keyboard-only users, and it doubles as power-user speed.
- **Screen-reader support** via semantic HTML; announce notifications, read
  messages; lean on **consistent, established patterns** so SR users stay
  oriented (consistency *is* an accessibility feature).
- **Contrast.** WCAG 2.0 AA requires ≥ 4.5:1 for normal text; Slack targets
  **7:1 (AAA)**. Offer high-contrast themes and adjustable zoom.
- [What we've learned designing for accessibility — Slack](https://slack.design/articles/what-weve-learned-about-designing-for-accessibility-from-our-users/)
- [Ways we make the Slack iOS app accessible](https://slack.engineering/ways-we-make-the-slack-ios-app-accessible/)
- [7 UI design principles — Figma](https://www.figma.com/resource-library/ui-design-principles/)

> **→ Mnemosyne.** Cross-cutting requirements for the whole overhaul: every nav
> destination and chat/turn action reachable by keyboard (also feeds the Dev Mode
> command line); verify the matrix green-on-black and normal themes both clear
> **4.5:1+** contrast (green-on-black can fail easily — check it); keep nav and
> action patterns consistent across views so the muscle memory Linear/Slack rely
> on actually forms.

---

## 9. Visual Hierarchy & Empty States

**Hierarchy fundamentals (Figma).** Use font size/weight, contrasting color, and
spacing to make the most important element obvious at a glance and to show how
elements relate — the antidote to a flat wall of equally-weighted cards.

**Empty states (Linear/Notion).** Treat blank screens as **next-step prompts**,
not dead ends: a clear message + a primary action, with simple monochrome
illustration that *blends in* and adds warmth without stealing focus.
- [7 UI design principles — Figma](https://www.figma.com/resource-library/ui-design-principles/)
- [Empty state UX rules that work — Eleken](https://www.eleken.co/blog-posts/empty-state-ux)
- [Empty state UI design — Setproduct](https://www.setproduct.com/blog/empty-state-ui-design)

> **→ Mnemosyne.** Phase 7 polish: establish real hierarchy so PLAY reads as
> "select your cast" first (large portraits/names) with editing/config demoted or
> moved off-surface — fixing the "9 equally-weighted cards" diagnosis. Design
> **empty states** for the no-character, no-world, no-session, and first-run
> cases as guided next steps (e.g. "No characters yet → Import a card / Create
> one / Get more characters") rather than blank panels.

---

## Coverage check — what's solid vs. what needs deeper research

**Well-covered (ready to cite in the spec):** navigation/IA, first-run
onboarding, magnetic chat scroll (with a near-drop-in implementation), pipeline/
agent-status feedback, Dev Mode aesthetic & command line, character-grid model,
settings consolidation, accessibility baseline, hierarchy & empty states.

**Worth deeper research before the relevant phase lands:**
1. **Multi-character / group-chat UX** (Phase 7) — how to show *several* active
   characters in one session without clutter; SillyTavern group chats and
   Discord-style presence are the leads, not yet studied in depth.
2. **State Map V1 panels** (Scene/Characters/Relationships/Objects/Timeline/
   Tensions/Memory) — node-graph and entity-inspector UX (think Obsidian graph,
   debugger variable inspectors, observability dashboards) is a distinct research
   track from this chat/nav set.
3. **Provenance / memory-inspector UI** — "trace any memory to the turn that
   created it" wants source-attribution and diff/timeline patterns worth their
   own pass.
4. **Concrete visual tokens** — exact spacing scale, type ramp, and contrast-
   checked palettes for both the normal and matrix themes (this doc covers
   principles, not final values).
5. **Mobile/responsive** — all examples here are desktop-first (matching Tauri),
   so deprioritized, but note if a companion view is ever planned.

---

## Source index

Navigation & IA: [Linear redesign](https://linear.app/now/how-we-redesigned-the-linear-ui) ·
[Command palette pattern](https://uxpatterns.dev/patterns/advanced/command-palette) ·
[Linear design — LogRocket](https://blog.logrocket.com/ux-design/linear-design/) ·
[Discord UI figure](https://www.researchgate.net/figure/The-Discord-user-interface-The-far-left-sidebar-lists-all-the-Discord-servers-the-user_fig1_337131371) ·
[New Discord design](https://whop.com/blog/discord-new-design/) ·
[Slack UI/UX deep dive](https://createbytes.com/insights/is-ui-ux-for-slack-application-on-point-review)
Onboarding: [Figma & Notion](https://help.figma.com/hc/en-us/articles/360046037373-Notion-and-Figma) ·
[SaaS onboarding screens](https://www.appcues.com/blog/saas-onboarding-screens)
Chat/scroll: [use-stick-to-bottom](https://github.com/stackblitz-labs/use-stick-to-bottom) ·
[Chatbot streaming scroll](https://tuffstuff9.hashnode.dev/intuitive-scrolling-for-chatbot-message-streaming) ·
[Pin scrolling — CSS-Tricks](https://css-tricks.com/books/greatest-css-tricks/pin-scrolling-to-bottom/) ·
[Slack design guidelines](https://api.slack.com/start/designing/guidelines) ·
[Chat UI design 2026 — UXPin](https://www.uxpin.com/studio/blog/chat-user-interface-design/)
Agent feedback: [AG-UI events](https://docs.ag-ui.com/concepts/events) ·
[Agent status monitoring](https://www.aiuxdesign.guide/patterns/agent-status-monitoring) ·
[AG-UI event types](https://www.copilotkit.ai/blog/master-the-17-ag-ui-event-types-for-building-agents-the-right-way)
Dev/terminal: [Warp themes](https://www.warp.dev/blog/how-we-designed-themes-for-the-terminal-a-peek-into-our-process) ·
[Warp on Raycast](https://www.raycast.com/warpdotdev/warp)
Character model: [SillyTavern docs](https://docs.sillytavern.app/) ·
[ST character design](https://docs.sillytavern.app/usage/core-concepts/characterdesign/)
Accessibility: [Slack a11y learnings](https://slack.design/articles/what-weve-learned-about-designing-for-accessibility-from-our-users/) ·
[Slack iOS a11y](https://slack.engineering/ways-we-make-the-slack-ios-app-accessible/)
Hierarchy/empty states: [Figma UI principles](https://www.figma.com/resource-library/ui-design-principles/) ·
[Empty state UX — Eleken](https://www.eleken.co/blog-posts/empty-state-ux) ·
[Empty state UI — Setproduct](https://www.setproduct.com/blog/empty-state-ui-design)
