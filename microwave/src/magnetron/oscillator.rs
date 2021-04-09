use serde::{Deserialize, Serialize};

use crate::audio::DEFAULT_SAMPLE_RATE;

use super::{
    control::Controller,
    functions,
    source::LfSource,
    util::{CombFilter, OnePoleLowPass},
    waveform::{InBuffer, OutSpec, Stage},
};

#[derive(Deserialize, Serialize)]
pub struct Oscillator<K> {
    pub kind: OscillatorKind,
    pub frequency: LfSource<K>,
    #[serde(flatten)]
    pub modulation: Modulation,
    #[serde(flatten)]
    pub out_spec: OutSpec<K>,
}

#[derive(Clone, Deserialize, Serialize)]
pub enum OscillatorKind {
    Sin,
    Sin3,
    Triangle,
    Square,
    Sawtooth,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "modulation")]
pub enum Modulation {
    None,
    ByPhase { mod_buffer: InBuffer },
    ByFrequency { mod_buffer: InBuffer },
}

impl<C: Controller> Oscillator<C> {
    pub fn create_stage(&self) -> Stage<C::Storage> {
        match self.kind {
            OscillatorKind::Sin => self.apply_signal_fn(functions::sin),
            OscillatorKind::Sin3 => self.apply_signal_fn(functions::sin3),
            OscillatorKind::Triangle => self.apply_signal_fn(functions::triangle),
            OscillatorKind::Square => self.apply_signal_fn(functions::square),
            OscillatorKind::Sawtooth => self.apply_signal_fn(functions::sawtooth),
        }
    }

    fn apply_signal_fn(
        &self,
        oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
    ) -> Stage<C::Storage> {
        match &self.modulation {
            Modulation::None => self.apply_no_modulation(oscillator_fn, 0.0),
            Modulation::ByPhase { mod_buffer } => {
                self.apply_variable_phase(oscillator_fn, mod_buffer.clone())
            }
            Modulation::ByFrequency { mod_buffer } => {
                self.apply_variable_frequency(oscillator_fn, mod_buffer.clone())
            }
        }
    }

    fn apply_no_modulation(
        &self,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
        mut phase: f64,
    ) -> Stage<C::Storage> {
        let mut frequency = self.frequency.clone();
        let mut out_spec = self.out_spec.clone();

        Box::new(move |buffers, control| {
            let frequency = frequency.next(control);
            buffers.read_0_and_write(&mut out_spec, control, || {
                let signal = oscillator_fn(phase);
                phase = (phase + control.sample_secs * frequency).rem_euclid(1.0);
                signal
            })
        })
    }

    fn apply_variable_phase(
        &self,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
        in_buffer: InBuffer,
    ) -> Stage<C::Storage> {
        let mut frequency = self.frequency.clone();
        let mut out_spec = self.out_spec.clone();

        let mut phase = 0.0;
        Box::new(move |buffers, control| {
            let frequency = frequency.next(control);
            buffers.read_1_and_write(&in_buffer, &mut out_spec, control, |s| {
                let signal = oscillator_fn((phase + s).rem_euclid(1.0));
                phase = (phase + control.sample_secs * frequency).rem_euclid(1.0);
                signal
            })
        })
    }

    fn apply_variable_frequency(
        &self,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
        in_buffer: InBuffer,
    ) -> Stage<C::Storage> {
        let mut frequency = self.frequency.clone();
        let mut out_spec = self.out_spec.clone();

        let mut phase = 0.0;
        Box::new(move |buffers, control| {
            let frequency = frequency.next(control);
            buffers.read_1_and_write(&in_buffer, &mut out_spec, control, |s| {
                let signal = oscillator_fn(phase);
                phase = (phase + control.sample_secs * (frequency + s)).rem_euclid(1.0);
                signal
            })
        })
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct StringSim<C> {
    pub buffer_size_secs: f64,
    pub frequency: LfSource<C>,
    pub cutoff: LfSource<C>,
    pub feedback: LfSource<C>,
    pub pluck_location: LfSource<C>,
    #[serde(flatten)]
    pub out_spec: OutSpec<C>,
}

impl<C: Controller> StringSim<C> {
    pub fn create_stage(&self) -> Stage<C::Storage> {
        let mut out_spec = self.out_spec.clone();
        let num_samples_in_buffer = (self.buffer_size_secs * DEFAULT_SAMPLE_RATE).ceil() as usize;

        let low_pass = OnePoleLowPass::new(0.0, DEFAULT_SAMPLE_RATE);
        let mut comb_filter = CombFilter::new(num_samples_in_buffer, 0.0, low_pass);

        let mut samples_processed = 0;

        let mut frequency = self.frequency.clone();
        let mut cutoff = self.cutoff.clone();
        let mut feedback = self.feedback.clone();
        let mut pluck_location = self.pluck_location.clone();
        Box::new(move |buffers, control| {
            let frequency = frequency.next(control);
            let cutoff = cutoff.next(control);
            let feedback = feedback.next(control);
            let pluck_location = pluck_location.next(control);

            comb_filter.set_feedback(-feedback);
            let low_pass = comb_filter.feedback_fn();
            low_pass.set_cutoff(cutoff, DEFAULT_SAMPLE_RATE);
            let intrinsic_delay = low_pass.intrinsic_delay_samples();

            let num_samples_to_skip_back = DEFAULT_SAMPLE_RATE / 2.0 / frequency - intrinsic_delay;

            // Subtract 1.0 since the first skip-back sample is implicit
            let offset = (num_samples_to_skip_back - 1.0) / num_samples_in_buffer as f64;

            let counter_wave_at =
                (pluck_location.max(0.0).min(1.0) * DEFAULT_SAMPLE_RATE / 2.0 / frequency).round()
                    as usize;

            buffers.read_0_and_write(&mut out_spec, control, || {
                let input = if samples_processed > counter_wave_at {
                    samples_processed += 1;
                    0.0
                } else if samples_processed == 0 || samples_processed == counter_wave_at {
                    samples_processed += 1;
                    1.0
                } else {
                    samples_processed += 1;
                    0.0
                };
                comb_filter.process_sample_fract(offset.max(0.0).min(1.0), input)
            })
        })
    }
}
