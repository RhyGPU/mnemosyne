# SillyTavern Settings Research

Reviewed: 2026-07-24

Source reviewed locally and read-only:

`C:\Windows\System32\SillyTavern-Launcher\SillyTavern`

Version reviewed: SillyTavern 1.18.0, commit `8172dcd0e`

## Research Boundary

This review studies feature coverage, information architecture, and mature roleplay workflows. Mnemosyne must not copy SillyTavern source, markup, CSS, wording, or component structure.

## What SillyTavern Separates Well

### Connection

Connection profiles own provider, endpoint, credentials, model, connection state, and automatic reconnection. This is distinct from generation behavior.

### Generation Presets

Generation controls are treated as reusable presets rather than connection details. The visible controls include response length, context size, temperature, nucleus and tail sampling, repetition handling, and sampler order.

### Prompt Formatting

Context templates, instruction templates, system prompts, stop sequences, tokenizer selection, and reasoning formatting live in a dedicated expert area. They are not mixed into basic connection setup.

### User Preferences

Appearance, message rendering, streaming presentation, auto-scroll, editing behavior, confirmation behavior, and character-list preferences are grouped as user experience controls.

### Extensions

Optional capabilities such as summarization, vector storage, translation, TTS, image generation, and quick replies are isolated from core settings.

## Patterns Worth Learning

- Keep provider connectivity separate from generation behavior.
- Make useful defaults work without exposing every sampler.
- Put expert controls behind clear progressive disclosure.
- Save changes immediately or make save/reset state unmistakable.
- Show provider capability and connection status close to the relevant control.
- Keep optional systems out of the core settings hierarchy.
- Allow presets to be restored without re-entering connection credentials.
- Use help text for controls whose effect is not obvious from the label.

## Patterns Mnemosyne Should Not Adopt

- Do not reproduce the extremely dense multi-column control wall.
- Do not expose every provider-specific sampler before Mnemosyne supports it reliably.
- Do not mix global controls with unrelated side panels.
- Do not make extensions or raw prompt templates part of the default roleplay workflow.
- Do not let compact typography and weak hierarchy undermine long-session readability.

## Mnemosyne Translation

### Full Settings

- **AI:** connection profiles, narrator/updater/repair provider assignment, model and timeout configuration.
- **Generation:** narrative mode, context mode, generation preset, creativity, response length, and supported OpenAI-compatible sampling controls.
- **Reading:** prose width, prose size, line height, and interface density.
- **Data:** session defaults, archive visibility, and local storage location.
- **About:** disclaimer, version, and product information.

### Compact Chat Drawer

Only frequent controls belong here:

- Narrative mode.
- Context mode.
- Generation preset.
- Creativity.
- Response-length limit.

### Dev Mode

- Developer override.
- Contract tests and compatibility diagnostics.
- Structured evaluator transport and fallback inspection.
- Benchmarks and dry runs.
- Raw payloads, prompts, and responses.
- Repair diagnostics and destructive state tools.

## First Implementation Slice

Status: completed.

1. Added real Generation and Reading categories.
2. Made the chat drawer a compact quick-settings surface.
3. Removed developer override, contract-test actions, raw compatibility badges, and structured evaluator internals from normal Settings.
4. Kept those controls available in Dev Mode.
5. Added only generation parameters supported by Mnemosyne's OpenAI-compatible request layer.
