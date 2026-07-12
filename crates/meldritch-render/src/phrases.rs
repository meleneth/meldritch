//! Deterministic phrase banks and quantized phrase cycling.

use meldritch_core::{Frame, Pattern, PatternId, Tempo};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhraseId(u64);

impl PhraseId {
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Phrase {
    id: PhraseId,
    variations: Vec<Pattern>,
    repeats: u32,
}

impl Phrase {
    pub fn new(
        id: PhraseId,
        variations: Vec<Pattern>,
        repeats: u32,
    ) -> Result<Self, PhraseBankError> {
        if variations.is_empty() {
            return Err(PhraseBankError::NoVariations(id));
        }
        if repeats == 0 {
            return Err(PhraseBankError::ZeroRepeats(id));
        }
        let length = variations[0].length_steps();
        let steps_per_beat = variations[0].steps_per_beat();
        if variations.iter().any(|pattern| {
            pattern.length_steps() != length || pattern.steps_per_beat() != steps_per_beat
        }) {
            return Err(PhraseBankError::IncompatibleVariations(id));
        }
        Ok(Self {
            id,
            variations,
            repeats,
        })
    }

    #[must_use]
    pub const fn id(&self) -> PhraseId {
        self.id
    }

    #[must_use]
    pub fn variations(&self) -> &[Pattern] {
        &self.variations
    }

    #[must_use]
    pub const fn repeats(&self) -> u32 {
        self.repeats
    }

    fn duration_frames(&self, tempo: Tempo) -> Frame {
        tempo.step_start_frame(
            u64::from(self.variations[0].length_steps()),
            self.variations[0].steps_per_beat(),
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhraseBankError {
    Empty,
    NoVariations(PhraseId),
    ZeroRepeats(PhraseId),
    IncompatibleVariations(PhraseId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PhraseBank {
    phrases: Vec<Phrase>,
}

impl PhraseBank {
    pub fn new(phrases: Vec<Phrase>) -> Result<Self, PhraseBankError> {
        if phrases.is_empty() {
            return Err(PhraseBankError::Empty);
        }
        Ok(Self { phrases })
    }

    #[must_use]
    pub fn phrases(&self) -> &[Phrase] {
        &self.phrases
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhraseTransition {
    pub phrase: PhraseId,
    pub pattern: PatternId,
    pub variation: usize,
    pub repeat: u32,
    pub frame: Frame,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PhraseCycler {
    bank: PhraseBank,
    tempo: Tempo,
    phrase_index: usize,
    repeat: u32,
    cycle: u64,
    active_variation: usize,
    next_frame: Frame,
}

impl PhraseCycler {
    #[must_use]
    pub fn new(bank: PhraseBank, tempo: Tempo, start_frame: Frame) -> Self {
        let active_variation = choose_variation(&bank.phrases[0], 0);
        let next_frame = start_frame.saturating_add(bank.phrases[0].duration_frames(tempo));
        Self {
            bank,
            tempo,
            phrase_index: 0,
            repeat: 0,
            cycle: 0,
            active_variation,
            next_frame,
        }
    }

    #[must_use]
    pub fn active(&self) -> PhraseTransition {
        self.transition(self.next_frame - self.active_phrase().duration_frames(self.tempo))
    }

    #[must_use]
    pub const fn next_frame(&self) -> Frame {
        self.next_frame
    }

    /// Advances through every boundary reached by `playhead`, returning the
    /// transitions in musical order.
    pub fn advance(&mut self, playhead: Frame) -> Vec<PhraseTransition> {
        let mut transitions = Vec::new();
        while playhead >= self.next_frame {
            let frame = self.next_frame;
            self.repeat += 1;
            if self.repeat >= self.active_phrase().repeats {
                self.repeat = 0;
                self.phrase_index = (self.phrase_index + 1) % self.bank.phrases.len();
                if self.phrase_index == 0 {
                    self.cycle = self.cycle.wrapping_add(1);
                }
            }
            self.active_variation =
                choose_variation(self.active_phrase(), self.cycle + u64::from(self.repeat));
            self.next_frame =
                frame.saturating_add(self.active_phrase().duration_frames(self.tempo));
            transitions.push(self.transition(frame));
        }
        transitions
    }

    fn active_phrase(&self) -> &Phrase {
        &self.bank.phrases[self.phrase_index]
    }

    fn transition(&self, frame: Frame) -> PhraseTransition {
        let phrase = self.active_phrase();
        PhraseTransition {
            phrase: phrase.id,
            pattern: phrase.variations[self.active_variation].id(),
            variation: self.active_variation,
            repeat: self.repeat,
            frame,
        }
    }
}

fn choose_variation(phrase: &Phrase, sequence: u64) -> usize {
    ((sequence ^ phrase.id.raw().wrapping_mul(0x9e37_79b9)) % phrase.variations.len() as u64)
        as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern(id: u64, length: u32) -> Pattern {
        Pattern::new(PatternId::new(id), length, 4).unwrap()
    }

    #[test]
    fn phrases_validate_variations_and_repeats() {
        let id = PhraseId::new(1);
        assert_eq!(
            Phrase::new(id, Vec::new(), 1),
            Err(PhraseBankError::NoVariations(id))
        );
        assert_eq!(
            Phrase::new(id, vec![pattern(1, 16)], 0),
            Err(PhraseBankError::ZeroRepeats(id))
        );
        assert_eq!(
            Phrase::new(id, vec![pattern(1, 16), pattern(2, 12)], 1),
            Err(PhraseBankError::IncompatibleVariations(id))
        );
        assert_eq!(PhraseBank::new(Vec::new()), Err(PhraseBankError::Empty));
    }

    #[test]
    fn cycler_switches_on_exact_pattern_boundaries_and_honors_repeats() {
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let bank = PhraseBank::new(vec![
            Phrase::new(PhraseId::new(1), vec![pattern(10, 16)], 2).unwrap(),
            Phrase::new(PhraseId::new(2), vec![pattern(20, 8)], 1).unwrap(),
        ])
        .unwrap();
        let mut cycler = PhraseCycler::new(bank, tempo, 0);
        assert_eq!(cycler.active().pattern, PatternId::new(10));
        assert!(cycler.advance(95_999).is_empty());
        assert_eq!(cycler.advance(96_000)[0].repeat, 1);
        let transition = cycler.advance(192_000)[0];
        assert_eq!(transition.phrase, PhraseId::new(2));
        assert_eq!(transition.pattern, PatternId::new(20));
        assert_eq!(transition.frame, 192_000);
        assert_eq!(cycler.next_frame(), 240_000);
    }

    #[test]
    fn delayed_polling_emits_every_transition_and_variations_are_repeatable() {
        let tempo = Tempo::new(120.0, 48_000).unwrap();
        let make_bank = || {
            PhraseBank::new(vec![
                Phrase::new(
                    PhraseId::new(7),
                    vec![pattern(70, 4), pattern(71, 4), pattern(72, 4)],
                    1,
                )
                .unwrap(),
                Phrase::new(PhraseId::new(8), vec![pattern(80, 4)], 1).unwrap(),
            ])
            .unwrap()
        };
        let mut first = PhraseCycler::new(make_bank(), tempo, 0);
        let mut second = PhraseCycler::new(make_bank(), tempo, 0);
        let first_transitions = first.advance(120_000);
        let second_transitions = second.advance(120_000);
        assert_eq!(first_transitions, second_transitions);
        assert_eq!(first_transitions.len(), 5);
        assert!(
            first_transitions
                .windows(2)
                .all(|pair| pair[0].frame < pair[1].frame)
        );
    }
}
