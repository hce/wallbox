use crate::e3dc::E3DCParams;
use crate::mennekes::MennekesParams;
use crate::*;
use log::{debug, error, info, warn};
use regex::Regex;
use std::io::{Read, Result, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::sleep;
use std::time::Duration;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CurrSettings {
    pub max_session_energy: Option<u32>,
}

pub fn wallbox_manager(cmp: WallboxManagerParams) -> Result<()> {
    let config: config::Config = {
        let config_file = std::fs::read_to_string(cmp.config_path).expect("Config file");
        toml::from_str(&config_file).expect("TOML parsing")
    };

    fern::Dispatch::new()
        // Perform allocation-free log formatting
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        .chain(std::io::stdout())
        .level(log::LevelFilter::Info)
        .chain(fern::log_file("wallbox-manager.log")?)
        .apply()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    info!("Wallbox manager initializing");

    let e3dc = Arc::new(
        E3DC::new(
            &config.e3dc.host,
            config.e3dc.port.unwrap_or(MODBUS_DEFAULT_PORT),
            std::time::Duration::from_secs(2),
        )
        .expect("Create e3dc object"),
    );
    let mennekes = Arc::new(
        Mennekes::new(
            &config.wallbox.host,
            config.wallbox.port.unwrap_or(MODBUS_DEFAULT_PORT),
            std::time::Duration::from_secs(2),
        )
        .expect("Create mennekes object"),
    );

    let curr_settings = Arc::new(Mutex::new(CurrSettings {
        max_session_energy: None,
    }));

    let (mennekes_send, mennekes_recv) = channel();
    if let Some(bind_to) = config.bind_to.as_ref() {
        let e3dc = e3dc.clone();
        let curr_settings = curr_settings.clone();
        let listener = std::net::TcpListener::bind(bind_to)?;
        listener.set_nonblocking(false)?;
        let (send_socket, recv_socket) = channel();
        std::thread::spawn(move || {
            handle_requests(e3dc, mennekes_recv, curr_settings, recv_socket)
        });
        std::thread::spawn(move || {
            for socket in listener.incoming() {
                if let Ok(socket) = socket {
                    match socket.peer_addr() {
                        Ok(peer_addr) => {
                            debug!("New connection from {:?}", peer_addr);
                            if let Err(e) = socket.set_nonblocking(true) {
                                debug!(
                                    "Error: Cannot set socket into non-blocking mode ({:?}); ignoring socket!", e
                                );
                            } else {
                                send_socket.send((socket, peer_addr)).expect("Channel");
                            }
                        }
                        Err(e) => {
                            debug!("Error: Unable to read socket's peer address: {:?}", e);
                        }
                    }
                }
            }
        });
    }

    let mut e3dcparams;
    let mut mennekesparams;
    let t1 = Timeouter::new(config.initial_connection_timeout);
    loop {
        if let Some(n) = e3dc.get_current_params() {
            e3dcparams = n;
            break;
        }
        if !t1.ok() {
            error!("Timeout while making initial connection to PV system");
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
            error!("Timeout while making initial connection to wallbox");
            std::process::exit(11);
        }
    }

    {
        let mennekes = mennekes.clone();
        thread::spawn(move || loop {
            if let Some(mennekesparams) = mennekes.get_current_params() {
                if let Err(e) = mennekes_send.send(mennekesparams) {
                    warn!("Unable to send mennekes params: {}", e.to_string());
                }
            }
            sleep(Duration::from_secs(1));
        });
    }
    info!("Successfully connected to the PV and EV systems.");

    info!("Starting main event loop");
    let mut current_rfid = None::<String>;
    loop {
        if let Some(n) = e3dc.get_current_params() {
            e3dcparams = n;
        }
        if let Some(n) = mennekes.get_current_params() {
            mennekesparams = n;
        }

        if mennekesparams.control_pilot == 0 {
            if let Some(vn) = current_rfid.take() {
                info!("Vehicle disconnected ({})", vn);
                if let Ok(mut cs) = curr_settings.lock() {
                    cs.max_session_energy = None;
                }
            }
            let msg = format!(
                "No vehicle connected, setting MAX_AMPS to the configured default of {}A",
                config.default_amps
            );
            mennekes.set_amps(config.default_amps, msg);
        } else {
            let current_vehicle = mennekesparams
                .user_id
                .as_ref()
                .map(std::string::String::as_str)
                .unwrap_or("")
                .to_uppercase();
            if let Some(vehicle_settings) = config.rfid.get(&current_vehicle) {
                if current_rfid.is_none()
                    || current_rfid.as_ref().unwrap().ne(&vehicle_settings.name)
                {
                    info!("Vehicle connected: {}", vehicle_settings.name);
                    current_rfid = Some(vehicle_settings.name.clone());
                    if let Ok(mut cs) = curr_settings.lock() {
                        cs.max_session_energy = vehicle_settings.max_charge;
                    }
                }
                let curr_session_energy = curr_settings
                    .lock()
                    .map(|i| i.max_session_energy)
                    .unwrap_or(None);
                if mennekesparams.charging_duration < config.initial_phase_duration {
                    let msg = format!(
                        "Vehicle {} connected for less than {} seconds, signalling {} amps",
                        vehicle_settings.name, config.initial_phase_duration, config.default_amps
                    );
                    mennekes.set_amps(config.default_amps, msg);
                    std::thread::sleep(std::time::Duration::from_secs(60));
                } else if curr_session_energy.is_some()
                    && curr_session_energy.unwrap() < mennekesparams.current_energy
                {
                    let msg = format!(
                        "Vehicle {} has charged {}Wh, the limit is {}Wh. Stopping the charging.",
                        vehicle_settings.name,
                        mennekesparams.current_energy,
                        curr_session_energy.unwrap()
                    );
                    mennekes.set_amps(0, msg);
                } else {
                    let charging_power = mennekesparams.power as i32;
                    let available_power =
                        e3dcparams.pv_power + charging_power - e3dcparams.haus_power;
                    let step_power =
                        (1/* amps */) * config.phase_voltage as i32 * config.phases.number() as i32;
                    let charging_power_computed = (mennekesparams.hems_current as i32) * step_power;
                    let minimum_charging_power = vehicle_settings
                        .minimum_charging_power
                        .unwrap_or(step_power * vehicle_settings.min_amp as i32);
                    debug!(
                        "PV_Power {}W HausPower {}W",
                        e3dcparams.pv_power, e3dcparams.haus_power
                    );
                    debug!("Charging power {}W Available power {}W Step power {}W ChargingPowerComputed {}W",
                        charging_power, available_power, step_power, charging_power_computed);
                    if available_power < minimum_charging_power {
                        if vehicle_settings.pv_only {
                            let msg = format!("Available PV power of {}Watts is less than minimum charging power of {}Watts. Halting charging.", available_power, minimum_charging_power);
                            mennekes.set_amps(0, msg);
                            std::thread::sleep(std::time::Duration::from_secs(120));
                            continue;
                        } else {
                            debug!("Available PV power of {}Watts is less than minimum charging power of {}Watts. Proceeding nevertheless.", available_power, minimum_charging_power);
                        }
                    }
                    let step_power_with_hysteresis = step_power + config.hysteresis_watts;
                    if available_power < charging_power
                        && mennekesparams.hems_current > vehicle_settings.min_amp
                    {
                        let num_amps = std::cmp::max(
                            vehicle_settings.min_amp,
                            std::cmp::min(
                                vehicle_settings.max_amp,
                                ((available_power as f64) / (step_power as f64)).floor() as u16,
                            ),
                        );
                        let msg = format!("Reducing charging current to {}A", num_amps);
                        mennekes.set_amps(num_amps, msg);
                    } else if available_power
                        > (charging_power_computed + step_power_with_hysteresis)
                        && mennekesparams.hems_current < vehicle_settings.max_amp
                    {
                        let set_to = std::cmp::max(
                            mennekesparams.hems_current + 1,
                            vehicle_settings.min_amp,
                        );
                        let msg = format!(
                            "Excessive power of {} Watts is available, increasing charging current to {}A"
                            , available_power, set_to
                        );
                        mennekes.set_amps(set_to, msg);
                    } else if mennekesparams.hems_current < vehicle_settings.min_amp {
                        let set_to =
                            std::cmp::max(mennekesparams.hems_current + 1, config.default_amps);
                        let msg = format!(
                            "HEMS current {}A < than min_amp of {}A, increasing power to {}A",
                            mennekesparams.hems_current, vehicle_settings.min_amp, set_to
                        );
                        mennekes.set_amps(set_to, msg);
                    }
                }
            } else {
                let msg = format!(
                    "Unknown RFID tag {}, setting MAX_AMPS to 0A!",
                    current_vehicle
                );
                mennekes.set_amps(0, msg);
                std::thread::sleep(std::time::Duration::from_secs(60));
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(20));
    }
}

#[derive(Serialize)]
struct CV {
    e3dc: E3DCParams,
    mennekes: MennekesParams,
    curr_session: Option<CurrSettings>,
}

fn handle_requests(
    e3dc: Arc<E3DC>,
    mennekes_recv: Receiver<MennekesParams>,
    curr_settings: Arc<Mutex<CurrSettings>>,
    new_sockets: Receiver<(TcpStream, SocketAddr)>,
) {
    let mut sockets: Vec<(TcpStream, SocketAddr)> = Vec::new();
    let interval = Duration::from_secs(1);

    let cur_values_e3dc;
    let cur_values_mennekes;

    loop {
        if let Some(cv) = e3dc.get_current_params() {
            cur_values_e3dc = cv;
            break;
        }
    }

    loop {
        if let Ok(cv) = mennekes_recv.try_recv() {
            cur_values_mennekes = cv;
            break;
        }
    }

    let cs = curr_settings.lock().map(|cs| (*cs).clone()).ok();
    let mut cv = CV {
        e3dc: cur_values_e3dc,
        mennekes: cur_values_mennekes,
        curr_session: cs,
    };

    let mut sockets_to_remove = Vec::new();
    let mut read_buf = [0u8; 1024];
    let re_set_energy = Regex::new("set-energy ([0-9]+)").expect("Our regex at 0x0132");
    loop {
        let mut load = serde_json::to_string(&cv).expect("serde_json");
        load.push('\n');
        let load_bytes = load.as_bytes();
        for (i, (socket, socket_peer_addr)) in sockets.iter_mut().enumerate() {
            if let Err(e) = socket.write_all(load_bytes) {
                debug!(
                    "Socket {} ({:?}) has an error: {:?} Removing from list of sockets",
                    i, socket_peer_addr, e
                );
                sockets_to_remove.push(i);
            }
            {
                if let Ok(read_bytes) = socket.read(&mut read_buf) {
                    if read_bytes > 0 {
                        let read_string = String::from_utf8_lossy(&read_buf[0..read_bytes]);
                        if let Some(set_energy) = re_set_energy.captures(&read_string) {
                            let m1 = set_energy.get(1).expect("Get capture at 0x0145").as_str();
                            let m1_p: u32 = m1.parse().expect("Parse number at 0x0146");
                            if let Ok(mut cs) = curr_settings.lock() {
                                cs.max_session_energy = Some(m1_p);
                            }
                        }
                    }
                }
            }
        }
        // Use pop here, we need to start from the end of the sockets array
        while let Some(socket_index) = sockets_to_remove.pop() {
            sockets.remove(socket_index);
        }
        while let Ok(new_socket) = new_sockets.try_recv() {
            sockets.push(new_socket);
        }
        std::thread::sleep(interval);

        if let Some(cve) = e3dc.get_current_params() {
            cv.e3dc = cve;
        }
        while let Ok(cvm) = mennekes_recv.try_recv() {
            cv.mennekes = cvm;
        }
        cv.curr_session = curr_settings.lock().map(|cs| (*cs).clone()).ok();
    }
}
