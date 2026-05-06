use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BodySex {
    Male,
    Female,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ArousalPhase {
    Neutral,
    Aware,
    Warm,
    Ready,
    Plateau,
    Peak,
    Orgasm,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArousalState {
    pub body_sex: BodySex,
    pub phase: ArousalPhase,
    pub level: f32,
    pub frustration: f32,
    pub sensitivity: f32,
    pub refractory_turns_remaining: u8,
    pub orgasm_count: u32,
    pub denied_peak_turns: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ArousalSignal {
    pub delta: f32,
    pub denied: bool,
    pub orgasm_allowed: bool,
    pub forced_orgasm: bool,
}

impl Default for ArousalState {
    fn default() -> Self {
        Self {
            body_sex: BodySex::Female,
            phase: ArousalPhase::Neutral,
            level: 0.0,
            frustration: 0.0,
            sensitivity: 1.0,
            refractory_turns_remaining: 0,
            orgasm_count: 0,
            denied_peak_turns: 0,
        }
    }
}

impl ArousalState {
    pub fn apply_signal(&mut self, signal: ArousalSignal) {
        if self.refractory_turns_remaining > 0 {
            self.refractory_turns_remaining -= 1;
        }

        let capped_delta = signal.delta.clamp(-30.0, 60.0);
        let refractory_modifier = if self.refractory_turns_remaining > 0 {
            0.2
        } else {
            1.0
        };
        let effective_delta = capped_delta * self.sensitivity * refractory_modifier;
        self.level = (self.level + effective_delta).clamp(0.0, 100.0);

        if signal.denied && self.level >= 90.0 && !signal.forced_orgasm {
            self.level = self.level.min(95.0);
            self.frustration = (self.frustration + effective_delta.abs() * 0.5 + 8.0).clamp(0.0, 100.0);
            self.sensitivity = (self.sensitivity + 0.08).clamp(0.5, 1.8);
            self.denied_peak_turns += 1;
            self.phase = ArousalPhase::Peak;
            return;
        }

        if self.level >= 98.0 && (signal.orgasm_allowed || signal.forced_orgasm) {
            self.register_orgasm();
            return;
        }

        self.phase = phase_for_level(self.level);
        if effective_delta < 0.0 {
            self.frustration = (self.frustration + effective_delta).clamp(0.0, 100.0);
        }
    }

    pub fn decay(&mut self) {
        if self.refractory_turns_remaining > 0 {
            self.refractory_turns_remaining -= 1;
        }

        let decay = if self.frustration > 40.0 { 4.0 } else { 8.0 };
        self.level = (self.level - decay).clamp(0.0, 100.0);
        self.frustration = (self.frustration - 5.0).clamp(0.0, 100.0);
        self.sensitivity = if self.sensitivity > 1.0 {
            (self.sensitivity - 0.05).max(1.0)
        } else {
            (self.sensitivity + 0.03).min(1.0)
        };
        self.phase = phase_for_level(self.level);
    }

    pub fn summary(&self) -> String {
        format!(
            "Arousal: {:?} phase, level {:.0}/100, frustration {:.0}/100, sensitivity {:.2}, refractory {} turns.",
            self.phase,
            self.level,
            self.frustration,
            self.sensitivity,
            self.refractory_turns_remaining
        )
    }

    fn register_orgasm(&mut self) {
        self.phase = ArousalPhase::Orgasm;
        self.orgasm_count += 1;
        self.denied_peak_turns = 0;
        self.frustration = (self.frustration - 35.0).clamp(0.0, 100.0);

        match self.body_sex {
            BodySex::Male => {
                self.level = 15.0;
                self.sensitivity = 0.55;
                self.refractory_turns_remaining = 3;
            }
            BodySex::Female => {
                self.level = 65.0;
                self.sensitivity = (self.sensitivity + 0.12).clamp(0.8, 1.8);
                self.refractory_turns_remaining = 0;
            }
        }
    }
}

fn phase_for_level(level: f32) -> ArousalPhase {
    match level {
        level if level < 10.0 => ArousalPhase::Neutral,
        level if level < 25.0 => ArousalPhase::Aware,
        level if level < 45.0 => ArousalPhase::Warm,
        level if level < 70.0 => ArousalPhase::Ready,
        level if level < 90.0 => ArousalPhase::Plateau,
        _ => ArousalPhase::Peak,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buildup_advances_phases() {
        let mut state = ArousalState::default();
        state.apply_signal(ArousalSignal {
            delta: 50.0,
            ..ArousalSignal::default()
        });

        assert_eq!(state.phase, ArousalPhase::Ready);
        assert_eq!(state.level, 50.0);
    }

    #[test]
    fn denial_caps_peak_and_builds_frustration() {
        let mut state = ArousalState::default();
        state.level = 88.0;
        state.apply_signal(ArousalSignal {
            delta: 30.0,
            denied: true,
            ..ArousalSignal::default()
        });

        assert_eq!(state.phase, ArousalPhase::Peak);
        assert_eq!(state.level, 95.0);
        assert!(state.frustration > 0.0);
        assert_eq!(state.denied_peak_turns, 1);
    }

    #[test]
    fn male_orgasm_enters_refractory() {
        let mut state = ArousalState {
            body_sex: BodySex::Male,
            level: 95.0,
            ..ArousalState::default()
        };
        state.apply_signal(ArousalSignal {
            delta: 10.0,
            orgasm_allowed: true,
            ..ArousalSignal::default()
        });

        assert_eq!(state.phase, ArousalPhase::Orgasm);
        assert_eq!(state.refractory_turns_remaining, 3);
        assert!(state.level < 25.0);
        assert!(state.sensitivity < 1.0);
    }

    #[test]
    fn female_orgasm_supports_multi_orgasm_plateau() {
        let mut state = ArousalState {
            body_sex: BodySex::Female,
            level: 95.0,
            ..ArousalState::default()
        };
        state.apply_signal(ArousalSignal {
            delta: 10.0,
            orgasm_allowed: true,
            ..ArousalSignal::default()
        });

        assert_eq!(state.phase, ArousalPhase::Orgasm);
        assert_eq!(state.refractory_turns_remaining, 0);
        assert!(state.level >= 60.0);

        state.apply_signal(ArousalSignal {
            delta: 40.0,
            orgasm_allowed: true,
            ..ArousalSignal::default()
        });

        assert_eq!(state.orgasm_count, 2);
    }

    #[test]
    fn decay_reduces_level_and_recovers_sensitivity() {
        let mut state = ArousalState {
            level: 50.0,
            phase: ArousalPhase::Ready,
            frustration: 20.0,
            sensitivity: 1.3,
            ..ArousalState::default()
        };
        state.decay();

        assert!(state.level < 50.0);
        assert!(state.frustration < 20.0);
        assert!(state.sensitivity < 1.3);
    }
}
