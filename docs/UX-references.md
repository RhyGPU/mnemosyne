# UX Reference Notes — Mnemosyne UI Overhaul

Synthesized from research into great chat apps (ChatGPT, Claude, Character.AI,
Discord, Slack, Linear) and modern web-UX principles, with how each was applied.

## Sources
- Chat UI Design (2026) — UXPin: https://www.uxpin.com/studio/blog/chat-user-interface-design/
- 16 Chat UI Design Patterns (2026) — Bricx Labs: https://bricxlabs.com/blogs/message-screen-ui-deisgn
- Designing AI chat interfaces — Setproduct: https://www.setproduct.com/blog/ai-chat-interface-ui-design
- AI Chat UI Best Practices (2026) — TheFrontKit: https://thefrontkit.com/blogs/ai-chat-ui-best-practices
- Chat app design best practices — CometChat: https://www.cometchat.com/blog/chat-app-design-best-practices
- UX Design Principles (2026) — UXPin: https://www.uxpin.com/studio/blog/ux-design-principles/
- Visual Hierarchy — IxDF: https://ixdf.org/literature/topics/visual-hierarchy
- 5 Principles of Visual Design — NN/g: https://www.nngroup.com/articles/principles-visual-design/
- Best AI roleplay chatbots / Character.AI memory + discovery: https://myanima.ai/blog/best-ai-role-play-chatbots

## Principles → how applied in Mnemosyne
| Principle (source) | Applied |
|---|---|
| Persistent session sidebar; switch without losing context (UXPin, Character.AI) | In-chat session sidebar (commit bb04ce4) |
| Streaming is baseline; announce tokens politely (Setproduct, a11y) | Magnetic scroll + `aria-live=polite`/`aria-atomic=false` (d688860); typing indicator (2266eb2) |
| Don't float the composer over messages; pad the stream (Setproduct) | Composer offset past sidebar; body has bottom padding (2d4db1f) |
| Empty states must describe capabilities, not "Ask anything…" (UXPin, Bricx) | Guided chat empty state + Home create CTAs (2266eb2, 2d4db1f) |
| Avoid generic placeholders (Bricx) | Character-specific composer placeholder + "/" hint |
| Visual hierarchy via size/contrast/whitespace; 1–2 typefaces, clear scale (IxDF, NN/g) | Dedicated Editor view, decluttered Home, two-pane Settings |
| Choose models in-app; don't force external config (control surfaces) | In-app model picker datalist (b7934fc) |
| Memory features visible (Character.AI pinned/auto memories) | Memory curation surfaced (from Codex baseline) |
| High contrast for long sessions; distinct modes | Warm human surface + phosphor terminal Dev Mode (5326f05) |

## Still to do (needs running-app verification)
- Migrate command-runner/payload tools into Dev Mode tabs, then retire the old Dev Console panel.
- Move memory/debug panels off Home into Dev Mode.
- Visual-hierarchy polish pass on Home cards (spacing/typography scale) with eyes on the running app.
- Consider flat full-width narrator messages (Claude/ChatGPT pattern) vs current bubbles — A/B with the user.
