# RP Chat UX Feature Checklist

Created 2026-06-26. References checked against current web sources during the UI pass.

## References

- Sendbird chat UI guidance: readable message bubbles, clear composer, prominent send action, chat switching, expressive/media attachments, and presence/status cues.
  https://sendbird.com/blog/resources-for-modern-chat-app-ui
- Muzli 2026 chat UI inspiration: modern chat UIs succeed through restraint, hierarchy, timestamp/receipt clarity, and interaction feedback that does not interrupt conversation flow.
  https://muz.li/inspiration/chat-ui/
- TheFrontKit AI chat UI best practices: streaming state, stop button, typing/working indicators, feedback, keyboard shortcuts, branching, pinning, and searchable history.
  https://thefrontkit.com/blogs/ai-chat-ui-best-practices
- SillyTavern World Info docs: lorebooks/world info dynamically inject relevant world, character, or instruction context into prompts.
  https://docs.sillytavern.app/usage/core-concepts/worldinfo/
- Moescape lorebook docs: lorebooks reinforce personality, background, world-building, and important events for consistency.
  https://docs.moescape.ai/tavern-chatbot-guide/lorebook
- SillyTavern user discussion: RP users strongly value lorebooks, pacing/GM instructions, trigger conditions, and cooldowns/probability for varied story behavior.
  https://www.reddit.com/r/SillyTavernAI/comments/1qb1d0k/deleted_by_user/

## Must-Have RP Chat Features

1. Session-first chat surface
   - Large readable transcript, stable composer, sticky header, jump-to-latest, and no editor/debug clutter while playing.
   - Mnemosyne status: partly implemented. Chat mode, magnetic scroll, stop button, image attachments, variants, rewind/hide, and restore turns exist.

2. Player persona control
   - The user should see who they are playing as and switch/add/edit persona without leaving chat.
   - Mnemosyne status: implemented in backend/UI modal, now surfaced in chat context strip and composer, with custom persona archive/restore.

3. World, scene, and plot visibility
   - RP users need quick awareness of location, current plot, and active scene premise.
   - Mnemosyne status: world state exists; now surfaced in chat context strip. Future: editable scene card in chat.

4. Memory and lorebook/world-info controls
   - Good RP chat needs durable memory, lore entries, triggerable world info, and user-visible curation.
   - Mnemosyne status: persistent memory, schemas, recent memories, consolidation, state evaluation, and recent-memory pin/unpin/restore UI exist. Future: first-class lorebook editor.

5. Character consistency controls
   - Character profile, voice, opening message, appearance, scenario, and relationship/psyche state should be inspectable and editable.
   - Mnemosyne status: editor exposes profile, opening message, psyche sliders, relationship state, avatar, import/export.

6. Branching, regeneration, and repair
   - RP users expect alternate responses, retry/fix, branch rewind, and recovery from bad turns.
   - Mnemosyne status: variants, regenerate latest user response, fix instruction, hide/rewind, restore hidden turns, evaluator repair jobs exist.

7. Composer affordances for RP actions
   - Common actions like OOC, setup/stage direction, persona switch, and state commands should not require memorizing slash commands.
   - Mnemosyne status: slash commands exist; now surfaced as composer shortcut pills.

8. Import/export and character-card compatibility
   - RP users bring characters from elsewhere and need portable session/checkpoint exports.
   - Mnemosyne status: JSON and `.mne` import/export, scenario bundles, world export, session checkpoint export, image assets exist. Future: broader card-format import polish.

9. Multi-character and GM mode
   - Advanced RP benefits from multiple active characters, narrator/GM pacing, off-script mode, and group relationships.
   - Mnemosyne status: UI multi-select is staged; backend still primary-character for chat. Future: true multi-active sessions and group state.

10. Trustworthy AI process feedback
    - AI chat should show when it is narrating, remembering, checking, repairing, failed, or safe to continue.
    - Mnemosyne status: evaluator job banner and Dev Mode trace exist; now a calm pipeline card appears in normal chat.

11. Searchable, recoverable history
   - Users need active/archived sessions, restoration, deletion, and eventually search/filter by character/world/tag.
   - Mnemosyne status: active/archived sessions, archived character/world/persona recovery, hidden-turn restore, folder open, pagination exist. Future: transcript search.

12. Polished interaction quality
    - Responsive layout, no overlapping controls, readable density, accessible focus states, and action-specific icons/tooltips.
    - Mnemosyne status: ongoing. This pass improves chat context density, composer affordances, and library character acquisition.
