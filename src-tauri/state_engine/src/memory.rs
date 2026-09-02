use uuid::Uuid;

use crate::soul::{current_timestamp, MemoryEntry, MemorySourceType, Soul, SpeechAct, TruthStatus};

pub trait Embedder {
    fn embed(&self, text: &str) -> Vec<f32>;
}

#[derive(Debug, Default)]
pub struct LexicalHashEmbedder;

impl Embedder for LexicalHashEmbedder {
    fn embed(&self, text: &str) -> Vec<f32> {
        const DIMS: usize = 32;
        let mut vector = vec![0.0; DIMS];
        for token in text
            .split(|character: char| !character.is_alphanumeric())
            .filter(|token| !token.is_empty())
        {
            let mut hash = 2166136261u32;
            for byte in token.to_lowercase().bytes() {
                hash ^= byte as u32;
                hash = hash.wrapping_mul(16777619);
            }
            vector[(hash as usize) % DIMS] += 1.0;
        }
        normalize(&mut vector);
        vector
    }
}

#[derive(Debug)]
pub struct MemoryScorer<E: Embedder> {
    pub embedder: E,
}

impl Default for MemoryScorer<LexicalHashEmbedder> {
    fn default() -> Self {
        Self {
            embedder: LexicalHashEmbedder,
        }
    }
}

impl<E: Embedder> MemoryScorer<E> {
    pub fn score(&self, soul: &Soul, new_memory: &MemoryEntry) -> f32 {
        let emotional_score = emotional_score(&new_memory.tag);
        let novelty_score = self.novelty_score(soul, &new_memory.content);
        let goal_score = compute_goal_relevance(soul, &new_memory.tag);
        let similar_count = count_similar_memories(soul, new_memory);
        let repetition_discount = 1.0 / (1.0 + similar_count as f32 * 0.3);
        let raw_score = (emotional_score * 0.4) + (novelty_score * 0.3) + (goal_score * 0.3);

        (raw_score * repetition_discount).clamp(0.0, 1.0)
    }

    fn novelty_score(&self, soul: &Soul, content: &str) -> f32 {
        if soul.memory.recent.is_empty() && soul.memory.core.is_empty() {
            return 1.0;
        }

        let new_embedding = self.embedder.embed(content);
        let mut average = vec![0.0; new_embedding.len()];
        let mut count = 0.0f32;

        for memory in &soul.memory.recent {
            add_embedding(&mut average, &self.embedder.embed(&memory.content));
            count += 1.0;
        }
        for memory in &soul.memory.core {
            add_embedding(&mut average, &self.embedder.embed(memory));
            count += 1.0;
        }

        if count == 0.0 {
            return 1.0;
        }

        for value in &mut average {
            *value /= count;
        }
        normalize(&mut average);

        (1.0 - cosine_similarity(&new_embedding, &average)).clamp(0.0, 1.0)
    }
}

pub fn create_scored_memory(soul: &Soul, content: &str, tag: &str) -> MemoryEntry {
    create_scored_memory_at(soul, content, tag, current_timestamp())
}

/// Same, with the creation time supplied by the caller.
///
/// Ledger replay must be a pure function of the ledger, so rebuilding a session
/// stamps memories with the turn's recorded time instead of the wall clock. With
/// `current_timestamp()` here, two rebuilds that straddle a second boundary
/// produced different `created_at_ms` and the projection stopped being
/// replay-equivalent.
pub fn create_scored_memory_at(
    soul: &Soul,
    content: &str,
    tag: &str,
    created_at: u64,
) -> MemoryEntry {
    let mut memory = MemoryEntry {
        archived: false,
        is_pinned: false,
        id: format!("mem_{}", Uuid::new_v4()),
        timestamp: created_at,
        content: content.trim().to_string(),
        salience: 50.0,
        tag: tag.trim().to_string(),
        retrieval_strength: 50.0,
        source_type: MemorySourceType::CurrentSession,
        source_session_id: None,
        source_conversation_id: None,
        source_message_id: None,
        source_entity_id: None,
        source_quote: None,
        is_lived_experience: true,
        is_imported_context: false,
        perceived_by_entity_id: None,
        target_entity_ids: Vec::new(),
        interpretation: None,
        confidence: None,
        objective_event_id: None,
        truth_status: TruthStatus::Unknown,
        architecture_verified: false,
        memory_slot: None,
        owner_soul_id: None,
        relevance_tags: Default::default(),
        knowledge_scope: None,
        speech_act: SpeechAct::Unspecified,
        is_active: true,
        invalidated_by_patch_id: None,
        superseded_by_memory_id: None,
        is_retconned: false,
    };
    let score = MemoryScorer::default().score(soul, &memory);
    let salience = (score * 100.0).round();
    memory.salience = salience;
    memory.retrieval_strength = salience;
    memory
}

/// Pin or unpin a stored memory. Pinning also restores an archived memory to
/// the active pool, since a pinned memory must be retrievable. Returns false
/// if no memory with the given id exists.
pub fn set_memory_pinned(soul: &mut Soul, memory_id: &str, pinned: bool) -> bool {
    let Some(memory) = soul
        .memory
        .recent
        .iter_mut()
        .find(|memory| memory.id == memory_id)
    else {
        return false;
    };
    memory.is_pinned = pinned;
    if pinned && memory.archived {
        memory.archived = false;
        memory.is_active = true;
    }
    true
}

/// Restore a cap-evicted (archived) memory to the active pool. Memories that
/// were invalidated or retconned (inactive but not archived) are not touched —
/// those represent corrections, not evictions. Returns false if no archived
/// memory with the given id exists.
pub fn restore_archived_memory(soul: &mut Soul, memory_id: &str) -> bool {
    let Some(memory) = soul
        .memory
        .recent
        .iter_mut()
        .find(|memory| memory.id == memory_id && memory.archived)
    else {
        return false;
    };
    memory.archived = false;
    memory.is_active = true;
    true
}

fn emotional_score(tag: &str) -> f32 {
    match tag {
        "identity_violation" | "betrayal" | "near_death" => 0.95,
        "trauma_trigger" | "control_gain" | "trust_break" => 0.85,
        "bonding" | "trust_building" | "intimacy" | "compassion" => 0.75,
        "introduction" | "dynamic_establishment" | "orientation" => 0.60,
        "boundary_setting" | "conflict_minor" => 0.50,
        "routine" | "small_talk" | "observation" => 0.30,
        _ => 0.50,
    }
}

fn compute_goal_relevance(soul: &Soul, tag: &str) -> f32 {
    match tag {
        "bonding" | "trust_building" | "intimacy" | "compassion" => {
            let belong = soul.global.maslow.get(2).copied().unwrap_or(50.0);
            (1.0 - belong / 100.0).clamp(0.3, 1.0)
        }
        "threat" | "danger" | "fear" => {
            let safety = soul.global.maslow.get(1).copied().unwrap_or(50.0);
            (1.0 - safety / 100.0).clamp(0.3, 1.0)
        }
        _ => 0.5,
    }
}

fn count_similar_memories(soul: &Soul, new_memory: &MemoryEntry) -> usize {
    let new_tokens = token_set(&new_memory.content);
    soul.memory
        .recent
        .iter()
        .filter(|memory| memory.tag == new_memory.tag)
        .filter(|memory| jaccard(&new_tokens, &token_set(&memory.content)) > 0.35)
        .count()
}

fn add_embedding(target: &mut [f32], source: &[f32]) {
    for (target, source) in target.iter_mut().zip(source) {
        *target += *source;
    }
}

fn normalize(vector: &mut [f32]) {
    let magnitude = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if magnitude == 0.0 {
        return;
    }
    for value in vector {
        *value /= magnitude;
    }
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}

fn token_set(text: &str) -> Vec<String> {
    let mut tokens = text
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| token.len() > 2)
        .map(|token| token.to_lowercase())
        .collect::<Vec<_>>();
    tokens.sort();
    tokens.dedup();
    tokens
}

fn jaccard(left: &[String], right: &[String]) -> f32 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }

    let intersection = left.iter().filter(|token| right.contains(token)).count();
    let union = left.len() + right.len() - intersection;
    intersection as f32 / union as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soul::new_default_soul;

    #[test]
    fn scoring_rewards_goal_relevant_memory() {
        let mut soul = new_default_soul("Aurora");
        soul.global.maslow[2] = 5.0;
        let memory = MemoryEntry {
            archived: false,
            is_pinned: false,
            id: "mem".into(),
            timestamp: 1,
            content: "Aurora accepts a careful promise and feels less alone.".into(),
            salience: 50.0,
            tag: "trust_building".into(),
            retrieval_strength: 50.0,
            source_type: MemorySourceType::CurrentSession,
            source_session_id: None,
            source_conversation_id: None,
            source_message_id: None,
            source_entity_id: None,
            source_quote: None,
            is_lived_experience: true,
            is_imported_context: false,
            perceived_by_entity_id: None,
            target_entity_ids: Vec::new(),
            interpretation: None,
            confidence: None,
            objective_event_id: None,
            truth_status: TruthStatus::Unknown,
            architecture_verified: false,
            memory_slot: None,
            owner_soul_id: None,
            relevance_tags: Default::default(),
            knowledge_scope: None,
            speech_act: SpeechAct::Unspecified,
            is_active: true,
            invalidated_by_patch_id: None,
            superseded_by_memory_id: None,
            is_retconned: false,
        };

        let score = MemoryScorer::default().score(&soul, &memory);
        assert!(score > 0.60);
    }

    #[test]
    fn repetition_discount_reduces_duplicate_score() {
        let mut soul = new_default_soul("Aurora");
        soul.memory.recent.push(MemoryEntry {
            archived: false,
            is_pinned: false,
            id: "old".into(),
            timestamp: 1,
            content: "Aurora accepts a careful promise from the user.".into(),
            salience: 75.0,
            tag: "trust_building".into(),
            retrieval_strength: 75.0,
            source_type: MemorySourceType::CurrentSession,
            source_session_id: None,
            source_conversation_id: None,
            source_message_id: None,
            source_entity_id: None,
            source_quote: None,
            is_lived_experience: true,
            is_imported_context: false,
            perceived_by_entity_id: None,
            target_entity_ids: Vec::new(),
            interpretation: None,
            confidence: None,
            objective_event_id: None,
            truth_status: TruthStatus::Unknown,
            architecture_verified: false,
            memory_slot: None,
            owner_soul_id: None,
            relevance_tags: Default::default(),
            knowledge_scope: None,
            speech_act: SpeechAct::Unspecified,
            is_active: true,
            invalidated_by_patch_id: None,
            superseded_by_memory_id: None,
            is_retconned: false,
        });

        let duplicate = MemoryEntry {
            archived: false,
            is_pinned: false,
            id: "new".into(),
            timestamp: 2,
            content: "Aurora accepts a careful promise from the user again.".into(),
            salience: 50.0,
            tag: "trust_building".into(),
            retrieval_strength: 50.0,
            source_type: MemorySourceType::CurrentSession,
            source_session_id: None,
            source_conversation_id: None,
            source_message_id: None,
            source_entity_id: None,
            source_quote: None,
            is_lived_experience: true,
            is_imported_context: false,
            perceived_by_entity_id: None,
            target_entity_ids: Vec::new(),
            interpretation: None,
            confidence: None,
            objective_event_id: None,
            truth_status: TruthStatus::Unknown,
            architecture_verified: false,
            memory_slot: None,
            owner_soul_id: None,
            relevance_tags: Default::default(),
            knowledge_scope: None,
            speech_act: SpeechAct::Unspecified,
            is_active: true,
            invalidated_by_patch_id: None,
            superseded_by_memory_id: None,
            is_retconned: false,
        };

        let fresh = MemoryEntry {
            archived: false,
            is_pinned: false,
            id: "fresh".into(),
            timestamp: 2,
            content: "A hidden map reveals a route through the service tunnels.".into(),
            salience: 50.0,
            tag: "orientation".into(),
            retrieval_strength: 50.0,
            source_type: MemorySourceType::CurrentSession,
            source_session_id: None,
            source_conversation_id: None,
            source_message_id: None,
            source_entity_id: None,
            source_quote: None,
            is_lived_experience: true,
            is_imported_context: false,
            perceived_by_entity_id: None,
            target_entity_ids: Vec::new(),
            interpretation: None,
            confidence: None,
            objective_event_id: None,
            truth_status: TruthStatus::Unknown,
            architecture_verified: false,
            memory_slot: None,
            owner_soul_id: None,
            relevance_tags: Default::default(),
            knowledge_scope: None,
            speech_act: SpeechAct::Unspecified,
            is_active: true,
            invalidated_by_patch_id: None,
            superseded_by_memory_id: None,
            is_retconned: false,
        };

        let scorer = MemoryScorer::default();
        assert!(scorer.score(&soul, &duplicate) < scorer.score(&soul, &fresh));
    }

    #[test]
    fn pinning_restores_archived_memory() {
        let mut soul = new_default_soul("Aurora");
        let mut memory = create_scored_memory(&soul, "Aurora keeps the brass key.", "orientation");
        memory.id = "mem_pin".into();
        memory.archived = true;
        memory.is_active = false;
        soul.memory.recent.push(memory);

        assert!(set_memory_pinned(&mut soul, "mem_pin", true));
        let memory = &soul.memory.recent[0];
        assert!(memory.is_pinned);
        assert!(!memory.archived);
        assert!(memory.is_active);

        assert!(set_memory_pinned(&mut soul, "mem_pin", false));
        assert!(!soul.memory.recent[0].is_pinned);
        assert!(!set_memory_pinned(&mut soul, "missing", true));
    }

    #[test]
    fn restore_only_touches_archived_memories() {
        let mut soul = new_default_soul("Aurora");
        let mut archived = create_scored_memory(&soul, "Aurora mapped the cellar.", "orientation");
        archived.id = "mem_archived".into();
        archived.archived = true;
        archived.is_active = false;
        let mut invalidated =
            create_scored_memory(&soul, "Aurora misread the letter.", "orientation");
        invalidated.id = "mem_invalidated".into();
        invalidated.is_active = false;
        soul.memory.recent.push(archived);
        soul.memory.recent.push(invalidated);

        assert!(restore_archived_memory(&mut soul, "mem_archived"));
        assert!(soul.memory.recent[0].is_active);
        assert!(!soul.memory.recent[0].archived);

        // Invalidated (inactive, not archived) memories represent corrections
        // and must not be resurrected by the archive-restore path.
        assert!(!restore_archived_memory(&mut soul, "mem_invalidated"));
        assert!(!soul.memory.recent[1].is_active);
    }
}
