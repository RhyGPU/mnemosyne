use serde::{Deserialize, Serialize};

use crate::soul::{MemoryEntry, MemorySourceType, SpeechAct, TruthStatus};

pub const MEMORY_V2_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryLayerV2 {
    Raw,
    Derived,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EpisodicMemoryKind {
    Episode,
    Testimony,
    Perception,
    Affect,
    Intention,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DerivedMemoryKind {
    Belief,
    Schema,
    RelationshipModel,
    SelfModel,
    Reflection,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryValidity {
    Valid,
    Stale,
    Superseded,
    Invalidated,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MemoryEvidenceRef {
    pub source_memory_id: String,
    pub source_patch_id: Option<String>,
    pub source_quote: Option<String>,
    pub relation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MemoryV2Entry {
    pub schema_version: u32,
    pub memory_id: String,
    pub conversation_id: String,
    pub branch_id: String,
    pub owner_entity_id: Option<String>,
    pub layer: MemoryLayerV2,
    pub episodic_kind: Option<EpisodicMemoryKind>,
    pub derived_kind: Option<DerivedMemoryKind>,
    pub content: String,
    pub source_patch_id: Option<String>,
    pub source_turn_id: Option<String>,
    pub source_message_id: Option<i64>,
    pub source_entity_id: Option<String>,
    pub source_quote: Option<String>,
    pub source_memory_ids: Vec<String>,
    pub supporting_evidence: Vec<MemoryEvidenceRef>,
    pub contradicting_evidence: Vec<MemoryEvidenceRef>,
    pub confidence: f32,
    pub truth_status: TruthStatus,
    pub validity: MemoryValidity,
    pub compiler_version: u32,
    pub created_at_ms: i64,
}

impl MemoryV2Entry {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != MEMORY_V2_SCHEMA_VERSION {
            return Err(format!(
                "unsupported Memory V2 schema {}",
                self.schema_version
            ));
        }
        if self.memory_id.trim().is_empty()
            || self.conversation_id.trim().is_empty()
            || self.branch_id.trim().is_empty()
            || self.content.trim().is_empty()
        {
            return Err("memory identity, branch, conversation, and content are required".into());
        }
        if !self.confidence.is_finite() || !(0.0..=1.0).contains(&self.confidence) {
            return Err("memory confidence must be finite and within 0..=1".into());
        }
        match self.layer {
            MemoryLayerV2::Raw if self.episodic_kind.is_none() || self.derived_kind.is_some() => {
                return Err("raw memory requires only an episodic kind".into());
            }
            MemoryLayerV2::Derived
                if self.derived_kind.is_none()
                    || self.episodic_kind.is_some()
                    || self.source_memory_ids.is_empty() =>
            {
                return Err(
                    "derived memory requires only a derived kind and source memory ids".into(),
                );
            }
            _ => {}
        }
        if self.layer == MemoryLayerV2::Derived
            && self.supporting_evidence.is_empty()
            && self.contradicting_evidence.is_empty()
        {
            return Err("derived memory must retain supporting or contradicting evidence".into());
        }
        Ok(())
    }
}

pub fn episodic_kind_for_legacy(memory: &MemoryEntry) -> EpisodicMemoryKind {
    let tag = memory.tag.to_ascii_lowercase();
    let slot = memory
        .memory_slot
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    if slot.contains("intention") || tag.contains("intention") || tag.contains("plan") {
        EpisodicMemoryKind::Intention
    } else if tag.contains("affect")
        || tag.contains("emotion")
        || tag.contains("fear")
        || tag.contains("desire")
    {
        EpisodicMemoryKind::Affect
    } else if memory.truth_status == TruthStatus::CharacterBelief
        || memory.truth_status == TruthStatus::UserClaimed
        || memory.source_type == MemorySourceType::UserClaimed
    {
        EpisodicMemoryKind::Testimony
    } else if memory.source_quote.is_some()
        || matches!(
            memory.knowledge_scope.as_deref(),
            Some("directly_observed" | "inferred")
        )
    {
        EpisodicMemoryKind::Perception
    } else {
        EpisodicMemoryKind::Episode
    }
}

pub fn project_legacy_memory(
    memory: &MemoryEntry,
    conversation_id: &str,
    branch_id: &str,
    source_patch_id: Option<String>,
    source_turn_id: Option<String>,
    compiler_version: u32,
) -> MemoryV2Entry {
    let validity = if !memory.is_active || memory.is_retconned {
        MemoryValidity::Invalidated
    } else if memory.superseded_by_memory_id.is_some() {
        MemoryValidity::Superseded
    } else {
        MemoryValidity::Valid
    };
    MemoryV2Entry {
        schema_version: MEMORY_V2_SCHEMA_VERSION,
        memory_id: memory.id.clone(),
        conversation_id: conversation_id.to_string(),
        branch_id: branch_id.to_string(),
        owner_entity_id: memory.owner_soul_id.clone(),
        layer: MemoryLayerV2::Raw,
        episodic_kind: Some(episodic_kind_for_legacy(memory)),
        derived_kind: None,
        content: memory.content.clone(),
        source_patch_id,
        source_turn_id,
        source_message_id: memory.source_message_id,
        source_entity_id: memory.source_entity_id.clone(),
        source_quote: memory.source_quote.clone(),
        source_memory_ids: Vec::new(),
        supporting_evidence: Vec::new(),
        contradicting_evidence: Vec::new(),
        confidence: memory.confidence.unwrap_or(0.5).clamp(0.0, 1.0),
        truth_status: memory.truth_status,
        validity,
        compiler_version,
        created_at_ms: i64::try_from(memory.timestamp).unwrap_or(i64::MAX),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn legacy() -> MemoryEntry {
        MemoryEntry {
            id: "memory-1".into(),
            timestamp: 1,
            content: "Aurora heard that the gate was locked.".into(),
            speech_act: SpeechAct::Unspecified,
            salience: 50.0,
            tag: "testimony".into(),
            retrieval_strength: 50.0,
            source_type: MemorySourceType::UserClaimed,
            source_session_id: None,
            source_conversation_id: None,
            source_message_id: Some(7),
            source_entity_id: Some("player".into()),
            source_quote: Some("the gate was locked".into()),
            is_lived_experience: false,
            is_imported_context: false,
            perceived_by_entity_id: Some("aurora".into()),
            target_entity_ids: vec!["gate".into()],
            interpretation: None,
            confidence: Some(0.8),
            objective_event_id: None,
            truth_status: TruthStatus::CharacterBelief,
            architecture_verified: false,
            memory_slot: None,
            owner_soul_id: Some("aurora".into()),
            relevance_tags: HashMap::new(),
            knowledge_scope: Some("heard_about".into()),
            is_active: true,
            invalidated_by_patch_id: None,
            superseded_by_memory_id: None,
            is_retconned: false,
            archived: false,
            is_pinned: false,
        }
    }

    #[test]
    fn testimony_remains_raw_and_does_not_become_world_truth() {
        let projected = project_legacy_memory(
            &legacy(),
            "conversation",
            "branch",
            Some("patch".into()),
            Some("turn".into()),
            2,
        );
        projected.validate().expect("valid projection");
        assert_eq!(projected.layer, MemoryLayerV2::Raw);
        assert_eq!(projected.episodic_kind, Some(EpisodicMemoryKind::Testimony));
        assert_eq!(projected.truth_status, TruthStatus::CharacterBelief);
    }

    #[test]
    fn derived_memory_without_evidence_is_rejected() {
        let mut projected =
            project_legacy_memory(&legacy(), "conversation", "branch", None, None, 2);
        projected.layer = MemoryLayerV2::Derived;
        projected.episodic_kind = None;
        projected.derived_kind = Some(DerivedMemoryKind::Belief);
        projected.source_memory_ids = vec!["memory-1".into()];
        assert!(projected.validate().is_err());
    }
}
