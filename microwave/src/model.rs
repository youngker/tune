use std::{
    collections::HashSet,
    ops::Deref,
    sync::{mpsc::Receiver, Arc},
};

use midir::MidiInputConnection;
use nannou::{
    event::{ElementState, KeyboardInput},
    prelude::*,
    winit::event::WindowEvent,
};
use tune::{
    key::{Keyboard, PianoKey},
    note::NoteLetter,
    pitch::{Pitch, Pitched, Ratio},
    scala::Scl,
};

use crate::{
    audio::AudioModel,
    control::LiveParameter,
    keyboard::{self, KeyboardLayout},
    piano::{PianoEngine, PianoEngineSnapshot},
    view::DynViewModel,
    KeyColor,
};

pub struct Model {
    pub audio: AudioModel,
    pub engine: Arc<PianoEngine>,
    pub engine_snapshot: PianoEngineSnapshot,
    pub scl: Scl,
    pub scl_key_colors: Vec<KeyColor>,
    pub reference_scl: Scl,
    pub keyboard: Keyboard,
    pub layout: KeyboardLayout,
    pub odd_limit: u16,
    pub midi_in: Option<MidiInputConnection<()>>,
    pub pitch_at_left_border: Pitch,
    pub pitch_at_right_border: Pitch,
    pub pressed_physical_keys: HashSet<(i8, i8)>,
    pub alt: bool,
    pub ctrl: bool,
    pub view_model: Option<DynViewModel>,
    pub view_updates: Receiver<DynViewModel>,
}

pub enum Event {
    Pressed(SourceId, Location, u8),
    Moved(SourceId, Location),
    Released(SourceId, u8),
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum SourceId {
    Mouse,
    Touchpad(u64),
    Keyboard(i8, i8),
    Midi(PianoKey),
}

pub enum Location {
    Pitch(Pitch),
    Degree(i32),
}

impl Model {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        audio: AudioModel,
        engine: Arc<PianoEngine>,
        engine_snapshot: PianoEngineSnapshot,
        scl: Scl,
        scl_key_colors: Vec<KeyColor>,
        keyboard: Keyboard,
        layout: KeyboardLayout,
        odd_limit: u16,
        midi_in: Option<MidiInputConnection<()>>,
        view_updates: Receiver<DynViewModel>,
    ) -> Self {
        Self {
            audio,
            engine,
            engine_snapshot,
            scl,
            scl_key_colors,
            reference_scl: Scl::builder().push_cents(100.0).build().unwrap(),
            keyboard,
            layout,
            odd_limit,
            midi_in,
            pitch_at_left_border: NoteLetter::A.in_octave(0).pitch(),
            pitch_at_right_border: NoteLetter::C.in_octave(8).pitch(),
            pressed_physical_keys: HashSet::new(),
            alt: false,
            ctrl: false,
            view_model: None,
            view_updates,
        }
    }

    pub fn update(&mut self) {
        for update in self.view_updates.try_iter() {
            self.view_model = Some(update);
        }
        self.engine.take_snapshot(&mut self.engine_snapshot);
    }

    pub fn keyboard_event(&mut self, (x, y): (i8, i8), pressed: bool) {
        let degree = self.keyboard.get_key(x.into(), y.into()).midi_number();

        let (event, net_change) = if pressed {
            (
                Event::Pressed(SourceId::Keyboard(x, y), Location::Degree(degree), 100),
                self.pressed_physical_keys.insert((x, y)),
            )
        } else {
            (
                Event::Released(SourceId::Keyboard(x, y), 100),
                self.pressed_physical_keys.remove(&(x, y)),
            )
        };

        // While a key is held down the pressed event is sent repeatedly. We ignore this case by checking net_change
        if net_change {
            self.engine.handle_event(event)
        }
    }
}

impl Deref for Model {
    type Target = PianoEngineSnapshot;
    fn deref(&self) -> &Self::Target {
        &self.engine_snapshot
    }
}

pub fn raw_event(_app: &App, model: &mut Model, event: &WindowEvent) {
    if let WindowEvent::KeyboardInput {
        input:
            KeyboardInput {
                scancode,
                state,
                virtual_keycode,
                ..
            },
        ..
    } = *event
    {
        let pressed = match state {
            ElementState::Pressed => true,
            ElementState::Released => false,
        };

        // We track modifiers by virtual key since winit(wasm32) confounds scancodes and virtual keycodes
        match virtual_keycode {
            Some(Key::LAlt | Key::RAlt) => model.alt = pressed,
            Some(Key::LControl | Key::RControl) => model.ctrl = pressed,
            _ => {}
        }

        if !model.alt {
            if let Some(key_coord) =
                keyboard::calc_hex_location(model.layout, scancode, virtual_keycode)
            {
                model.keyboard_event(key_coord, pressed);
            }
        }
    }
}

pub fn key_pressed(_app: &App, model: &mut Model, key: Key) {
    let engine = &model.engine;
    match key {
        Key::T if model.alt => engine.toggle_tuning_mode(),
        Key::E if model.alt => engine.toggle_envelope_type(),
        Key::O if model.alt => engine.toggle_synth_mode(),
        Key::L if model.alt => engine.toggle_parameter(LiveParameter::Legato),
        Key::F1 => engine.toggle_parameter(LiveParameter::Sound1),
        Key::F2 => engine.toggle_parameter(LiveParameter::Sound2),
        Key::F3 => engine.toggle_parameter(LiveParameter::Sound3),
        Key::F4 => engine.toggle_parameter(LiveParameter::Sound4),
        Key::F5 => engine.toggle_parameter(LiveParameter::Sound5),
        Key::F6 => engine.toggle_parameter(LiveParameter::Sound6),
        Key::F7 => engine.toggle_parameter(LiveParameter::Sound7),
        Key::F8 => engine.toggle_parameter(LiveParameter::Sound8),
        Key::F9 => engine.toggle_parameter(LiveParameter::Sound9),
        Key::F10 => engine.toggle_parameter(LiveParameter::Sound10),
        Key::Space => engine.toggle_parameter(LiveParameter::Foot),
        Key::Up if !model.alt => engine.dec_program(),
        Key::Down if !model.alt => engine.inc_program(),
        Key::Left if model.alt => engine.change_ref_note_by(-1),
        Key::Right if model.alt => engine.change_ref_note_by(1),
        Key::Left if !model.alt => engine.change_root_offset_by(-1),
        Key::Right if !model.alt => engine.change_root_offset_by(1),
        _ => {}
    }
}

pub fn mouse_pressed(app: &App, model: &mut Model, button: MouseButton) {
    if button == MouseButton::Left {
        position_event(
            app,
            model,
            app.mouse.position(),
            SourceId::Mouse,
            |location| Event::Pressed(SourceId::Mouse, location, 100),
        );
    }
}

pub fn mouse_moved(app: &App, model: &mut Model, position: Point2) {
    position_event(app, model, position, SourceId::Mouse, |location| {
        Event::Moved(SourceId::Mouse, location)
    });
}

pub fn mouse_released(_app: &App, model: &mut Model, button: MouseButton) {
    if button == MouseButton::Left {
        model
            .engine
            .handle_event(Event::Released(SourceId::Mouse, 100));
    }
}

pub fn mouse_wheel(
    _app: &App,
    model: &mut Model,
    mouse_scroll_delta: MouseScrollDelta,
    _: TouchPhase,
) {
    let (mut x_delta, mut y_delta) = match mouse_scroll_delta {
        MouseScrollDelta::LineDelta(x, y) => (10.0 * x as f64, 10.0 * y as f64),
        MouseScrollDelta::PixelDelta(pos) => (pos.x, pos.y),
    };

    if model.alt {
        let tmp = x_delta;
        x_delta = -y_delta;
        y_delta = tmp;
    }

    if x_delta.abs() > y_delta.abs() {
        let ratio = Ratio::between_pitches(model.pitch_at_left_border, model.pitch_at_right_border)
            .repeated(x_delta / 500.0);
        model.pitch_at_left_border = model.pitch_at_left_border * ratio;
        model.pitch_at_right_border = model.pitch_at_right_border * ratio;
    } else {
        let ratio = Ratio::from_semitones(y_delta / 10.0);
        let lowest = model.pitch_at_left_border * ratio;
        let highest = model.pitch_at_right_border / ratio;
        if lowest < highest {
            model.pitch_at_left_border = lowest;
            model.pitch_at_right_border = highest;
        }
    }
}

pub fn touch(app: &App, model: &mut Model, event: TouchEvent) {
    let id = SourceId::Touchpad(event.id);
    match event.phase {
        TouchPhase::Started => position_event(app, model, event.position, id, |location| {
            Event::Pressed(id, location, 100)
        }),
        TouchPhase::Moved => {
            position_event(app, model, event.position, id, |location| {
                Event::Moved(id, location)
            });
        }
        TouchPhase::Ended | TouchPhase::Cancelled => {
            model.engine.handle_event(Event::Released(id, 100))
        }
    }
}

fn position_event(
    app: &App,
    model: &Model,
    position: Point2,
    id: SourceId,
    to_event: impl Fn(Location) -> Event,
) {
    let x_normalized = position.x / app.window_rect().w() + 0.5;
    let y_normalized = position.y / app.window_rect().h() + 0.5;

    let keyboard_range =
        Ratio::between_pitches(model.pitch_at_left_border, model.pitch_at_right_border);
    let pitch = model.pitch_at_left_border * keyboard_range.repeated(x_normalized);

    if let SourceId::Mouse = id {
        model
            .engine
            .set_parameter(LiveParameter::Breath, y_normalized.into());
    }
    model.engine.handle_event(to_event(Location::Pitch(pitch)));
    model.engine.set_key_pressure(id, y_normalized.into());
}

pub fn update(_: &App, model: &mut Model, _: Update) {
    model.update()
}
