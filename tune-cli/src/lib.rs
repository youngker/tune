mod dto;
mod est;
mod live;
mod mos;
mod mts;
mod scala;
mod scale;

use std::{
    fmt::{self, Debug, Display},
    fs::File,
    io::{self, Write},
    path::PathBuf,
};

use clap::Parser;
use est::EstOptions;
use io::Read;
use live::LiveOptions;
use mos::MosCommand;
use mts::MtsOptions;
use scala::{KbmCommand, SclOptions};
use scale::{DiffOptions, DumpOptions, ScaleCommand};
use shared::midi;
use tune::scala::{KbmBuildError, SclBuildError};

#[doc(hidden)]
pub mod shared;

#[derive(Parser)]
#[command(version)]
struct MainOptions {
    /// Write output to a file instead of stdout
    #[arg(long = "of")]
    output_file: Option<PathBuf>,

    #[command(subcommand)]
    command: MainCommand,
}

#[derive(Parser)]
enum MainCommand {
    /// Create a scale file
    #[command(name = "scl")]
    Scl(SclOptions),

    /// Create a keyboard mapping file
    #[command(subcommand, name = "kbm")]
    Kbm(KbmCommand),

    /// Analyze equal-step tunings
    #[command(name = "est")]
    Est(EstOptions),

    /// Find MOS scales from generators or vice versa
    #[command(subcommand, name = "mos")]
    Mos(MosCommand),

    /// Print a scale to stdout
    #[command(subcommand, name = "scale")]
    Scale(ScaleCommand),

    /// Display details of a scale
    #[command(name = "dump")]
    Dump(DumpOptions),

    /// Display differences between a source scale and a target scale
    #[command(name = "diff")]
    Diff(DiffOptions),

    /// Print MIDI Tuning Standard messages and/or send them to MIDI devices
    #[command(name = "mts")]
    Mts(MtsOptions),

    /// Enable synthesizers with limited tuning support to be played in any tuning.
    /// This is achieved by reading MIDI data from a sequencer/keyboard and sending modified MIDI data to a synthesizer.
    /// The sequencer/keyboard and synthesizer can be the same device. In this case, remember to disable local keyboard playback.
    #[command(name = "live")]
    Live(LiveOptions),

    /// List MIDI devices
    #[command(name = "devices")]
    Devices,
}

impl MainOptions {
    fn run(self) -> Result<(), CliError> {
        let output: Box<dyn Write> = match self.output_file {
            Some(output_file) => Box::new(File::create(output_file)?),
            None => Box::new(io::stdout()),
        };

        let mut app = App {
            input: Box::new(io::stdin()),
            output,
            error: Box::new(io::stderr()),
        };

        self.command.run(&mut app)
    }
}

impl MainCommand {
    fn run(self, app: &mut App) -> CliResult<()> {
        match self {
            MainCommand::Scl(options) => options.run(app)?,
            MainCommand::Kbm(options) => options.run(app)?,
            MainCommand::Est(options) => options.run(app)?,
            MainCommand::Mos(options) => options.run(app)?,
            MainCommand::Scale(options) => options.run(app)?,
            MainCommand::Dump(options) => options.run(app)?,
            MainCommand::Diff(options) => options.run(app)?,
            MainCommand::Mts(options) => options.run(app)?,
            MainCommand::Live(options) => options.run(app)?,
            MainCommand::Devices => midi::print_midi_devices(&mut app.output, "tune-cli")?,
        }
        Ok(())
    }
}

pub fn run_in_shell_env(args: impl IntoIterator<Item = String>) -> CliResult<()> {
    let options = match MainOptions::try_parse_from(args) {
        Err(err) => {
            return if err.use_stderr() {
                Err(CliError::CommandError(err.to_string()))
            } else {
                println!("{err}");
                Ok(())
            };
        }
        Ok(options) => options,
    };

    options.run()
}

pub fn run_in_wasm_env(
    args: impl IntoIterator<Item = String>,
    input: impl Read,
    mut output: impl Write,
    error: impl Write,
) -> CliResult<()> {
    let command = match MainCommand::try_parse_from(args) {
        Err(err) => {
            return if err.use_stderr() {
                Err(CliError::CommandError(err.to_string()))
            } else {
                output.write_all(err.to_string().as_bytes())?;
                Ok(())
            }
        }
        Ok(command) => command,
    };

    let mut app = App {
        input: Box::new(input),
        output: Box::new(output),
        error: Box::new(error),
    };

    command.run(&mut app)
}

struct App<'a> {
    input: Box<dyn 'a + Read>,
    output: Box<dyn 'a + Write>,
    error: Box<dyn 'a + Write>,
}

impl App<'_> {
    pub fn write(&mut self, message: impl Display) -> io::Result<()> {
        write!(self.output, "{message}")
    }

    pub fn writeln(&mut self, message: impl Display) -> io::Result<()> {
        writeln!(self.output, "{message}")
    }

    pub fn errln(&mut self, message: impl Display) -> io::Result<()> {
        writeln!(self.error, "{message}")
    }

    pub fn read(&mut self) -> &mut dyn Read {
        &mut self.input
    }
}

pub type CliResult<T> = Result<T, CliError>;

pub enum CliError {
    IoError(io::Error),
    CommandError(String),
}

impl Debug for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::IoError(err) => write!(f, "IO error / {err}"),
            CliError::CommandError(err) => write!(f, "The command failed / {err}"),
        }
    }
}

impl From<String> for CliError {
    fn from(v: String) -> Self {
        CliError::CommandError(v)
    }
}

impl From<SclBuildError> for CliError {
    fn from(v: SclBuildError) -> Self {
        CliError::CommandError(format!("Could not create scale ({v:?})"))
    }
}

impl From<KbmBuildError> for CliError {
    fn from(v: KbmBuildError) -> Self {
        CliError::CommandError(format!("Could not create keyboard mapping ({v:?})"))
    }
}

impl From<io::Error> for CliError {
    fn from(v: io::Error) -> Self {
        CliError::IoError(v)
    }
}
