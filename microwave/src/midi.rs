use std::{
    fmt::Debug,
    hash::Hash,
    io::Write,
    sync::{
        mpsc::{self, Sender},
        Arc,
    },
};

use midir::MidiInputConnection;
use tune::{
    midi::{ChannelMessage, ChannelMessageType},
    pitch::Pitch,
    scala::{KbmRoot, Scl},
    tuner::{MidiTunerMessage, MidiTunerMessageHandler, TunableMidi},
};
use tune_cli::{
    shared::midi::{self, MidiInArgs, MidiOutArgs, MidiSource, TuningMethod},
    CliResult,
};

use crate::{
    piano::{Backend, PianoEngine},
    tunable::TunableBackend,
};

pub struct MidiOutBackend<I, S> {
    info_sender: Sender<I>,
    device: String,
    tuning_method: TuningMethod,
    curr_program: usize,
    backend: TunableBackend<S, TunableMidi<MidiOutHandler>>,
}

pub fn create<I, S: Copy + Eq + Hash>(
    info_sender: Sender<I>,
    target_port: &str,
    midi_out_args: MidiOutArgs,
    tuning_method: TuningMethod,
) -> CliResult<MidiOutBackend<I, S>> {
    let (device, mut midi_out) = midi::connect_to_out_device("microwave", target_port)?;

    let (midi_send, midi_recv) = mpsc::channel::<MidiTunerMessage>();

    crate::task::spawn(async move {
        for message in midi_recv {
            message.send_to(|m| midi_out.send(m).unwrap());
        }
    });

    let target = midi_out_args.get_midi_target(MidiOutHandler { midi_send })?;
    let synth = midi_out_args.create_synth(target, tuning_method);

    Ok(MidiOutBackend {
        info_sender,
        device,
        tuning_method,
        curr_program: 0,
        backend: TunableBackend::new(synth),
    })
}

impl<I: From<MidiInfo> + Send, S: Copy + Eq + Hash + Debug + Send> Backend<S>
    for MidiOutBackend<I, S>
{
    fn set_tuning(&mut self, tuning: (&Scl, KbmRoot)) {
        self.backend.set_tuning(tuning);
    }

    fn set_no_tuning(&mut self) {
        self.backend.set_no_tuning();
    }

    fn send_status(&mut self) {
        let is_tuned = self.backend.is_tuned();

        self.info_sender
            .send(
                MidiInfo {
                    device: self.device.clone(),
                    program_number: self.curr_program,
                    tuning_method: is_tuned.then(|| self.tuning_method),
                }
                .into(),
            )
            .unwrap();
    }

    fn start(&mut self, id: S, degree: i32, pitch: Pitch, velocity: u8) {
        self.backend.start(id, degree, pitch, velocity);
    }

    fn update_pitch(&mut self, id: S, degree: i32, pitch: Pitch, velocity: u8) {
        self.backend.update_pitch(id, degree, pitch, velocity);
    }

    fn update_pressure(&mut self, id: S, pressure: u8) {
        self.backend.update_pressure(id, pressure);
    }

    fn stop(&mut self, id: S, velocity: u8) {
        self.backend.stop(id, velocity);
    }

    fn program_change(&mut self, mut update_fn: Box<dyn FnMut(usize) -> usize + Send>) {
        self.curr_program = update_fn(self.curr_program).min(127);

        self.backend
            .send_monophonic_message(ChannelMessageType::ProgramChange {
                program: u8::try_from(self.curr_program).unwrap(),
            });
    }

    fn control_change(&mut self, controller: u8, value: u8) {
        self.backend
            .send_monophonic_message(ChannelMessageType::ControlChange { controller, value });
    }

    fn channel_pressure(&mut self, pressure: u8) {
        self.backend
            .send_monophonic_message(ChannelMessageType::ChannelPressure { pressure });
    }

    fn pitch_bend(&mut self, value: i16) {
        self.backend
            .send_monophonic_message(ChannelMessageType::PitchBendChange { value });
    }

    fn toggle_envelope_type(&mut self) {}

    fn has_legato(&self) -> bool {
        true
    }
}

pub fn connect_to_midi_device(
    mut engine: Arc<PianoEngine>,
    target_port: &str,
    midi_in_args: MidiInArgs,
    midi_logging: bool,
) -> CliResult<(String, MidiInputConnection<()>)> {
    let midi_source = midi_in_args.get_midi_source()?;

    Ok(midi::connect_to_in_device(
        "microwave",
        target_port,
        move |message| process_midi_event(message, &mut engine, &midi_source, midi_logging),
    )?)
}

fn process_midi_event(
    message: &[u8],
    engine: &mut Arc<PianoEngine>,
    midi_source: &MidiSource,
    midi_logging: bool,
) {
    let stderr = std::io::stderr();
    let mut stderr = stderr.lock();
    if let Some(channel_message) = ChannelMessage::from_raw_message(message) {
        if midi_logging {
            writeln!(stderr, "[DEBUG] MIDI message received:").unwrap();
            writeln!(stderr, "{channel_message:#?}").unwrap();
            writeln!(stderr,).unwrap();
        }
        if midi_source.channels.contains(&channel_message.channel()) {
            engine.handle_midi_event(
                channel_message.message_type(),
                midi_source.get_offset(channel_message.channel()),
            );
        }
    } else {
        writeln!(stderr, "[WARNING] Unsupported MIDI message received:").unwrap();
        for i in message {
            writeln!(stderr, "{i:08b}").unwrap();
        }
        writeln!(stderr).unwrap();
    }
}

struct MidiOutHandler {
    midi_send: Sender<MidiTunerMessage>,
}

impl MidiTunerMessageHandler for MidiOutHandler {
    fn handle(&mut self, message: MidiTunerMessage) {
        self.midi_send.send(message).unwrap();
    }
}

pub struct MidiInfo {
    pub device: String,
    pub tuning_method: Option<TuningMethod>,
    pub program_number: usize,
}
