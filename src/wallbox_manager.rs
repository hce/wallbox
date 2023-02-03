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
