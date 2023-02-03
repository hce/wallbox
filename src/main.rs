#[macro_use]
extern crate clap;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate toml;

mod config;
mod e3dc;
mod mennekes;
mod pac2200;
mod timeouter;

mod energy_meter;
mod wallbox_manager;

use clap::{Args, Parser, Subcommand, ValueEnum};
use e3dc::E3DC;
use mennekes::Mennekes;
use pac2200::Pac2200;
use timeouter::Timeouter;

use energy_meter::energy_meter;
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
    #[command(arg_required_else_help = true)]
    WallboxManager(WallboxManagerParams),

    #[command(arg_required_else_help = true)]
    EnergyMeter(EnergyMeterParams),
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct WallboxManagerParams {
    #[arg(short, long)]
    pub config_path: std::path::PathBuf,
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
}

fn main() {
    let args = Cli::parse();

    let result = match args.command {
        Commands::EnergyMeter(emp) => energy_meter(emp),
        Commands::WallboxManager(cmp) => wallbox_manager(cmp),
    };
    if let Err(e) = result {
        eprintln!("Error: {:?}", e);
        std::process::exit(1);
    }
}
