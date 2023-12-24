extern crate clap;
extern crate flate2;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate fern;
extern crate serde_json;
extern crate toml;

mod config;
mod dctr;
mod devnull;
mod e3dc;
mod mennekes;
mod pac2200;
mod timeouter;

mod decompress_stream;
mod energy_meter;
mod residual_current_monitor;
mod wallbox_manager;

use clap::{Args, Parser, Subcommand};
use dctr::Dctr;
use devnull::DevNullFile;
use e3dc::E3DC;
use mennekes::Mennekes;
use pac2200::Pac2200;
use std::path::PathBuf;
use timeouter::Timeouter;

use decompress_stream::decompress_stream;
use energy_meter::energy_meter;
use residual_current_monitor::residual_current_monitor;
use wallbox_manager::wallbox_manager;

const MODBUS_DEFAULT_PORT: u16 = 502;

/// Modbus manager
#[derive(Debug, Parser)] // requires `derive` feature
#[command(name = "mb")]
#[command(about = "Handles Wallbox, PV system and PAC energy meter", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Charge your electric car with your PV system's surplus energy
    #[command(arg_required_else_help = true)]
    WallboxManager(WallboxManagerParams),

    /// Read out, make available and log your energy meter's
    /// measurements
    #[command(arg_required_else_help = true)]
    EnergyMeter(EnergyMeterParams),

    /// Decompress incomplete gzip streams
    #[command(arg_required_else_help = true)]
    DecompressStream(DecompressStreamParams),

    /// Monitor and logresidual currents and take action
    /// when defined thresholds are exceeded
    #[command(arg_required_else_help = true)]
    ResidualCurrentMonitor(ResidualCurrentMonitorParams),
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct WallboxManagerParams {
    #[arg(short, long)]
    pub config_path: PathBuf,
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct EnergyMeterParams {
    #[arg(short = 'H', long)]
    pub meter_host: String,

    #[arg(short = 'P', long)]
    pub meter_port: Option<u16>,

    #[arg(short, long)]
    pub bind_to: Option<String>,

    #[arg(short, long)]
    pub polling_interval: Option<u64>,

    #[arg(short, long)]
    pub log_to: Option<PathBuf>,

    #[arg(short = 'i', long)]
    pub log_flush_interval: Option<u64>,

    #[arg(short = 'c', long)]
    pub write_to_console: Option<bool>,
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct DecompressStreamParams {
    #[arg(short, long)]
    pub file_name: Vec<PathBuf>,

    #[arg(short, long)]
    pub output_to_sqlite: Option<String>,
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct ResidualCurrentMonitorParams {
    /// The RCM's hostname to connect to. Please note that the
    /// Doepke RCM's have modbus disabled by default and you'll
    /// need to use their proprietary software to enable modbus.
    /// This usually has to be done only once per RCM. Do note
    /// that not all versions of the software support enabling
    /// the modbus; if in doubt, contact the manufacturer for
    /// the right version.
    #[arg(short = 'H', long)]
    pub host_name: String,

    /// The RCM's port to connect to
    #[arg(short = 'P', long)]
    pub port: Option<u16>,

    /// The host+port to bind to. TCP connections can be made
    /// to this port to receive current data from the RCM system.
    #[arg(short, long)]
    pub bind_to: Option<String>,

    /// The polling interval in seconds. 1 is a reasonable default.
    #[arg(short, long)]
    pub polling_interval: Option<u64>,

    /// A *directory* to log to. The log file name is based
    /// on the current EPOCH time so old logs aren't overwritten
    /// on restart. Since the logs are compressed, we cannot simply
    /// use the append mode reliably.
    #[arg(short, long)]
    pub log_to: Option<PathBuf>,

    /// The log flush interval in seconds. This
    /// can be a high value such as 3600 to preserve
    /// the lifetime of flash memory
    #[arg(short = 'i', long)]
    pub log_flush_interval: Option<u64>,

    /// Log the output to the console
    /// the process is attached to
    #[arg(short = 'c', long)]
    pub write_to_console: Option<bool>,

    /// A unix command to execute when the RCM triggers
    /// the alarm of type A.
    #[arg(long)]
    pub alarm_a_command: Option<String>,

    /// A unix command to execute when the RCM triggers
    /// the alarm of type B.
    #[arg(long)]
    pub alarm_b_command: Option<String>,
}

fn main() {
    let args = Cli::parse();

    let result = match args.command {
        Commands::DecompressStream(dsp) => decompress_stream(dsp),
        Commands::EnergyMeter(emp) => energy_meter(emp),
        Commands::WallboxManager(cmp) => wallbox_manager(cmp),
        Commands::ResidualCurrentMonitor(rcm) => residual_current_monitor(rcm),
    };
    if let Err(e) = result {
        eprintln!("Error: {:?}", e);
        std::process::exit(1);
    }
}
