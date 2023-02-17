use byteorder::ReadBytesExt;
use modbus::*;
use std::io::Result;
use std::ops::Add;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{spawn, JoinHandle};

const WAIT_AFTER_ERROR: u64 = 8;

pub struct Mennekes {
    #[allow(unused)]
    handler: JoinHandle<()>,
    do_run: Arc<AtomicBool>,
    params: Arc<Mutex<Option<MennekesParams>>>,
    action: Sender<MennekesAction>,
}

#[derive(Clone)]
struct SetAmpsAction {
    pub max_hems_current: u16,
}

#[derive(Clone)]
struct AuthorizeUserAction {
    pub user_id: String,
}

#[derive(Clone)]
enum MennekesAction {
    SetAmps(SetAmpsAction),
    AuthorizeUser(AuthorizeUserAction),
}

#[derive(Debug, Clone, Serialize)]
pub struct MennekesParams {
    pub update: u64,
    pub control_pilot: u16,
    pub f_i_ac: u16,
    pub f_i_dc: u16,
    pub i_l1: u32,
    pub i_l2: u32,
    pub i_l3: u32,
    pub energy: u32,
    pub power: u32,
    pub u_l1: u32,
    pub u_l2: u32,
    pub u_l3: u32,
    pub max_allowed_current_signalled: u16,
    pub start_time: u32,
    pub ev_required_energy: u32,
    pub max_allowed_ev_current: u16,
    pub current_energy: u32,
    pub charging_duration: u32,
    pub user_id: Option<String>,
    pub hems_current: u16,
}

impl Mennekes {
    pub fn new(
        host_name: &str,
        port: u16,
        polling_interval: std::time::Duration,
    ) -> Result<Mennekes> {
        let host_name = String::from(host_name);
        let do_run = Arc::new(AtomicBool::new(true));
        let do_run_clone = do_run.clone();
        let params = Arc::new(Mutex::new(None));
        let params_clone = params.clone();
        let (action, receive_action) = channel();
        let handler = spawn(move || {
            while do_run_clone.load(Ordering::Relaxed) {
                let mut cfg = tcp::Config::default();
                cfg.tcp_port = port;
                match tcp::Transport::new_with_cfg(&host_name, cfg) {
                    Ok(mut client) => {
                        while do_run_clone.load(Ordering::Relaxed) {
                            let mut fetch = || {
                                let control_pilot = {
                                    let registers =
                                        client.read_holding_registers(104, 1).map_err(|e| {
                                            std::io::Error::new(std::io::ErrorKind::Other, e)
                                        })?;
                                    let buf = binary::unpack_bytes(&registers);
                                    let mut slice = buf.as_slice();
                                    slice.read_u16::<byteorder::BE>()?
                                };

                                let (f_i_ac, f_i_dc) = {
                                    let registers =
                                        client.read_holding_registers(136, 2).map_err(|e| {
                                            std::io::Error::new(std::io::ErrorKind::Other, e)
                                        })?;
                                    let buf = binary::unpack_bytes(&registers);
                                    let mut slice = buf.as_slice();
                                    (
                                        slice.read_u16::<byteorder::BE>()?,
                                        slice.read_u16::<byteorder::BE>()?,
                                    )
                                };

                                let (i_l1, i_l2, i_l3, energy, power, u_l1, u_l2, u_l3) = {
                                    let registers =
                                        client.read_holding_registers(212, 16).map_err(|e| {
                                            std::io::Error::new(std::io::ErrorKind::Other, e)
                                        })?;
                                    let buf = binary::unpack_bytes(&registers);
                                    let mut slice = buf.as_slice();
                                    (
                                        slice.read_u32::<byteorder::BE>()?,
                                        slice.read_u32::<byteorder::BE>()?,
                                        slice.read_u32::<byteorder::BE>()?,
                                        slice.read_u32::<byteorder::BE>()?,
                                        slice.read_u32::<byteorder::BE>()?,
                                        slice.read_u32::<byteorder::BE>()?,
                                        slice.read_u32::<byteorder::BE>()?,
                                        slice.read_u32::<byteorder::BE>()?,
                                    )
                                };

                                let (max_allowed_current_signalled, start_time) = {
                                    let registers =
                                        client.read_holding_registers(706, 3).map_err(|e| {
                                            std::io::Error::new(std::io::ErrorKind::Other, e)
                                        })?;
                                    let buf = binary::unpack_bytes(&registers);
                                    let mut slice = buf.as_slice();
                                    (
                                        slice.read_u16::<byteorder::BE>()?,
                                        slice.read_u32::<byteorder::BE>()?,
                                    )
                                };

                                let (
                                    ev_required_energy,
                                    max_allowed_ev_current,
                                    current_energy,
                                    charging_duration,
                                ) = {
                                    let registers =
                                        client.read_holding_registers(713, 7).map_err(|e| {
                                            std::io::Error::new(std::io::ErrorKind::Other, e)
                                        })?;
                                    let buf = binary::unpack_bytes(&registers);
                                    let mut slice = buf.as_slice();
                                    (
                                        slice.read_u32::<byteorder::BE>()?,
                                        slice.read_u16::<byteorder::BE>()?,
                                        slice.read_u32::<byteorder::BE>()?,
                                        slice.read_u32::<byteorder::BE>()?,
                                    )
                                };

                                let user_id = {
                                    let registers =
                                        client.read_holding_registers(720, 10).map_err(|e| {
                                            std::io::Error::new(std::io::ErrorKind::Other, e)
                                        })?;
                                    let buf = binary::unpack_bytes(&registers);
                                    let user_id = String::from_utf8(buf)
                                        .map_err(|e| {
                                            std::io::Error::new(std::io::ErrorKind::Other, e)
                                        })?
                                        .trim()
                                        .to_string();
                                    if user_id.is_empty() {
                                        None
                                    } else {
                                        Some(user_id)
                                    }
                                };

                                let hems_current = {
                                    let registers =
                                        client.read_holding_registers(1000, 1).map_err(|e| {
                                            std::io::Error::new(std::io::ErrorKind::Other, e)
                                        })?;
                                    let buf = binary::unpack_bytes(&registers);
                                    let mut slice = buf.as_slice();
                                    slice.read_u16::<byteorder::BE>()?
                                };

                                let update = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|r| r.as_secs())
                                    .map_err(|e| {
                                        std::io::Error::new(std::io::ErrorKind::Other, e)
                                    })?;
                                Ok::<_, std::io::Error>(MennekesParams {
                                    update,
                                    control_pilot,
                                    f_i_ac,
                                    f_i_dc,
                                    i_l1,
                                    i_l2,
                                    i_l3,
                                    energy,
                                    power,
                                    u_l1,
                                    u_l2,
                                    u_l3,
                                    max_allowed_current_signalled,
                                    start_time,
                                    ev_required_energy,
                                    max_allowed_ev_current,
                                    current_energy,
                                    charging_duration,
                                    user_id,
                                    hems_current,
                                })
                            };
                            match fetch() {
                                Ok(mennekesparams) => {
                                    if let Ok(mut p) = params_clone.lock() {
                                        *p = Some(mennekesparams);
                                    } else {
                                        eprintln!(
                                            "Cannot update params, cannot acquire mutex lock"
                                        );
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Error while reading from mennekes wallbox: {:?}", e);
                                    break;
                                }
                            }

                            let wait_until = std::time::SystemTime::now().add(polling_interval);
                            while std::time::SystemTime::now() < wait_until {
                                if let Ok(action) = receive_action.try_recv() {
                                    match action {
                                        MennekesAction::SetAmps(amps) => {
                                            if amps.max_hems_current > 16 {
                                                eprintln!(
                                                    "Illegal HEMS current of {} Amps",
                                                    amps.max_hems_current
                                                );
                                            } else {
                                                let hems_current = (|| {
                                                    let registers = client
                                                        .read_holding_registers(1000, 1)
                                                        .map_err(|e| {
                                                            std::io::Error::new(
                                                                std::io::ErrorKind::Other,
                                                                e,
                                                            )
                                                        })?;
                                                    let buf = binary::unpack_bytes(&registers);
                                                    let mut slice = buf.as_slice();
                                                    slice.read_u16::<byteorder::BE>()
                                                })(
                                                );
                                                if hems_current.is_err() {
                                                    eprintln!(
                                                        "Unable to read HEMS current: {:?}",
                                                        hems_current.err()
                                                    );
                                                } else {
                                                    let hems_current = hems_current.unwrap();
                                                    if hems_current == amps.max_hems_current {
                                                        eprintln!(
                                                            "{} == {}, not changing value",
                                                            hems_current, amps.max_hems_current
                                                        );
                                                    } else {
                                                        match client.write_single_register(1000, amps.max_hems_current) {
                                                            Ok(_) => eprintln!("{} -> {}, changed.", hems_current, amps.max_hems_current),
                                                            Err(e) => eprintln!("Unable to change HEMS current! {:?}", e)
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        MennekesAction::AuthorizeUser(user) => {
                                            let regs =
                                                modbus::binary::pack_bytes(user.user_id.as_bytes());
                                            if regs.is_err() {
                                                eprintln!(
                                                    "Unable to pack bytes to authorize user: {:?}",
                                                    regs.err()
                                                );
                                            } else {
                                                let regs = regs.unwrap();
                                                match client.write_multiple_registers(1110, &regs) {
                                                    Ok(_) => eprintln!("User authorized."),
                                                    Err(e) => eprintln!(
                                                        "Unable to authorize user! {:?}",
                                                        e
                                                    ),
                                                }
                                            }
                                        }
                                    }
                                }
                                std::thread::sleep(std::time::Duration::from_secs(1));
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error while connecting to mennekes wallbox: {:?}", e);
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(WAIT_AFTER_ERROR));
            }
            ()
        });

        Ok(Mennekes {
            handler,
            do_run,
            params,
            action,
        })
    }

    pub fn get_current_params(&self) -> Option<MennekesParams> {
        if let Ok(mut l) = self.params.lock() {
            l.take()
        } else {
            eprintln!("Unable to acquire mutex lock when fetching params!");
            None
        }
    }

    pub fn set_amps(&self, max_hems_current: u16) {
        let action = MennekesAction::SetAmps(SetAmpsAction { max_hems_current });
        self.action.send(action).expect("set_amps");
    }

    #[allow(unused)]
    pub fn authorize_user(&self, user_id: String) {
        let action = MennekesAction::AuthorizeUser(AuthorizeUserAction { user_id });
        self.action.send(action).expect("authorize_user");
    }
}

impl Drop for Mennekes {
    fn drop(&mut self) {
        self.do_run.store(false, Ordering::Relaxed);
    }
}
