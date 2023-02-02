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

use clap::Parser;
use e3dc::E3DC;
use mennekes::Mennekes;
use pac2200::Pac2200;
use timeouter::Timeouter;

const MODBUS_DEFAULT_PORT: u16 = 502;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = "Wallbox charging manager")]
struct WallboxCommandLineParameters {
    #[arg(short, long)]
    config_path: std::path::PathBuf,
}

fn main() {
    let wclp: WallboxCommandLineParameters = WallboxCommandLineParameters::parse();

    let config: config::Config = {
        let config_file = std::fs::read_to_string(wclp.config_path).expect("Config file");
        toml::from_str(&config_file).expect("TOML parsing")
    };

    let e3dc = E3DC::new(
        &config.e3dc.host,
        config.e3dc.port.unwrap_or(MODBUS_DEFAULT_PORT),
        std::time::Duration::from_secs(2),
    )
    .expect("Create e3dc object");
    let mennekes = Mennekes::new(
        &config.wallbox.host,
        config.wallbox.port.unwrap_or(MODBUS_DEFAULT_PORT),
        std::time::Duration::from_secs(60),
    )
    .expect("Create mennekes object");
    let pac2200 = if let Some(pac_conn) = &config.pac2200 {
        Some(
            Pac2200::new(
                &pac_conn.host,
                pac_conn.port.unwrap_or(MODBUS_DEFAULT_PORT),
                std::time::Duration::from_secs(1),
            )
            .expect("Create pac2200 object"),
        )
    } else {
        None
    };

    let mut e3dcparams;
    let mut mennekesparams;
    let mut pac2200params;
    let t1 = Timeouter::new(config.initial_connection_timeout);
    loop {
        if let Some(n) = e3dc.get_current_params() {
            e3dcparams = n;
            break;
        }
        if !t1.ok() {
            eprintln!("Timeout while making initial connection to PV system");
            std::process::exit(10);
        }
    }
    let t2 = Timeouter::new(config.initial_connection_timeout);
    loop {
        if let Some(n) = mennekes.get_current_params() {
            mennekesparams = n;
            break;
        }
        if !t2.ok() {
            eprintln!("Timeout while making initial connection to wallbox");
            std::process::exit(11);
        }
    }
    if let Some(pac2200) = &pac2200 {
        let t3 = Timeouter::new(config.initial_connection_timeout);
        loop {
            if let Some(n) = pac2200.get_current_params() {
                pac2200params = Some(n);
                break;
            }
            if !t3.ok() {
                eprintln!("Timeout while making initial connection to PAC2200");
                std::process::exit(12);
            }
        }
    }
    loop {
        if let Some(n) = e3dc.get_current_params() {
            e3dcparams = n;
        }
        if let Some(n) = mennekes.get_current_params() {
            mennekesparams = n;
        }
        if let Some(pac2200) = &pac2200 {
            if let Some(n) = pac2200.get_current_params() {
                pac2200params = Some(n);
            }
        }

        if mennekesparams.control_pilot == 0 {
            eprintln!("No vehicle connected, setting MAX_AMPS to 8");
            mennekes.set_amps(8);
        } else {
            if mennekesparams.charging_duration < 180 {
                eprintln!("Vehicle connected for less than 3 minutes, signalling 8 amps");
                mennekes.set_amps(8);
            } else {
                let available_power = e3dcparams.pv_power - e3dcparams.haus_power;
                let charging_power = mennekesparams.power as i32;
                if available_power < charging_power && mennekesparams.hems_current > 8 {
                    eprintln!("Less power available than required, reducing charging current");
                    mennekes.set_amps(mennekesparams.hems_current - 1);
                } else if available_power > (charging_power + 900)
                    && mennekesparams.hems_current < 16
                {
                    eprintln!("Some excessive power is available, increasing charging current!");
                    mennekes.set_amps(mennekesparams.hems_current + 1);
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(10));
    }
}
