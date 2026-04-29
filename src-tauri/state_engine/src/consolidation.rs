use std::collections::HashMap;

use crate::{
    memory::MemoryScorer,
    soul::{current_timestamp, MemoryEntry, SchemaEntry, Soul},
};

pub fn consolidate_soul(soul: &mut Soul) {
    let scorer = MemoryScorer::default();
    let mut middle_by_tag: HashMap<String, Vec<MemoryEntry>> = HashMap::new();
    let mut retained = Vec::new();
    let memories = std::mem::take(&mut soul.memory.recent);

    for memory in memories {
        if memory.retrieval_strength <= 30.0 {
            continue;
        }

        let score = scorer.score(soul, &memory);
        if score > 0.70 {
            soul.memory.core.push(summarize_core_memory(&memory));
        } else if score < 0.30 {
            continue;
        } else {
            middle_by_tag
                .entry(memory.tag.clone())
                .or_default()
                .push(memory.clone());
            retained.push(memory);
        }
    }

    for (tag, memories) in middle_by_tag {
        if memories.len() >= 3 {
            merge_schema(soul, &tag, &memories);
        }
    }

    retained.sort_by(|left, right| {
        right
            .salience
            .partial_cmp(&left.salience)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    retained.truncate(4);
    soul.memory.recent = retained;
    soul.turns_since_consolidation = 0;
    soul.last_updated = current_timestamp() as i64;
}

fn summarize_core_memory(memory: &MemoryEntry) -> String {
    format!("{}: {}", title_case_tag(&memory.tag), memory.content)
}

fn merge_schema(soul: &mut Soul, tag: &str, memories: &[MemoryEntry]) {
    let summary = format!(
        "{} recurring pattern across {} memories, most recently: {}",
        title_case_tag(tag),
        memories.len(),
        memories
            .last()
            .map(|memory| memory.content.as_str())
            .unwrap_or("unspecified")
    );

    if let Some(existing) = soul
        .memory
        .schemas
        .iter_mut()
        .find(|schema| schema.schema_type == tag)
    {
        existing.count += memories.len() as u64;
        existing.summary = summary;
    } else {
        soul.memory.schemas.push(SchemaEntry {
            schema_type: tag.to_string(),
            summary,
            count: memories.len() as u64,
        });
    }
}

fn title_case_tag(tag: &str) -> String {
    tag.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soul::new_default_soul;

    #[test]
    fn consolidation_promotes_discards_and_merges() {
        let mut soul = new_default_soul("Aurora");
        soul.memory.core.clear();
        for index in 0..4 {
            soul.memory.recent.push(MemoryEntry {
                id: format!("mid_{index}"),
                timestamp: index,
                content: format!("The room repeats a routine check number {index}."),
                salience: 50.0,
                tag: "routine".into(),
                retrieval_strength: 50.0,
            });
        }
        soul.memory.recent.push(MemoryEntry {
            id: "strong".into(),
            timestamp: 10,
            content: "Aurora survives a near death confrontation and chooses to keep moving."
                .into(),
            salience: 95.0,
            tag: "near_death".into(),
            retrieval_strength: 95.0,
        });
        soul.memory.recent.push(MemoryEntry {
            id: "weak".into(),
            timestamp: 11,
            content: "A forgettable wall mark is noticed.".into(),
            salience: 10.0,
            tag: "observation".into(),
            retrieval_strength: 10.0,
        });

        consolidate_soul(&mut soul);

        assert!(soul
            .memory
            .core
            .iter()
            .any(|memory| memory.contains("near death")));
        assert!(soul
            .memory
            .schemas
            .iter()
            .any(|schema| schema.schema_type == "routine"));
        assert!(soul.memory.recent.len() <= 4);
        assert!(!soul.memory.recent.iter().any(|memory| memory.id == "weak"));
        assert_eq!(soul.turns_since_consolidation, 0);
    }
}

