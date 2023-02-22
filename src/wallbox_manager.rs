use crate::e3dc::E3DCParams;
use crate::mennekes::MennekesParams;
use crate::*;
use log::{debug, error, info, warn};
use std::io::{Result, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::Duration;

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
            std::time::Duration::from_secs(15),
        )
        .expect("Create mennekes object"),
    );

    let (mennekes_send, mennekes_recv) = channel();
    if let Some(bind_to) = config.bind_to.as_ref() {
        let e3dc = e3dc.clone();
        let listener = std::net::TcpListener::bind(bind_to)?;
        listener.set_nonblocking(false)?;
        let (send_socket, recv_socket) = channel();
        std::thread::spawn(move || handle_requests(e3dc, mennekes_recv, recv_socket));
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

    info!("Making initial connection to the PV system...");
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

    info!("Making initial connection to the EV charger...");
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
    if let Err(e) = mennekes_send.send(mennekesparams.clone()) {
        warn!("Unable to send mennekes params: {}", e.to_string());
    }

    info!("Starting main event loop");
    let mut current_rfid = None::<String>;
    loop {
        if let Some(n) = e3dc.get_current_params() {
            e3dcparams = n;
        }
        if let Some(n) = mennekes.get_current_params() {
            mennekesparams = n;
            if let Err(e) = mennekes_send.send(mennekesparams.clone()) {
                warn!("Unable to send mennekes params: {}", e.to_string());
            }
        }

        if mennekesparams.control_pilot == 0 {
            if let Some(vn) = current_rfid.take() {
                info!("Vehicle disconnected ({})", vn);
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
                }
                if mennekesparams.charging_duration < config.initial_phase_duration {
                    let msg = format!(
                        "Vehicle {} connected for less than {} seconds, signalling {} amps",
                        vehicle_settings.name, config.initial_phase_duration, config.default_amps
                    );
                    mennekes.set_amps(config.default_amps, msg);
                    std::thread::sleep(std::time::Duration::from_secs(60));
                } else if vehicle_settings.max_charge.is_some()
                    && vehicle_settings.max_charge.unwrap() < mennekesparams.current_energy
                {
                    let msg = format!(
                        "Vehicle {} has charged {}Wh, the limit is {}Wh. Stopping the charging.",
                        vehicle_settings.name,
                        mennekesparams.current_energy,
                        vehicle_settings.max_charge.unwrap()
                    );
                    mennekes.set_amps(0, msg);
                    std::thread::sleep(std::time::Duration::from_secs(60));
                } else {
                    let charging_power = mennekesparams.power as i32;
                    let available_power =
                        e3dcparams.pv_power + charging_power - e3dcparams.haus_power;
                    let step_power =
                        (1/* amps */) * config.phase_voltage as i32 * config.phases.number() as i32;
                    let minimum_charging_power = step_power * vehicle_settings.min_amp as i32;
                    if e3dcparams.pv_power < minimum_charging_power {
                        if vehicle_settings.pv_only {
                            let msg = format!("Available PV power of {}Watts is less than minimum charging power of {}Watts. Halting charging.", available_power, minimum_charging_power);
                            mennekes.set_amps(0, msg);
                            std::thread::sleep(std::time::Duration::from_secs(60));
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
                        let msg = format!(
                            "Less power available than required, setting charging current to {} amps",
                            num_amps
                        );
                        mennekes.set_amps(num_amps, msg);
                        std::thread::sleep(std::time::Duration::from_secs(20));
                    } else if available_power > (charging_power + step_power_with_hysteresis)
                        && mennekesparams.hems_current < vehicle_settings.max_amp
                    {
                        let set_to = std::cmp::max(
                            mennekesparams.hems_current + 1,
                            vehicle_settings.min_amp,
                        );
                        let msg = format!(
                            "Some excessive power is available, increasing charging current by 1 amp to {}A"
                            , set_to
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
                        std::thread::sleep(std::time::Duration::from_secs(20));
                    }
                }
            } else {
                let msg = format!(
                    "Unknown RFID tag {}, setting MAX_AMPS to 0!",
                    current_vehicle
                );
                mennekes.set_amps(0, msg);
                std::thread::sleep(std::time::Duration::from_secs(600));
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(20));
    }
}

#[derive(Serialize)]
struct CV {
    e3dc: E3DCParams,
    mennekes: MennekesParams,
}

fn handle_requests(
    e3dc: Arc<E3DC>,
    mennekes_recv: Receiver<MennekesParams>,
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

    let mut cv = CV {
        e3dc: cur_values_e3dc,
        mennekes: cur_values_mennekes,
    };

    let mut sockets_to_remove = Vec::new();
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
        if let Ok(cvm) = mennekes_recv.try_recv() {
            cv.mennekes = cvm;
        }
    }
}
