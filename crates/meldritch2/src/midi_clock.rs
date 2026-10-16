/// A MIDI timestamp mapped to an absolute audio frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameStamp {
    pub frame: u64,
    pub status: ClockStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockStatus {
    Anchored,
    Mapped,
    Late,
    Reset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockError {
    ZeroSampleRate,
    LeadTooLong,
    AudioFrameRegressed,
    FrameOverflow,
}

/// Maps connection-relative microsecond timestamps to absolute audio frames.
/// The caller supplies the first frame that audio has not rendered yet.
#[derive(Clone)]
pub struct MidiFrameClock {
    sample_rate: u32,
    lead_frames: u32,
    anchor: Option<(u64, u64)>,
    last_midi_us: u64,
    last_audio_frame: u64,
}

impl MidiFrameClock {
    pub fn new(sample_rate: u32, lead_frames: u32) -> Result<Self, ClockError> {
        if sample_rate == 0 {
            return Err(ClockError::ZeroSampleRate);
        }
        if lead_frames > sample_rate {
            return Err(ClockError::LeadTooLong);
        }
        Ok(Self {
            sample_rate,
            lead_frames,
            anchor: None,
            last_midi_us: 0,
            last_audio_frame: 0,
        })
    }

    pub fn map(
        &mut self,
        midi_us: u64,
        first_unrendered_frame: u64,
    ) -> Result<FrameStamp, ClockError> {
        if self.anchor.is_some() && first_unrendered_frame < self.last_audio_frame {
            return Err(ClockError::AudioFrameRegressed);
        }
        let reset = self.anchor.is_some() && midi_us < self.last_midi_us;
        if self.anchor.is_none() || reset {
            let frame = first_unrendered_frame
                .checked_add(u64::from(self.lead_frames))
                .ok_or(ClockError::FrameOverflow)?;
            self.anchor = Some((midi_us, frame));
            self.last_midi_us = midi_us;
            self.last_audio_frame = first_unrendered_frame;
            return Ok(FrameStamp {
                frame,
                status: if reset {
                    ClockStatus::Reset
                } else {
                    ClockStatus::Anchored
                },
            });
        }
        let (anchor_us, anchor_frame) = self.anchor.expect("anchor checked");
        let delta_us = midi_us - anchor_us;
        let delta_frames = u128::from(delta_us) * u128::from(self.sample_rate) / 1_000_000;
        let delta_frames = u64::try_from(delta_frames).map_err(|_| ClockError::FrameOverflow)?;
        let mapped = anchor_frame
            .checked_add(delta_frames)
            .ok_or(ClockError::FrameOverflow)?;
        self.last_midi_us = midi_us;
        self.last_audio_frame = first_unrendered_frame;
        Ok(if mapped < first_unrendered_frame {
            FrameStamp {
                frame: first_unrendered_frame,
                status: ClockStatus::Late,
            }
        } else {
            FrameStamp {
                frame: mapped,
                status: ClockStatus::Mapped,
            }
        })
    }
}
