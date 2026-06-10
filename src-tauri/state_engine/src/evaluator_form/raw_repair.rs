use serde_json::Value;

use crate::evaluator_form::{
    normalize_eval_form_value, EvalFormRepairTrace, EvalFormResponse,
};

pub fn parse_eval_form_response(raw_json: &str) -> Result<EvalFormResponse, String> {
    parse_eval_form_response_with_trace(raw_json).map(|(response, _)| response)
}

pub fn parse_eval_form_response_with_trace(
    raw_json: &str,
) -> Result<(EvalFormResponse, EvalFormRepairTrace), String> {
    let mut trace = EvalFormRepairTrace {
        json_extract_status: "not_started".into(),
        ..EvalFormRepairTrace::default()
    };
    let stripped = strip_json_fences(raw_json);
    let (extracted, extracted_status) = extract_first_balanced_json_object(&stripped)
        .map(|json| (json, "success".to_string()))
        .unwrap_or_else(|| (stripped.clone(), "not_found_used_full_text".to_string()));
    trace.json_extract_status = extracted_status;

    match serde_json::from_str::<Value>(&extracted) {
        Ok(mut value) => {
            unwrap_top_level_form_envelope(&mut value, &mut trace);
            normalize_eval_form_value(&mut value, &mut trace);
            let response = serde_json::from_value(value).map_err(|err| {
                format!("invalid EvalFormResponse JSON after normalization: {err}")
            })?;
            trace.salvage_success = true;
            Ok((response, trace))
        }
        Err(first_err) => {
            trace.strict_parse_failed_but_salvage_attempted = true;
            let repaired = repair_common_json_drift(&extracted, &mut trace);
            let mut value = serde_json::from_str::<Value>(&repaired).map_err(|err| {
                format!("invalid EvalFormResponse JSON: {first_err}; repair failed: {err}")
            })?;
            unwrap_top_level_form_envelope(&mut value, &mut trace);
            normalize_eval_form_value(&mut value, &mut trace);
            let response = serde_json::from_value(value)
                .map_err(|err| format!("invalid EvalFormResponse JSON after repair: {err}"))?;
            trace.salvage_success = true;
            Ok((response, trace))
        }
    }
}

pub fn unwrap_top_level_form_envelope(value: &mut Value, trace: &mut EvalFormRepairTrace) {
    const ENVELOPE_KEYS: [&str; 4] = ["evaluator_form_v1", "form", "eval_form", "response"];

    let Some(object) = value.as_object() else {
        return;
    };

    let nested = ENVELOPE_KEYS.iter().find_map(|key| {
        object
            .get(*key)
            .filter(|nested| has_form_row_array(nested))
            .cloned()
    });

    if let Some(nested) = nested {
        *value = nested;
        trace
            .raw_form_repair_warnings
            .push("top_level_evaluator_form_v1_envelope_unwrapped".into());
        trace.raw_form_repair_applied = true;
    }
}

fn has_form_row_array(value: &Value) -> bool {
    const ROW_KEYS: [&str; 6] = [
        "event_rows",
        "object_rows",
        "relationship_rows",
        "relationship_event_rows",
        "memory_rows",
        "review_rows",
    ];

    let Some(object) = value.as_object() else {
        return false;
    };

    ROW_KEYS
        .iter()
        .any(|key| object.get(*key).is_some_and(Value::is_array))
}

pub fn strip_json_fences(raw: &str) -> String {
    let trimmed = raw.trim();
    let stripped = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed)
        .trim();
    stripped
        .strip_suffix("```")
        .unwrap_or(stripped)
        .trim()
        .to_string()
}

pub fn extract_first_balanced_json_object(raw: &str) -> Option<String> {
    let mut start = None;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (index, ch) in raw.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => {
                if start.is_none() {
                    start = Some(index);
                }
                depth += 1;
            }
            '}' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    let start = start?;
                    return Some(raw[start..=index].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

pub fn repair_unescaped_quotes_in_field(raw: &str, field_name: &str, trace: &mut EvalFormRepairTrace) -> String {
    let mut out = String::new();
    let mut current_pos = 0;
    
    while let Some(field_pos) = raw[current_pos..].find(field_name) {
        let absolute_field_pos = current_pos + field_pos;
        out.push_str(&raw[current_pos..absolute_field_pos]);
        
        let after_field = absolute_field_pos + field_name.len();
        if let Some(colon_offset) = raw[after_field..].find(':') {
            let colon_pos = after_field + colon_offset;
            if let Some(quote_offset) = raw[colon_pos..].find('"') {
                let start_quote_pos = colon_pos + quote_offset;
                
                let search_start = start_quote_pos + 1;
                let mut end_quote_pos = None;
                
                let bytes = raw.as_bytes();
                for i in search_start..bytes.len() {
                    if bytes[i] == b'"' {
                        let mut is_escaped = false;
                        let mut k = i;
                        while k > search_start && bytes[k - 1] == b'\\' {
                            is_escaped = !is_escaped;
                            k -= 1;
                        }
                        if !is_escaped {
                            let mut next = i + 1;
                            while next < bytes.len() && bytes[next].is_ascii_whitespace() {
                                next += 1;
                            }
                            if next < bytes.len() {
                                let ch = bytes[next];
                                if ch == b'}' || ch == b']' {
                                    end_quote_pos = Some(i);
                                    break;
                                } else if ch == b',' {
                                    let mut probe = next + 1;
                                    while probe < bytes.len() && bytes[probe].is_ascii_whitespace() {
                                        probe += 1;
                                    }
                                    if probe < bytes.len() && bytes[probe] == b'"' {
                                        let mut key_end = probe + 1;
                                        while key_end < bytes.len() && bytes[key_end] != b'"' {
                                            key_end += 1;
                                        }
                                        if key_end < bytes.len() {
                                            let mut colon = key_end + 1;
                                            while colon < bytes.len() && bytes[colon].is_ascii_whitespace() {
                                                colon += 1;
                                            }
                                            if colon < bytes.len() && bytes[colon] == b':' {
                                                end_quote_pos = Some(i);
                                                break;
                                            }
                                        }
                                    } else if probe < bytes.len() && (bytes[probe] == b'}' || bytes[probe] == b']') {
                                        end_quote_pos = Some(i);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                
                if let Some(end_pos) = end_quote_pos {
                    let content = &raw[search_start..end_pos];
                    
                    let mut escaped_content = String::new();
                    let content_bytes = content.as_bytes();
                    let mut idx = 0;
                    while idx < content_bytes.len() {
                        if content_bytes[idx] == b'"' {
                            let mut backslash_count = 0;
                            let mut k = idx;
                            while k > 0 && content_bytes[k - 1] == b'\\' {
                                backslash_count += 1;
                                k -= 1;
                            }
                            let is_escaped = backslash_count % 2 == 1;
                            if !is_escaped {
                                escaped_content.push('\\');
                            }
                            escaped_content.push('"');
                        } else {
                            escaped_content.push(content_bytes[idx] as char);
                        }
                        idx += 1;
                    }
                    
                    if escaped_content != content {
                        trace.raw_form_repair_warnings.push(format!("unescaped quotes in {} repaired", field_name));
                        trace.raw_form_repair_applied = true;
                    }
                    
                    out.push_str(&raw[absolute_field_pos..=start_quote_pos]);
                    out.push_str(&escaped_content);
                    out.push('"');
                    
                    current_pos = end_pos + 1;
                    continue;
                }
            }
        }
        
        out.push_str(&raw[absolute_field_pos..after_field]);
        current_pos = after_field;
    }
    
    out.push_str(&raw[current_pos..]);
    out
}

pub fn repair_common_json_drift(raw: &str, trace: &mut EvalFormRepairTrace) -> String {
    let mut repaired = raw.replace(['“', '”'], "\"").replace(['‘', '’'], "'");
    if repaired != raw {
        trace
            .raw_form_repair_warnings
            .push("smart quotes normalized".into());
        trace.raw_form_repair_applied = true;
    }
    let without_trailing_commas = remove_trailing_commas(&repaired);
    if without_trailing_commas != repaired {
        trace
            .raw_form_repair_warnings
            .push("trailing commas removed".into());
        trace.raw_form_repair_applied = true;
        repaired = without_trailing_commas;
    }
    let quoted_and = repair_quoted_string_and_string(&repaired);
    if quoted_and != repaired {
        trace
            .raw_form_repair_warnings
            .push("quoted string-and-string evidence repaired".into());
        trace.raw_form_repair_applied = true;
        repaired = quoted_and;
    }
    let repaired_evidence = repair_unescaped_quotes_in_field(&repaired, "evidence_quote", trace);
    if repaired_evidence != repaired {
        repaired = repaired_evidence;
    }
    let repaired_summary = repair_unescaped_quotes_in_field(&repaired, "objective_summary", trace);
    if repaired_summary != repaired {
        repaired = repaired_summary;
    }
    let repaired_content = repair_unescaped_quotes_in_field(&repaired, "content", trace);
    if repaired_content != repaired {
        repaired = repaired_content;
    }
    repaired
}

pub fn remove_trailing_commas(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == ',' {
            let mut lookahead = chars.clone();
            while matches!(lookahead.peek(), Some(next) if next.is_whitespace()) {
                lookahead.next();
            }
            if matches!(lookahead.peek(), Some('}' | ']')) {
                continue;
            }
        }
        out.push(ch);
    }
    out
}

pub fn repair_quoted_string_and_string(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let bytes = raw.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'"' {
            out.push(bytes[index] as char);
            index += 1;
            continue;
        }
        let Some((first, after_first)) = read_json_string(raw, index) else {
            out.push(bytes[index] as char);
            index += 1;
            continue;
        };
        let mut probe = after_first;
        while probe < bytes.len() && bytes[probe].is_ascii_whitespace() {
            probe += 1;
        }
        if !raw[probe..].starts_with("and") {
            out.push_str(&raw[index..after_first]);
            index = after_first;
            continue;
        }
        let after_and = probe + 3;
        let is_word_continuation = bytes
            .get(after_and)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_');
        if is_word_continuation {
            out.push_str(&raw[index..after_first]);
            index = after_first;
            continue;
        }
        let mut second_start = after_and;
        while second_start < bytes.len() && bytes[second_start].is_ascii_whitespace() {
            second_start += 1;
        }
        if bytes.get(second_start) != Some(&b'"') {
            out.push_str(&raw[index..after_first]);
            index = after_first;
            continue;
        }
        let Some((second, after_second)) = read_json_string(raw, second_start) else {
            out.push_str(&raw[index..after_first]);
            index = after_first;
            continue;
        };
        out.push('"');
        out.push_str(&escape_json_string(&format!("{first}; {second}")));
        out.push('"');
        index = after_second;
    }
    out
}

pub fn read_json_string(raw: &str, start: usize) -> Option<(String, usize)> {
    let bytes = raw.as_bytes();
    if bytes.get(start) != Some(&b'"') {
        return None;
    }
    let mut escaped = false;
    let mut value = String::new();
    let mut index = start + 1;
    while index < bytes.len() {
        let ch = bytes[index] as char;
        if escaped {
            value.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some((value, index + 1));
        } else {
            value.push(ch);
        }
        index += 1;
    }
    None
}

pub fn escape_json_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
