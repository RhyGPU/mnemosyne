use crate::soul::Soul;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

pub fn validate_soul(soul: &Soul) -> Result<(), String> {
    if soul.schema_version != CURRENT_SCHEMA_VERSION {
        return Err(format!(
            "unsupported Soul schema version {}; expected {}",
            soul.schema_version, CURRENT_SCHEMA_VERSION
        ));
    }

    if soul.character_name.trim().is_empty() {
        return Err("Soul character_name cannot be empty".into());
    }

    Ok(())
}
