use crate::soul::Relationship;

pub fn relationship_surface_summary(relationship: &Relationship) -> String {
    let mut lines = Vec::new();

    if relationship.asshole_bias >= 65.0 && relationship.trustable_bias >= 60.0 {
        lines.push(
            "Reads them as abrasive and difficult, but increasingly reliable when it matters."
                .to_string(),
        );
    } else if relationship.trustable_bias >= 60.0 {
        lines.push("Has begun to read them as reliable when it matters.".to_string());
    } else if relationship.asshole_bias >= 65.0 {
        lines.push("Reads them as abrasive and difficult.".to_string());
    }

    if relationship.reappraisal_state_code == 2 {
        lines.push(
            "An earlier impression is under pressure; recent behavior does not fit it cleanly."
                .to_string(),
        );
    } else if relationship.reappraisal_state_code == 3 {
        lines.push("The old read has been revised by recent behavior.".to_string());
    } else if relationship.reappraisal_state_code == 4 {
        lines.push("The current read has been reinforced by a repeated pattern.".to_string());
    }

    if relationship.boundary_pressure <= 15.0 && relationship.autonomy_respect_bias >= 35.0 {
        lines.push("The sense of being cornered has eased because they left room to choose.".to_string());
    } else if relationship.boundary_pressure >= 65.0 {
        lines.push("The interaction feels cornering and boundary pressure is defining.".to_string());
    } else if relationship.boundary_pressure >= 35.0 {
        lines.push("There is noticeable pressure around boundaries.".to_string());
    }

    if lines.is_empty() {
        let trust_band = numeric_band(relationship.trust);
        let comfort_band = numeric_band(relationship.comfort);
        if relationship.conflict >= 45.0 {
            lines.push(format!(
                "Conflict is {}, while trust feels {} and comfort feels {}.",
                numeric_band(relationship.conflict),
                trust_band,
                comfort_band
            ));
        } else {
            lines.push(format!(
                "Trust feels {} and comfort feels {}; no dominant bias has taken over.",
                trust_band, comfort_band
            ));
        }
    }

    lines.join(" ")
}

fn numeric_band(value: f32) -> &'static str {
    match value.clamp(0.0, 100.0) {
        value if value < 10.0 => "absent",
        value if value < 25.0 => "faint",
        value if value < 45.0 => "noticeable",
        value if value < 65.0 => "strong",
        value if value < 85.0 => "defining",
        _ => "locked",
    }
}
