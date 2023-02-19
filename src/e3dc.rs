use byteorder::ReadBytesExt;
use modbus::*;
use std::io::{Read, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{spawn, JoinHandle};

const WAIT_AFTER_ERROR: u64 = 8;

pub struct E3DC {
    #[allow(unused)]
    handler: JoinHandle<()>,
    do_run: Arc<AtomicBool>,
    params: Arc<Mutex<Option<E3DCParams>>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct E3DCParams {
    pub update: u64,
    pub magic: u16,
    pub v1: u8,
    pub v2: u8,
    pub v3: u16,
    pub version_strings: Vec<String>,
    pub pv_power: i32,
    pub batt_power: i32,
    pub haus_power: i32,
    pub netz_power: i32,
    pub misc_1: i32,
    pub misc_2: i32,
    pub misc_3: i32,
    pub autarky: u8,
    pub self_utilisation: u8,
    pub akku_charge_percentage: u16,
    pub emergency_power: u16,
    pub s1v: u16,
    pub s2v: u16,
    pub s1a: u16,
    pub s2a: u16,
    pub s1p: u16,
    pub s2p: u16,
}

fn read_big_little_i32<R: Read>(mut inbuf: R) -> Result<i32> {
    let a = inbuf.read_u8()?;
    let b = inbuf.read_u8()?;
    let c = inbuf.read_u8()?;
    let d = inbuf.read_u8()?;
    let buf = &[c, d, a, b];
    (&buf[..]).read_i32::<byteorder::BE>()
}

impl E3DC {
    pub fn new(host_name: &str, port: u16, polling_interval: std::time::Duration) -> Result<E3DC> {
        let host_name = String::from(host_name);
        let do_run = Arc::new(AtomicBool::new(true));
        let do_run_clone = do_run.clone();
        let params = Arc::new(Mutex::new(None));
        let params_clone = params.clone();
        let handler = spawn(move || {
            while do_run_clone.load(Ordering::Relaxed) {
                let mut cfg = tcp::Config::default();
                cfg.tcp_port = port;
                match tcp::Transport::new_with_cfg(&host_name, cfg) {
                    Ok(mut client) => {
                        while do_run_clone.load(Ordering::Relaxed) {
                            let registers = client.read_holding_registers(40000, 126);
                            if registers.is_err() {
                                eprintln!(
                                    "Error while reading from e3dc system: {:?}",
                                    registers.err()
                                );
                                break;
                            } else {
                                let registers = registers.unwrap();
                                let fetch = || {
                                    let buf = binary::unpack_bytes(&registers);
                                    let mut slice = buf.as_slice();

                                    let magic = slice.read_u16::<byteorder::BE>()?;
                                    let v1 = slice.read_u8()?;
                                    let v2 = slice.read_u8()?;
                                    let v3 = slice.read_u16::<byteorder::BE>()?;
                                    let mut version_strings = Vec::new();
                                    let mut str_buf = [0; 32];
                                    for _i in 0..4 {
                                        slice.read_exact(&mut str_buf)?;
                                        let version_string = String::from_utf8(str_buf.to_vec())
                                            .map_err(|e| {
                                                std::io::Error::new(std::io::ErrorKind::Other, e)
                                            })?;
                                        version_strings.push(version_string);
                                    }

                                    let pv_power = read_big_little_i32(&mut slice)?;
                                    let batt_power = read_big_little_i32(&mut slice)?;
                                    let haus_power = read_big_little_i32(&mut slice)?;
                                    let netz_power = read_big_little_i32(&mut slice)?;
                                    let misc_1 = read_big_little_i32(&mut slice)?;
                                    let misc_2 = read_big_little_i32(&mut slice)?;
                                    let misc_3 = read_big_little_i32(&mut slice)?;
                                    let autarky = slice.read_u8()?;
                                    let self_utilisation = slice.read_u8()?;
                                    let akku_charge_percentage =
                                        slice.read_u16::<byteorder::BE>()?;
                                    let emergency_power = slice.read_u16::<byteorder::BE>()?;
                                    // skip 22 bytes
                                    for _i in 0..11 {
                                        slice.read_i16::<byteorder::BE>()?;
                                    }
                                    let s1v = slice.read_u16::<byteorder::BE>()?;
                                    let s2v = slice.read_u16::<byteorder::BE>()?;
                                    let _s3v = slice.read_u16::<byteorder::BE>()?;
                                    let s1a = slice.read_u16::<byteorder::BE>()?;
                                    let s2a = slice.read_u16::<byteorder::BE>()?;
                                    let _s3a = slice.read_u16::<byteorder::BE>()?;
                                    let s1p = slice.read_u16::<byteorder::BE>()?;
                                    let s2p = slice.read_u16::<byteorder::BE>()?;
                                    let _s3p = slice.read_u16::<byteorder::BE>()?;

                                    let update = std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .map(|r| r.as_secs())
                                        .map_err(|e| {
                                            std::io::Error::new(std::io::ErrorKind::Other, e)
                                        })?;
                                    Ok::<_, std::io::Error>(E3DCParams {
                                        update,
                                        magic,
                                        v1,
                                        v2,
                                        v3,
                                        version_strings,
                                        pv_power,
                                        batt_power,
                                        haus_power,
                                        netz_power,
                                        misc_1,
                                        misc_2,
                                        misc_3,
                                        autarky,
                                        self_utilisation,
                                        akku_charge_percentage,
                                        emergency_power,
                                        s1v,
                                        s2v,
                                        s1a,
                                        s2a,
                                        s1p,
                                        s2p,
                                    })
                                };
                                match fetch() {
                                    Ok(e3dcparams) => {
                                        if let Ok(mut p) = params_clone.lock() {
                                            *p = Some(e3dcparams);
                                        } else {
                                            eprintln!(
                                                "Cannot update params, cannot acquire mutex lock"
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Error while reading from e3dc system: {:?}", e);
                                        break;
                                    }
                                }
                            }

                            std::thread::sleep(polling_interval);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error while connecting to e3dc system: {:?}", e);
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(WAIT_AFTER_ERROR));
            }
            ()
        });

        Ok(E3DC {
            handler,
            do_run,
            params,
        })
    }

    pub fn get_current_params(&self) -> Option<E3DCParams> {
        if let Ok(mut l) = self.params.lock() {
            l.take()
        } else {
            eprintln!("Unable to acquire mutex lock when fetching params!");
            None
        }
    }
}

impl Drop for E3DC {
    fn drop(&mut self) {
        self.do_run.store(false, Ordering::Relaxed);
    }
}
