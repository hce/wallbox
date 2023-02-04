use crate::*;
use std::io::Result;

pub fn wallbox_manager(cmp: WallboxManagerParams) -> Result<()> {
    let config: config::Config = {
        let config_file = std::fs::read_to_string(cmp.config_path).expect("Config file");
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

    let mut e3dcparams;
    let mut mennekesparams;
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

    loop {
        if let Some(n) = e3dc.get_current_params() {
            e3dcparams = n;
        }
        if let Some(n) = mennekes.get_current_params() {
            mennekesparams = n;
        }

        if mennekesparams.control_pilot == 0 {
            eprintln!(
                "No vehicle connected, setting MAX_AMPS to the minimum of {}A",
                config.min_amp
            );
            mennekes.set_amps(config.min_amp);
        } else {
            if mennekesparams.charging_duration < config.initial_phase_duration {
                eprintln!(
                    "Vehicle connected for less than {} seconds, signalling {} amps",
                    config.initial_phase_duration, config.min_amp
                );
                mennekes.set_amps(config.min_amp);
            } else {
                let available_power = e3dcparams.pv_power - e3dcparams.haus_power;
                let charging_power = mennekesparams.power as i32;
                let step_power =
                    (1/* amps */) * config.phase_voltage as i32 * config.phases.number() as i32;
                let step_power_with_hysteresis = step_power + config.hysteresis_watts;
                if available_power < charging_power && mennekesparams.hems_current > config.min_amp
                {
                    let num_amps = std::cmp::max(
                        config.min_amp,
                        std::cmp::min(
                            config.max_amp,
                            ((available_power as f64) / (step_power as f64)).floor() as u16,
                        ),
                    );
                    eprintln!(
                        "Less power available than required, setting charging current to {} amps",
                        num_amps
                    );
                    mennekes.set_amps(num_amps);
                } else if available_power > (charging_power + step_power_with_hysteresis)
                    && mennekesparams.hems_current < config.max_amp
                {
                    eprintln!(
                        "Some excessive power is available, increasing charging current by 1 amp"
                    );
                    mennekes.set_amps(mennekesparams.hems_current + 1);
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(10));
    }
}
