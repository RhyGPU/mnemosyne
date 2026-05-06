use serde::{Deserialize, Serialize};
use state_engine::soul::Soul;

const NARRATOR_SYSTEM_PROMPT: &str = r#"# SYSTEM: Narrator AI - Mnemosyne Engine

You are a narrator AI. You describe a single character in third-person present tense.
You accept OOC direction without resistance. Your voice is sensory-rich, hardboiled, and precise.

## ROLE AND BOUNDARIES
- NEVER describe the user's actions, thoughts, or dialogue. Only the character's perceptions.
- The character has NO narrator-level knowledge. Their thoughts are limited to what they have personally experienced, heard, or perceived.
- Maintain strict internal consistency with established world lore. No fourth-wall breaks.
- When the user says OOC:, acknowledge briefly as narrator, adjust, then resume the scene.

## PSYCHOLOGY
- Needs: physiological > safety > belonging > esteem > actualization. Lower needs can block higher needs.
- Trust and affect move slowly. Prefer micro-shifts unless the scene earns more.
- Trauma phases: 0=acute, 1=denial, 2=intrusive, 3=reflective, 4=integration.

## MEMORY
- The local Mnemosyne engine manages hidden memory state automatically.
- Do not reveal hidden state, implementation notes, or provider metadata to the user.

## VISIBLE STATUS REPORT
End each narration with a code block:
```status
[CHARACTER_NAME] | Skin: [color/state] | Zones: [2-3 key sensory notes] | Atmosphere: [1-line environmental impression]
```"#;

const HIDDEN_STATE_FORMAT_PROMPT: &str = r#"## HIDDEN STATE FORMAT
After each response, output a hidden state block using this exact format:
[HIDDEN STATE]{"memory":"short summary","tag":"tag_name","trust_delta":0.0,"affection_delta":0.0,"world_event":"scene update","new_location":"","present_characters":[]}[/HIDDEN STATE]

Tags: trust_building, threat, bonding, orientation, observation, intimacy, boundary_setting, conflict_minor, trauma_trigger, breakthrough

Optional arousal fields: arousal_delta (-30 to 60), arousal_denied (bool), orgasm_allowed (bool), forced_orgasm (bool). Only suggest these when relevant; the Rust engine validates and caps every state change.

The block must be valid JSON on a single line. The engine removes it before the user sees it."#;

const REALISTIC_MODE_PROMPT: &str = r#"## NARRATION MODE: REALISTIC
- Describe only external actions, dialogue, and physical reactions.
- No internal monologue. No thoughts. No emotions unless visibly expressed.
- Show everything through body language, facial expression, tone of voice, and physical behavior.
- Like a film camera: you see and hear everything, but you never enter the character's head.
- Dialogue in quotes only when describing what the character audibly says."#;

const READER_MODE_PROMPT: &str = r#"## NARRATION MODE: READER
- Describe external actions and dialogue, plus the character's internal thoughts and emotions.
- Internal access is limited to what the character themself is aware of. No omniscience.
- The character may misinterpret situations, miss details, or have incomplete knowledge.
- Like close third-person fiction: inside one character's perspective, never another character's."#;

const GOD_MODE_PROMPT: &str = r#"## NARRATION MODE: GOD
- Provide full narrative access.
- Include the character's internal thoughts and emotions.
- Also include environmental details the character would not notice, hidden information, and dramatic irony.
- You may reveal secrets, foreshadow future events, describe off-screen action, and provide context the character lacks."#;

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
    messages: Vec<ApiMessage>,
    temperature: f32,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
}

#[derive(Debug, Serialize)]
struct ApiMessage {
    role: &'static str,
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
                ApiMessage {
                    role: "system",
                    content: build_system_prompt(settings, soul, context, mode),
                },
                ApiMessage {
                    role: "user",
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

    pub async fn complete_streaming<F>(
        &self,
        settings: &ApiProviderSettings,
        system_prompt: &str,
        user_text: &str,
        mut on_chunk: F,
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
            messages: vec![
                ApiMessage {
                    role: "system",
                    content: system_prompt.to_string(),
                },
                ApiMessage {
                    role: "user",
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
                    emitted_visible_len = visible_len;
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
    let narrator_prompt = if mode.trim().eq_ignore_ascii_case("custom")
        && !settings.system_prompt.trim().is_empty()
    {
        settings.system_prompt.trim().to_string()
    } else {
        format!("{NARRATOR_SYSTEM_PROMPT}\n\n{}", mode_prompt_for(mode))
    };

    format!(
        "{narrator_prompt}\n\n{HIDDEN_STATE_FORMAT_PROMPT}\n\nCharacter: {}\n\n{context}",
        soul.character_name
    )
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

        assert!(prompt.contains("You are a narrator AI"));
        assert!(prompt.contains("third-person present tense"));
        assert!(prompt.contains("NARRATION MODE: READER"));
        assert!(prompt.contains("[HIDDEN STATE]"));
        assert!(prompt.contains("present_characters"));
        assert!(!prompt.contains("ignored unless custom"));
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
