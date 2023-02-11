use std::{
    collections::HashSet,
    fmt::{self, Write},
    ops::Range,
};
use geom::Range as NannouRange;
use nannou::prelude::*;
use nannou::color::rgb_u32;
use tune::{
    note::Note,
    pitch::{Pitch, Pitched, Ratio},
    scala::KbmRoot,
    tuning::Scale,
};
use tune_cli::shared::midi::TuningMethod;

use crate::{
    control::LiveParameter, fluid::FluidInfo, midi::MidiInfo, synth::WaveformInfo, KeyColor, Model,
};

pub trait ViewModel: Send + 'static {
    fn pitch_range(&self) -> Option<Range<Pitch>>;

    fn write_info(&self, target: &mut String) -> fmt::Result;
}

pub type DynViewModel = Box<dyn ViewModel>;

impl<T: ViewModel> From<T> for DynViewModel {
    fn from(data: T) -> Self {
        Box::new(data)
    }
}

pub fn view(app: &App, model: &Model, frame: Frame) {
    let draw = app.draw();
    let window_rect =
        app.window_rect().pad(app.window_rect().w() / 10.0);
    let total_range =
        Ratio::between_pitches(model.pitch_at_left_border, model.pitch_at_right_border);
    let octave_width = Ratio::octave().num_equal_steps_of_size(total_range) as f32;

    let kbm_root = model.kbm.kbm_root();
    let selected_tuning = (&model.scl, kbm_root);
    let reference_tuning = (
        &model.reference_scl,
        KbmRoot::from(Note::from_piano_key(kbm_root.ref_key)),
    );

    let keyboard_rect = Rect::from_w_h(window_rect.w(), window_rect.h() / 4.0);
    let lower_keyboard_rect = keyboard_rect.align_bottom_of(window_rect);

    draw.background().color(rgb_u32(0x2E3440));
    render_scale_lines(model, &draw, window_rect, octave_width, selected_tuning);
    render_keyboard(
        model,
        &draw,
        lower_keyboard_rect,
        octave_width,
        reference_tuning,
        |key| get_12edo_key_color(key + kbm_root.ref_key.midi_number()),
    );

    render_just_ratios_with_deviations(model, &draw, window_rect, octave_width);
    render_recording_indicator(model, &draw, window_rect);
    draw.to_frame(app, &frame).unwrap();
}

fn render_scale_lines(
    model: &Model,
    draw: &Draw,
    window_rect: Rect,
    octave_width: f32,
    tuning: impl Scale,
) {
    let leftmost_degree = tuning
        .find_by_pitch_sorted(model.pitch_at_left_border)
        .approx_value;
    let rightmost_degree = tuning
        .find_by_pitch_sorted(model.pitch_at_right_border)
        .approx_value;

    let pitch_range = model.view_model.as_ref().and_then(|m| m.pitch_range());

    for degree in leftmost_degree..=rightmost_degree {
        let pitch = tuning.sorted_pitch_of(degree);

        let pitch_position = Ratio::between_pitches(model.pitch_at_left_border, pitch).as_octaves()
            as f32
            * octave_width;

        let pitch_position_on_screen = (pitch_position - 0.5) * window_rect.w();

        let line_color = match pitch_range.as_ref().filter(|r| !r.contains(&pitch)) {
            None => GRAY,
            Some(_) => GRAY,
        };

        let line_color = match degree {
            0 => rgb_u32(0x434C5E),
            _ => rgb_u32(0x434C5E),
        };

        draw.line()
            .start(Point2::new(pitch_position_on_screen, window_rect.top()))
            .end(Point2::new(pitch_position_on_screen, window_rect.bottom()))
            .color(line_color)
            .weight(1.0);
        draw.ellipse()
            .x_y(pitch_position_on_screen, window_rect.top())
            .radius(2.0)
            .color(rgb_u32(0x4C566A));
    }
}

fn render_just_ratios_with_deviations(
    model: &Model,
    draw: &Draw,
    window_rect: Rect,
    octave_width: f32,
) {
    let mut freqs_hz = model
        .pressed_keys
        .values()
        .map(|pressed_key| pressed_key.pitch)
        .collect::<Vec<_>>();
    freqs_hz.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mut curr_slice_window = freqs_hz.as_slice();
    while let Some((second, others)) = curr_slice_window.split_last() {
        let pitch_position = Ratio::between_pitches(model.pitch_at_left_border, *second)
            .as_octaves() as f32
            * octave_width;

        let pitch_position_on_screen = (pitch_position - 0.5) * window_rect.w();

        draw.line()
            .start(Point2::new(pitch_position_on_screen, window_rect.top()))
            .end(Point2::new(pitch_position_on_screen, window_rect.bottom()))
            .color(rgb_u32(0x4C566A))
            .weight(2.0);

        let mut curr_rect = Rect {
            x: NannouRange::new(pitch_position_on_screen, pitch_position_on_screen + 1000.0),
            y: NannouRange::from_pos_and_len(0.0, 24.0),
        }
        .align_top_of(window_rect);

        // draw.text(&format!("{:.0} Hz", second.as_hz()))
        //     .xy(curr_rect.xy())
        //     .wh(curr_rect.wh())
        //     .left_justify()
        //     .color(RED)
        //     .font_size(24);

        for first in others.iter() {
            let approximation =
                Ratio::between_pitches(*first, *second).nearest_fraction(model.odd_limit);

            let width =
                approximation.deviation.as_octaves() as f32 * octave_width * (window_rect.w() - 100.0);
            let deviation_bar_rect = Rect {
                x: NannouRange::new(pitch_position_on_screen - width, pitch_position_on_screen),
                y: NannouRange::from_pos_and_len(0.0, 24.0),
            }
            .below(curr_rect);

            draw.rect()
                .xy(deviation_bar_rect.xy())
                .wh(deviation_bar_rect.wh())
                .color(DEEPSKYBLUE);

            let deviation_text_rect = curr_rect.below(curr_rect);

            draw.text(&format!(
                "{}/{} [{:.0}c]",
                approximation.numer,
                approximation.denom,
                approximation.deviation.as_cents().abs()
            ))
            .xy(deviation_text_rect.xy())
            .wh(deviation_text_rect.wh())
            .left_justify()
            .color(BLACK)
            .font_size(24);

            curr_rect = deviation_text_rect;
        }
        curr_slice_window = others;
    }
}

fn render_keyboard(
    model: &Model,
    draw: &Draw,
    rect: Rect,
    octave_width: f32,
    tuning: impl Scale,
    get_key_color: impl Fn(i32) -> KeyColor,
) {
    let highlighted_keys: HashSet<_> = model
        .pressed_keys
        .values()
        .map(|pressed_key| tuning.find_by_pitch_sorted(pressed_key.pitch).approx_value)
        .collect();

    let leftmost_key = tuning
        .find_by_pitch_sorted(model.pitch_at_left_border)
        .approx_value;
    let rightmost_key = tuning
        .find_by_pitch_sorted(model.pitch_at_right_border)
        .approx_value;

    let (mut mid, mut right) = Default::default();

    for iterated_key in (leftmost_key - 1)..=(rightmost_key + 1) {
        let pitch = tuning.sorted_pitch_of(iterated_key);
        let coord = Ratio::between_pitches(model.pitch_at_left_border, pitch).as_octaves() as f32
            * octave_width;

        let left = mid;
        mid = right;
        right = Some(coord);

        if let (Some(left), Some(mid), Some(right)) = (left, mid, right) {
            let drawn_key = iterated_key - 1;

            let mut key_color = match get_key_color(drawn_key) {
                KeyColor::White => rgb_u32(0x434C5E),
                KeyColor::Black => rgb_u32(0x4C566A),
                KeyColor::Red => DARKRED,
                KeyColor::Green => FORESTGREEN,
                KeyColor::Blue => MEDIUMBLUE,
                KeyColor::Cyan => LIGHTSEAGREEN,
                KeyColor::Magenta => MEDIUMVIOLETRED,
                KeyColor::Yellow => GOLDENROD,
            }
            .into_format::<f32>()
            .into_linear();

            if highlighted_keys.contains(&drawn_key) {
                let gray = DIMGRAY.into_format::<f32>().into_linear();
                key_color = (key_color + gray * 2.0) / 3.0;
            }

            let pos = (left + right) / 4.0 + mid / 2.0;
            let width = (left - right) / 2.0;

            let key_rect = Rect::from_x_y_w_h(
                rect.left() + pos * rect.w(),
                rect.y(),
                width * rect.w(),
                rect.h(),
            );

            draw.line()
                .start(Point2::new(key_rect.x(), key_rect.y()))
                .end(Point2::new(key_rect.x(), key_rect.y()-30.0))
                .color(key_color)
                .weight(4.0);
        }
    }
    draw.line()
        .start(Point2::new(rect.left(), rect.y()))
        .end(Point2::new(rect.right(), rect.y()))
        .color(rgb_u32(0x81A1C1))
        .weight(1.0);
}

fn render_recording_indicator(model: &Model, draw: &Draw, window_rect: Rect) {
    let rect = Rect::from_w_h(100.0, 100.0)
        .top_right_of(window_rect)
        .pad(10.0);
    if model.storage.is_active(LiveParameter::Foot) {
        draw.ellipse().xy(rect.xy()).wh(rect.wh()).color(FIREBRICK);
    }
}

fn get_12edo_key_color(key: i32) -> KeyColor {
    if [1, 3, 6, 8, 10].contains(&key.rem_euclid(12)) {
        KeyColor::Black
    } else {
        KeyColor::White
    }
}

impl ViewModel for WaveformInfo {
    fn pitch_range(&self) -> Option<Range<Pitch>> {
        None
    }

    fn write_info(&self, target: &mut String) -> fmt::Result {
        writeln!(
            target,
            "Output [Alt+O]: Waveform\n\
             Waveform [Up/Down]: {waveform_number} - {waveform_name}\n\
             Envelope [Alt+E]: {envelope_name}{is_default_indicator}",
            waveform_number = self.waveform_number,
            waveform_name = self.waveform_name,
            envelope_name = self.envelope_name,
            is_default_indicator = if self.is_default_envelope {
                ""
            } else {
                " (default) "
            }
        )
    }
}

impl ViewModel for FluidInfo {
    fn pitch_range(&self) -> Option<Range<Pitch>> {
        Some(Note::from_midi_number(0).pitch()..Note::from_midi_number(127).pitch())
    }

    fn write_info(&self, target: &mut String) -> fmt::Result {
        let tuning_method = match self.is_tuned {
            true => "Single Note Tuning Change",
            false => "None. Tuning channels exceeded! Change tuning mode.",
        };

        writeln!(
            target,
            "Output [Alt+O]: Soundfont\n\
             Soundfont File: {soundfont_file}\n\
             Tuning method: {tuning_method}\n\
             Program [Up/Down]: {program_number} - {program_name}",
            soundfont_file = self.soundfont_file_location.as_deref().unwrap_or("Unknown"),
            program_number = self
                .program
                .map(|p| p.to_string())
                .as_deref()
                .unwrap_or("Unknown"),
            program_name = self.program_name.as_deref().unwrap_or("Unknown"),
        )
    }
}

impl ViewModel for MidiInfo {
    fn pitch_range(&self) -> Option<Range<Pitch>> {
        Some(Note::from_midi_number(0).pitch()..Note::from_midi_number(127).pitch())
    }

    fn write_info(&self, target: &mut String) -> fmt::Result {
        let tuning_method = match self.tuning_method {
            Some(TuningMethod::FullKeyboard) => "Single Note Tuning Change",
            Some(TuningMethod::FullKeyboardRt) => "Single Note Tuning Change (realtime)",
            Some(TuningMethod::Octave1) => "Scale/Octave Tuning (1-Byte)",
            Some(TuningMethod::Octave1Rt) => "Scale/Octave Tuning (1-Byte) (realtime)",
            Some(TuningMethod::Octave2) => "Scale/Octave Tuning (2-Byte)",
            Some(TuningMethod::Octave2Rt) => "Scale/Octave Tuning (2-Byte) (realtime)",
            Some(TuningMethod::ChannelFineTuning) => "Channel Fine Tuning",
            Some(TuningMethod::PitchBend) => "Pitch Bend",
            None => "None. Tuning channels exceeded! Change tuning mode.",
        };

        writeln!(
            target,
            "Output [Alt+O]: MIDI\n\
             Device: {device}\n\
             Tuning method: {tuning_method}\n\
             Program [Up/Down]: {program_number}",
            device = self.device,
            program_number = self.program_number,
        )
    }
}

impl ViewModel for () {
    fn pitch_range(&self) -> Option<Range<Pitch>> {
        None
    }

    fn write_info(&self, target: &mut String) -> fmt::Result {
        writeln!(target, "Output [Alt+O]: No Audio")
    }
}
