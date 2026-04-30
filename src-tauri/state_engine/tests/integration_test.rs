use state_engine::{
    consolidation::consolidate_soul,
    context_compiler::{compile_context_for_messages, ContextMessage},
    hidden_state::HiddenState,
    soul::new_default_soul,
};

#[test]
fn full_ten_turn_session_triggers_consolidation() {
    let mut soul = new_default_soul("Aurora");
    let turns = [
        (
            "A promise lands carefully.",
            "trust_building",
            3.0,
            1.0,
            "A safety promise changed the emotional pressure.",
        ),
        (
            "The room becomes easier to map.",
            "orientation",
            1.0,
            0.5,
            "The room gained clearer spatial definition.",
        ),
        (
            "A shared childhood memory warms the exchange.",
            "bonding",
            1.0,
            3.0,
            "A shared memory created a warmer bond.",
        ),
        (
            "Possible danger tightens her attention.",
            "threat",
            0.0,
            0.0,
            "The scene tightened around a possible danger.",
        ),
        (
            "A neutral detail settles into the scene.",
            "observation",
            1.0,
            1.0,
            "The conversation continued without rupture.",
        ),
        (
            "A second neutral detail repeats the rhythm.",
            "observation",
            1.0,
            1.0,
            "Another neutral exchange added texture.",
        ),
        (
            "A third neutral detail becomes a pattern.",
            "observation",
            1.0,
            1.0,
            "A repeated neutral exchange formed a pattern.",
        ),
        (
            "Trust is tested again by a steady voice.",
            "trust_building",
            3.0,
            1.0,
            "A second safety promise reinforced trust.",
        ),
        (
            "The scene orientation sharpens again.",
            "orientation",
            1.0,
            0.5,
            "The route through the scene became clearer.",
        ),
        (
            "The final turn before sleep holds together.",
            "bonding",
            1.0,
            3.0,
            "A final warm exchange closed the cycle.",
        ),
    ];

    for (memory, tag, trust_delta, affection_delta, event) in turns {
        HiddenState {
            memory: Some(memory.into()),
            tag: Some(tag.into()),
            trust_delta: Some(trust_delta),
            affection_delta: Some(affection_delta),
            world_event: Some(event.into()),
            new_location: None,
            present_characters: Some(vec!["Aurora".into()]),
        }
        .apply_to_soul(&mut soul);
        soul.turn_counter += 1;
        soul.turns_since_consolidation += 1;
    }

    consolidate_soul(&mut soul);

    let context = compile_context_for_messages(
        &soul,
        &[ContextMessage {
            role: "user".into(),
            content: "Where do things stand?".into(),
        }],
    );

    assert_eq!(soul.turns_since_consolidation, 0);
    assert!(soul.memory.recent.len() <= 4);
    assert!(soul.memory.core.len() >= 1);
    assert!(soul.memory.schemas.iter().any(|schema| schema.count >= 3));
    assert!(context.estimated_tokens < 2_000);
}
