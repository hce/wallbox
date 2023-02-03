extern crate clap;
extern crate flate2;
extern crate serde;
#[macro_use]
extern crate serde_derive;
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
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct DecompressStreamParams {
    #[arg(short, long)]
    pub file_name: PathBuf,
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct ResidualCurrentMonitorParams {
    #[arg(short = 'H', long)]
    pub host_name: String,

    #[arg(short = 'P', long)]
    pub port: Option<u16>,

    #[arg(short, long)]
    pub bind_to: Option<String>,

    #[arg(short, long)]
    pub polling_interval: Option<u64>,

    #[arg(short, long)]
    pub log_to: Option<PathBuf>,

    #[arg(short = 'i', long)]
    pub log_flush_interval: Option<u64>,
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
