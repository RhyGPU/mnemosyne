use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    compiler::MEMORY_COMPILER_CONTRACT_VERSION,
    memory_v2::{
        DerivedMemoryKind, EpisodicMemoryKind, MemoryEvidenceRef, MemoryLayerV2, MemoryV2Entry,
        MemoryValidity, MEMORY_V2_SCHEMA_VERSION,
    },
    soul::TruthStatus,
};

pub const CONSOLIDATION_MIN_SUPPORT: usize = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConsolidationReport {
    pub proposals: Vec<MemoryV2Entry>,
    pub rejected: Vec<ConsolidationRejection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsolidationRejection {
    pub memory_id: String,
    pub code: String,
    pub message: String,
}

pub fn propose_derived_memories(
    raw_memories: &[MemoryV2Entry],
    created_at_ms: i64,
) -> ConsolidationReport {
    let valid_raw = raw_memories
        .iter()
        .filter(|memory| {
            memory.layer == MemoryLayerV2::Raw && memory.validity == MemoryValidity::Valid
        })
        .collect::<Vec<_>>();
    let mut groups = BTreeMap::<(String, String, EpisodicMemoryKind), Vec<&MemoryV2Entry>>::new();
    for memory in valid_raw {
        let Some(kind) = memory.episodic_kind else {
            continue;
        };
        groups
            .entry((
                memory.conversation_id.clone(),
                memory.branch_id.clone(),
                kind,
            ))
            .or_default()
            .push(memory);
    }

    let mut proposals = Vec::new();
    for ((conversation_id, branch_id, kind), memories) in groups {
        for mut memories in topical_clusters(memories) {
            if memories.len() < CONSOLIDATION_MIN_SUPPORT {
                continue;
            }
            memories.sort_by(|left, right| {
                left.created_at_ms
                    .cmp(&right.created_at_ms)
                    .then_with(|| left.memory_id.cmp(&right.memory_id))
            });
            let source_memory_ids = memories
                .iter()
                .map(|memory| memory.memory_id.clone())
                .collect::<Vec<_>>();
            let derived_kind = derived_kind_for(kind);
            let memory_id = stable_derived_id(
                &conversation_id,
                &branch_id,
                derived_kind_label(derived_kind),
                &source_memory_ids,
            );
            let baseline_polarity = statement_polarity(&memories[0].content);
            let mut supporting_evidence = Vec::new();
            let mut contradicting_evidence = Vec::new();
            for memory in &memories {
                let contradicts = baseline_polarity != 0
                    && statement_polarity(&memory.content) != 0
                    && statement_polarity(&memory.content) != baseline_polarity;
                let evidence = MemoryEvidenceRef {
                    source_memory_id: memory.memory_id.clone(),
                    source_patch_id: memory.source_patch_id.clone(),
                    source_quote: memory.source_quote.clone(),
                    relation: if contradicts {
                        "contradicts".into()
                    } else {
                        "supports".into()
                    },
                };
                if contradicts {
                    contradicting_evidence.push(evidence);
                } else {
                    supporting_evidence.push(evidence);
                }
            }
            let contradiction_penalty = if contradicting_evidence.is_empty() {
                1.0
            } else {
                0.65
            };
            let confidence = (memories.iter().map(|memory| memory.confidence).sum::<f32>()
                / memories.len() as f32
                * evidence_coverage(&memories)
                * contradiction_penalty)
                .clamp(0.0, 0.95);
            let summaries = memories
                .iter()
                .take(3)
                .map(|memory| memory.content.trim())
                .collect::<Vec<_>>()
                .join(" / ");
            let contested = if contradicting_evidence.is_empty() {
                ""
            } else {
                " (contested)"
            };
            proposals.push(MemoryV2Entry {
                schema_version: MEMORY_V2_SCHEMA_VERSION,
                memory_id,
                conversation_id: conversation_id.clone(),
                branch_id: branch_id.clone(),
                owner_entity_id: common_owner(&memories),
                layer: MemoryLayerV2::Derived,
                episodic_kind: None,
                derived_kind: Some(derived_kind),
                content: format!(
                    "{} pattern{} grounded in {} memories: {}",
                    derived_kind_label(derived_kind),
                    contested,
                    memories.len(),
                    summaries
                ),
                source_patch_id: None,
                source_turn_id: None,
                source_message_id: None,
                source_entity_id: None,
                source_quote: None,
                source_memory_ids: source_memory_ids.clone(),
                supporting_evidence: supporting_evidence.clone(),
                contradicting_evidence: contradicting_evidence.clone(),
                confidence,
                truth_status: TruthStatus::CharacterBelief,
                validity: MemoryValidity::Valid,
                compiler_version: MEMORY_COMPILER_CONTRACT_VERSION,
                created_at_ms,
            });

            if let (Some(owner), Some(counterpart)) =
                (common_owner(&memories), common_source_entity(&memories))
            {
                if owner != counterpart
                    && matches!(
                        kind,
                        EpisodicMemoryKind::Testimony
                            | EpisodicMemoryKind::Perception
                            | EpisodicMemoryKind::Affect
                    )
                {
                    proposals.push(MemoryV2Entry {
                        schema_version: MEMORY_V2_SCHEMA_VERSION,
                        memory_id: stable_derived_id(
                            &conversation_id,
                            &branch_id,
                            "relationship_model",
                            &source_memory_ids,
                        ),
                        conversation_id: conversation_id.clone(),
                        branch_id: branch_id.clone(),
                        owner_entity_id: Some(owner.clone()),
                        layer: MemoryLayerV2::Derived,
                        episodic_kind: None,
                        derived_kind: Some(DerivedMemoryKind::RelationshipModel),
                        content: format!(
                            "Relationship evidence between {owner} and {counterpart} is grounded in {} memories{}.",
                            memories.len(),
                            contested
                        ),
                        source_patch_id: None,
                        source_turn_id: None,
                        source_message_id: None,
                        source_entity_id: Some(counterpart),
                        source_quote: None,
                        source_memory_ids: source_memory_ids.clone(),
                        supporting_evidence: supporting_evidence.clone(),
                        contradicting_evidence: contradicting_evidence.clone(),
                        confidence,
                        truth_status: TruthStatus::CharacterBelief,
                        validity: MemoryValidity::Valid,
                        compiler_version: MEMORY_COMPILER_CONTRACT_VERSION,
                        created_at_ms,
                    });
                }
            }
        }
    }
    validate_consolidation_proposals(raw_memories, proposals)
}

pub fn validate_consolidation_proposals(
    raw_memories: &[MemoryV2Entry],
    proposals: Vec<MemoryV2Entry>,
) -> ConsolidationReport {
    let sources = raw_memories
        .iter()
        .map(|memory| (memory.memory_id.as_str(), memory))
        .collect::<HashMap<_, _>>();
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    let mut seen = HashSet::new();
    for proposal in proposals {
        let result = proposal.validate().and_then(|_| {
            if !seen.insert(proposal.memory_id.clone()) {
                return Err("duplicate derived memory id".into());
            }
            if proposal.source_memory_ids.len() < CONSOLIDATION_MIN_SUPPORT {
                return Err("derived memory has insufficient source coverage".into());
            }
            for source_id in &proposal.source_memory_ids {
                let source = sources
                    .get(source_id.as_str())
                    .ok_or_else(|| format!("unknown source memory {source_id}"))?;
                if source.layer != MemoryLayerV2::Raw
                    || source.validity != MemoryValidity::Valid
                    || source.conversation_id != proposal.conversation_id
                    || source.branch_id != proposal.branch_id
                {
                    return Err(format!("invalid or cross-branch source memory {source_id}"));
                }
            }
            let evidence_ids = proposal
                .supporting_evidence
                .iter()
                .chain(proposal.contradicting_evidence.iter())
                .map(|evidence| evidence.source_memory_id.as_str())
                .collect::<HashSet<_>>();
            if proposal
                .source_memory_ids
                .iter()
                .any(|source_id| !evidence_ids.contains(source_id.as_str()))
            {
                return Err("every source memory must have an evidence edge".into());
            }
            Ok(())
        });
        match result {
            Ok(()) => accepted.push(proposal),
            Err(message) => rejected.push(ConsolidationRejection {
                memory_id: proposal.memory_id,
                code: "invalid_consolidation_proposal".into(),
                message,
            }),
        }
    }
    ConsolidationReport {
        proposals: accepted,
        rejected,
    }
}

fn derived_kind_for(kind: EpisodicMemoryKind) -> DerivedMemoryKind {
    match kind {
        EpisodicMemoryKind::Episode | EpisodicMemoryKind::Perception => DerivedMemoryKind::Schema,
        EpisodicMemoryKind::Testimony => DerivedMemoryKind::Belief,
        EpisodicMemoryKind::Affect => DerivedMemoryKind::SelfModel,
        EpisodicMemoryKind::Intention => DerivedMemoryKind::Reflection,
    }
}

fn common_owner(memories: &[&MemoryV2Entry]) -> Option<String> {
    let first = memories.first()?.owner_entity_id.clone();
    memories
        .iter()
        .all(|memory| memory.owner_entity_id == first)
        .then_some(first)
        .flatten()
}

fn common_source_entity(memories: &[&MemoryV2Entry]) -> Option<String> {
    let first = memories.first()?.source_entity_id.clone();
    memories
        .iter()
        .all(|memory| memory.source_entity_id == first)
        .then_some(first)
        .flatten()
}

fn topical_clusters(mut memories: Vec<&MemoryV2Entry>) -> Vec<Vec<&MemoryV2Entry>> {
    memories.sort_by(|left, right| left.memory_id.cmp(&right.memory_id));
    let tokens = memories
        .iter()
        .map(|memory| topic_tokens(&memory.content))
        .collect::<Vec<_>>();
    let mut visited = vec![false; memories.len()];
    let mut clusters = Vec::new();
    for start in 0..memories.len() {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut pending = vec![start];
        let mut component = Vec::new();
        while let Some(index) = pending.pop() {
            component.push(memories[index]);
            for candidate in 0..memories.len() {
                if !visited[candidate] && !tokens[index].is_disjoint(&tokens[candidate]) {
                    visited[candidate] = true;
                    pending.push(candidate);
                }
            }
        }
        clusters.push(component);
    }
    clusters
}

fn topic_tokens(content: &str) -> HashSet<String> {
    const STOPWORDS: &[&str] = &[
        "about",
        "after",
        "again",
        "also",
        "been",
        "before",
        "being",
        "from",
        "have",
        "into",
        "just",
        "more",
        "says",
        "that",
        "their",
        "there",
        "these",
        "they",
        "this",
        "through",
        "visitor",
        "guard",
        "reports",
        "remains",
        "with",
        "그리고",
        "그러나",
        "대한",
        "에서",
        "으로",
        "있다",
        "했다",
    ];
    content
        .to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| token.chars().count() >= 3 && !STOPWORDS.contains(token))
        .map(str::to_string)
        .collect()
}

fn statement_polarity(content: &str) -> i8 {
    let normalized = content.to_lowercase();
    const NEGATIVE: &[&str] = &[
        " not ",
        "never",
        "unlocked",
        "open instead",
        "false",
        "incorrect",
        "아니",
        "않",
        "없",
        "해제",
        "거짓",
    ];
    if NEGATIVE.iter().any(|marker| normalized.contains(marker)) {
        -1
    } else {
        1
    }
}

fn evidence_coverage(memories: &[&MemoryV2Entry]) -> f32 {
    let grounded = memories
        .iter()
        .filter(|memory| memory.source_patch_id.is_some() || memory.source_quote.is_some())
        .count();
    (grounded as f32 / memories.len() as f32).max(0.5)
}

fn derived_kind_label(kind: DerivedMemoryKind) -> &'static str {
    match kind {
        DerivedMemoryKind::Belief => "Belief",
        DerivedMemoryKind::Schema => "Schema",
        DerivedMemoryKind::RelationshipModel => "Relationship model",
        DerivedMemoryKind::SelfModel => "Self model",
        DerivedMemoryKind::Reflection => "Reflection",
    }
}

fn stable_derived_id(
    conversation_id: &str,
    branch_id: &str,
    kind: &str,
    sources: &[String],
) -> String {
    let mut hash = 1469598103934665603_u64;
    for part in std::iter::once(conversation_id)
        .chain(std::iter::once(branch_id))
        .chain(std::iter::once(kind))
        .chain(sources.iter().map(String::as_str))
    {
        for byte in part.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(1099511628211);
        }
    }
    format!("derived-{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(id: &str) -> MemoryV2Entry {
        MemoryV2Entry {
            schema_version: MEMORY_V2_SCHEMA_VERSION,
            memory_id: id.into(),
            conversation_id: "conversation".into(),
            branch_id: "branch".into(),
            owner_entity_id: Some("aurora".into()),
            layer: MemoryLayerV2::Raw,
            episodic_kind: Some(EpisodicMemoryKind::Testimony),
            derived_kind: None,
            content: format!("Evidence {id}"),
            source_patch_id: Some(format!("patch-{id}")),
            source_turn_id: Some(format!("turn-{id}")),
            source_message_id: None,
            source_entity_id: Some("player".into()),
            source_quote: Some(format!("quote {id}")),
            source_memory_ids: Vec::new(),
            supporting_evidence: Vec::new(),
            contradicting_evidence: Vec::new(),
            confidence: 0.8,
            truth_status: TruthStatus::CharacterBelief,
            validity: MemoryValidity::Valid,
            compiler_version: 2,
            created_at_ms: 1,
        }
    }

    #[test]
    fn consolidation_is_deterministic_and_evidence_complete() {
        let inputs = vec![raw("b"), raw("a")];
        let first = propose_derived_memories(&inputs, 10);
        let replay = propose_derived_memories(&inputs, 10);
        assert_eq!(first, replay);
        let belief = first
            .proposals
            .iter()
            .find(|proposal| proposal.derived_kind == Some(DerivedMemoryKind::Belief))
            .expect("belief");
        assert_eq!(belief.source_memory_ids, vec!["a", "b"]);
        assert_eq!(belief.supporting_evidence.len(), 2);
        assert!(first
            .proposals
            .iter()
            .any(|proposal| proposal.derived_kind == Some(DerivedMemoryKind::RelationshipModel)));
    }

    #[test]
    fn cross_branch_source_is_rejected() {
        let inputs = vec![raw("a"), raw("b")];
        let mut proposal = propose_derived_memories(&inputs, 10).proposals.remove(0);
        proposal.branch_id = "other".into();
        let report = validate_consolidation_proposals(&inputs, vec![proposal]);
        assert!(report.proposals.is_empty());
        assert_eq!(report.rejected.len(), 1);
    }

    #[test]
    fn unrelated_memories_are_not_consolidated() {
        let mut first = raw("a");
        first.content = "The northern gate is locked.".into();
        let mut second = raw("b");
        second.content = "Mira prefers jasmine tea.".into();
        let report = propose_derived_memories(&[first, second], 10);
        assert!(report.proposals.is_empty());
    }

    #[test]
    fn explicit_counterevidence_is_retained() {
        let mut first = raw("a");
        first.content = "The northern gate is locked.".into();
        let mut second = raw("b");
        second.content = "The northern gate is not locked.".into();
        let report = propose_derived_memories(&[first, second], 10);
        let belief = report
            .proposals
            .iter()
            .find(|proposal| proposal.derived_kind == Some(DerivedMemoryKind::Belief))
            .expect("contested belief");
        assert_eq!(belief.supporting_evidence.len(), 1);
        assert_eq!(belief.contradicting_evidence.len(), 1);
        assert!(belief.content.contains("contested"));
    }
}
