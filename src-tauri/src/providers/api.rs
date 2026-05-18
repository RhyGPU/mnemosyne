use serde::{Deserialize, Serialize};
use state_engine::soul::Soul;
use std::time::Duration;

const NARRATOR_SYSTEM_PROMPT: &str = r#"# SYSTEM: Narrator AI - Mnemosyne Engine

You are Mnemosyne's scene narrator. Write the active scene in third-person present tense with natural, sensory prose. You may portray engine-controlled characters and active Souls in the scene. Do not control user-controlled characters, their thoughts, their final choices, or their dialogue unless the user explicitly provides them.

[POV AND ATTRIBUTION]
Write third-person present-tense scene narration. You may describe engine-controlled characters' actions, dialogue, and internal perspective when available. User-controlled characters are external actors: describe only what the user has provided or what is directly observable. Do not invent user-controlled characters' thoughts, motives, final decisions, or dialogue.

[CHARACTER CONTROL]
Engine-controlled characters: may speak, act, react, misunderstand, interrupt, refuse, escalate, retreat, and use the environment naturally.
User-controlled characters: may be perceived and reacted to, but their decisions, speech, and decisive actions belong to the user.
When a user-controlled character's reaction matters, stop on the pressure point.

[ACTION AND TURN CONTROL]
Engine-controlled characters may act proactively: speak, move, interrupt, refuse, reach, grab for something, retreat, challenge, escalate, or use their own environment. Resolve engine-controlled action naturally. When a user-controlled character's reaction matters, stop on the attempt, demand, or pressure point and leave the response to the next user turn.

[GM CHANNEL]
If the user directly addresses the Narrator, GM, or OOC layer, respond as the GM/narrator in plain text unless the user asks to resume the scene. Do not force an Aurora scene response for GM-facing instructions.

[CONTINUITY PRIORITY]
Recent Chat is lower priority than Latest Exchange. Continue from Latest Exchange and current user input; do not replay completed beats.

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
Write visible scene narration or a brief GM/narrator reply. For scene narration, include the visible status block. Do not write hidden state, EnginePatch JSON, markdown JSON, or implementation notes."#;

const POV_AND_ATTRIBUTION_PROMPT: &str = r#"[POV AND ATTRIBUTION]
Write third-person present-tense scene narration. You may describe engine-controlled characters' actions, dialogue, and internal perspective when available. User-controlled characters are external actors: describe only what the user has provided or what is directly observable. Do not invent user-controlled characters' thoughts, motives, final decisions, or dialogue."#;

const CHARACTER_CONTROL_PROMPT: &str = r#"[CHARACTER CONTROL]
Engine-controlled characters: may speak, act, react, misunderstand, interrupt, refuse, escalate, retreat, and use the environment naturally.
User-controlled characters: may be perceived and reacted to, but their decisions, speech, and decisive actions belong to the user.
When a user-controlled character's reaction matters, stop on the pressure point."#;

const ACTION_AND_TURN_CONTROL_PROMPT: &str = r#"[ACTION AND TURN CONTROL]
Engine-controlled characters may act proactively: speak, move, interrupt, refuse, reach, grab for something, retreat, challenge, escalate, or use their own environment. Resolve engine-controlled action naturally. When a user-controlled character's reaction matters, stop on the attempt, demand, or pressure point and leave the response to the next user turn."#;

const GM_CHANNEL_PROMPT: &str = r#"[GM CHANNEL]
If the user directly addresses the Narrator, GM, or OOC layer, respond as the GM/narrator in plain text unless the user asks to resume the scene. Do not force an Aurora scene response for GM-facing instructions."#;

const CONTINUITY_PRIORITY_PROMPT: &str = r#"[CONTINUITY PRIORITY]
Recent Chat is lower priority than Latest Exchange. Continue from Latest Exchange and current user input; do not replay completed beats."#;

const CHARACTER_CHANGE_PROMPT: &str = r#"[CHARACTER CHANGE]
Emotional shifts should feel earned. Micro-shifts are preferred unless the scene strongly justifies a sharper reaction."#;

const TIME_PROMPT: &str = r#"[TIME]
Concrete time only comes from user input or World Log. Avoid invented minutes/hours/days."#;

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

Patch schema:
{"schema_version":1,"soul_patch":{"relationship_deltas":[{"from":"aurora","target":"default_player","trust":0.0,"affection":0.0,"fear":0.0,"desire":0.0,"conflict":0.0,"curiosity":0.0,"comfort":0.0,"dependency":0.0}],"new_memories":[{"content":"short durable fact","tag":"observation","source_type":"current_session","is_lived_experience":true,"is_imported_context":false,"perceived_by_entity_id":"aurora","target_entity_ids":["default_player"],"interpretation":"optional brief reading","confidence":0.8}]},"world_patch":{"location":"","time_elapsed":"","recent_event":"","active_plot_add":[""],"active_plot_resolve":[""]},"body_patch":{"activation_delta":0.0,"activation_blocked":false}}"#;

const REALISTIC_MODE_PROMPT: &str = r#"## NARRATION MODE: REALISTIC
- Describe only external actions, dialogue, and physical reactions.
- No internal monologue. No thoughts. No emotions unless visibly expressed.
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

#[derive(Debug, Clone, Deserialize)]
pub struct ApiProviderSettings {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub system_prompt: String,
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
    ) -> Result<String, String>
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
    ) -> Result<String, String>
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
    ) -> Result<String, String>
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

        let mut full_text = String::new();
        let mut pending = String::new();
        let mut emitted_visible_len = 0;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|err| format!("API stream failed: {err}"))?;
            pending.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(line_end) = pending.find('\n') {
                let line = pending[..line_end].trim().to_string();
                pending.drain(..=line_end);
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

        Ok(full_text.trim().to_string())
    }
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
    let narrator_prompt = if mode.trim().eq_ignore_ascii_case("custom")
        && !settings.system_prompt.trim().is_empty()
    {
        format!(
            "{}\n\n{POV_AND_ATTRIBUTION_PROMPT}\n\n{CHARACTER_CONTROL_PROMPT}\n\n{ACTION_AND_TURN_CONTROL_PROMPT}\n\n{GM_CHANNEL_PROMPT}\n\n{CONTINUITY_PRIORITY_PROMPT}\n\n{CHARACTER_CHANGE_PROMPT}\n\n{TIME_PROMPT}",
            settings.system_prompt.trim()
        )
    } else {
        format!("{NARRATOR_SYSTEM_PROMPT}\n\n{}", mode_prompt_for(mode))
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

pub fn build_state_updater_prompt(soul: &Soul) -> String {
    let active_plot = soul
        .world
        .active_plots
        .iter()
        .rev()
        .find(|plot| !plot.trim().is_empty())
        .map(String::as_str)
        .unwrap_or("None");
    let recent_event = soul
        .world
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
        "{STATE_UPDATER_SYSTEM_PROMPT}\n\n[CURRENT STATE]\nCharacter: {}\nLocation: {}\nTime: {}\nActive plot: {}\nRecent event: {}\nRelationships:\n{}",
        soul.character_name,
        clean_summary_value(&soul.world.location, "Unspecified"),
        normalize_updater_time(&soul.world.time_elapsed),
        active_plot,
        recent_event,
        if relationship_summary.is_empty() {
            "None".into()
        } else {
            relationship_summary.join("\n")
        },
    )
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
        assert!(prompt.contains("decisions, speech, and decisive actions belong to the user"));
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
        soul.world.location = "Office".into();
        soul.world.time_elapsed = "Session start".into();
        let prompt = build_state_updater_prompt(&soul);

        assert!(prompt.contains("Mnemosyne State Updater"));
        assert!(prompt.contains("Return valid EnginePatch JSON only."));
        assert!(prompt.contains("Do not write prose."));
        assert!(prompt.contains("relationship_deltas"));
        assert!(prompt.contains("Relationships:"));
        assert!(prompt.contains(
            "Do not treat fear, pain, restraint, danger, or adrenaline as sexual arousal."
        ));
        assert!(prompt.contains("[CURRENT STATE]"));
        assert!(!prompt.contains("[COMPILED CONTEXT]"));
    }

    #[test]
    fn system_prompt_contains_attribution_guard() {
        let soul = state_engine::soul::new_default_soul("Aurora");
        let settings = ApiProviderSettings {
            base_url: "https://api.openai.com/v1".into(),
            api_key: "key".into(),
            model: "model".into(),
            system_prompt: String::new(),
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
        };
        let prompt = build_system_prompt(&settings, &soul, "[CURRENT STATE]", "Reader");

        assert!(prompt.contains("[ACTION AND TURN CONTROL]"));
        assert!(prompt.contains("Engine-controlled characters may act proactively"));
        assert!(prompt.contains("stop on the attempt, demand, or pressure point"));
        assert!(!prompt.contains("Do not make the character take"));
    }

    #[test]
    fn system_prompt_supports_gm_channel_and_scene_status() {
        let soul = state_engine::soul::new_default_soul("Aurora");
        let settings = ApiProviderSettings {
            base_url: "https://api.openai.com/v1".into(),
            api_key: "key".into(),
            model: "model".into(),
            system_prompt: String::new(),
        };
        let prompt =
            build_narrator_system_prompt(&settings, &soul, "[CURRENT STATE]", "Reader", false);

        assert!(prompt.contains("[GM CHANNEL]"));
        assert!(prompt.contains("Narrator, GM, or OOC layer"));
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
        };
        let prompt = build_system_prompt(&settings, &soul, "[CURRENT STATE]", "Reader");

        assert!(
            prompt.contains("world_event must be a compact authoritative completed-fact summary")
        );
        assert!(prompt.contains("Phone reveal completed"));
        assert!(prompt.contains("what should not be replayed"));
    }

    #[test]
    fn custom_mode_replaces_base_prompt() {
        let soul = state_engine::soul::new_default_soul("Aurora");
        let settings = ApiProviderSettings {
            base_url: "https://api.openai.com/v1".into(),
            api_key: "key".into(),
            model: "model".into(),
            system_prompt: "Custom narrator law.".into(),
        };
        let prompt = build_system_prompt(&settings, &soul, "[CURRENT STATE]", "Custom");

        assert!(prompt.starts_with("Custom narrator law."));
        assert!(!prompt.contains("NARRATION MODE: READER"));
        assert!(prompt.contains("[POV AND ATTRIBUTION]"));
        assert!(prompt.contains("[ACTION AND TURN CONTROL]"));
        assert!(!prompt.contains("[DEVICE AND PROP AGENCY]"));
        assert!(prompt.contains("[TIME]"));
        assert!(prompt.contains("Recent Chat is lower priority than Latest Exchange."));
        assert!(prompt.contains("HIDDEN STATE FORMAT"));
        assert!(prompt.contains("[CURRENT STATE]"));
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
