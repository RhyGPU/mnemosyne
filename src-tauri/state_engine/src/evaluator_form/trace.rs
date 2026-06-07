use crate::evaluator_form::types::EvalFormRowRejection;

pub fn format_honest_ui_status(
    patch_applied: bool,
    materialized_soul_updated: bool,
    materialized_session_world_updated: bool,
    rejected_rows: &[EvalFormRowRejection],
) -> String {
    let was_applied = patch_applied && materialized_soul_updated && materialized_session_world_updated;
    let rows_rejected = rejected_rows.len();

    if was_applied {
        if rows_rejected == 0 {
            "State updated".to_string()
        } else {
            let mut object_count = 0;
            let mut relationship_count = 0;
            let mut memory_count = 0;
            let mut event_count = 0;
            let mut review_count = 0;

            for r in rejected_rows {
                match r.row_kind.as_str() {
                    "object" => object_count += 1,
                    "relationship" | "relationship_event" => relationship_count += 1,
                    "memory" => memory_count += 1,
                    "event" => event_count += 1,
                    "review" => review_count += 1,
                    _ => {}
                }
            }

            let mut kinds = Vec::new();
            if object_count > 0 {
                kinds.push(format!("{} object row{}", object_count, if object_count == 1 { "" } else { "s" }));
            }
            if relationship_count > 0 {
                kinds.push(format!("{} relationship row{}", relationship_count, if relationship_count == 1 { "" } else { "s" }));
            }
            if memory_count > 0 {
                kinds.push(format!("{} memory row{}", memory_count, if memory_count == 1 { "" } else { "s" }));
            }
            if event_count > 0 {
                kinds.push(format!("{} event row{}", event_count, if event_count == 1 { "" } else { "s" }));
            }
            if review_count > 0 {
                kinds.push(format!("{} review row{}", review_count, if review_count == 1 { "" } else { "s" }));
            }

            if kinds.len() == 1 {
                format!("State updated; {} skipped", kinds[0])
            } else {
                format!("State updated; {} evaluator rows skipped", rows_rejected)
            }
        }
    } else {
        "State update failed".to_string()
    }
}
