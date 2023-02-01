use std::thread::{JoinHandle, spawn};
use std::sync::mpsc::{channel, Sender};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::io::{Read, Result};
use std::ops::Add;
use byteorder::ReadBytesExt;
use modbus::*;

const WAIT_AFTER_ERROR: u64 = 8;

pub struct Pac2200 {
    handler: JoinHandle<()>,
    do_run: Arc<AtomicBool>,
    params: Arc<Mutex<Option<Pac2200Params>>>,
}

#[derive(Debug, Clone)]
pub struct Pac2200Params {
    pub update: u64,
    pub u_l1: f32,
    pub u_l2: f32,
    pub u_l3: f32,
    pub u_l1l2: f32,
    pub u_l2l3: f32,
    pub u_l1l3: f32,
    pub i_l1: f32,
    pub i_l2: f32,
    pub i_l3: f32,
    pub pva_l1: f32,
    pub pva_l2: f32,
    pub pva_l3: f32,
    pub p_l1: f32,
    pub p_l2: f32,
    pub p_l3: f32,
    pub pvar_l1: f32,
    pub pvar_l2: f32,
    pub pvar_l3: f32,
    pub pf_l1: f32,
    pub pf_l2: f32,
    pub pf_l3: f32,
    pub frequency: f32,
    pub u_avg_ln: f32,
    pub u_avg_ll: f32,
    pub i_avg: f32,
    pub p_avg: f32,
    pub pva_avg: f32,
    pub pvar_avg: f32,
    pub pf_tot: f32,
    pub i_n: f32,
}

impl Pac2200 {
    pub fn new(host_name: &str, port: u16, polling_interval: std::time::Duration) -> Result<Pac2200> {
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
                            let mut fetch = || {
                                let registers = client.read_holding_registers(1, 72).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                                let buf = binary::unpack_bytes(&registers);
                                let mut slice = buf.as_slice();

                                let u_l1 = slice.read_f32::<byteorder::BE>()?;
                                let u_l2 = slice.read_f32::<byteorder::BE>()?;
                                let u_l3 = slice.read_f32::<byteorder::BE>()?;

                                let u_l1l2 = slice.read_f32::<byteorder::BE>()?;
                                let u_l2l3 = slice.read_f32::<byteorder::BE>()?;
                                let u_l1l3 = slice.read_f32::<byteorder::BE>()?;

                                let i_l1 = slice.read_f32::<byteorder::BE>()?;
                                let i_l2 = slice.read_f32::<byteorder::BE>()?;
                                let i_l3 = slice.read_f32::<byteorder::BE>()?;

                                let pva_l1 = slice.read_f32::<byteorder::BE>()?;
                                let pva_l2 = slice.read_f32::<byteorder::BE>()?;
                                let pva_l3 = slice.read_f32::<byteorder::BE>()?;

                                let p_l1 = slice.read_f32::<byteorder::BE>()?;
                                let p_l2 = slice.read_f32::<byteorder::BE>()?;
                                let p_l3 = slice.read_f32::<byteorder::BE>()?;

                                let pvar_l1 = slice.read_f32::<byteorder::BE>()?;
                                let pvar_l2 = slice.read_f32::<byteorder::BE>()?;
                                let pvar_l3 = slice.read_f32::<byteorder::BE>()?;

                                let pf_l1 = slice.read_f32::<byteorder::BE>()?;
                                let pf_l2 = slice.read_f32::<byteorder::BE>()?;
                                let pf_l3 = slice.read_f32::<byteorder::BE>()?;

                                let mut _dummybuf = &mut [0u8; 24];
                                slice.read_exact(_dummybuf)?;

                                let frequency = slice.read_f32::<byteorder::BE>()?;

                                let u_avg_ln= slice.read_f32::<byteorder::BE>()?;
                                let u_avg_ll= slice.read_f32::<byteorder::BE>()?;
                                let i_avg = slice.read_f32::<byteorder::BE>()?;

                                let p_avg = slice.read_f32::<byteorder::BE>()?;
                                let pva_avg = slice.read_f32::<byteorder::BE>()?;
                                let pvar_avg = slice.read_f32::<byteorder::BE>()?;

                                let pf_tot = slice.read_f32::<byteorder::BE>()?;

                                let i_n = slice.read_f32::<byteorder::BE>()?;


                                let update = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|r| r.as_secs())
                                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                                Ok::<_, std::io::Error>(Pac2200Params {
                                    update,
                                    u_l1,
                                    u_l2,
                                    u_l3,
                                    u_l1l2,
                                    u_l2l3,
                                    u_l1l3,
                                    i_l1,
                                    i_l2,
                                    i_l3,
                                    pva_l1,
                                    pva_l2,
                                    pva_l3,
                                    p_l1,
                                    p_l2,
                                    p_l3,
                                    pvar_l1,
                                    pvar_l2,
                                    pvar_l3,
                                    pf_l1,
                                    pf_l2,
                                    pf_l3,
                                    frequency,
                                    u_avg_ln,
                                    u_avg_ll,
                                    i_avg,
                                    p_avg,
                                    pva_avg,
                                    pvar_avg,
                                    pf_tot,
                                    i_n
                                })
                            };
                            match fetch() {
                                Ok(pac2200params) => {
                                    if let Ok(mut p) = params_clone.lock() {
                                        *p = Some(pac2200params);
                                    } else {
                                        eprintln!("Cannot update params, cannot acquire mutex lock");
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Error while reading from pac2200 meter: {:?}", e);
                                    break;
                                }
                            }

                            let wait_until = std::time::SystemTime::now().add(polling_interval);
                            while std::time::SystemTime::now() < wait_until {
                                std::thread::sleep(std::time::Duration::from_secs(1));
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error while connecting to pac2200 meter: {:?}", e);
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(WAIT_AFTER_ERROR));
            }
            ()
        });

        Ok(Pac2200 {
            handler,
            do_run,
            params,
        })
    }

    pub fn get_current_params(&self) -> Option<Pac2200Params> {
        if let Ok(mut l) = self.params.lock() {
            l.take()
        } else {
            eprintln!("Unable to acquire mutex lock when fetching params!");
            None
        }
    }
}

impl Drop for Pac2200 {
    fn drop(&mut self) {
        self.do_run.store(false, Ordering::Relaxed);
    }
}
