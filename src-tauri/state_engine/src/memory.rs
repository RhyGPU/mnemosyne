use uuid::Uuid;

use crate::soul::{current_timestamp, MemoryEntry, Soul};

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
    let mut memory = MemoryEntry {
        id: format!("mem_{}", Uuid::new_v4()),
        timestamp: current_timestamp(),
        content: content.trim().to_string(),
        salience: 50.0,
        tag: tag.trim().to_string(),
        retrieval_strength: 50.0,
    };
    let score = MemoryScorer::default().score(soul, &memory);
    let salience = (score * 100.0).round();
    memory.salience = salience;
    memory.retrieval_strength = salience;
    memory
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
            id: "mem".into(),
            timestamp: 1,
            content: "Aurora accepts a careful promise and feels less alone.".into(),
            salience: 50.0,
            tag: "trust_building".into(),
            retrieval_strength: 50.0,
        };

        let score = MemoryScorer::default().score(&soul, &memory);
        assert!(score > 0.60);
    }

    #[test]
    fn repetition_discount_reduces_duplicate_score() {
        let mut soul = new_default_soul("Aurora");
        soul.memory.recent.push(MemoryEntry {
            id: "old".into(),
            timestamp: 1,
            content: "Aurora accepts a careful promise from the user.".into(),
            salience: 75.0,
            tag: "trust_building".into(),
            retrieval_strength: 75.0,
        });

        let duplicate = MemoryEntry {
            id: "new".into(),
            timestamp: 2,
            content: "Aurora accepts a careful promise from the user again.".into(),
            salience: 50.0,
            tag: "trust_building".into(),
            retrieval_strength: 50.0,
        };

        let fresh = MemoryEntry {
            id: "fresh".into(),
            timestamp: 2,
            content: "A hidden map reveals a route through the service tunnels.".into(),
            salience: 50.0,
            tag: "orientation".into(),
            retrieval_strength: 50.0,
        };

        let scorer = MemoryScorer::default();
        assert!(scorer.score(&soul, &duplicate) < scorer.score(&soul, &fresh));
    }
}

