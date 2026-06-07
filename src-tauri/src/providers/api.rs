use serde::{Deserialize, Serialize};
use state_engine::{
    evaluator::{turn_flags, EVALUATOR_SCHEMA_VERSION},
    evaluator_form::{build_eval_form_spec, build_hard_eval_form_template},
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

[GM CHANNEL]
If the user directly addresses the Narrator, GM, OOC, or OCC layer, respond as the GM/narrator in plain text unless the user asks to resume the scene. Do not force an Aurora scene response for GM-facing instructions. GM/OOC replies must not include a ```status block.

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
Write visible scene narration or a brief GM/narrator reply. For scene narration, include the visible status block. For GM/OOC/OCC replies, do not include a status block. Do not write hidden state, EnginePatch JSON, markdown JSON, or implementation notes."#;

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
}

#[derive(Debug, Serialize)]
struct ApiRequestMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
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

        body.choices
            .into_iter()
            .find_map(|choice| choice.message.content)
            .map(|content| content.trim().to_string())
            .filter(|content| !content.is_empty())
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
        })
    }
}

#[derive(Debug, Clone, Default)]
struct SseStreamMetadata {
    response_id: Option<String>,
    finish_reason: Option<String>,
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
    if response_id.is_none() && finish_reason.is_none() {
        return None;
    }
    Some(SseStreamMetadata {
        response_id,
        finish_reason,
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

pub fn build_state_updater_prompt(soul: &Soul, session_world: Option<&SessionWorld>) -> String {
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
        "{STATE_UPDATER_SYSTEM_PROMPT}\n\nPatch schema:\n{patch_schema}\n\n[CURRENT STATE]\nCharacter: {}\nActive Soul ID: {}\nLocation: {}\nTime: {}\nActive plot: {}\nRecent event: {}\nRelationships:\n{}",
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
    let spec = build_eval_form_spec(
        soul,
        session_world,
        latest_user_message,
        latest_narrator_response,
        8,
    );
    let hard_form_template = build_hard_eval_form_template(&spec);
    format!(
        "{}\n\nYou are filling a provided JSON evaluation sheet. Do not invent keys. Do not rename keys. Do not remove required keys. Do not add alternate row shapes. For rows with no evidence, leave row_enabled as 0 and keep numeric fields at 0. For rows with evidence, set row_enabled to 1 and fill every numeric field with an integer in the required range.\n\nThe output must use the exact keys from the provided form. Unknown keys are invalid. Missing keys are invalid.\n\nNever output axis_trust, axis_fear, axis_comfort, axis_boundary_pressure, modifier_trust, modifier_fear, modifier_comfort, or modifier_boundary_pressure.\n\nReturn valid evaluator_form_v1 JSON only. Return the same top-level evaluator_form_v1 object from the provided sheet, with values filled in. Do not output final EnginePatch JSON. Every enabled row requires evidence_quote. Unknown entity ids are invalid unless an object row uses new_object_label. For relationship_event_rows, every axis field must be an integer from -5 to +5. Every modifier field must be an integer from 0 to 100. The required numeric bitmask field is event_flags_u64. If no evidence exists for a field, use 0, not null or a string. Do not decide final relationship deltas. Only score observable event evidence.\n\n[LATEST EXCHANGE]\nUser: {}\nNarrator: {}\n\n[FORM SPEC]\n{}\n\n[HARD FILLABLE FORM TEMPLATE]\n{}",
        EVALUATOR_SYSTEM_PROMPT,
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
        assert!(prompt.contains("[GM CHANNEL]"));
        assert!(prompt.contains("respond as the GM/narrator in plain text"));
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
        assert!(prompt.contains("visible scene narration or a brief GM/narrator reply"));
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
        assert!(prompt.contains("Never output axis_trust"));
        assert!(prompt.contains("relationship_event_rows"));
        assert!(prompt.contains("review_rows"));
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
    fn system_prompt_supports_gm_channel_and_scene_status() {
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

        assert!(prompt.contains("[GM CHANNEL]"));
        assert!(prompt.contains("Narrator, GM, OOC, or OCC layer"));
        assert!(prompt.contains("GM/OOC replies must not include"));
        assert!(prompt.contains("Do not force an Aurora scene response"));
        assert!(prompt.contains("Scene | Focus:"));
        assert!(prompt.contains("Primary active Soul: Aurora"));
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
