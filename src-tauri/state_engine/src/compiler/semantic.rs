use serde::{Deserialize, Serialize};

use super::{
    bind::{BindingReport, BindingStatus, BoundCandidate},
    diagnostics::{CompilerDiagnostic, CompilerStage, DiagnosticSeverity},
    perception::{EpistemicMode, EvidenceSource, PerceptionKind, TemporalAnchor},
    source::SourceEnvelope,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SemanticDisposition {
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ValidatedCandidate {
    pub candidate: BoundCandidate,
    pub disposition: SemanticDisposition,
    pub normalized_temporal_anchor_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SemanticReport {
    pub source_hash: String,
    pub candidates: Vec<ValidatedCandidate>,
    pub diagnostics: Vec<CompilerDiagnostic>,
}

pub trait SemanticAnalyzer {
    fn analyze(&self, source: &SourceEnvelope, bindings: &BindingReport) -> SemanticReport;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DeterministicSemanticAnalyzer;

impl SemanticAnalyzer for DeterministicSemanticAnalyzer {
    fn analyze(&self, source: &SourceEnvelope, bindings: &BindingReport) -> SemanticReport {
        let mut diagnostics = bindings.diagnostics.clone();
        if source.validate().is_err() || bindings.source_hash != source.source_hash() {
            diagnostics.push(CompilerDiagnostic {
                stage: CompilerStage::Semantic,
                severity: DiagnosticSeverity::Error,
                code: "binding_source_mismatch".into(),
                message: "binding report does not belong to the active source envelope".into(),
                candidate_id: None,
                field_path: None,
            });
            return SemanticReport {
                source_hash: source.source_hash().into(),
                candidates: bindings
                    .candidates
                    .iter()
                    .cloned()
                    .map(|candidate| ValidatedCandidate {
                        candidate,
                        disposition: SemanticDisposition::Rejected,
                        normalized_temporal_anchor_ms: None,
                    })
                    .collect(),
                diagnostics,
            };
        }

        let candidates = bindings
            .candidates
            .iter()
            .cloned()
            .map(|candidate| {
                let candidate_id = candidate.candidate.candidate_id.clone();
                let mut rejected = false;
                let mut reject = |code: &str, message: String, field_path: Option<&str>| {
                    rejected = true;
                    diagnostics.push(CompilerDiagnostic {
                        stage: CompilerStage::Semantic,
                        severity: DiagnosticSeverity::Error,
                        code: code.into(),
                        message,
                        candidate_id: Some(candidate_id.clone()),
                        field_path: field_path.map(str::to_string),
                    });
                };

                if candidate
                    .bindings
                    .iter()
                    .any(|binding| binding.status != BindingStatus::Bound)
                {
                    reject(
                        "candidate_has_unresolved_entities",
                        "candidate cannot be accepted until every entity reference is bound".into(),
                        None,
                    );
                }
                if candidate.candidate.source_hash != source.source_hash() {
                    reject(
                        "candidate_source_mismatch",
                        "candidate source hash does not match active source".into(),
                        None,
                    );
                }

                let perception = &candidate.candidate.perception;
                let evidence_text = match perception.evidence.source {
                    EvidenceSource::UserMessage => source.user_text(),
                    EvidenceSource::AssistantMessage => source.assistant_text(),
                };
                if !normalized_contains(evidence_text, &perception.evidence.quote) {
                    reject(
                        "evidence_quote_not_found",
                        "evidence quote is not a continuous normalized substring of its source"
                            .into(),
                        Some("evidence.quote"),
                    );
                }
                if let (Some(start), Some(end)) =
                    (perception.evidence.start_char, perception.evidence.end_char)
                {
                    let extracted = char_slice(evidence_text, start as usize, end as usize);
                    if extracted.as_deref() != Some(perception.evidence.quote.as_str()) {
                        reject(
                            "evidence_offsets_mismatch",
                            "evidence offsets do not select the declared exact quote".into(),
                            Some("evidence"),
                        );
                    }
                }
                if perception.extraction_confidence < 0.2 {
                    reject(
                        "confidence_below_semantic_floor",
                        "candidate confidence is below the semantic acceptance floor".into(),
                        Some("extraction_confidence"),
                    );
                }

                match perception.epistemic_mode {
                    EpistemicMode::DirectlyObserved
                        if bound_entity(&candidate, "perceiver_ref").is_none() =>
                    {
                        reject(
                            "direct_observation_without_perceiver",
                            "direct observation requires a bound perceiver".into(),
                            Some("perceiver_ref"),
                        );
                    }
                    EpistemicMode::StatedBy if bound_entity(&candidate, "actor_ref").is_none() => {
                        reject(
                            "statement_without_speaker",
                            "stated_by requires a bound actor/speaker".into(),
                            Some("actor_ref"),
                        );
                    }
                    EpistemicMode::NarratorDescribed
                        if perception.evidence.source != EvidenceSource::AssistantMessage =>
                    {
                        reject(
                            "narrator_description_wrong_source",
                            "narrator_described evidence must come from the assistant message"
                                .into(),
                            Some("evidence.source"),
                        );
                    }
                    _ => {}
                }

                if perception.temporal.anchor == TemporalAnchor::AfterCurrentTurn
                    && perception.kind != PerceptionKind::Intention
                {
                    reject(
                        "future_fact_not_intention",
                        "future material may only compile as an intention".into(),
                        Some("temporal.anchor"),
                    );
                }
                if perception.kind == PerceptionKind::Correction
                    && !contains_correction_cue(evidence_text)
                {
                    reject(
                        "correction_without_explicit_cue",
                        "correction candidates require explicit correction or retcon wording"
                            .into(),
                        Some("kind"),
                    );
                }

                let normalized_temporal_anchor_ms = match perception.temporal.anchor {
                    TemporalAnchor::CurrentTurn => Some(source.observed_at_ms()),
                    TemporalAnchor::BeforeCurrentTurn => {
                        Some(source.observed_at_ms().saturating_sub(1))
                    }
                    TemporalAnchor::AfterCurrentTurn => {
                        Some(source.observed_at_ms().saturating_add(1))
                    }
                    TemporalAnchor::Absolute | TemporalAnchor::Unknown => None,
                };
                ValidatedCandidate {
                    candidate,
                    disposition: if rejected {
                        SemanticDisposition::Rejected
                    } else {
                        SemanticDisposition::Accepted
                    },
                    normalized_temporal_anchor_ms,
                }
            })
            .collect();

        SemanticReport {
            source_hash: source.source_hash().into(),
            candidates,
            diagnostics,
        }
    }
}

pub(crate) fn bound_entity<'a>(candidate: &'a BoundCandidate, field_path: &str) -> Option<&'a str> {
    candidate
        .bindings
        .iter()
        .find(|binding| binding.field_path == field_path && binding.status == BindingStatus::Bound)
        .and_then(|binding| binding.resolved_entity_id.as_deref())
}

fn normalized_contains(source: &str, quote: &str) -> bool {
    let quote = normalize_for_match(quote);
    !quote.is_empty() && normalize_for_match(source).contains(&quote)
}

fn normalize_for_match(value: &str) -> String {
    value
        .chars()
        .filter_map(|character| match character {
            '*' | '_' | '`' | '~' => None,
            '\u{2018}' | '\u{2019}' | '\u{201B}' => Some('\''),
            '\u{201C}' | '\u{201D}' | '\u{201F}' => Some('"'),
            character if character.is_alphanumeric() => {
                Some(character.to_lowercase().next().unwrap_or(character))
            }
            _ => Some(' '),
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn char_slice(value: &str, start: usize, end: usize) -> Option<String> {
    if start >= end {
        return None;
    }
    let characters = value.chars().collect::<Vec<_>>();
    (end <= characters.len()).then(|| characters[start..end].iter().collect())
}

fn contains_correction_cue(value: &str) -> bool {
    let lower = value.to_lowercase();
    [
        "actually",
        "correction",
        "correct that",
        "retcon",
        "instead",
        "아니",
        "정정",
        "수정",
    ]
    .iter()
    .any(|cue| lower.contains(cue))
}
