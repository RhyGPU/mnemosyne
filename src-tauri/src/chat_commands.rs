#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatCommandKind {
    Ooc,
    Setup,
    State,
    Status,
    Persona,
    Ask,
    Help,
    Unknown(String),
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateSubcommandKind {
    Show,
    Update,
    Review,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersonaSubcommandKind {
    List,
    Lookup,
    Change,
    Add,
    Edit,
    Help,
    Unknown(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AskMode {
    Auto,
    Plan,
    Apply,
    Diff,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedChatCommand {
    pub kind: ChatCommandKind,
    pub state_subcommand: Option<StateSubcommandKind>,
    pub persona_subcommand: Option<PersonaSubcommandKind>,
    pub ask_mode: AskMode,
    pub body: String,
    pub raw: String,
}

impl ParsedChatCommand {
    fn none(raw: &str) -> Self {
        Self {
            kind: ChatCommandKind::None,
            state_subcommand: None,
            persona_subcommand: None,
            ask_mode: AskMode::Auto,
            body: String::new(),
            raw: raw.to_string(),
        }
    }

    pub fn detected(&self) -> bool {
        !matches!(self.kind, ChatCommandKind::None)
    }

    pub fn kind_label(&self) -> String {
        match &self.kind {
            ChatCommandKind::Ooc => "ooc".into(),
            ChatCommandKind::Setup => "setup".into(),
            ChatCommandKind::State => "state".into(),
            ChatCommandKind::Status => "status".into(),
            ChatCommandKind::Persona => "persona".into(),
            ChatCommandKind::Ask => "ask".into(),
            ChatCommandKind::Help => "help".into(),
            ChatCommandKind::Unknown(command) => format!("unknown:{command}"),
            ChatCommandKind::None => "none".into(),
        }
    }
}

pub fn parse_chat_command(input: &str) -> ParsedChatCommand {
    let trimmed = input.trim_start();
    if !trimmed.starts_with('/') {
        return ParsedChatCommand::none(input);
    }

    let raw = trimmed.to_string();
    let without_slash = &trimmed[1..];
    let token_end = without_slash
        .find(char::is_whitespace)
        .unwrap_or(without_slash.len());
    let token = &without_slash[..token_end];
    let body = without_slash[token_end..].trim().to_string();
    let token_lower = token.to_ascii_lowercase();

    let mut parsed = ParsedChatCommand {
        kind: match token_lower.as_str() {
            "ooc" => ChatCommandKind::Ooc,
            "setup" => ChatCommandKind::Setup,
            "state" => ChatCommandKind::State,
            "status" => ChatCommandKind::Status,
            "persona" => ChatCommandKind::Persona,
            "ask" => ChatCommandKind::Ask,
            "help" => ChatCommandKind::Help,
            _ => ChatCommandKind::Unknown(token.to_string()),
        },
        state_subcommand: None,
        persona_subcommand: None,
        ask_mode: AskMode::Auto,
        body,
        raw,
    };

    if matches!(parsed.kind, ChatCommandKind::State) {
        parsed.state_subcommand = Some(parse_state_subcommand(&parsed.body));
    }
    if matches!(parsed.kind, ChatCommandKind::Status) {
        parsed.state_subcommand = Some(StateSubcommandKind::Show);
    }
    if matches!(parsed.kind, ChatCommandKind::Persona) {
        parsed.persona_subcommand = Some(parse_persona_subcommand(&parsed.body));
    }
    if matches!(parsed.kind, ChatCommandKind::Ask) {
        parsed.ask_mode = parse_ask_mode(&mut parsed.body);
    }

    parsed
}

pub fn parse_persona_subcommand(body: &str) -> PersonaSubcommandKind {
    let token = first_token(body).unwrap_or_default();
    match token.to_ascii_lowercase().as_str() {
        "" => PersonaSubcommandKind::Help,
        "list" => PersonaSubcommandKind::List,
        "lookup" | "show" | "current" => PersonaSubcommandKind::Lookup,
        "change" | "select" | "use" => PersonaSubcommandKind::Change,
        "add" | "new" | "create" => PersonaSubcommandKind::Add,
        "edit" | "update" => PersonaSubcommandKind::Edit,
        other => PersonaSubcommandKind::Unknown(other.to_string()),
    }
}

pub fn parse_state_subcommand(body: &str) -> StateSubcommandKind {
    let token = first_token(body).unwrap_or_default();
    match token.to_ascii_lowercase().as_str() {
        "" | "show" => StateSubcommandKind::Show,
        "update" => StateSubcommandKind::Update,
        "review" => StateSubcommandKind::Review,
        other => StateSubcommandKind::Unknown(other.to_string()),
    }
}

fn parse_ask_mode(body: &mut String) -> AskMode {
    let Some(token) = first_token(body) else {
        return AskMode::Auto;
    };
    let normalized = token.trim_start_matches('-').to_ascii_lowercase();
    let mode = match normalized.as_str() {
        "plan" => AskMode::Plan,
        "apply" => AskMode::Apply,
        "diff" => AskMode::Diff,
        _ => AskMode::Auto,
    };
    if mode != AskMode::Auto {
        *body = body[token.len()..].trim_start().to_string();
    }
    mode
}

fn first_token(value: &str) -> Option<&str> {
    let trimmed = value.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    let end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    Some(&trimmed[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ooc_command() {
        let parsed = parse_chat_command("   /OOC hello");
        assert_eq!(parsed.kind, ChatCommandKind::Ooc);
        assert_eq!(parsed.body, "hello");
    }

    #[test]
    fn parse_setup_command() {
        let parsed = parse_chat_command("/setup keep the chain on");
        assert_eq!(parsed.kind, ChatCommandKind::Setup);
        assert_eq!(parsed.body, "keep the chain on");
    }

    #[test]
    fn parse_state_show_command() {
        let parsed = parse_chat_command("/state show aurora");
        assert_eq!(parsed.kind, ChatCommandKind::State);
        assert_eq!(parsed.state_subcommand, Some(StateSubcommandKind::Show));
        assert_eq!(parsed.body, "show aurora");
    }

    #[test]
    fn parse_state_update_command() {
        let parsed = parse_chat_command("/state update scene door chain stays engaged");
        assert_eq!(parsed.kind, ChatCommandKind::State);
        assert_eq!(parsed.state_subcommand, Some(StateSubcommandKind::Update));
        assert_eq!(parsed.body, "update scene door chain stays engaged");
    }

    #[test]
    fn parse_status_command() {
        let parsed = parse_chat_command("/status aurora");
        assert_eq!(parsed.kind, ChatCommandKind::Status);
        assert_eq!(parsed.state_subcommand, Some(StateSubcommandKind::Show));
        assert_eq!(parsed.body, "aurora");
    }

    #[test]
    fn parse_persona_list_command() {
        let parsed = parse_chat_command("/persona list");
        assert_eq!(parsed.kind, ChatCommandKind::Persona);
        assert_eq!(parsed.persona_subcommand, Some(PersonaSubcommandKind::List));
        assert_eq!(parsed.body, "list");
    }

    #[test]
    fn parse_persona_change_command() {
        let parsed = parse_chat_command("/persona change preset_female");
        assert_eq!(parsed.kind, ChatCommandKind::Persona);
        assert_eq!(
            parsed.persona_subcommand,
            Some(PersonaSubcommandKind::Change)
        );
        assert_eq!(parsed.body, "change preset_female");
    }

    #[test]
    fn parse_ask_command() {
        let parsed = parse_chat_command("/ask update Aurora status");
        assert_eq!(parsed.kind, ChatCommandKind::Ask);
        assert_eq!(parsed.ask_mode, AskMode::Auto);
        assert_eq!(parsed.body, "update Aurora status");
    }

    #[test]
    fn parse_ask_plan_command() {
        let parsed = parse_chat_command("/ask plan update Aurora status");
        assert_eq!(parsed.kind, ChatCommandKind::Ask);
        assert_eq!(parsed.ask_mode, AskMode::Plan);
        assert_eq!(parsed.body, "update Aurora status");
    }

    #[test]
    fn parse_ask_apply_command() {
        let parsed = parse_chat_command("/ask --apply update Aurora status");
        assert_eq!(parsed.kind, ChatCommandKind::Ask);
        assert_eq!(parsed.ask_mode, AskMode::Apply);
        assert_eq!(parsed.body, "update Aurora status");
    }

    #[test]
    fn parse_help_command() {
        let parsed = parse_chat_command("/help");
        assert_eq!(parsed.kind, ChatCommandKind::Help);
        assert_eq!(parsed.body, "");
    }

    #[test]
    fn parse_unknown_slash_command() {
        let parsed = parse_chat_command("/diag now");
        assert_eq!(parsed.kind, ChatCommandKind::Unknown("diag".into()));
        assert_eq!(parsed.body, "now");
    }

    #[test]
    fn non_slash_message_is_none() {
        let parsed = parse_chat_command("OOC: legacy stays outside slash parser");
        assert_eq!(parsed.kind, ChatCommandKind::None);
        assert!(!parsed.detected());
    }
}
