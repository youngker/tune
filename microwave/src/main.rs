mod assets;
mod audio;
mod bench;
mod control;
mod fluid;
mod keyboard;
mod keypress;
mod magnetron;
mod midi;
mod model;
mod piano;
mod synth;
mod task;
mod tunable;
mod view;

use std::{cell::RefCell, env, io, path::PathBuf, sync::mpsc};

use ::magnetron::spec::Creator;
use assets::MicrowaveConfig;
use audio::{AudioModel, AudioOptions, AudioStage};
use clap::Parser;
use control::{LiveParameter, LiveParameterMapper, LiveParameterStorage, ParameterValue};
use keyboard::KeyboardLayout;
use model::{Model, SourceId};
use nannou::{
    app::{self, App},
    wgpu::Backends,
};
use piano::{Backend, NoAudio, PianoEngine};
use ringbuf::RingBuffer;
use tune::{
    key::{Keyboard, PianoKey},
    note::NoteLetter,
    pitch::Ratio,
    scala::{Kbm, Scl},
    temperament::{EqualTemperament, TemperamentPreference},
};
use tune_cli::{
    shared::{
        self,
        midi::{MidiInArgs, MidiOutArgs, TuningMethod},
        KbmOptions, SclCommand,
    },
    CliResult,
};

#[derive(Parser)]
#[command(version)]
enum MainOptions {
    /// Start the microwave GUI
    #[command(name = "run")]
    Run(RunOptions),

    /// Use a keyboard mapping with the given reference note
    #[command(name = "ref-note")]
    WithRefNote {
        #[command(flatten)]
        kbm: KbmOptions,

        #[command(flatten)]
        options: RunOptions,
    },

    /// Use a kbm file
    #[command(name = "kbm-file")]
    UseKbmFile {
        /// The location of the kbm file to import
        kbm_file_location: PathBuf,

        #[command(flatten)]
        options: RunOptions,
    },

    /// List MIDI devices
    #[command(name = "devices")]
    Devices,

    /// Run benchmark
    #[command(name = "bench")]
    Bench {
        /// Analyze benchmark
        #[arg(long = "analyze")]
        analyze: bool,
    },
}

const TUN_METHOD_ARG: &str = "tun-method";
#[derive(Parser)]
struct RunOptions {
    /// MIDI input device
    #[arg(long = "midi-in")]
    midi_in_device: Option<String>,

    #[command(flatten)]
    midi_in_args: MidiInArgs,

    /// MIDI output device
    #[arg(long = "midi-out")]
    midi_out_device: Option<String>,

    #[command(flatten)]
    midi_out_args: MidiOutArgs,

    /// MIDI-out tuning method
    #[arg(long = TUN_METHOD_ARG)]
    midi_tuning_method: Option<TuningMethod>,

    /// Waveforms file location (waveform synth)
    #[arg(
        long = "cfg-loc",
        env = "MICROWAVE_CFG_LOC",
        default_value = "microwave.yml"
    )]
    waveforms_file_location: PathBuf,

    /// Number of waveform buffers to allocate
    #[arg(long = "wv-bufs", default_value = "8")]
    num_waveform_buffers: usize,

    #[command(flatten)]
    control_change: ControlChangeParameters,

    /// Enable logging
    #[arg(long = "log")]
    logging: bool,

    /// Enable soundfont rendering using the soundfont file at the given location
    #[arg(long = "sf-loc", env = "MICROWAVE_SF_LOC")]
    soundfont_file_location: Option<PathBuf>,

    #[command(flatten)]
    audio: AudioParameters,

    /// Program number that should be selected at startup
    #[arg(long = "pg", default_value = "0")]
    program_number: u8,

    /// Use porcupine layout when possible
    #[arg(long = "porcupine")]
    use_porcupine: bool,

    /// Primary step width (right direction) when playing on the computer keyboard
    #[arg(long = "p-step")]
    primary_step: Option<i16>,

    /// Secondary step width (down/right direction) when playing on the computer keyboard
    #[arg(long = "s-step")]
    secondary_step: Option<i16>,

    /// Physical keyboard layout.
    /// [ansi] Large backspace key, horizontal enter key, large left shift key.
    /// [var] Subdivided backspace key, large enter key, large left shift key.
    /// [iso] Large backspace key, vertical enter key, subdivided left shift key.
    #[arg(long = "keyb", default_value = "iso")]
    keyboard_layout: KeyboardLayout,

    /// Odd limit for frequency ratio indicators
    #[arg(long = "lim", default_value = "11")]
    odd_limit: u16,

    /// Render a second scale-specific keyboard using the given color pattern (e.g. wgrwwgrwgrwgrwwgr for 17-EDO)
    #[arg(long = "kb2", value_parser = parse_keyboard_colors)]
    second_keyboard_colors: Option<KeyColors>,

    #[command(subcommand)]
    scl: Option<SclCommand>,
}

#[derive(Parser)]
struct ControlChangeParameters {
    /// Modulation control number - generic controller
    #[arg(long = "modulation-ccn", default_value = "1")]
    modulation_ccn: u8,

    /// Breath control number - triggered by vertical mouse movement
    #[arg(long = "breath-ccn", default_value = "2")]
    breath_ccn: u8,

    /// Foot switch control number - controls recording
    #[arg(long = "foot-ccn", default_value = "4")]
    foot_ccn: u8,

    /// Volume control number - controls magnetron output level
    #[arg(long = "volume-ccn", default_value = "7")]
    volume_ccn: u8,

    /// Expression control number - generic controller
    #[arg(long = "expression-ccn", default_value = "11")]
    expression_ccn: u8,

    /// Damper pedal control number - generic controller
    #[arg(long = "damper-ccn", default_value = "64")]
    damper_ccn: u8,

    /// Sostenuto pedal control number - generic controller
    #[arg(long = "sostenuto-ccn", default_value = "66")]
    sostenuto_ccn: u8,

    /// Soft pedal control number - generic controller
    #[arg(long = "soft-ccn", default_value = "67")]
    soft_ccn: u8,

    /// Legato switch control number - controls legato option
    #[arg(long = "legato-ccn", default_value = "68")]
    legato_ccn: u8,

    /// Sound 1 control number. Triggered by F1 key
    #[arg(long = "sound-1-ccn", default_value = "70")]
    sound_1_ccn: u8,

    /// Sound 2 control number. Triggered by F2 key
    #[arg(long = "sound-2-ccn", default_value = "71")]
    sound_2_ccn: u8,

    /// Sound 3 control number. Triggered by F3 key
    #[arg(long = "sound-3-ccn", default_value = "72")]
    sound_3_ccn: u8,

    /// Sound 4 control number. Triggered by F4 key
    #[arg(long = "sound-4-ccn", default_value = "73")]
    sound_4_ccn: u8,

    /// Sound 5 control number. Triggered by F5 key
    #[arg(long = "sound-5-ccn", default_value = "74")]
    sound_5_ccn: u8,

    /// Sound 6 control number. Triggered by F6 key
    #[arg(long = "sound-6-ccn", default_value = "75")]
    sound_6_ccn: u8,

    /// Sound 7 control number. Triggered by F7 key
    #[arg(long = "sound-7-ccn", default_value = "76")]
    sound_7_ccn: u8,

    /// Sound 8 control number. Triggered by F8 key
    #[arg(long = "sound-8-ccn", default_value = "77")]
    sound_8_ccn: u8,

    /// Sound 9 control number. Triggered by F9 key
    #[arg(long = "sound-9-ccn", default_value = "78")]
    sound_9_ccn: u8,

    /// Sound 10 control number. Triggered by F10 key
    #[arg(long = "sound-10-ccn", default_value = "79")]
    sound_10_ccn: u8,
}

#[derive(Parser)]
struct AudioParameters {
    /// Enable audio-in
    #[arg(long = "audio-in")]
    audio_in_enabled: bool,

    /// Audio-out buffer size in frames
    #[arg(long = "out-buf", default_value = "1024")]
    out_buffer_size: u32,

    /// Audio-in buffer size in frames
    #[arg(long = "in-buf", default_value = "1024")]
    in_buffer_size: u32,

    /// Size of the ring buffer piping data from audio-in to audio-out in frames
    #[arg(long = "exc-buf", default_value = "8192")]
    exchange_buffer_size: usize,

    /// Sample rate [Hz]. If no value is specified the audio device's preferred value will be used
    #[arg(long = "s-rate")]
    sample_rate: Option<u32>,

    /// Prefix for wav file recordings
    #[arg(long = "wav-prefix", default_value = "microwave")]
    wav_file_prefix: String,
}

#[derive(Clone)]
struct KeyColors(Vec<KeyColor>);

#[derive(Clone, Copy)]
pub enum KeyColor {
    White,
    Red,
    Green,
    Blue,
    Cyan,
    Magenta,
    Yellow,
    Black,
}

fn parse_keyboard_colors(src: &str) -> Result<KeyColors, String> {
    src.chars()
        .map(|c| match c {
            'w' => Ok(KeyColor::White),
            'r' => Ok(KeyColor::Red),
            'g' => Ok(KeyColor::Green),
            'b' => Ok(KeyColor::Blue),
            'c' => Ok(KeyColor::Cyan),
            'm' => Ok(KeyColor::Magenta),
            'y' => Ok(KeyColor::Yellow),
            'k' => Ok(KeyColor::Black),
            c => Err(c),
        })
        .collect::<Result<Vec<_>, char>>()
        .map(KeyColors)
        .map_err(|c| format!("Received an invalid character '{c}'. Only wrgbcmyk are allowed."))
}

fn main() {
    let options = if env::args().len() < 2 {
        println!("[WARNING] Use a subcommand, e.g. `microwave run` to start microwave properly");
        MainOptions::parse_from(["microwave", "run"])
    } else {
        MainOptions::parse()
    };

    match create_model_from_main_options(options) {
        Ok(None) => {}
        Ok(Some(model)) => run_app(model),
        Err(err) => {
            eprintln!("[FAIL] {err:?}");
        }
    }
}

fn create_model_from_main_options(options: MainOptions) -> CliResult<Option<Model>> {
    match options {
        MainOptions::Run(options) => create_model_from_run_options(
            Kbm::builder(NoteLetter::D.in_octave(4)).build()?,
            options,
        )
        .map(Some),
        MainOptions::WithRefNote { kbm, options } => {
            create_model_from_run_options(kbm.to_kbm()?, options).map(Some)
        }
        MainOptions::UseKbmFile {
            kbm_file_location,
            options,
        } => create_model_from_run_options(shared::import_kbm_file(&kbm_file_location)?, options)
            .map(Some),
        MainOptions::Devices => {
            let stdout = io::stdout();
            shared::midi::print_midi_devices(stdout.lock(), "microwave")?;
            Ok(None)
        }
        MainOptions::Bench { analyze } => {
            if analyze {
                bench::analyze_benchmark()?;
            } else {
                bench::run_benchmark()?;
            }
            Ok(None)
        }
    }
}

fn create_model_from_run_options(kbm: Kbm, options: RunOptions) -> CliResult<Model> {
    let scl = options
        .scl
        .as_ref()
        .map(|command| command.to_scl(None))
        .transpose()
        .map_err(|x| format!("error ({x:?})"))?
        .unwrap_or_else(|| {
            Scl::builder()
                .push_ratio(Ratio::from_semitones(1))
                .build()
                .unwrap()
        });

    let keyboard = create_keyboard(&scl, &options);

    let (info_send, info_recv) = mpsc::channel();

    let (audio_in_prod, audio_in_cons) =
        RingBuffer::new(options.audio.exchange_buffer_size * 2).split();
    let mut audio_stages = Vec::<Box<dyn AudioStage<((), LiveParameterStorage)>>>::new();
    let mut backends = Vec::<Box<dyn Backend<SourceId>>>::new();

    if let Some(target_port) = options.midi_out_device {
        let midi_backend = midi::create(
            info_send.clone(),
            &target_port,
            options.midi_out_args,
            options
                .midi_tuning_method
                .ok_or_else(|| format!("MIDI out requires --{TUN_METHOD_ARG} argument"))?,
        )?;
        backends.push(Box::new(midi_backend));
    }

    let output_stream_params =
        audio::get_output_stream_params(options.audio.out_buffer_size, options.audio.sample_rate);
    let sample_rate = output_stream_params.1.sample_rate;
    let sample_rate_hz_u32 = sample_rate.0;
    let sample_rate_hz_f64 = f64::from(sample_rate_hz_u32);

    let (fluid_backend, fluid_synth) = fluid::create(
        info_send.clone(),
        options.soundfont_file_location.as_deref(),
        sample_rate_hz_f64,
    )?;
    if options.soundfont_file_location.is_some() {
        backends.push(Box::new(fluid_backend));
        audio_stages.push(Box::new(fluid_synth));
    }

    let mut config = MicrowaveConfig::load(&options.waveforms_file_location)?;

    let effect_templates = config
        .effect_templates
        .drain(..)
        .map(|spec| (spec.name, spec.value))
        .collect();

    let creator = Creator::new(effect_templates, Default::default());

    let effects: Vec<_> = config
        .effects
        .iter()
        .map(|spec| creator.create(spec))
        .collect();

    let (waveform_backend, waveform_synth) = synth::create(
        info_send.clone(),
        config,
        options.num_waveform_buffers,
        options.audio.out_buffer_size,
        sample_rate_hz_f64,
        audio_in_cons,
    );
    backends.push(Box::new(waveform_backend));
    audio_stages.push(Box::new(waveform_synth));
    backends.push(Box::new(NoAudio::new(info_send)));
    for effect in effects {
        audio_stages.push(effect);
    }

    let mut storage = LiveParameterStorage::default();
    storage.set_parameter(LiveParameter::Volume, 100.0.as_f64());
    storage.set_parameter(LiveParameter::Legato, 1.0);

    let (storage_send, storage_recv) = mpsc::channel();

    let (engine, engine_snapshot) = PianoEngine::new(
        scl.clone(),
        kbm,
        backends,
        options.program_number,
        options.control_change.to_parameter_mapper(),
        storage,
        storage_send,
    );

    let audio = AudioModel::new(
        audio_stages,
        output_stream_params,
        options.audio.into_options(),
        storage,
        storage_recv,
        audio_in_prod,
    );

    let midi_in = options
        .midi_in_device
        .map(|midi_in_device| {
            midi::connect_to_midi_device(
                engine.clone(),
                &midi_in_device,
                options.midi_in_args,
                options.logging,
            )
        })
        .transpose()?
        .map(|(_, connection)| connection);

    Ok(Model::new(
        audio,
        engine,
        engine_snapshot,
        scl,
        options
            .second_keyboard_colors
            .map(|colors| colors.0)
            .unwrap_or_else(Vec::new),
        keyboard,
        options.keyboard_layout,
        options.odd_limit,
        midi_in,
        info_recv,
    ))
}

fn create_keyboard(scl: &Scl, config: &RunOptions) -> Keyboard {
    let preference = if config.use_porcupine {
        TemperamentPreference::Porcupine
    } else {
        TemperamentPreference::PorcupineWhenMeantoneIsBad
    };

    let average_step_size = scl.period().divided_into_equal_steps(scl.num_items());

    let temperament = EqualTemperament::find()
        .with_preference(preference)
        .by_step_size(average_step_size);

    let keyboard = Keyboard::root_at(PianoKey::from_midi_number(0))
        .with_steps_of(&temperament)
        .coprime();

    let primary_step = config
        .primary_step
        .unwrap_or_else(|| keyboard.primary_step());
    let secondary_step = config
        .secondary_step
        .unwrap_or_else(|| keyboard.secondary_step());

    keyboard.with_steps(primary_step, secondary_step)
}

fn run_app(model: Model) {
    // Since ModelFn is not a closure we need this workaround to pass the calculated model
    thread_local!(static MODEL: RefCell<Option<Model>> = Default::default());

    MODEL.with(|m| m.borrow_mut().replace(model));

    app::Builder::new(|app| {
        create_window(app);
        MODEL.with(|m| m.borrow_mut().take().unwrap())
    })
    .backends(Backends::PRIMARY | Backends::GL)
    .update(model::update)
    .run();
}

fn create_window(app: &App) {
    app.new_window()
        .maximized(true)
        .title("Microwave - Microtonal Waveform Synthesizer by Woyten")
        .raw_event(model::raw_event)
        .key_pressed(model::key_pressed)
        .mouse_pressed(model::mouse_pressed)
        .mouse_moved(model::mouse_moved)
        .mouse_released(model::mouse_released)
        .mouse_wheel(model::mouse_wheel)
        .touch(model::touch)
        .view(view::view)
        .build()
        .unwrap();
}

impl ControlChangeParameters {
    fn to_parameter_mapper(&self) -> LiveParameterMapper {
        let mut mapper = LiveParameterMapper::new();
        mapper.push_mapping(LiveParameter::Modulation, self.modulation_ccn);
        mapper.push_mapping(LiveParameter::Breath, self.breath_ccn);
        mapper.push_mapping(LiveParameter::Foot, self.foot_ccn);
        mapper.push_mapping(LiveParameter::Volume, self.volume_ccn);
        mapper.push_mapping(LiveParameter::Expression, self.expression_ccn);
        mapper.push_mapping(LiveParameter::Damper, self.damper_ccn);
        mapper.push_mapping(LiveParameter::Sostenuto, self.sostenuto_ccn);
        mapper.push_mapping(LiveParameter::Soft, self.soft_ccn);
        mapper.push_mapping(LiveParameter::Legato, self.legato_ccn);
        mapper.push_mapping(LiveParameter::Sound1, self.sound_1_ccn);
        mapper.push_mapping(LiveParameter::Sound2, self.sound_2_ccn);
        mapper.push_mapping(LiveParameter::Sound3, self.sound_3_ccn);
        mapper.push_mapping(LiveParameter::Sound4, self.sound_4_ccn);
        mapper.push_mapping(LiveParameter::Sound5, self.sound_5_ccn);
        mapper.push_mapping(LiveParameter::Sound6, self.sound_6_ccn);
        mapper.push_mapping(LiveParameter::Sound7, self.sound_7_ccn);
        mapper.push_mapping(LiveParameter::Sound8, self.sound_8_ccn);
        mapper.push_mapping(LiveParameter::Sound9, self.sound_9_ccn);
        mapper.push_mapping(LiveParameter::Sound10, self.sound_10_ccn);
        mapper
    }
}

impl AudioParameters {
    fn into_options(self) -> AudioOptions {
        AudioOptions {
            audio_in_enabled: self.audio_in_enabled,
            output_buffer_size: self.out_buffer_size,
            input_buffer_size: self.in_buffer_size,
            exchange_buffer_size: self.exchange_buffer_size,
            wav_file_prefix: self.wav_file_prefix,
        }
    }
}
