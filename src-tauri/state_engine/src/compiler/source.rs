use serde::{Deserialize, Serialize};

use super::diagnostics::CompilerContractError;

pub const SOURCE_ENVELOPE_SCHEMA_VERSION: u32 = 1;

/// Engine-owned identity for one creating exchange.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceIdentity {
    pub conversation_id: String,
    pub branch_id: String,
    pub turn_id: String,
    pub parent_turn_id: Option<String>,
    pub user_message_id: i64,
    pub assistant_message_id: i64,
    pub assistant_variant_id: Option<i64>,
}

/// Immutable, code-created source authority presented to every compiler stage.
///
/// Fields are private so callers cannot construct an unsealed envelope through
/// normal Rust APIs. Deserialized envelopes must still pass `validate`, which
/// recomputes the source hash.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceEnvelope {
    schema_version: u32,
    identity: SourceIdentity,
    active_soul_ids: Vec<String>,
    user_text: String,
    assistant_text: String,
    parent_state_hash: Option<String>,
    observed_at_ms: i64,
    source_hash: String,
}

impl SourceEnvelope {
    pub fn new(
        identity: SourceIdentity,
        active_soul_ids: Vec<String>,
        user_text: impl Into<String>,
        assistant_text: impl Into<String>,
        parent_state_hash: Option<String>,
        observed_at_ms: i64,
    ) -> Result<Self, CompilerContractError> {
        let mut active_soul_ids = active_soul_ids
            .into_iter()
            .map(|id| id.trim().to_string())
            .collect::<Vec<_>>();
        active_soul_ids.sort();
        active_soul_ids.dedup();
        let mut envelope = Self {
            schema_version: SOURCE_ENVELOPE_SCHEMA_VERSION,
            identity,
            active_soul_ids,
            user_text: user_text.into(),
            assistant_text: assistant_text.into(),
            parent_state_hash,
            observed_at_ms,
            source_hash: String::new(),
        };
        envelope.validate_fields()?;
        envelope.source_hash = envelope.recompute_hash();
        Ok(envelope)
    }

    pub fn validate(&self) -> Result<(), CompilerContractError> {
        self.validate_fields()?;
        let expected = self.recompute_hash();
        if self.source_hash != expected {
            return Err(CompilerContractError::new(
                "source_hash_mismatch",
                format!(
                    "source envelope hash mismatch: expected {expected}, got {}",
                    self.source_hash
                ),
            ));
        }
        Ok(())
    }

    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn identity(&self) -> &SourceIdentity {
        &self.identity
    }

    pub fn active_soul_ids(&self) -> &[String] {
        &self.active_soul_ids
    }

    pub fn user_text(&self) -> &str {
        &self.user_text
    }

    pub fn assistant_text(&self) -> &str {
        &self.assistant_text
    }

    pub fn parent_state_hash(&self) -> Option<&str> {
        self.parent_state_hash.as_deref()
    }

    pub fn observed_at_ms(&self) -> i64 {
        self.observed_at_ms
    }

    pub fn source_hash(&self) -> &str {
        &self.source_hash
    }

    fn validate_fields(&self) -> Result<(), CompilerContractError> {
        if self.schema_version != SOURCE_ENVELOPE_SCHEMA_VERSION {
            return Err(CompilerContractError::new(
                "unsupported_source_schema",
                format!("unsupported source schema {}", self.schema_version),
            ));
        }
        for (field, value) in [
            ("conversation_id", self.identity.conversation_id.as_str()),
            ("branch_id", self.identity.branch_id.as_str()),
            ("turn_id", self.identity.turn_id.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(CompilerContractError::new(
                    "missing_source_identity",
                    format!("{field} must not be empty"),
                ));
            }
        }
        if self.identity.user_message_id <= 0 || self.identity.assistant_message_id <= 0 {
            return Err(CompilerContractError::new(
                "invalid_source_message_id",
                "source message ids must be positive",
            ));
        }
        if self.active_soul_ids.is_empty()
            || self.active_soul_ids.iter().any(|id| id.trim().is_empty())
        {
            return Err(CompilerContractError::new(
                "invalid_active_souls",
                "at least one non-empty active soul id is required",
            ));
        }
        if self.user_text.trim().is_empty() && self.assistant_text.trim().is_empty() {
            return Err(CompilerContractError::new(
                "empty_source_exchange",
                "source exchange cannot be entirely empty",
            ));
        }
        Ok(())
    }

    fn recompute_hash(&self) -> String {
        let mut parts = vec![
            self.schema_version.to_string(),
            self.identity.conversation_id.clone(),
            self.identity.branch_id.clone(),
            self.identity.turn_id.clone(),
            self.identity.parent_turn_id.clone().unwrap_or_default(),
            self.identity.user_message_id.to_string(),
            self.identity.assistant_message_id.to_string(),
            self.identity
                .assistant_variant_id
                .map(|value| value.to_string())
                .unwrap_or_default(),
            self.observed_at_ms.to_string(),
            self.parent_state_hash.clone().unwrap_or_default(),
            self.user_text.clone(),
            self.assistant_text.clone(),
        ];
        parts.extend(self.active_soul_ids.iter().cloned());
        stable_digest("source_envelope", parts.iter().map(String::as_str))
    }
}

pub(crate) fn stable_digest<'a>(
    namespace: &str,
    parts: impl IntoIterator<Item = &'a str>,
) -> String {
    let mut hash: u64 = 1469598103934665603;
    hash_bytes(&mut hash, namespace.as_bytes());
    for part in parts {
        hash_bytes(&mut hash, &(part.len() as u64).to_le_bytes());
        hash_bytes(&mut hash, part.as_bytes());
    }
    format!("fnv1a64:{hash:016x}")
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= *byte as u64;
        *hash = hash.wrapping_mul(1099511628211);
    }
}
