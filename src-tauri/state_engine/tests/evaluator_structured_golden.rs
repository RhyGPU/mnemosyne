use serde::Deserialize;
use serde_json::Value;
use state_engine::{
    evaluator::EvaluatorConversionContext,
    evaluator_structured::{compile_evaluator_ops_to_engine_patch, EvaluatorStructuredOutputV1},
    soul::Soul,
};

#[derive(Debug, Deserialize)]
struct GoldenCorpus {
    corpus_version: u32,
    description: String,
    cases: Vec<GoldenCase>,
}

#[derive(Debug, Deserialize)]
struct GoldenCase {
    id: String,
    user: String,
    narrator: String,
    output: Value,
    #[serde(default)]
    expected: GoldenExpectation,
}

#[derive(Debug, Default, Deserialize)]
struct GoldenExpectation {
    #[serde(default)]
    accepted: usize,
    #[serde(default)]
    rejected: usize,
    #[serde(default)]
    memories: usize,
    #[serde(default)]
    world_events: usize,
    #[serde(default)]
    object_updates: usize,
    #[serde(default)]
    scene_updates: usize,
    #[serde(default)]
    relationship_updates: usize,
    error_contains: Option<String>,
    #[serde(default)]
    memory_source_message_id_is_null: bool,
}

fn corpus() -> GoldenCorpus {
    serde_json::from_str(include_str!("fixtures/evaluator_structured_v1_golden.json"))
        .expect("golden corpus must remain valid JSON")
}

fn context<'a>(case: &'a GoldenCase) -> EvaluatorConversionContext<'a> {
    EvaluatorConversionContext {
        active_soul_id: "aurora",
        active_soul_ids: vec!["aurora".into()],
        latest_user_message: &case.user,
        latest_narrator_response: &case.narrator,
        session_world: None,
        baseline_recent_event_id: None,
    }
}

#[test]
fn evaluator_v1_golden_corpus_is_deterministic_and_matches_expectations() {
    let corpus = corpus();
    assert_eq!(corpus.corpus_version, 1);
    assert!(!corpus.description.trim().is_empty());
    assert!(
        corpus.cases.len() >= 10,
        "starter corpus unexpectedly shrank"
    );

    for case in &corpus.cases {
        let output: EvaluatorStructuredOutputV1 = serde_json::from_value(case.output.clone())
            .unwrap_or_else(|error| panic!("{}: fixture output failed to parse: {error}", case.id));
        let soul = Soul::default_for_character("Aurora");
        let first = compile_evaluator_ops_to_engine_patch(&output, &context(case), &soul);

        if let Some(expected_error) = case.expected.error_contains.as_deref() {
            let error = first.unwrap_err_or_else(&case.id);
            assert!(
                error.contains(expected_error),
                "{}: expected error containing {expected_error:?}, got {error:?}",
                case.id
            );
            continue;
        }

        let first = first.unwrap_or_else(|error| panic!("{}: {error}", case.id));
        let second = compile_evaluator_ops_to_engine_patch(&output, &context(case), &soul)
            .unwrap_or_else(|error| panic!("{} replay: {error}", case.id));
        assert_eq!(
            first.patch, second.patch,
            "{}: patch replay diverged",
            case.id
        );
        assert_eq!(
            serde_json::to_vec(&first.patch).expect("serialize first patch"),
            serde_json::to_vec(&second.patch).expect("serialize replay patch"),
            "{}: serialized patch replay diverged",
            case.id
        );
        assert_eq!(
            first.accepted_candidate_ids.len(),
            case.expected.accepted,
            "{}: accepted candidate count",
            case.id
        );
        assert_eq!(
            first.rejected_candidates.len(),
            case.expected.rejected,
            "{}: rejected candidate count",
            case.id
        );

        let memories = first
            .patch
            .soul_patch
            .as_ref()
            .map_or(0, |patch| patch.new_memories.len());
        let relationships = first
            .patch
            .soul_patch
            .as_ref()
            .map_or(0, |patch| patch.relationship_deltas.len());
        let world_events = first
            .patch
            .world_patch
            .as_ref()
            .map_or(0, |patch| patch.event_operations.len());
        let object_updates = first
            .patch
            .world_patch
            .as_ref()
            .map_or(0, |patch| patch.object_observation_operations.len());
        let scene_updates = usize::from(
            first
                .patch
                .world_patch
                .as_ref()
                .and_then(|patch| patch.scene_state.as_ref())
                .is_some(),
        );

        assert_eq!(memories, case.expected.memories, "{}: memories", case.id);
        assert_eq!(
            relationships, case.expected.relationship_updates,
            "{}: relationship updates",
            case.id
        );
        assert_eq!(
            world_events, case.expected.world_events,
            "{}: world events",
            case.id
        );
        assert_eq!(
            object_updates, case.expected.object_updates,
            "{}: object updates",
            case.id
        );
        assert_eq!(
            scene_updates, case.expected.scene_updates,
            "{}: scene updates",
            case.id
        );

        if case.expected.memory_source_message_id_is_null {
            assert!(
                first
                    .patch
                    .soul_patch
                    .as_ref()
                    .expect("expected soul patch")
                    .new_memories
                    .iter()
                    .all(|memory| memory.source_message_id.is_none()),
                "{}: evaluator-controlled message id escaped lowering",
                case.id
            );
        }
    }
}

trait ResultTestExt<T> {
    fn unwrap_err_or_else(self, case_id: &str) -> String;
}

impl<T> ResultTestExt<T> for Result<T, String> {
    fn unwrap_err_or_else(self, case_id: &str) -> String {
        match self {
            Ok(_) => panic!("{case_id}: expected semantic rejection"),
            Err(error) => error,
        }
    }
}
