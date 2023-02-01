mod e3dc;
mod mennekes;
mod pac2200;

use e3dc::E3DC;
use mennekes::Mennekes;
use pac2200::Pac2200;

fn main() {
    let e3dc = E3DC::new("localhost", 5020, std::time::Duration::from_secs(2)).expect("Create e3dc object");
    let mennekes = Mennekes::new("localhost", 5021, std::time::Duration::from_secs(60)).expect("Create mennekes object");
    let pac2200 = Pac2200::new("localhost", 5022,std::time::Duration::from_secs(1)).expect("Create pac2200 object");

    let mut e3dcparams;
    let mut mennekesparams;
    let mut pac2200params;
    loop {
        if let Some(n) = e3dc.get_current_params() {
            e3dcparams = n;
            break;
        }
    }
    loop {
        if let Some(n) = mennekes.get_current_params() {
            mennekesparams = n;
            break;
        }
    }
    loop {
        if let Some(n) =  pac2200.get_current_params() {
            pac2200params = n;
            break;
        }
    }
    loop {
        if let Some(n) =  e3dc.get_current_params() {
            e3dcparams = n;
        }
        if let Some(n) =  mennekes.get_current_params() {
            mennekesparams = n;
        }
        if let Some(n) =  pac2200.get_current_params() {
            pac2200params = n;
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
                } else if available_power > (charging_power + 900)  && mennekesparams.hems_current < 16{
                    eprintln!("Some excessive power is available, increasing charging current!");
                    mennekes.set_amps(mennekesparams.hems_current + 1);
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(10));
    }
}
