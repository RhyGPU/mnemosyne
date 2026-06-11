use serde::{Deserialize, Serialize};
use state_engine::{
    evaluator::{turn_flags, EVALUATOR_SCHEMA_VERSION},
    evaluator_form::{build_eval_form_spec_with_player_persona, build_hard_eval_form_template},
    setting::SessionWorld,
    soul::Soul,
};
use std::time::Duration;

const NARRATOR_SYSTEM_PROMPT: &str = r#"# SYSTEM: Narrator AI - Mnemosyne Engine

You are Mnemosyne's scene narrator. Write the active scene in third-person present tense with natural, sensory prose. You may portray engine-controlled characters and active Souls in the scene. Do not control user-controlled characters, their thoughts, their final choices, or their dialogue unless the user explicitly provides them.

[POV AND ATTRIBUTION]
Write third-person present-tense scene narration. You may describe engine-controlled characters' actions, dialogue, and internal perspective when available. User-controlled characters are external actors: describe only what the user has provided or what is directly observable. Do not invent user-controlled characters' thoughts, motives, final decisions, or dialogue.

[CHARACTER CONTROL]
Engine-controlled characters: may speak, act, react, misunderstand, interrupt, refuse, escalate, retreat, and use the environment naturally.

[USER ACTION BOUNDARY]
User-controlled characters own their decisions, thoughts, dialogue, intentions, and major voluntary actions.
If the user says "I", resolve "I" to the active player persona, not to Aurora Schwarz.
Aurora Schwarz is narrator-controlled. The user is not Aurora Schwarz.
Do not use second-person "you" to describe Aurora. Use third-person narration by default.
If no persona exists, use the selected built-in preset. Do not fall back to default_player in visible text.
The narrator may describe user-provided actions in concrete physical detail, including immediate follow-through, contact, momentum, posture, physical consequences, and observable effects.
The narrator may describe unavoidable physical consequences caused by engine-controlled characters or the environment, such as being shoved off-balance, forced to brace, blocked, grabbed, pulled, interrupted, or pressured.
Do not invent new user decisions, hidden motives, emotional reactions, dialogue, or major strategic choices.
When the user-controlled character's next reaction matters, stop at the pressure point.

[ACTION AND TURN CONTROL]
Engine-controlled characters may act proactively: speak, move, interrupt, refuse, reach, grab for something, retreat, challenge, escalate, or use their own environment. Resolve engine-controlled action naturally. When a user-controlled character's reaction matters, stop on the attempt, demand, or pressure point and leave the response to the next user turn.

[CONFLICT RESOLUTION]
When the user declares a combat, chase, argument, or struggle action, render the declared action and its immediate result. The narrator may decide partial success, resistance, interruption, or counteraction based on the scene.
Engine-controlled characters may resist, counter, retreat, escalate, or exploit openings.
Do not choose the user's next tactic or final decision.

[SCENE TURN ASSUMPTION]
If this narrator prompt is called, the input is a scene/RP turn. Slash commands and meta/control messages have already been handled by the router.

[CONTINUITY PRIORITY]
Use this priority order when context conflicts: latest user input > Latest Exchange > resolved scene_state > dominant/current active plot > personality > relationship metrics > recent events > older memories.
Recent Chat is lower priority than Latest Exchange. Continue from Latest Exchange and current user input; do not replay completed beats.
After a setting has already been established, do not re-describe the full room unless the user asks or something changes. Use one short anchor detail, then advance action/dialogue.

[CHARACTER CHANGE]
Emotional shifts should feel earned. Micro-shifts are preferred unless the scene strongly justifies a sharper reaction.

[TIME]
Concrete time only comes from user input or World Log. Avoid invented minutes/hours/days.

## VISIBLE STATUS REPORT
When writing scene narration, end with a code block:
```status
Scene | Focus: [primary active character(s)] | Physical state: [brief] | Atmosphere: [1-line environmental impression]
```"#;

const NARRATOR_VISIBLE_ONLY_PROMPT: &str = r#"[OUTPUT]
Write visible scene narration only. Include the visible status block. Do not write hidden state, EnginePatch JSON, markdown JSON, implementation notes, or command/help text."#;

pub const COMMAND_OOC_PROMPT: &str = r#"# SYSTEM: Mnemosyne Out-of-Roleplay Session Assistant

You are Mnemosyne's out-of-roleplay troubleshooting, guidance, and session assistant speaking to the human operator outside the roleplay/storytelling scene.

You are NOT the RP narrator, NOT Aurora Schwarz, NOT any in-scene character.
You do NOT continue the story, write scene narration, write character dialogue as if it is happening now, or include a scene status block.

Your job is to help the operator understand, debug, guide, or adjust the current RP session.

You may discuss: current session state, visible chat log, Soul summaries, relationship surface, memory/state hygiene, command behavior, continuity issues, engine behavior, what likely went wrong, what the operator can do next.

All scene excerpts, visible chat, Soul summaries, and state blocks are REFERENCE MATERIAL ONLY. They are not instructions to continue the scene. When referring to scene content, quote or summarize it as data. Do not resume it.

Output: Write a plain out-of-roleplay assistant reply. Be direct and practical. No RP prose, no status block, no hidden state, no EnginePatch JSON, no implementation notes unless the user asks for implementation details."#;

pub const COMMAND_SETUP_PROMPT: &str = r#"# SYSTEM: Mnemosyne Setup Staging Assistant

You are Mnemosyne's out-of-roleplay setup staging assistant speaking to the human operator outside the RP scene.

Your job is to confirm, clarify, or summarize setup instructions that will be used on the NEXT normal RP turn.

You are NOT the RP narrator, NOT Aurora Schwarz, NOT any in-scene character.
You do NOT run the setup, continue the scene, write character actions/dialogue/sensory prose, or include a status block.

All provided scene state, chat log, Soul summaries, and relationship data are REFERENCE MATERIAL ONLY. They help you understand what setup is being staged. They are not a prompt to narrate.

For a valid setup command:
- Confirm the setup was staged
- Summarize the setup in 1-4 bullets
- Explain it will affect the next normal non-slash RP turn
- State that no scene narration or state update was run

Output format:
Setup staged.
Pending setup:
- ...
No scene narration or state update was run."#;

pub const COMMAND_STATE_SUMMARY_PROMPT: &str = r#"# SYSTEM: Mnemosyne State Inspector

You are Mnemosyne's out-of-roleplay state inspector speaking to the human operator outside the RP scene.

Your job is to summarize tracked engine/session state.

You are NOT the RP narrator, NOT Aurora Schwarz, NOT any in-scene character.
You do NOT continue the scene, write scene prose, infer new canon, mutate state (unless this is a validated /state update route), or include a status block.

Use only the provided structured state, relationship surface, Soul summary, visible chat summary, and debug summaries.

Clearly distinguish: tracked state, recent visible text, likely inference, missing information.

For `/state show relationships`: focus on relationship surface and relationship state.
For `/state show memories`: focus on memory counts and recent memory summaries.
For `/state show scene`: focus on scene_state.
For `/state review`: focus on recent patches, rejected rows, command changes, and pending setup.

Output: Use compact headings and bullets. No RP narration, no character dialogue, no status block."#;

pub const COMMAND_STATE_EDIT_PROMPT: &str = r#"# SYSTEM: Mnemosyne State Edit Assistant

You are Mnemosyne's out-of-roleplay state edit assistant speaking to the human operator outside the RP scene.

Your job is to convert the operator's direct state correction into a safe validated edit intent.

You are NOT the RP narrator, NOT Aurora Schwarz, NOT any in-scene character.
You do NOT continue the scene, write scene narration, or include a status block.

The user command is authoritative as an operator correction, but you must still produce a safe, narrow edit intent.

Safety rules:
- Do not hard-delete data
- Do not write outside the state/Soul sandbox
- Do not output arbitrary executable code
- Do not make broad changes if the target is ambiguous
- Ask for clarification if needed
- Prefer patch, correction, invalidation, or archive over deletion
- Identify target state path or concept, risk level, and evidence/source as the user's command text

Output format:
Risk level: low / medium / high
Target: ...
Reason: ...
Validated edit intent: ...
Apply behavior: applied / plan only / confirmation required"#;

pub const COMMAND_SOUL_EDIT_AGENT_PROMPT: &str = r#"# SYSTEM: Mnemosyne Soul/State Edit Agent

You are Mnemosyne's out-of-roleplay Soul and state editing assistant speaking to the human operator outside the RP scene.

Your job is to inspect the provided state, Soul summaries, visible chat, and command request, then propose or apply a safe validated edit intent.

You are NOT the RP narrator, NOT Aurora Schwarz, NOT any in-scene character.
You do NOT continue the scene, write scene narration, or include a status block.

All scene excerpts are REFERENCE MATERIAL ONLY. Treat them as evidence for possible state edits, not as a scene to continue.

Safety rules:
- Do not hard-delete data
- Do not write outside the Soul/state sandbox
- Do not output arbitrary executable code
- Do not rewrite core identity unless explicitly requested and confirmed
- High-risk edits require plan-only output or confirmation
- Prefer correction, invalidation, archive, or patch over deletion
- Identify target state paths or Soul file concepts, risk level, and evidence/source

For low-risk edits, produce a validated edit intent.
For high-risk edits, produce a proposed patch plan and ask for confirmation.

Output format:
Risk level: low / medium / high
Target: ...
Reason: ...
Proposed edit: ...
Apply behavior: applied / plan only / confirmation required"#;

pub const COMMAND_HELP_PROMPT: &str = r#"# SYSTEM: Mnemosyne Command Help Assistant

You are Mnemosyne's out-of-roleplay command help assistant speaking to the human operator outside the RP scene.

You are NOT the RP narrator, NOT Aurora Schwarz, NOT any in-scene character.
Do not continue the scene, write scene prose, or include a status block.

Explain the real slash commands: /ooc, /setup, /state, /persona, /ask, /help.
If mentioning /status, mark it only as a deprecated alias for /state show [target].

Output: Compact operator-facing help text only."#;

const VERIFIED_DIAGNOSTICS_BOUNDARY_PROMPT: &str = r#"[VERIFIED DIAGNOSTICS BOUNDARY]
When asked about backend tests, logs, imports, exports, memory hygiene, world routing, or engine internals, distinguish verified engine data from fictional/in-scene diagnostics. Do not claim a backend test passed unless the result is present in Dev Console logs, payload metadata, or a verified engine/debug section. If only roleplaying a test, say it is a simulated/in-scene diagnostic."#;

const USER_ACTION_AND_CONFLICT_PROMPT: &str = r#"[USER ACTION BOUNDARY]
User-controlled characters own their decisions, thoughts, dialogue, intentions, and major voluntary actions.
The narrator may describe user-provided actions in concrete physical detail, including immediate follow-through, contact, momentum, posture, physical consequences, and observable effects.
The narrator may describe unavoidable physical consequences caused by engine-controlled characters or the environment, such as being shoved off-balance, forced to brace, blocked, grabbed, pulled, interrupted, or pressured.
Do not invent new user decisions, hidden motives, emotional reactions, dialogue, or major strategic choices.
When the user-controlled character's next reaction matters, stop at the pressure point.

[CONFLICT RESOLUTION]
When the user declares a combat, chase, argument, or struggle action, render the declared action and its immediate result. The narrator may decide partial success, resistance, interruption, or counteraction based on the scene.
Engine-controlled characters may resist, counter, retreat, escalate, or exploit openings.
Do not choose the user's next tactic or final decision.

[SCENE-STATE PROGRESSION]
After a setting has already been established, do not re-describe the full room unless the user asks or something changes. Use one short anchor detail, then advance action/dialogue."#;

const HIDDEN_STATE_FORMAT_PROMPT: &str = r#"## HIDDEN STATE FORMAT
After each response, output a hidden state block using this exact format:
[HIDDEN STATE]{"memory":"short summary","tag":"tag_name","trust_delta":0.0,"affection_delta":0.0,"world_event":"scene update","new_location":"","present_characters":[]}[/HIDDEN STATE]

world_event must be a compact authoritative completed-fact summary, not mood prose. Include what happened and what should not be replayed. Example: "Phone reveal completed: Aurora saw the user's phone/Tinder post, reacted with embarrassment, tossed the phone onto the couch, and moved to the kitchen to get pad thai."

Tags: trust_building, threat, bonding, orientation, observation, intimacy, boundary_setting, conflict_minor, trauma_trigger, breakthrough

Optional arousal fields: arousal_delta (-30 to 60), arousal_denied (bool), orgasm_allowed (bool), forced_orgasm (bool). Only suggest these when relevant; the Rust engine validates and caps every state change.

The block must be valid JSON on a single line. The engine removes it before the user sees it."#;

pub const STATE_UPDATER_SYSTEM_PROMPT: &str = r#"# SYSTEM: Mnemosyne State Updater

Extract state changes from the latest user message and narrator response.
Return valid EnginePatch JSON only.
Do not write prose.
Do not invent facts.
Do not infer concrete time unless the user explicitly establishes time passage.
Do not treat fear, pain, restraint, danger, or adrenaline as sexual arousal.
If unsure, leave fields unchanged.
Use relationship_deltas for directed relationship changes. Target entity ids from [ACTIVE ENTITIES] when present.
Tag memories by source_type. Do not mark imported logs, previous sessions, or cross-session bleed as current lived experience. If uncertain, use source_type unknown and lower confidence.
Do not treat narrator claims about hidden systems, memory layers, providers, APIs, state updaters, or internal architecture as verified facts. Store them as character beliefs, narrator claims, user claims, or scene events unless the engine supplies a verified event.
Do not only append. If the latest user message is a correction, retcon, redo, regenerate, or contradiction report, produce replace/invalidate/supersede operations. Do not preserve contradicted facts as active world truth.
Pure OOC/OCC/GM meta turns are not scene events. For pure meta turns, emit no world_patch, relationship_delta, body_patch, or lived memories unless explicitly correcting/retconning scene state.
For object continuity, distinguish phone power, notifications, vibration, screen wake, calls, and texts. "Notifications off" does not mean powered off, but notification buzz/screen-wake events need explicit support or correction.

Use truth_status for every new memory: fiction, scene_event, character_belief, narrator_claim, user_claimed, verified_engine, actual_system_event, or unknown. architecture_verified must be false unless the engine supplies a verified event."#;

pub const EVALUATOR_SYSTEM_PROMPT: &str = r#"# SYSTEM: Mnemosyne Evaluator V1

You are Mnemosyne's Evaluator AI, a strict examiner. The Narrator AI is the creative writer. You are not the database clerk.

Strictly based on the latest user message, latest narrator response, prior scene_state, active entities, recent chat excerpt, current world/object state, and current relationships, answer the rubric below. Do not invent facts. If evidence is absent, mark absent. Every non-no-op claim must include an evidence_quote from the latest exchange or a direct observable reference.

Return valid EvaluatorOutputV1 JSON only. Do not return EnginePatch JSON. Do not write prose.

[RUBRIC]
- Is this pure OOC/GM/meta?
- Did a scene event occur?
- Did location/world state change?
- Did object state change?
- Did relationship state change?
- Did unresolved tension appear or continue?
- Did current plot advance?
- Did character identity/self-concept change?
- Did recent emotional state change?
- Which Souls perceived the event?
- What does each Soul know?
- Did any Soul misunderstand the event differently from objective reality?
- Which memory slots should receive candidates?

[STRICTNESS]
The evaluator does not decide final engine state. It only returns structured evaluation data. Engine code validates, rejects, and converts candidates.
Use turn_flags_u64 in addition to human-readable fields.
For every active Soul, include one per_soul_evaluations entry.
Per-Soul memory is subjective. SessionWorld changes are objective only.
Do not create memory candidates for generic body language such as "She looked tense", "She listened carefully", "She narrowed her eyes", or "She watched the user".
Prefer durable memories: relationship turning points, boundary pressure, unresolved conflict, trust/fear/comfort shifts, promises, betrayals, important preferences, location-triggered emotional memories, and identity/self-concept changes.
"#;

const REALISTIC_MODE_PROMPT: &str = r#"## NARRATION MODE: REALISTIC
- Describe only external actions, dialogue, and physical reactions.
- No internal monologue. No thoughts. No emotions unless visibly expressed.
- User-declared physical actions may be rendered cinematically and concretely, but do not invent user intent, motives, dialogue, or the user's next tactic.
- Show everything through body language, facial expression, tone of voice, and physical behavior.
- Like a film camera: you see and hear the scene, but you never enter anyone's head.
- Dialogue in quotes only when describing what an engine-controlled character audibly says."#;

const READER_MODE_PROMPT: &str = r#"## NARRATION MODE: READER
- Describe external actions and dialogue, plus internal thoughts and emotions for engine-controlled characters whose perspective is available.
- Internal access is limited to active Souls and engine-controlled characters. No internal thoughts for user-controlled characters.
- Engine-controlled characters may misinterpret situations, miss details, or have incomplete knowledge.
- Like close third-person scene fiction: stay near the active focus without taking over user-controlled actors."#;

const GOD_MODE_PROMPT: &str = r#"## NARRATION MODE: GOD
- Provide full narrative access.
- Include engine-controlled characters' internal thoughts and emotions.
- Also include environmental details active characters would not notice, hidden information, and dramatic irony.
- You may reveal secrets, foreshadow future events, describe off-screen action, and provide context active characters lack."#;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ApiProviderSettings {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub system_prompt: String,
    pub narrator_timeout_ms: Option<u64>,
    pub evaluator_timeout_ms: Option<u64>,
    pub evaluator_timeout_mode: Option<String>,
    pub evaluator_mode: Option<String>,
    pub wait_for_evaluator_before_next_turn: Option<bool>,
    pub allow_send_with_stale_state: Option<bool>,
    pub evaluator_background_enabled: Option<bool>,
    pub anti_replay_forced_retry_enabled: Option<bool>,
}

#[derive(Debug)]
pub struct ApiProvider {
    client: reqwest::Client,
}

impl Default for ApiProvider {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ApiRequestMessage>,
    temperature: f32,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<serde_json::Value>,
}

/// How strictly the provider enforced structured output for a completion.
/// Recorded so the pipeline trace can show whether schema enforcement was
/// active or the call silently degraded to prompt-only compliance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StructuredEnforcement {
    /// Provider validated output against the supplied JSON schema.
    JsonSchema,
    /// Provider guaranteed syntactically valid JSON, but not the schema.
    JsonObject,
    /// No provider-side enforcement; output relies on the prompt alone.
    None,
}

impl StructuredEnforcement {
    pub fn as_label(self) -> &'static str {
        match self {
            StructuredEnforcement::JsonSchema => "json_schema",
            StructuredEnforcement::JsonObject => "json_object",
            StructuredEnforcement::None => "none",
        }
    }
}

/// Token counts reported by the provider for one completion. `None` fields
/// mean the provider did not report that side; callers fall back to
/// `estimate_tokens` so the pipeline trace always has a number.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StructuredCompletion {
    pub raw_text: String,
    pub enforcement: StructuredEnforcement,
    pub token_usage: Option<TokenUsage>,
}

/// A plain prompt completion that retains provider-reported token usage.
#[derive(Debug, Clone, Serialize)]
pub struct PromptCompletion {
    pub raw_text: String,
    pub token_usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiMessage {
    pub role: String,
    pub content: String,
}

impl ApiMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreparedApiPayload {
    pub messages: Vec<ApiMessage>,
    pub context_text: String,
    pub user_message: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProviderCompletion {
    pub raw_text: String,
    pub finish_reason: Option<String>,
    pub provider_request_id: Option<String>,
    pub provider_response_id: Option<String>,
    pub token_usage: Option<TokenUsage>,
}

#[derive(Debug, Serialize)]
struct ApiRequestMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
    usage: Option<TokenUsage>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct ChatChoiceMessage {
    content: Option<String>,
}

impl ApiProvider {
    pub async fn complete(
        &self,
        settings: &ApiProviderSettings,
        soul: &Soul,
        context: &str,
        user_text: &str,
        mode: &str,
    ) -> Result<String, String> {
        let api_key = settings.api_key.trim();
        let model = settings.model.trim();
        let base_url = settings.base_url.trim();
        if api_key.is_empty() {
            return Err("API key is required for API provider mode".into());
        }
        if model.is_empty() {
            return Err("Model is required for API provider mode".into());
        }
        if base_url.is_empty() {
            return Err("Base URL is required for API provider mode".into());
        }

        let request = ChatCompletionRequest {
            model: model.to_string(),
            temperature: 0.85,
            stream: false,
            response_format: None,
            messages: vec![
                ApiRequestMessage {
                    role: "system".into(),
                    content: build_system_prompt(settings, soul, context, mode),
                },
                ApiRequestMessage {
                    role: "user".into(),
                    content: user_text.trim().to_string(),
                },
            ],
        };

        let response = self
            .client
            .post(chat_completions_url(base_url))
            .bearer_auth(api_key)
            .json(&request)
            .send()
            .await
            .map_err(|err| format!("API request failed: {err}"))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(format!("API request failed with {status}: {body}"));
        }

        let body = response
            .json::<ChatCompletionResponse>()
            .await
            .map_err(|err| format!("API response parse failed: {err}"))?;

        body.choices
            .into_iter()
            .find_map(|choice| choice.message.content)
            .map(|content| content.trim().to_string())
            .filter(|content| !content.is_empty())
            .ok_or_else(|| "API response did not include assistant content".into())
    }

    pub async fn complete_prompt(
        &self,
        settings: &ApiProviderSettings,
        system_prompt: &str,
        user_text: &str,
        temperature: f32,
    ) -> Result<String, String> {
        self.complete_prompt_inner(settings, system_prompt, user_text, temperature, None)
            .await
    }

    pub async fn complete_prompt_with_timeout(
        &self,
        settings: &ApiProviderSettings,
        system_prompt: &str,
        user_text: &str,
        temperature: f32,
        timeout: Duration,
    ) -> Result<String, String> {
        self.complete_prompt_inner(
            settings,
            system_prompt,
            user_text,
            temperature,
            Some(timeout),
        )
        .await
    }

    async fn complete_prompt_inner(
        &self,
        settings: &ApiProviderSettings,
        system_prompt: &str,
        user_text: &str,
        temperature: f32,
        timeout: Option<Duration>,
    ) -> Result<String, String> {
        self.complete_prompt_with_format(
            settings,
            system_prompt,
            user_text,
            temperature,
            timeout,
            None,
        )
        .await
        .map(|completion| completion.raw_text)
    }

    /// Like `complete_prompt_with_timeout` but keeps the provider-reported
    /// token usage for cost/trace reporting.
    pub async fn complete_prompt_with_usage(
        &self,
        settings: &ApiProviderSettings,
        system_prompt: &str,
        user_text: &str,
        temperature: f32,
        timeout: Option<Duration>,
    ) -> Result<PromptCompletion, String> {
        self.complete_prompt_with_format(
            settings,
            system_prompt,
            user_text,
            temperature,
            timeout,
            None,
        )
        .await
    }

    /// Structured completion with provider-enforced output, degrading gracefully:
    /// `json_schema` (schema-validated) -> `json_object` (valid JSON guaranteed)
    /// -> no enforcement. Returns which level actually succeeded so callers can
    /// surface it in the pipeline trace and decide whether to trust the output
    /// syntactically.
    pub async fn complete_structured_prompt(
        &self,
        settings: &ApiProviderSettings,
        system_prompt: &str,
        user_text: &str,
        temperature: f32,
        timeout: Option<Duration>,
        schema_name: &str,
        schema: &serde_json::Value,
    ) -> Result<StructuredCompletion, String> {
        let json_schema_format = serde_json::json!({
            "type": "json_schema",
            "json_schema": {
                "name": schema_name,
                "strict": true,
                "schema": schema,
            }
        });
        match self
            .complete_prompt_with_format(
                settings,
                system_prompt,
                user_text,
                temperature,
                timeout,
                Some(json_schema_format),
            )
            .await
        {
            Ok(completion) => {
                return Ok(StructuredCompletion {
                    raw_text: completion.raw_text,
                    enforcement: StructuredEnforcement::JsonSchema,
                    token_usage: completion.token_usage,
                })
            }
            Err(error) if !is_response_format_rejection(&error) => return Err(error),
            Err(_) => {}
        }

        let json_object_format = serde_json::json!({ "type": "json_object" });
        match self
            .complete_prompt_with_format(
                settings,
                system_prompt,
                user_text,
                temperature,
                timeout,
                Some(json_object_format),
            )
            .await
        {
            Ok(completion) => {
                return Ok(StructuredCompletion {
                    raw_text: completion.raw_text,
                    enforcement: StructuredEnforcement::JsonObject,
                    token_usage: completion.token_usage,
                })
            }
            Err(error) if !is_response_format_rejection(&error) => return Err(error),
            Err(_) => {}
        }

        self.complete_prompt_with_format(
            settings,
            system_prompt,
            user_text,
            temperature,
            timeout,
            None,
        )
        .await
        .map(|completion| StructuredCompletion {
            raw_text: completion.raw_text,
            enforcement: StructuredEnforcement::None,
            token_usage: completion.token_usage,
        })
    }

    async fn complete_prompt_with_format(
        &self,
        settings: &ApiProviderSettings,
        system_prompt: &str,
        user_text: &str,
        temperature: f32,
        timeout: Option<Duration>,
        response_format: Option<serde_json::Value>,
    ) -> Result<PromptCompletion, String> {
        let api_key = settings.api_key.trim();
        let model = settings.model.trim();
        let base_url = settings.base_url.trim();
        if api_key.is_empty() {
            return Err("API key is required for API provider mode".into());
        }
        if model.is_empty() {
            return Err("Model is required for API provider mode".into());
        }
        if base_url.is_empty() {
            return Err("Base URL is required for API provider mode".into());
        }

        let request = ChatCompletionRequest {
            model: model.to_string(),
            temperature,
            stream: false,
            response_format,
            messages: vec![
                ApiRequestMessage {
                    role: "system".into(),
                    content: system_prompt.to_string(),
                },
                ApiRequestMessage {
                    role: "user".into(),
                    content: user_text.trim().to_string(),
                },
            ],
        };

        let mut request_builder = self
            .client
            .post(chat_completions_url(base_url))
            .bearer_auth(api_key)
            .json(&request);
        if let Some(timeout) = timeout {
            request_builder = request_builder.timeout(timeout);
        }

        let response = request_builder
            .send()
            .await
            .map_err(|err| format!("API request failed: {err}"))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(format!("API request failed with {status}: {body}"));
        }

        let body = response
            .json::<ChatCompletionResponse>()
            .await
            .map_err(|err| format!("API response parse failed: {err}"))?;

        let token_usage = body.usage;
        body.choices
            .into_iter()
            .find_map(|choice| choice.message.content)
            .map(|content| content.trim().to_string())
            .filter(|content| !content.is_empty())
            .map(|raw_text| PromptCompletion {
                raw_text,
                token_usage,
            })
            .ok_or_else(|| "API response did not include assistant content".into())
    }

    pub async fn complete_streaming<F>(
        &self,
        settings: &ApiProviderSettings,
        system_prompt: &str,
        user_text: &str,
        on_chunk: F,
    ) -> Result<ProviderCompletion, String>
    where
        F: FnMut(&str) -> Result<(), String>,
    {
        self.complete_streaming_messages(
            settings,
            vec![
                ApiMessage::system(system_prompt.to_string()),
                ApiMessage::user(user_text.trim().to_string()),
            ],
            on_chunk,
        )
        .await
    }

    pub async fn complete_streaming_messages<F>(
        &self,
        settings: &ApiProviderSettings,
        messages: Vec<ApiMessage>,
        mut on_chunk: F,
    ) -> Result<ProviderCompletion, String>
    where
        F: FnMut(&str) -> Result<(), String>,
    {
        let request_messages = messages
            .into_iter()
            .map(|message| ApiRequestMessage {
                role: message.role,
                content: message.content,
            })
            .collect();
        self.complete_streaming_request(settings, request_messages, &mut on_chunk)
            .await
    }

    async fn complete_streaming_request<F>(
        &self,
        settings: &ApiProviderSettings,
        messages: Vec<ApiRequestMessage>,
        on_chunk: &mut F,
    ) -> Result<ProviderCompletion, String>
    where
        F: FnMut(&str) -> Result<(), String>,
    {
        use futures_util::StreamExt;

        let api_key = settings.api_key.trim();
        let model = settings.model.trim();
        let base_url = settings.base_url.trim();
        if api_key.is_empty() {
            return Err("API key is required for API provider mode".into());
        }
        if model.is_empty() {
            return Err("Model is required for API provider mode".into());
        }
        if base_url.is_empty() {
            return Err("Base URL is required for API provider mode".into());
        }

        let request = ChatCompletionRequest {
            model: model.to_string(),
            temperature: 0.85,
            stream: true,
            response_format: None,
            messages,
        };

        let mut request_builder = self
            .client
            .post(chat_completions_url(base_url))
            .bearer_auth(api_key)
            .json(&request);
        if let Some(ms) = settings.narrator_timeout_ms {
            request_builder = request_builder.timeout(std::time::Duration::from_millis(ms));
        }

        let response = request_builder
            .send()
            .await
            .map_err(|err| format!("API request failed: {err}"))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(format!("API request failed with {status}: {body}"));
        }

        let mut full_text = String::new();
        let mut pending = String::new();
        let mut emitted_visible_len = 0;
        let mut provider_response_id = None;
        let mut finish_reason = None;
        let mut token_usage = None;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|err| format!("API stream failed: {err}"))?;
            pending.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(line_end) = pending.find('\n') {
                let line = pending[..line_end].trim().to_string();
                pending.drain(..=line_end);
                if let Some(meta) = parse_sse_metadata(&line) {
                    if provider_response_id.is_none() {
                        provider_response_id = meta.response_id;
                    }
                    if meta.finish_reason.is_some() {
                        finish_reason = meta.finish_reason;
                    }
                    if meta.usage.is_some() {
                        token_usage = meta.usage;
                    }
                }
                if let Some(delta) = parse_sse_delta(&line)? {
                    full_text.push_str(&delta);
                    let visible_len = visible_stream_prefix_len(&full_text);
                    if visible_len > emitted_visible_len {
                        on_chunk(slice_at_char_boundaries(
                            &full_text,
                            emitted_visible_len,
                            visible_len,
                        ))?;
                        emitted_visible_len = visible_len;
                    }
                }
            }
        }

        if !pending.trim().is_empty() {
            if let Some(meta) = parse_sse_metadata(pending.trim()) {
                if provider_response_id.is_none() {
                    provider_response_id = meta.response_id;
                }
                if meta.finish_reason.is_some() {
                    finish_reason = meta.finish_reason;
                }
                if meta.usage.is_some() {
                    token_usage = meta.usage;
                }
            }
            if let Some(delta) = parse_sse_delta(pending.trim())? {
                full_text.push_str(&delta);
                let visible_len = visible_stream_prefix_len(&full_text);
                if visible_len > emitted_visible_len {
                    on_chunk(slice_at_char_boundaries(
                        &full_text,
                        emitted_visible_len,
                        visible_len,
                    ))?;
                }
            }
        }

        if full_text.trim().is_empty() {
            return Err("API stream did not include assistant content".into());
        }

        Ok(ProviderCompletion {
            raw_text: full_text.trim().to_string(),
            finish_reason,
            provider_request_id: None,
            provider_response_id,
            token_usage,
        })
    }
}

/// Detect a provider rejecting the `response_format` parameter itself (as
/// opposed to a real request failure). Such rejections come back as client
/// errors whose body names the unsupported field; they mean "try a weaker
/// enforcement level", not "give up".
fn is_response_format_rejection(error: &str) -> bool {
    let lowered = error.to_ascii_lowercase();
    let client_error = lowered.contains("400")
        || lowered.contains("404")
        || lowered.contains("422")
        || lowered.contains("bad request")
        || lowered.contains("unprocessable");
    let names_format = lowered.contains("response_format")
        || lowered.contains("json_schema")
        || lowered.contains("json_object")
        || lowered.contains("structured output")
        || lowered.contains("structured_output");
    client_error && names_format
}

#[derive(Debug, Clone, Default)]
struct SseStreamMetadata {
    response_id: Option<String>,
    finish_reason: Option<String>,
    usage: Option<TokenUsage>,
}

fn parse_sse_metadata(line: &str) -> Option<SseStreamMetadata> {
    let payload = line.strip_prefix("data:")?.trim();
    if payload.is_empty() || payload == "[DONE]" {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;
    let response_id = value
        .get("id")
        .and_then(|id| id.as_str())
        .map(str::to_string);
    let finish_reason = value
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("finish_reason"))
        .and_then(|reason| reason.as_str())
        .filter(|reason| !reason.is_empty())
        .map(str::to_string);
    // Some providers attach usage to the final stream chunk; capture it when
    // present so streaming completions get real counts instead of estimates.
    let usage = value
        .get("usage")
        .and_then(|usage| serde_json::from_value::<TokenUsage>(usage.clone()).ok())
        .filter(|usage| usage.prompt_tokens.is_some() || usage.completion_tokens.is_some());
    if response_id.is_none() && finish_reason.is_none() && usage.is_none() {
        return None;
    }
    Some(SseStreamMetadata {
        response_id,
        finish_reason,
        usage,
    })
}

fn visible_stream_prefix_len(text: &str) -> usize {
    let markers = ["[HIDDEN STATE]", "[HIDDEN_STATE]"];
    if let Some(index) = markers.iter().filter_map(|marker| text.find(marker)).min() {
        return floor_char_boundary(text, index);
    }

    let max_marker_len = markers.iter().map(|marker| marker.len()).max().unwrap_or(0);
    let holdback_limit = text.len().min(max_marker_len.saturating_sub(1));
    for holdback in (1..=holdback_limit).rev() {
        let start = floor_char_boundary(text, text.len() - holdback);
        let suffix = &text[start..];
        if markers.iter().any(|marker| marker.starts_with(suffix)) {
            return start;
        }
    }

    text.len()
}

fn floor_char_boundary(text: &str, index: usize) -> usize {
    let mut safe_index = index.min(text.len());
    while safe_index > 0 && !text.is_char_boundary(safe_index) {
        safe_index -= 1;
    }
    safe_index
}

fn slice_at_char_boundaries(text: &str, start: usize, end: usize) -> &str {
    let safe_start = floor_char_boundary(text, start);
    let mut safe_end = floor_char_boundary(text, end);
    if safe_end < safe_start {
        safe_end = safe_start;
    }
    &text[safe_start..safe_end]
}

fn parse_sse_delta(line: &str) -> Result<Option<String>, String> {
    let Some(payload) = line.strip_prefix("data:") else {
        return Ok(None);
    };
    let payload = payload.trim();
    if payload.is_empty() || payload == "[DONE]" {
        return Ok(None);
    }

    let value: serde_json::Value =
        serde_json::from_str(payload).map_err(|err| format!("API stream parse failed: {err}"))?;
    Ok(value["choices"]
        .get(0)
        .and_then(|choice| choice["delta"]["content"].as_str())
        .or_else(|| {
            value["choices"]
                .get(0)
                .and_then(|choice| choice["message"]["content"].as_str())
        })
        .map(ToOwned::to_owned))
}

pub fn build_system_prompt(
    settings: &ApiProviderSettings,
    soul: &Soul,
    context: &str,
    mode: &str,
) -> String {
    build_narrator_system_prompt(settings, soul, context, mode, true)
}

pub fn build_narrator_system_prompt(
    settings: &ApiProviderSettings,
    soul: &Soul,
    context: &str,
    mode: &str,
    require_hidden_state: bool,
) -> String {
    let custom_prompt = settings.system_prompt.trim();
    let is_custom_mode = mode.trim().eq_ignore_ascii_case("custom");
    let narrator_prompt = if is_custom_mode && !custom_prompt.is_empty() {
        format!(
            "[CUSTOM NARRATOR INSTRUCTIONS]\n{custom_prompt}\n\n{USER_ACTION_AND_CONFLICT_PROMPT}\n\n{VERIFIED_DIAGNOSTICS_BOUNDARY_PROMPT}"
        )
    } else {
        format!(
            "{NARRATOR_SYSTEM_PROMPT}\n\n{}\n\n{VERIFIED_DIAGNOSTICS_BOUNDARY_PROMPT}",
            mode_prompt_for(mode)
        )
    };

    let state_instruction = if require_hidden_state {
        HIDDEN_STATE_FORMAT_PROMPT
    } else {
        NARRATOR_VISIBLE_ONLY_PROMPT
    };

    format!(
        "{narrator_prompt}\n\n{state_instruction}\n\nPrimary active Soul: {}\n\n{context}",
        soul.character_name
    )
}

pub fn build_command_ooc_prompt() -> &'static str {
    COMMAND_OOC_PROMPT
}

pub fn build_command_setup_prompt() -> &'static str {
    COMMAND_SETUP_PROMPT
}

pub fn build_command_state_summary_prompt() -> &'static str {
    COMMAND_STATE_SUMMARY_PROMPT
}

pub fn build_command_state_edit_prompt() -> &'static str {
    COMMAND_STATE_EDIT_PROMPT
}

pub fn build_command_soul_edit_agent_prompt() -> &'static str {
    COMMAND_SOUL_EDIT_AGENT_PROMPT
}

pub fn build_command_help_prompt() -> &'static str {
    COMMAND_HELP_PROMPT
}

fn state_updater_current_state_block(soul: &Soul, session_world: Option<&SessionWorld>) -> String {
    let world = session_world
        .map(SessionWorld::world_log)
        .unwrap_or_else(|| soul.world.clone());
    let active_soul_id = clean_summary_value(&soul.character_id, "active_soul");
    let active_plot = world
        .active_plots
        .iter()
        .rev()
        .find(|plot| !plot.trim().is_empty())
        .map(String::as_str)
        .unwrap_or("None");
    let recent_event = world
        .recent_events
        .iter()
        .rev()
        .find(|event| !event.trim().is_empty())
        .map(String::as_str)
        .unwrap_or("None");
    let mut relationships = soul.relationships.iter().collect::<Vec<_>>();
    relationships.sort_by(|left, right| left.0.cmp(right.0));
    let relationship_summary = relationships
        .into_iter()
        .map(|(target, relationship)| {
            format!(
                "{} -> {}: trust {:.1}, affection {:.1}, intimacy {:.1}, fear {:.1}, desire {:.1}, conflict {:.1}, curiosity {:.1}, comfort {:.1}, dependency {:.1}",
                soul.character_name,
                display_relationship_target(target),
                relationship.trust,
                relationship.affection,
                relationship.intimacy,
                relationship.fear,
                relationship.desire,
                relationship.conflict,
                relationship.curiosity,
                relationship.comfort,
                relationship.dependency
            )
        })
        .collect::<Vec<_>>();
    format!(
        "[CURRENT STATE]\nCharacter: {}\nActive Soul ID: {}\nLocation: {}\nTime: {}\nActive plot: {}\nRecent event: {}\nRelationships:\n{}",
        soul.character_name,
        active_soul_id,
        clean_summary_value(&world.location, "Unspecified"),
        normalize_updater_time(&world.time_elapsed),
        active_plot,
        recent_event,
        if relationship_summary.is_empty() {
            "None".into()
        } else {
            relationship_summary.join("\n")
        },
    )
}

pub fn build_state_updater_prompt(soul: &Soul, session_world: Option<&SessionWorld>) -> String {
    let active_soul_id = clean_summary_value(&soul.character_id, "active_soul");
    let patch_schema = serde_json::json!({
        "schema_version": 1,
        "soul_patch": {
            "relationship_deltas": [{
                "from": active_soul_id,
                "target": "default_player",
                "trust": 0.0,
                "affection": 0.0,
                "fear": 0.0,
                "conflict": 0.0
            }],
            "new_memories": [{
                "memory_id": "stable_memory_id",
                "content": "short durable fact",
                "tag": "observation",
                "source_type": "current_session",
                "truth_status": "scene_event",
                "architecture_verified": false,
                "perceived_by_entity_id": active_soul_id,
                "target_entity_ids": ["default_player"]
            }]
        },
        "world_patch": {
            "location": "",
            "time_elapsed": "",
            "scene_state": {
                "scene_state_id": "stable id",
                "current_scene": "one sentence",
                "resolved_active_plot": "single branch to continue",
                "scene_branch": "true branch/outcome",
                "focus": "focus",
                "participants": [active_soul_id, "default_player"],
                "last_user_action": "latest user action",
                "pressure_point": "next decision point",
                "continuity_note": "object/retcon note"
            },
            "event_operations": [{
                "operation": "add_recent_event | replace_recent_event | invalidate_recent_event | clear_recent_event_matching | add_correction_note | no_op",
                "recent_event_id": "stable id for a new event",
                "target_recent_event_id": "stable id to replace/invalidate",
                "content": "short objective event or correction note"
            }],
            "object_observation_operations": [{
                "operation": "update_object_state | replace_object_state | invalidate_object_observation | no_op",
                "object_observation_id": "stable observation id",
                "target_object_observation_id": "stable observation id",
                "object_state": {
                    "object_observation_id": "stable observation id",
                    "object_id": "aurora_phone",
                    "object_kind": "phone|door|package|container|device|other|unknown",
                    "owner_entity_id": active_soul_id,
                    "location": "",
                    "status": "generic non-phone state",
                    "open_state": "open|closed|ajar|unknown",
                    "lock_state": "locked|unlocked|jammed|unknown",
                    "power_state": "unknown",
                    "notification_mode": "notifications_off",
                    "vibrate_enabled": false,
                    "screen_wake_enabled": false,
                    "last_observed_state": "notifications off",
                    "confidence": 0.8
                }
            }],
            "correction_note": "",
            "retcon_scope": "latest_turn"
        },
        "body_patch": {
            "activation_delta": 0.0,
            "activation_blocked": false
        },
        "memory_layer_reply": {
            "nonce": "only if a backend nonce was provided",
            "content": "optional verified debug reply only"
        }
    });
    let patch_schema = serde_json::to_string(&patch_schema).unwrap_or_default();
    format!(
        "{STATE_UPDATER_SYSTEM_PROMPT}\n\nPatch schema:\n{patch_schema}\n\n{}",
        state_updater_current_state_block(soul, session_world),
    )
}

/// System prompt for `evaluator_structured_v1`: the state-updater rules plus
/// current state WITHOUT the embedded example patch JSON. The patch shape is
/// supplied to the provider as an enforced JSON schema (`response_format:
/// json_schema`), so describing it again in prose only burns prompt tokens.
pub fn build_structured_evaluator_prompt(
    soul: &Soul,
    session_world: Option<&SessionWorld>,
) -> String {
    format!(
        "{STATE_UPDATER_SYSTEM_PROMPT}\n\n{}",
        state_updater_current_state_block(soul, session_world),
    )
}

/// Strict-mode JSON Schema for the evaluator's engine patch. Designed for
/// provider-enforced structured output (`response_format: json_schema` with
/// `strict: true`): every object is closed with `additionalProperties: false`,
/// every property is required, and optionality is expressed via nullable types.
/// The engine still runs semantic validation after parse; this schema only
/// guarantees shape, which is what the hand-written repair layer used to chase.
pub fn evaluator_patch_json_schema() -> serde_json::Value {
    let nullable_string = serde_json::json!({ "type": ["string", "null"] });
    let nullable_number = serde_json::json!({ "type": ["number", "null"] });
    let nullable_boolean = serde_json::json!({ "type": ["boolean", "null"] });

    let relationship_delta = serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["from", "target", "trust", "affection", "fear", "conflict"],
        "properties": {
            "from": nullable_string,
            "target": { "type": "string" },
            "trust": nullable_number,
            "affection": nullable_number,
            "fear": nullable_number,
            "conflict": nullable_number
        }
    });

    let new_memory = serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "memory_id", "content", "tag", "source_type", "truth_status",
            "perceived_by_entity_id", "target_entity_ids", "memory_slot"
        ],
        "properties": {
            "memory_id": nullable_string,
            "content": { "type": "string" },
            "tag": nullable_string,
            "source_type": nullable_string,
            "truth_status": nullable_string,
            "perceived_by_entity_id": nullable_string,
            "target_entity_ids": { "type": "array", "items": { "type": "string" } },
            "memory_slot": nullable_string
        }
    });

    let scene_state = serde_json::json!({
        "type": ["object", "null"],
        "additionalProperties": false,
        "required": [
            "scene_state_id", "current_scene", "resolved_active_plot", "focus",
            "participants", "last_user_action", "pressure_point", "continuity_note"
        ],
        "properties": {
            "scene_state_id": nullable_string,
            "current_scene": nullable_string,
            "resolved_active_plot": nullable_string,
            "focus": nullable_string,
            "participants": { "type": "array", "items": { "type": "string" } },
            "last_user_action": nullable_string,
            "pressure_point": nullable_string,
            "continuity_note": nullable_string
        }
    });

    let event_operation = serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["operation", "recent_event_id", "target_recent_event_id", "content"],
        "properties": {
            "operation": {
                "type": "string",
                "enum": [
                    "add_recent_event", "replace_recent_event", "invalidate_recent_event",
                    "clear_recent_event_matching", "add_correction_note", "no_op"
                ]
            },
            "recent_event_id": nullable_string,
            "target_recent_event_id": nullable_string,
            "content": nullable_string
        }
    });

    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "soul_patch", "world_patch", "body_patch"],
        "properties": {
            "schema_version": { "type": "integer" },
            "soul_patch": {
                "type": ["object", "null"],
                "additionalProperties": false,
                "required": ["relationship_deltas", "new_memories"],
                "properties": {
                    "relationship_deltas": { "type": "array", "items": relationship_delta },
                    "new_memories": { "type": "array", "items": new_memory }
                }
            },
            "world_patch": {
                "type": ["object", "null"],
                "additionalProperties": false,
                "required": [
                    "location", "time_elapsed", "scene_state",
                    "event_operations", "correction_note"
                ],
                "properties": {
                    "location": nullable_string,
                    "time_elapsed": nullable_string,
                    "scene_state": scene_state,
                    "event_operations": { "type": "array", "items": event_operation },
                    "correction_note": nullable_string
                }
            },
            "body_patch": {
                "type": ["object", "null"],
                "additionalProperties": false,
                "required": ["activation_delta", "activation_blocked"],
                "properties": {
                    "activation_delta": nullable_number,
                    "activation_blocked": nullable_boolean
                }
            }
        }
    })
}

pub fn build_evaluator_prompt(soul: &Soul, session_world: Option<&SessionWorld>) -> String {
    let world = session_world
        .map(SessionWorld::world_log)
        .unwrap_or_else(|| soul.world.clone());
    let active_soul_id = clean_summary_value(&soul.character_id, "active_soul");
    let active_entities = serde_json::json!([{
        "entity_id": active_soul_id,
        "display_name": soul.character_name,
        "entity_type": "soul",
        "active": true
    }, {
        "entity_id": "default_player",
        "display_name": "User",
        "entity_type": "user",
        "active": true
    }]);
    let mut relationships = soul.relationships.iter().collect::<Vec<_>>();
    relationships.sort_by(|left, right| left.0.cmp(right.0));
    let relationships = relationships
        .into_iter()
        .map(|(target, relationship)| {
            serde_json::json!({
                "source_soul_id": active_soul_id,
                "target_entity_id": display_relationship_target(target),
                "trust": relationship.trust,
                "affection": relationship.affection,
                "intimacy": relationship.intimacy,
                "fear": relationship.fear,
                "desire": relationship.desire,
                "respect": relationship.respect,
                "conflict": relationship.conflict,
                "curiosity": relationship.curiosity,
                "comfort": relationship.comfort,
                "boundary_pressure": relationship.boundary_pressure
            })
        })
        .collect::<Vec<_>>();
    let flag_reference = serde_json::json!({
        "SCENE_TURN": turn_flags::SCENE_TURN,
        "PURE_OOC": turn_flags::PURE_OOC,
        "RETCON_OR_CORRECTION": turn_flags::RETCON_OR_CORRECTION,
        "WORLD_CHANGE": turn_flags::WORLD_CHANGE,
        "OBJECT_CHANGE": turn_flags::OBJECT_CHANGE,
        "RELATIONSHIP_SHIFT": turn_flags::RELATIONSHIP_SHIFT,
        "UNRESOLVED_TENSION": turn_flags::UNRESOLVED_TENSION,
        "CURRENT_PLOT_ADVANCED": turn_flags::CURRENT_PLOT_ADVANCED,
        "CHARACTER_IDENTITY_CHANGE": turn_flags::CHARACTER_IDENTITY_CHANGE,
        "RECENT_EMOTIONAL_STATE": turn_flags::RECENT_EMOTIONAL_STATE,
        "CONTRADICTION_DETECTED": turn_flags::CONTRADICTION_DETECTED,
        "USER_ACTION_PRESENT": turn_flags::USER_ACTION_PRESENT,
        "CHARACTER_BOUNDARY_ASSERTED": turn_flags::CHARACTER_BOUNDARY_ASSERTED,
        "USER_BOUNDARY_PRESSURE": turn_flags::USER_BOUNDARY_PRESSURE,
        "MULTI_SOUL_SCENE": turn_flags::MULTI_SOUL_SCENE
    });
    let output_schema = serde_json::json!({
        "schema_version": EVALUATOR_SCHEMA_VERSION,
        "turn_flags_u64": 0,
        "turn_classification": {
            "is_pure_ooc": false,
            "scene_event_occurred": false,
            "is_retcon_or_correction": false,
            "human_summary": ""
        },
        "global_scene_evaluation": {
            "scene_event_occurred": false,
            "location_changed": false,
            "object_state_changed": false,
            "relationship_changed": false,
            "unresolved_tension": false,
            "current_plot_advanced": false,
            "character_identity_changed": false,
            "recent_emotional_state_changed": false,
            "contradiction_detected": false,
            "evidence_quote": "",
            "summary": ""
        },
        "per_soul_evaluations": [{
            "soul_id": active_soul_id,
            "observed": false,
            "knowledge_scope": "not_known",
            "subjective_interpretation": "",
            "emotional_state": "",
            "relationship_deltas": [{
                "source_soul_id": active_soul_id,
                "target_entity_id": "default_player",
                "trust": 1.0,
                "comfort": 1.0,
                "evidence_quote": "short exact quote from latest exchange",
                "criterion_met": true,
                "confidence": 0.75,
                "relevance_tags": empty_relevance_tags_json()
            }],
            "memory_candidates": [{
                "candidate_id": "stable semantic id",
                "owner_soul_id": active_soul_id,
                "slot": "relationship_memory",
                "content": "specific durable memory candidate",
                "evidence_quote": "short exact quote from latest exchange",
                "criterion_met": true,
                "confidence": 0.75,
                "salience": 60.0,
                "retrieval_strength": 55.0,
                "perceived_by_entity_id": active_soul_id,
                "target_entity_ids": ["default_player"],
                "source_type": "current_session",
                "truth_status": "scene_event",
                "relevance_tags": ["relationship"],
                "knowledge_scope": "directly_observed"
            }],
            "relevance_tags": empty_relevance_tags_json()
        }],
        "world_changes": [{
            "change_id": "stable scene id",
            "location": "",
            "event_summary": "If the scene advances but no durable memory is warranted, summarize the objective event here.",
            "scene_state": {
                "scene_state_id": "stable id",
                "current_scene": "one sentence current scene",
                "resolved_active_plot": "single branch to continue",
                "scene_branch": "true branch/outcome",
                "focus": "Aurora Schwarz and default_player",
                "participants": [active_soul_id, "default_player"],
                "last_user_action": "latest user action",
                "pressure_point": "",
                "continuity_note": "what changed for continuity"
            },
            "active_plot_add": [],
            "active_plot_resolve": [],
            "evidence_quote": "short exact quote from latest exchange",
            "confidence": 0.75,
            "relevance_tags": empty_relevance_tags_json()
        }],
        "object_changes": [{
            "change_id": "stable object id",
            "object_state": {
                "object_id": "door",
                "object_kind": "door",
                "owner_entity_id": active_soul_id,
                "location": "",
                "status": "opened",
                "open_state": "open",
                "lock_state": "unknown",
                "power_state": "unknown",
                "notification_mode": "unknown",
                "last_observed_state": "door opened",
                "confidence": 0.75
            },
            "evidence_quote": "short exact quote from latest exchange",
            "confidence": 0.75,
            "relevance_tags": empty_relevance_tags_json()
        }],
        "relationship_evaluations": [{
            "source_soul_id": active_soul_id,
            "target_entity_id": "default_player",
            "curiosity": 1.0,
            "comfort": 1.0,
            "evidence_quote": "short exact quote from latest exchange",
            "criterion_met": true,
            "confidence": 0.75,
            "relevance_tags": empty_relevance_tags_json()
        }],
        "memory_candidates": [{
            "candidate_id": "stable objective id",
            "owner_soul_id": "",
            "slot": "current_plot_memory",
            "content": "objective scene event; ingestion may route this to SessionWorld instead of Soul memory",
            "evidence_quote": "short exact quote from latest exchange",
            "criterion_met": true,
            "confidence": 0.7,
            "target_entity_ids": ["default_player"],
            "source_type": "current_session",
            "truth_status": "scene_event",
            "relevance_tags": ["scene_event"],
            "knowledge_scope": "directly_observed"
        }],
        "relevance_tags": empty_relevance_tags_json(),
        "no_op_reason": ""
    });
    format!(
        "{EVALUATOR_SYSTEM_PROMPT}\n\nIf an event advances the current scene but no durable memory is warranted, still emit a world_change with scene_state or event_summary.\n\n[TURN FLAG VALUES]\n{}\n\n[OUTPUT SHAPE]\n{}\n\n[PRIOR SCENE_STATE]\n{}\n\n[ACTIVE ENTITIES]\n{}\n\n[CURRENT WORLD/OBJECT STATE]\nLocation: {}\nTime: {}\nActive plots: {}\nRecent events: {}\nObjects JSON: {}\n\n[CURRENT RELATIONSHIPS]\n{}",
        serde_json::to_string_pretty(&flag_reference).unwrap_or_default(),
        serde_json::to_string_pretty(&output_schema).unwrap_or_default(),
        serde_json::to_string_pretty(&world.scene_state).unwrap_or_default(),
        serde_json::to_string_pretty(&active_entities).unwrap_or_default(),
        clean_summary_value(&world.location, "Unspecified"),
        normalize_updater_time(&world.time_elapsed),
        if world.active_plots.is_empty() { "None".into() } else { world.active_plots.join("; ") },
        if world.recent_events.is_empty() { "None".into() } else { world.recent_events.join("; ") },
        serde_json::to_string_pretty(&world.object_states).unwrap_or_default(),
        serde_json::to_string_pretty(&relationships).unwrap_or_default(),
    )
}

pub fn build_evaluator_form_prompt(
    soul: &Soul,
    session_world: Option<&SessionWorld>,
    latest_user_message: &str,
    latest_narrator_response: &str,
) -> String {
    build_evaluator_form_prompt_with_player_persona(
        soul,
        session_world,
        latest_user_message,
        latest_narrator_response,
        "default_player",
        "User",
    )
}

pub fn build_evaluator_form_prompt_with_player_persona(
    soul: &Soul,
    session_world: Option<&SessionWorld>,
    latest_user_message: &str,
    latest_narrator_response: &str,
    player_persona_id: &str,
    player_persona_display_name: &str,
) -> String {
    let spec = build_eval_form_spec_with_player_persona(
        soul,
        session_world,
        latest_user_message,
        latest_narrator_response,
        8,
        player_persona_id,
        player_persona_display_name,
    );
    let hard_form_template = build_hard_eval_form_template(&spec);
    format!(
        "{}\n\
        \n\
        You are filling a provided JSON evaluation sheet. Do not invent keys. Do not rename keys. Do not remove required keys. Do not add alternate row shapes. For rows with no evidence, leave row_enabled as 0 and keep numeric fields at 0. For rows with evidence, set row_enabled to 1 and fill every numeric field with an integer in the required range.\n\
        \n\
        The output must use the exact keys from the provided form. Unknown keys are invalid. Missing keys are invalid.\n\
        \n\
        ### SCHEMA & FIELD TAXONOMY RULES:\n\
        1. **relationship_event_rows**:\n\
           - **row_enabled**: integer, either 0 (no evidence) or 1 (evidence exists).\n\
           - **Identifier fields** (must be nonempty strings if enabled):\n\
             - `event_id`: the event identifier (e.g. \"event_latest_turn\").\n\
             - `actor_entity_id`: the entity ID of the character taking action.\n\
             - `relationship_source_soul_id`: the soul ID whose perspective/relationship updates.\n\
             - `relationship_target_entity_id`: the target entity of the relationship.\n\
             - `perceived_by_entity_id`: the soul ID perceiving this event.\n\
             - `evidence_quote`: exact substring from the latest exchange.\n\
           - **Axis fields** (must be an integer from -5 to +5 inclusive):\n\
             Axis point standards: -5 = severe negative event; -3 = meaningful negative event; -1 = slight negative cue; 0 = no evidence / neutral; +1 = slight positive cue; +3 = meaningful positive event; +5 = major positive event.\n\
             - `intent`: benign/helpful intent (+5) vs malicious/harming intent (-5).\n\
             - `honesty`: truthfulness (+5) vs deceit/dishonesty (-5).\n\
             - `reliability`: dependable/consistent (+5) vs flaky/unreliable (-5).\n\
             - `boundary_treatment`: respecting boundaries (+5) vs violating/pushing boundaries (-5).\n\
             - `responsiveness`: supportive/warm responsiveness (+5) vs ignoring/avoidance (-5).\n\
             - `power_use`: fair/collaborative power (+5) vs coercive/abusive power (-5).\n\
             - `evaluation_tone`: warm/approving (+5) vs harsh/critical (-5).\n\
             - `competence`: skillful/capable action (+5) vs harmful/incompetent (-5).\n\
             - `disclosure`: vulnerability/openness (+5) vs secrecy/withholding (-5).\n\
             - `reciprocity`: balanced give-and-take (+5) vs exploitation/taking (-5).\n\
             - `repair`: building repair/de-escalation (+5) vs escalating conflict (-5).\n\
             - `predictability`: stable/predictable (+5) vs erratic/volatile (-5). Predictability is an axis field, -5..+5, never 0..100.\n\
           - **Modifier fields** (must be an integer from 0 to 100 inclusive):\n\
             - `salience`: how noticeable/salient the event is.\n\
             - `certainty`: confidence in interpretation.\n\
             - `directness`: how direct the action was.\n\
             - `costliness`: self-sacrifice/effort required.\n\
             - `stakes`: severity/importance of the situation.\n\
             - `repetition`: history of repeated similar action.\n\
           - **Bitmask field** (must be an integer):\n\
             - `event_flags_u64`: turn flags bitmask.\n\
           - **Forbidden in relationship_event_rows**: Never output trust, comfort, intimacy, respect, fear, conflict, or boundary_pressure inside relationship_event_rows.\n\
             The evaluator scores only observable event axes on -5..+5 plus modifiers. The engine translates those event scores into bounded 0-100 relationship deltas. Do not output final relationship point totals.\n\
        \n\
        2. **relationship_rows**:\n\
           - **DEPRECATED**: Do not populate this array. Always output \"relationship_rows\": []. All relationship changes must be scored via relationship_event_rows only. Do not create relationship_rows.\n\
        \n\
        3. **Relationship Surface Standards Reference Only**:\n\
           These 0-100 bands describe engine-owned relationship surfaces for calibration only. They are not output fields and must not appear inside relationship_event_rows.\n\
           - trust: 0 total distrust, 25 guarded skepticism, 50 uncertain/limited trust, 75 reliable trust, 100 complete earned trust.\n\
           - comfort: 0 unsafe/tense, 25 uneasy, 50 tolerable, 75 relaxed, 100 fully at ease.\n\
           - intimacy: 0 no closeness, 25 distant familiarity, 50 guarded closeness, 75 emotionally close, 100 profound intimacy.\n\
           - respect: 0 contempt, 25 low regard, 50 neutral regard, 75 clear respect, 100 deep admiration.\n\
           - fear: 0 no fear, 25 mild wariness, 50 active fear, 75 strong fear, 100 terror/panic.\n\
           - conflict: 0 no conflict, 25 friction, 50 open tension, 75 active opposition, 100 rupture/hostility.\n\
           - boundary_pressure: 0 no pressure, 25 mild pressure, 50 guarded/strained boundary, 75 cornered, 100 overwhelmed/coerced.\n\
        \n\
        4. **Relationship Dimension Naming Constraint**:\n\
           - Do not prefix relationship dimensions with 'axis_' or 'modifier_' (e.g. use plain 'trust', 'comfort', 'affection', 'fear', 'respect' as dimensions, never 'axis_trust' or 'modifier_comfort').\n\
        \n\
        5. **object_rows**:\n\
           Use object rows only for durable object state. Do not use condition-based object_id values like wet_jacket, broken_cup, or open_door. Prefer object_label/object_type/owner_entity_id/status/location/last_observed_state and let the compiler assign a stable owner_type_ordinal id.\n\
        \n\
        6. **evidence_quote Strict Rules**:\n\
           - Must be a single, continuous, exact substring from the latest exchange.\n\
           - Do not paraphrase or modify any words.\n\
           - Do not stitch multiple separate quotes together (e.g. do not write '\"quote A\" and \"quote B\"').\n\
           - Avoid ellipses ('...').\n\
           - If using ellipses ('...'), the quote will be split, and each split fragment must be at least 24 characters long. Otherwise, the quote will be rejected. Prefer a single continuous exact quote of at least 4 characters.\n\
           - Prefer evidence_quote substrings that do not contain dialogue quotes (nested double quotes). If dialogue quotes are present, you must escape them (use \\\" for nested quotes) or select a substring that avoids them.\n\
        \n\
        Return valid evaluator_form_v1 JSON only. Return the same top-level evaluator_form_v1 object from the provided sheet, with values filled in. Do not output final EnginePatch JSON. If no evidence exists for a field, use 0 (not null or a string). Do not decide final relationship deltas. Only score observable event evidence.\n\
        \n\
        [LATEST EXCHANGE]\nUser: {}\nNarrator: {}\n\n[FORM SPEC]\n{}\n\n[HARD FILLABLE FORM TEMPLATE]\n{}",
        EVALUATOR_SYSTEM_PROMPT,
        latest_user_message,
        latest_narrator_response,
        serde_json::to_string_pretty(&spec).unwrap_or_default(),
        serde_json::to_string_pretty(&hard_form_template).unwrap_or_default(),
    )
}

pub const CURRENT_EVALUATOR_CONTRACT_VERSION: i32 = 1;
pub const CURRENT_EVALUATOR_PROMPT_VERSION: i32 = 1;

pub fn build_evaluator_form_prompt_compact_with_player_persona(
    soul: &Soul,
    session_world: Option<&SessionWorld>,
    latest_user_message: &str,
    latest_narrator_response: &str,
    player_persona_id: &str,
    player_persona_display_name: &str,
) -> String {
    let spec = build_eval_form_spec_with_player_persona(
        soul,
        session_world,
        latest_user_message,
        latest_narrator_response,
        8,
        player_persona_id,
        player_persona_display_name,
    );
    let hard_form_template = build_hard_eval_form_template(&spec);
    format!(
        "# SYSTEM: Mnemosyne Compact Evaluator V1\n\
        You are filling a provided JSON evaluation sheet based on the latest exchange.\n\
        \n\
        ### GENERAL RULES:\n\
        1. Return valid evaluator_form_v1 JSON only. Unknown keys are invalid.\n\
        2. Keep relationship_rows empty: \"relationship_rows\": [].\n\
        3. Do not invent keys or alternate shapes. Do not rename keys.\n\
        4. For rows with no evidence, leave row_enabled as 0 and keep numeric fields at 0.\n\
        5. For rows with evidence, set row_enabled to 1 and fill every numeric field.\n\
        \n\
        ### SCHEMA & FIELD TAXONOMY RULES:\n\
        - **relationship_event_rows**:\n\
          - **row_enabled**: 0 or 1.\n\
          - **Identifier fields** (must be nonempty strings if enabled): `event_id`, `actor_entity_id`, `relationship_source_soul_id`, `relationship_target_entity_id`, `perceived_by_entity_id`, `evidence_quote`.\n\
          - **Axis fields** (must be an integer from -5 to +5): `intent`, `honesty`, `reliability`, `boundary_treatment`, `responsiveness`, `power_use`, `evaluation_tone`, `competence`, `disclosure`, `reciprocity`, `repair`, `predictability`. Predictability is -5..+5, never 0..100.\n\
          - **Modifier fields** (must be an integer from 0 to 100): `salience`, `certainty`, `directness`, `costliness`, `stakes`, `repetition`.\n\
          - **Bitmask field**: `event_flags_u64`.\n\
          - **Forbidden keys**: Do not output trust, comfort, intimacy, respect, fear, conflict, or boundary_pressure inside relationship_event_rows.\n\
        - **memory_rows**:\n\
          - Fill out memory candidates only for durable shifts/events. Verify memory slots are valid.\n\
        \n\
        ### evidence_quote STRICT RULES:\n\
        - Must be a single, continuous, exact substring from the latest exchange. No ellipses.\n\
        - Do not paraphrase or modify any words. Must be at least 4 characters.\n\
        - Escape nested double quotes or select a substring that avoids them.\n\
        \n\
        [LATEST EXCHANGE]\nUser: {}\nNarrator: {}\n\n[FORM SPEC]\n{}\n\n[HARD FILLABLE FORM TEMPLATE]\n{}",
        latest_user_message,
        latest_narrator_response,
        serde_json::to_string_pretty(&spec).unwrap_or_default(),
        serde_json::to_string_pretty(&hard_form_template).unwrap_or_default(),
    )
}

fn empty_relevance_tags_json() -> serde_json::Value {
    serde_json::json!({
        "setting_tags": {},
        "location_tags": {},
        "interacted_entities": {},
        "event_type_tags": {},
        "object_tags": {},
        "emotional_tags": {},
        "memory_slot_tags": {},
        "per_soul_relevance": {}
    })
}

fn display_relationship_target(target: &str) -> String {
    if target.eq_ignore_ascii_case("user") || target.eq_ignore_ascii_case("default_player") {
        "default_player".into()
    } else {
        target.trim().to_string()
    }
}

fn clean_summary_value<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    let value = value.trim();
    if value.is_empty() {
        fallback
    } else {
        value
    }
}

fn normalize_updater_time(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "Unknown".into();
    }
    let prefix = "Session start";
    let Some(found) = trimmed.find(prefix) else {
        return trimmed.to_string();
    };
    let after_prefix = found + prefix.len();
    let suffix = trimmed[after_prefix..]
        .trim_start_matches(['.', ' ', '-', ':'])
        .trim();
    if suffix.is_empty() {
        prefix.into()
    } else {
        suffix.to_string()
    }
}

fn mode_prompt_for(mode: &str) -> &'static str {
    match mode.trim().to_lowercase().as_str() {
        "realistic" => REALISTIC_MODE_PROMPT,
        "god" => GOD_MODE_PROMPT,
        "custom" => READER_MODE_PROMPT,
        _ => READER_MODE_PROMPT,
    }
}

fn chat_completions_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/chat/completions")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_request_omits_response_format_when_none() {
        let request = ChatCompletionRequest {
            model: "test-model".into(),
            temperature: 0.2,
            stream: false,
            response_format: None,
            messages: vec![ApiRequestMessage {
                role: "user".into(),
                content: "hello".into(),
            }],
        };
        let serialized = serde_json::to_string(&request).expect("serializes");
        assert!(!serialized.contains("response_format"));
        assert!(!serialized.contains("stream"));
    }

    #[test]
    fn chat_request_serializes_json_schema_response_format() {
        let request = ChatCompletionRequest {
            model: "test-model".into(),
            temperature: 0.0,
            stream: false,
            response_format: Some(serde_json::json!({
                "type": "json_schema",
                "json_schema": {
                    "name": "evaluator_patch",
                    "strict": true,
                    "schema": {"type": "object"}
                }
            })),
            messages: Vec::new(),
        };
        let serialized = serde_json::to_string(&request).expect("serializes");
        assert!(serialized.contains("\"response_format\""));
        assert!(serialized.contains("\"json_schema\""));
        assert!(serialized.contains("\"strict\":true"));
    }

    #[test]
    fn chat_response_captures_token_usage_when_reported() {
        let body: ChatCompletionResponse = serde_json::from_str(
            r#"{
                "choices": [{"message": {"content": "hi"}}],
                "usage": {"prompt_tokens": 120, "completion_tokens": 45, "total_tokens": 165}
            }"#,
        )
        .expect("deserializes");
        assert_eq!(
            body.usage,
            Some(TokenUsage {
                prompt_tokens: Some(120),
                completion_tokens: Some(45),
            })
        );

        let without: ChatCompletionResponse =
            serde_json::from_str(r#"{"choices": [{"message": {"content": "hi"}}]}"#)
                .expect("deserializes");
        assert_eq!(without.usage, None);
    }

    #[test]
    fn sse_metadata_captures_usage_from_final_chunk() {
        let meta = parse_sse_metadata(
            r#"data: {"id":"resp-1","choices":[],"usage":{"prompt_tokens":80,"completion_tokens":33}}"#,
        )
        .expect("metadata parsed");
        assert_eq!(
            meta.usage,
            Some(TokenUsage {
                prompt_tokens: Some(80),
                completion_tokens: Some(33),
            })
        );

        // A usage-only chunk (no id, no finish_reason) must still be surfaced.
        let usage_only =
            parse_sse_metadata(r#"data: {"usage":{"prompt_tokens":10,"completion_tokens":2}}"#)
                .expect("usage-only chunk parsed");
        assert!(usage_only.usage.is_some());

        // An empty usage object is not a report.
        assert!(parse_sse_metadata(r#"data: {"usage":{}}"#).is_none());
    }

    /// Strict-mode structured outputs require every object schema to be closed
    /// (additionalProperties: false) and to require every declared property.
    fn assert_strict_object_invariants(value: &serde_json::Value, path: &str) {
        if let Some(object) = value.as_object() {
            let is_object_schema = object
                .get("type")
                .map(|kind| match kind {
                    serde_json::Value::String(kind) => kind == "object",
                    serde_json::Value::Array(kinds) => kinds.iter().any(|kind| kind == "object"),
                    _ => false,
                })
                .unwrap_or(false);
            if is_object_schema {
                assert_eq!(
                    object.get("additionalProperties"),
                    Some(&serde_json::Value::Bool(false)),
                    "object schema at {path} must close additionalProperties"
                );
                let properties = object
                    .get("properties")
                    .and_then(|properties| properties.as_object())
                    .unwrap_or_else(|| panic!("object schema at {path} must declare properties"));
                let required = object
                    .get("required")
                    .and_then(|required| required.as_array())
                    .unwrap_or_else(|| panic!("object schema at {path} must declare required"));
                for key in properties.keys() {
                    assert!(
                        required.iter().any(|entry| entry == key),
                        "property {key} at {path} must be required (use nullable types for optionality)"
                    );
                }
            }
            for (key, child) in object {
                assert_strict_object_invariants(child, &format!("{path}/{key}"));
            }
        } else if let Some(array) = value.as_array() {
            for (index, child) in array.iter().enumerate() {
                assert_strict_object_invariants(child, &format!("{path}[{index}]"));
            }
        }
    }

    #[test]
    fn evaluator_patch_schema_satisfies_strict_mode() {
        let schema = evaluator_patch_json_schema();
        assert_strict_object_invariants(&schema, "root");
    }

    #[test]
    fn schema_shaped_output_parses_into_engine_patch() {
        // A maximal output a schema-enforced model could produce; it must be
        // accepted by the engine's patch parser without repair.
        let raw = serde_json::json!({
            "schema_version": 1,
            "soul_patch": {
                "relationship_deltas": [{
                    "from": "aurora",
                    "target": "default_player",
                    "trust": 1.5,
                    "affection": null,
                    "fear": null,
                    "conflict": 0.0
                }],
                "new_memories": [{
                    "memory_id": null,
                    "content": "Aurora noticed the visitor's steady answer.",
                    "tag": "observation",
                    "source_type": "current_session",
                    "truth_status": "scene_event",
                    "perceived_by_entity_id": "aurora",
                    "target_entity_ids": ["default_player"],
                    "memory_slot": "relationship_memory"
                }]
            },
            "world_patch": {
                "location": "the apartment kitchen",
                "time_elapsed": null,
                "scene_state": null,
                "event_operations": [{
                    "operation": "add_recent_event",
                    "recent_event_id": "evt_1",
                    "target_recent_event_id": null,
                    "content": "Aurora answered the visitor's question."
                }],
                "correction_note": null
            },
            "body_patch": null
        })
        .to_string();

        let patch: state_engine::patch::EnginePatch =
            serde_json::from_str(&raw).expect("schema-shaped output parses into EnginePatch");
        let soul_patch = patch.soul_patch.expect("soul patch present");
        assert_eq!(soul_patch.new_memories.len(), 1);
        assert_eq!(soul_patch.relationship_deltas.len(), 1);
        assert_eq!(
            patch.world_patch.expect("world patch").location.as_deref(),
            Some("the apartment kitchen")
        );
    }

    #[test]
    fn response_format_rejection_is_detected() {
        assert!(is_response_format_rejection(
            "API request failed with 400 Bad Request: {\"error\":\"response_format is not supported\"}"
        ));
        assert!(is_response_format_rejection(
            "API request failed with 422: json_schema unsupported for this model"
        ));
        // Real failures must not be mistaken for format rejections.
        assert!(!is_response_format_rejection(
            "API request failed with 401 Unauthorized: invalid api key"
        ));
        assert!(!is_response_format_rejection(
            "API request failed with 400 Bad Request: model not found"
        ));
        assert!(!is_response_format_rejection(
            "API request failed with 500: internal error in json_schema validator"
        ));
    }

    const SCENE_TURN_ROUTER_ASSUMPTION: &str = "If this narrator prompt is called, the input is a scene/RP turn. Slash commands and meta/control messages have already been handled by the router.";
    const SCENE_ONLY_OUTPUT_RULE: &str = "Write visible scene narration only. Include the visible status block. Do not write hidden state, EnginePatch JSON, markdown JSON, implementation notes, or command/help text.";

    fn assert_scene_only_narrator_prompt(prompt: &str) {
        assert!(prompt.contains("[SCENE TURN ASSUMPTION]"));
        assert!(prompt.contains(SCENE_TURN_ROUTER_ASSUMPTION));
        assert!(prompt.contains("Scene | Focus:"));
        assert!(!prompt.contains("[GM CHANNEL]"));
        assert!(!prompt.contains("OOC"));
        assert!(!prompt.contains("OCC"));
        assert!(!prompt.contains("GM/OOC"));
        assert!(!prompt.contains("GM"));
        assert!(!prompt.contains("brief GM/narrator reply"));
        assert!(!prompt.contains("unless the user asks to resume the scene"));
        assert!(!prompt.contains("Do not force an Aurora scene response"));
    }

    fn assert_scene_only_visible_prompt(prompt: &str) {
        assert!(prompt.contains("[OUTPUT]"));
        assert!(prompt.contains(SCENE_ONLY_OUTPUT_RULE));
        assert!(!prompt.contains("brief GM/narrator reply"));
        assert!(!prompt.contains("OOC"));
        assert!(!prompt.contains("OCC"));
        assert!(!prompt.contains("GM"));
    }

    #[test]
    fn builds_chat_completions_url() {
        assert_eq!(
            chat_completions_url("https://api.openai.com/v1"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            chat_completions_url("https://openrouter.ai/api/v1/chat/completions"),
            "https://openrouter.ai/api/v1/chat/completions"
        );
    }

    #[test]
    fn builds_reader_narrator_prompt_by_default() {
        let soul = state_engine::soul::new_default_soul("Aurora");
        let settings = ApiProviderSettings {
            base_url: "https://api.openai.com/v1".into(),
            api_key: "key".into(),
            model: "model".into(),
            system_prompt: "ignored unless custom".into(),
            ..Default::default()
        };
        let prompt = build_system_prompt(&settings, &soul, "[CURRENT STATE]", "Reader");

        assert!(prompt.contains("You are Mnemosyne's scene narrator"));
        assert!(prompt.contains("third-person present tense"));
        assert!(prompt.contains("engine-controlled characters and active Souls"));
        assert!(prompt.contains("Do not control user-controlled characters"));
        assert!(prompt.contains("NARRATION MODE: READER"));
        assert!(prompt.contains("[POV AND ATTRIBUTION]"));
        assert!(prompt.contains("User-controlled characters are external actors"));
        assert!(prompt.contains("[CHARACTER CONTROL]"));
        assert!(prompt.contains("[USER ACTION BOUNDARY]"));
        assert!(prompt.contains("decisions, thoughts, dialogue, intentions"));
        assert!(prompt.contains("[ACTION AND TURN CONTROL]"));
        assert!(prompt.contains("Engine-controlled characters may act proactively"));
        assert!(prompt.contains("stop on the attempt, demand, or pressure point"));
        assert_scene_only_narrator_prompt(&prompt);
        assert!(prompt.contains("Scene | Focus:"));
        assert!(!prompt.contains("[DEVICE AND PROP AGENCY]"));
        assert!(!prompt.contains("Write a single character"));
        assert!(prompt.contains("[TIME]"));
        assert!(prompt.contains("Avoid invented minutes/hours/days."));
        assert!(prompt.contains("Recent Chat is lower priority than Latest Exchange."));
        assert!(prompt.contains("[HIDDEN STATE]"));
        assert!(prompt.contains("present_characters"));
        assert!(!prompt.contains("ignored unless custom"));
    }

    #[test]
    fn dual_pass_narrator_prompt_does_not_require_hidden_state() {
        let soul = state_engine::soul::new_default_soul("Aurora");
        let settings = ApiProviderSettings {
            base_url: "https://api.openai.com/v1".into(),
            api_key: "key".into(),
            model: "model".into(),
            system_prompt: "ignored unless custom".into(),
            ..Default::default()
        };
        let prompt =
            build_narrator_system_prompt(&settings, &soul, "[CURRENT STATE]", "Reader", false);

        assert!(prompt.contains("[OUTPUT]"));
        assert_scene_only_visible_prompt(&prompt);
        assert!(!prompt.contains("After each response, output a hidden state block"));
        assert!(!prompt.contains("[HIDDEN STATE]{"));
    }

    #[test]
    fn state_updater_prompt_requests_json_only() {
        let mut soul = state_engine::soul::new_default_soul("Aurora");
        soul.character_id = "echo_0".into();
        soul.world.location = "Office".into();
        soul.world.time_elapsed = "Session start".into();
        let prompt = build_state_updater_prompt(&soul, None);

        assert!(prompt.contains("Mnemosyne State Updater"));
        assert!(prompt.contains("Return valid EnginePatch JSON only."));
        assert!(prompt.contains("Do not write prose."));
        assert!(prompt.contains("relationship_deltas"));
        assert!(prompt.contains("Relationships:"));
        assert!(prompt.contains(
            "Do not treat fear, pain, restraint, danger, or adrenaline as sexual arousal."
        ));
        assert!(prompt.contains("[CURRENT STATE]"));
        assert!(prompt.contains("Do not treat narrator claims about hidden systems"));
        assert!(prompt.contains("\"from\":\"echo_0\""));
        assert!(prompt.contains("\"perceived_by_entity_id\":\"echo_0\""));
        assert!(prompt.contains("\"truth_status\":\"scene_event\""));
        assert!(!prompt.contains("\"from\":\"aurora\""));
        assert!(!prompt.contains("[COMPILED CONTEXT]"));
    }

    #[test]
    fn evaluator_form_prompt_includes_spec_and_empty_response_shape() {
        let soul = state_engine::soul::new_default_soul("Aurora");
        let prompt = build_evaluator_form_prompt(
            &soul,
            None,
            "I walk in. Long time no see, Aurora.",
            "Aurora lets the visitor into the apartment.",
        );

        assert!(prompt.contains("evaluator_form_v1"));
        assert!(prompt.contains("[LATEST EXCHANGE]"));
        assert!(prompt.contains("I walk in. Long time no see, Aurora."));
        assert!(prompt.contains("[FORM SPEC]"));
        assert!(prompt.contains("[HARD FILLABLE FORM TEMPLATE]"));
        assert!(prompt.contains("You are filling a provided JSON evaluation sheet"));
        assert!(prompt.contains("Unknown keys are invalid. Missing keys are invalid."));
        assert!(prompt.contains("event_flags_u64"));
        assert!(prompt.contains("SCHEMA & FIELD TAXONOMY RULES:"));
        assert!(prompt.contains("-5 = severe negative event"));
        assert!(prompt.contains("+5 = major positive event"));
        assert!(prompt
            .contains("`intent`: benign/helpful intent (+5) vs malicious/harming intent (-5)"));
        assert!(prompt.contains("`salience`: how noticeable/salient the event is"));
        assert!(prompt.contains("trust: 0 total distrust"));
        assert!(prompt.contains("boundary_pressure: 0 no pressure"));
        assert!(prompt.contains("Predictability is an axis field, -5..+5, never 0..100"));
        assert!(prompt.contains("Never output trust, comfort, intimacy, respect, fear, conflict, or boundary_pressure inside relationship_event_rows"));
        assert!(prompt.contains("Relationship Surface Standards Reference Only"));
        assert!(prompt.contains(
            "The engine translates those event scores into bounded 0-100 relationship deltas"
        ));
        assert!(prompt.contains("Do not output final relationship point totals"));
        assert!(prompt.contains("Always output \"relationship_rows\": []"));
        assert!(
            prompt.contains("Do not prefix relationship dimensions with 'axis_' or 'modifier_'")
        );
        assert!(prompt.contains("evidence_quote Strict Rules"));
        assert!(prompt.contains("Must be a single, continuous, exact substring"));
        assert!(prompt.contains("Do not stitch multiple separate quotes together"));
        assert!(prompt.contains("relationship_event_rows"));
        assert!(prompt.contains("review_rows"));

        let relationship_event_schema = prompt
            .split("1. **relationship_event_rows**:")
            .nth(1)
            .and_then(|tail| tail.split("2. **relationship_rows**:").next())
            .expect("relationship event schema section");
        assert!(!relationship_event_schema.contains("- trust: 0 total distrust"));
        assert!(!relationship_event_schema.contains("- comfort: 0 unsafe/tense"));
    }

    #[test]
    fn prompt_includes_hard_relationship_event_template() {
        let soul = state_engine::soul::new_default_soul("Aurora");
        let prompt = build_evaluator_form_prompt(
            &soul,
            None,
            "I wait outside.",
            "Aurora watches from behind the chain.",
        );

        assert!(prompt.contains("[HARD FILLABLE FORM TEMPLATE]"));
        assert!(prompt.contains("\"row_enabled\": 0"));
        assert!(prompt.contains("\"actor_entity_id\": \"default_player\""));
        assert!(prompt.contains("\"relationship_source_soul_id\""));
        assert!(prompt.contains("\"relationship_target_entity_id\": \"default_player\""));
        assert!(prompt.contains("\"intent\": 0"));
        assert!(prompt.contains("\"salience\": 0"));
        assert!(prompt.contains("\"event_flags_u64\": 0"));
        assert!(prompt.contains("\"object_rows\""));
        assert!(prompt.contains("\"object_label\": \"\""));
        assert!(prompt.contains("\"object_type\": \"\""));
        assert!(prompt.contains("\"owner_entity_id\": \"default_player\""));
        assert!(prompt.contains("\"status\": \"\""));
        assert!(prompt.contains("\"last_observed_state\": \"\""));
        assert!(prompt.contains("\"new_character_rows\""));
        assert!(prompt.contains("\"scene_participants\""));
        assert!(!prompt.contains("\"axis_trust\""));
        assert!(!prompt.contains("\"modifier_trust\""));
    }

    #[test]
    fn system_prompt_contains_attribution_guard() {
        let soul = state_engine::soul::new_default_soul("Aurora");
        let settings = ApiProviderSettings {
            base_url: "https://api.openai.com/v1".into(),
            api_key: "key".into(),
            model: "model".into(),
            system_prompt: String::new(),
            ..Default::default()
        };
        let prompt = build_system_prompt(&settings, &soul, "[CURRENT STATE]", "Reader");

        assert!(prompt.contains("[POV AND ATTRIBUTION]"));
        assert!(prompt.contains("Do not invent user-controlled characters' thoughts"));
        assert!(prompt.contains("thoughts, motives, final decisions, or dialogue"));
        assert!(!prompt.contains("Character dialogue and narrator prose are not user statements."));
    }

    #[test]
    fn system_prompt_contains_time_section() {
        let soul = state_engine::soul::new_default_soul("Aurora");
        let settings = ApiProviderSettings {
            base_url: "https://api.openai.com/v1".into(),
            api_key: "key".into(),
            model: "model".into(),
            system_prompt: String::new(),
            ..Default::default()
        };
        let prompt = build_system_prompt(&settings, &soul, "[CURRENT STATE]", "Reader");

        assert!(prompt.contains("[TIME]"));
        assert!(prompt.contains("Concrete time only comes from user input or World Log."));
        assert!(prompt.contains("Avoid invented minutes/hours/days."));
    }

    #[test]
    fn system_prompt_contains_action_and_turn_control() {
        let soul = state_engine::soul::new_default_soul("Aurora");
        let settings = ApiProviderSettings {
            base_url: "https://api.openai.com/v1".into(),
            api_key: "key".into(),
            model: "model".into(),
            system_prompt: String::new(),
            ..Default::default()
        };
        let prompt = build_system_prompt(&settings, &soul, "[CURRENT STATE]", "Reader");

        assert!(prompt.contains("[ACTION AND TURN CONTROL]"));
        assert!(prompt.contains("Engine-controlled characters may act proactively"));
        assert!(prompt.contains("stop on the attempt, demand, or pressure point"));
        assert!(!prompt.contains("Do not make the character take"));
    }

    #[test]
    fn system_prompt_allows_user_declared_physical_actions_without_user_invention() {
        let soul = state_engine::soul::new_default_soul("Aurora");
        let settings = ApiProviderSettings {
            base_url: "https://api.openai.com/v1".into(),
            api_key: "key".into(),
            model: "model".into(),
            system_prompt: String::new(),
            ..Default::default()
        };
        let prompt = build_system_prompt(&settings, &soul, "[CURRENT STATE]", "Reader");

        assert!(prompt.contains("[USER ACTION BOUNDARY]"));
        assert!(prompt.contains("describe user-provided actions in concrete physical detail"));
        assert!(prompt.contains("contact, momentum, posture"));
        assert!(prompt.contains("Do not invent new user decisions"));
        assert!(prompt.contains("hidden motives, emotional reactions, dialogue"));
    }

    #[test]
    fn system_prompt_contains_conflict_resolution_and_progression_rules() {
        let soul = state_engine::soul::new_default_soul("Aurora");
        let settings = ApiProviderSettings {
            base_url: "https://api.openai.com/v1".into(),
            api_key: "key".into(),
            model: "model".into(),
            system_prompt: String::new(),
            ..Default::default()
        };
        let prompt = build_system_prompt(&settings, &soul, "[CURRENT STATE]", "Realistic");

        assert!(prompt.contains("[CONFLICT RESOLUTION]"));
        assert!(prompt.contains("partial success, resistance, interruption, or counteraction"));
        assert!(prompt.contains("Engine-controlled characters may resist, counter"));
        assert!(prompt.contains("Do not choose the user's next tactic"));
        assert!(prompt.contains("do not re-describe the full room"));
        assert!(prompt.contains("User-declared physical actions may be rendered cinematically"));
        assert!(prompt.contains("do not invent user intent"));
    }

    #[test]
    fn system_prompt_is_scene_only_after_slash_router() {
        let soul = state_engine::soul::new_default_soul("Aurora");
        let settings = ApiProviderSettings {
            base_url: "https://api.openai.com/v1".into(),
            api_key: "key".into(),
            model: "model".into(),
            system_prompt: String::new(),
            ..Default::default()
        };
        let prompt =
            build_narrator_system_prompt(&settings, &soul, "[CURRENT STATE]", "Reader", false);

        assert_scene_only_narrator_prompt(&prompt);
        assert_scene_only_visible_prompt(&prompt);
        assert!(prompt.contains("Primary active Soul: Aurora"));
    }

    #[test]
    fn frontend_preview_prompt_matches_scene_only_behavior() {
        let frontend_source = include_str!("../../../src/tauri.ts");

        assert!(frontend_source.contains("[SCENE TURN ASSUMPTION]"));
        assert!(frontend_source.contains(SCENE_TURN_ROUTER_ASSUMPTION));
        assert!(frontend_source.contains(SCENE_ONLY_OUTPUT_RULE));
        assert!(!frontend_source.contains("[GM CHANNEL]"));
        assert!(!frontend_source.contains("brief GM/narrator reply"));
        assert!(!frontend_source.contains("GM-facing instructions"));
        assert!(!frontend_source.contains("unless the user asks to resume the scene"));
    }

    #[test]
    fn command_prompts_are_separate_from_rp_narrator_prompt() {
        for prompt in [
            build_command_ooc_prompt(),
            build_command_setup_prompt(),
            build_command_state_summary_prompt(),
            build_command_state_edit_prompt(),
            build_command_soul_edit_agent_prompt(),
            build_command_help_prompt(),
        ] {
            assert!(prompt.contains("out-of-roleplay"));
            assert!(prompt.contains("NOT the RP narrator"));
            assert!(prompt.contains("status block"));
            assert!(!prompt.contains("Scene | Focus:"));
            assert!(!prompt.contains("[SCENE TURN ASSUMPTION]"));
            assert!(!prompt.contains("Primary active Soul"));
        }
        assert!(build_command_ooc_prompt().contains("REFERENCE MATERIAL ONLY"));
        assert!(build_command_setup_prompt().contains("REFERENCE MATERIAL ONLY"));
        assert!(build_command_soul_edit_agent_prompt().contains("REFERENCE MATERIAL ONLY"));
        assert!(build_command_help_prompt().contains("/status"));
        assert!(build_command_help_prompt().contains("deprecated alias"));
    }

    #[test]
    fn hidden_state_prompt_describes_world_event_as_completed_scene_fact() {
        let soul = state_engine::soul::new_default_soul("Aurora");
        let settings = ApiProviderSettings {
            base_url: "https://api.openai.com/v1".into(),
            api_key: "key".into(),
            model: "model".into(),
            system_prompt: String::new(),
            ..Default::default()
        };
        let prompt = build_system_prompt(&settings, &soul, "[CURRENT STATE]", "Reader");

        assert!(
            prompt.contains("world_event must be a compact authoritative completed-fact summary")
        );
        assert!(prompt.contains("Phone reveal completed"));
        assert!(prompt.contains("what should not be replayed"));
    }

    #[test]
    fn custom_mode_uses_custom_prompt_as_primary_instructions() {
        let soul = state_engine::soul::new_default_soul("Aurora");
        let settings = ApiProviderSettings {
            base_url: "https://api.openai.com/v1".into(),
            api_key: "key".into(),
            model: "model".into(),
            system_prompt: "Custom narrator law.".into(),
            ..Default::default()
        };
        let prompt = build_system_prompt(&settings, &soul, "[CURRENT STATE]", "Custom");

        assert!(prompt.starts_with("[CUSTOM NARRATOR INSTRUCTIONS]\nCustom narrator law."));
        assert!(prompt.contains("[VERIFIED DIAGNOSTICS BOUNDARY]"));
        assert!(
            prompt.find("[CUSTOM NARRATOR INSTRUCTIONS]").unwrap()
                < prompt.find("[CURRENT STATE]").unwrap()
        );
        assert!(!prompt.contains("# SYSTEM: Narrator AI - Mnemosyne Engine"));
        assert!(!prompt.contains("NARRATION MODE: READER"));
        assert!(!prompt.contains("[POV AND ATTRIBUTION]"));
        assert!(prompt.contains("HIDDEN STATE FORMAT"));
        assert!(prompt.contains("Primary active Soul: Aurora"));
        assert!(prompt.contains("[CURRENT STATE]"));
    }

    #[test]
    fn custom_mode_with_empty_prompt_uses_default_prompt_without_custom_block() {
        let soul = state_engine::soul::new_default_soul("Aurora");
        let settings = ApiProviderSettings {
            base_url: "https://api.openai.com/v1".into(),
            api_key: "key".into(),
            model: "model".into(),
            system_prompt: "   ".into(),
            ..Default::default()
        };
        let prompt = build_system_prompt(&settings, &soul, "[CURRENT STATE]", "Custom");

        assert!(prompt.contains("You are Mnemosyne's scene narrator"));
        assert!(!prompt.contains("[CUSTOM NARRATOR INSTRUCTIONS]"));
        assert!(prompt.contains("Primary active Soul: Aurora"));
    }

    #[test]
    fn reader_mode_ignores_custom_prompt_text() {
        let soul = state_engine::soul::new_default_soul("Aurora");
        let settings = ApiProviderSettings {
            base_url: "https://api.openai.com/v1".into(),
            api_key: "key".into(),
            model: "model".into(),
            system_prompt: "Custom narrator law.".into(),
            ..Default::default()
        };
        let prompt = build_system_prompt(&settings, &soul, "[CURRENT STATE]", "Reader");

        assert!(prompt.contains("NARRATION MODE: READER"));
        assert!(!prompt.contains("[CUSTOM NARRATOR INSTRUCTIONS]"));
        assert!(!prompt.contains("Custom narrator law."));
    }

    #[test]
    fn narrator_prompt_contains_verified_diagnostics_boundary() {
        let soul = state_engine::soul::new_default_soul("Aurora");
        let settings = ApiProviderSettings {
            base_url: "https://api.openai.com/v1".into(),
            api_key: "key".into(),
            model: "model".into(),
            system_prompt: String::new(),
            ..Default::default()
        };
        let prompt = build_system_prompt(&settings, &soul, "[CURRENT STATE]", "Reader");

        assert!(prompt.contains("[VERIFIED DIAGNOSTICS BOUNDARY]"));
        assert!(prompt.contains("Do not claim a backend test passed unless"));
        assert!(prompt.contains("simulated/in-scene diagnostic"));
    }

    #[test]
    fn streaming_visible_prefix_holds_back_hidden_marker() {
        let partial = "Visible text.\n\n[HIDDEN";
        assert_eq!(
            visible_stream_prefix_len(partial),
            "Visible text.\n\n".len()
        );
        let full = "Visible text.\n\n[HIDDEN STATE]{\"tag\":\"observation\"}";
        assert_eq!(visible_stream_prefix_len(full), "Visible text.\n\n".len());
    }

    #[test]
    fn streaming_visible_prefix_handles_em_dash_utf8() {
        let text = "Visible — text [HIDDEN";
        let visible_len = visible_stream_prefix_len(text);
        let chunk = slice_at_char_boundaries(text, 0, visible_len);
        assert_eq!(chunk, "Visible — text ");
    }

    #[test]
    fn streaming_visible_prefix_handles_korean_utf8() {
        let text = "장면이 조용하다 [HIDDEN";
        let visible_len = visible_stream_prefix_len(text);
        let chunk = slice_at_char_boundaries(text, 0, visible_len);
        assert_eq!(chunk, "장면이 조용하다 ");
    }

    #[test]
    fn streaming_visible_prefix_handles_emoji_utf8() {
        let text = "She smiles 🙂 [HIDDEN";
        let visible_len = visible_stream_prefix_len(text);
        let chunk = slice_at_char_boundaries(text, 0, visible_len);
        assert_eq!(chunk, "She smiles 🙂 ");
    }

    #[test]
    fn streaming_partial_hidden_marker_after_multibyte_text() {
        let mut emitted_visible_len = 0;
        let mut full_text = String::new();

        full_text.push_str("숨이 멎는 듯한 — pause 🙂 ");
        let visible_len = visible_stream_prefix_len(&full_text);
        let first_chunk = slice_at_char_boundaries(&full_text, emitted_visible_len, visible_len);
        assert_eq!(first_chunk, "숨이 멎는 듯한 — pause 🙂 ");
        emitted_visible_len = visible_len;

        full_text.push_str("[HID");
        let visible_len = visible_stream_prefix_len(&full_text);
        let second_chunk = slice_at_char_boundaries(&full_text, emitted_visible_len, visible_len);
        assert_eq!(second_chunk, "");
        emitted_visible_len = visible_len;

        full_text.push_str("DEN STATE]{\"tag\":\"observation\"}");
        let visible_len = visible_stream_prefix_len(&full_text);
        let third_chunk = slice_at_char_boundaries(&full_text, emitted_visible_len, visible_len);
        assert_eq!(third_chunk, "");
    }
}
