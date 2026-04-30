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
}

fn build_system_prompt(
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
}
