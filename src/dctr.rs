use byteorder::ReadBytesExt;
use modbus::*;
use std::io::{Read, Result};
use std::ops::Add;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{spawn, JoinHandle};

const WAIT_AFTER_ERROR: u64 = 8;

pub struct Dctr {
    #[allow(unused)]
    handler: JoinHandle<()>,
    do_run: Arc<AtomicBool>,
    params: Arc<Mutex<Option<DctrParams>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DctrParams {
    pub update: u64,
    pub validity: u16,
    pub f_i: Currents,
    pub raised_alarm_a: AlarmBitField,
    pub raised_alarm_b: AlarmBitField,
    pub thresholds_a: Currents,
    pub activated_alarms_a: AlarmBitField,
    pub thresholds_b: Currents,
    pub activated_alarms_b: AlarmBitField,
    pub alarm_delay: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Currents {
    pub dc: u16,
    pub ac_total: u16,
    pub ac_50hz: u16,
    pub ac_lt100hz: u16,
    pub ac_150hz: u16,
    pub ac_100hz_1khz: u16,
    pub ac_gt1khz: u16,
    pub ac_gt10khz: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmBitField {
    pub dc: bool,
    pub ac_total: bool,
    pub ac_50hz: bool,
    pub ac_lt100hz: bool,
    pub ac_150hz: bool,
    pub ac_100hz_1khz: bool,
    pub ac_gt1khz: bool,
    pub ac_gt10khz: bool,
}

impl Default for AlarmBitField {
    fn default() -> Self {
        AlarmBitField {
            dc: false,
            ac_total: false,
            ac_50hz: false,
            ac_lt100hz: false,
            ac_150hz: false,
            ac_100hz_1khz: false,
            ac_gt1khz: false,
            ac_gt10khz: false,
        }
    }
}

impl AlarmBitField {
    pub fn from_int(mut i: u16) -> AlarmBitField {
        let ac_gt10khz = (i & 1) == 1;
        i >>= 1;
        let ac_gt1khz = (i & 1) == 1;
        i >>= 1;
        let ac_100hz_1khz = (i & 1) == 1;
        i >>= 1;
        let ac_150hz = (i & 1) == 1;
        i >>= 1;
        let ac_lt100hz = (i & 1) == 1;
        i >>= 1;
        let ac_50hz = (i & 1) == 1;
        i >>= 1;
        let ac_total = (i & 1) == 1;
        i >>= 1;
        let dc = (i & 1) == 1;
        AlarmBitField {
            dc,
            ac_total,
            ac_50hz,
            ac_lt100hz,
            ac_150hz,
            ac_100hz_1khz,
            ac_gt1khz,
            ac_gt10khz,
        }
    }
}

fn read_currents<R: Read>(slice: &mut R) -> Result<Currents> {
    let dc = slice.read_u16::<byteorder::BE>()?;
    let ac_total = slice.read_u16::<byteorder::BE>()?;
    let ac_50hz = slice.read_u16::<byteorder::BE>()?;
    let ac_lt100hz = slice.read_u16::<byteorder::BE>()?;
    let ac_150hz = slice.read_u16::<byteorder::BE>()?;
    let ac_100hz_1khz = slice.read_u16::<byteorder::BE>()?;
    let ac_gt1khz = slice.read_u16::<byteorder::BE>()?;
    let ac_gt10khz = slice.read_u16::<byteorder::BE>()?;
    Ok(Currents {
        dc,
        ac_total,
        ac_50hz,
        ac_lt100hz,
        ac_150hz,
        ac_100hz_1khz,
        ac_gt1khz,
        ac_gt10khz,
    })
}

impl Dctr {
    pub fn new(host_name: &str, port: u16, polling_interval: std::time::Duration) -> Result<Dctr> {
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
                                let registers =
                                    client.read_holding_registers(0, 36).map_err(|e| {
                                        std::io::Error::new(std::io::ErrorKind::Other, e)
                                    })?;
                                let buf = binary::unpack_bytes(&registers);
                                let mut slice = buf.as_slice();

                                let validity = slice.read_u16::<byteorder::BE>()?;
                                let f_i = read_currents(&mut slice)?;
                                let alarm_a = slice.read_u16::<byteorder::BE>()?;
                                let alarm_b = slice.read_u16::<byteorder::BE>()?;
                                {
                                    let mut skip_buf = vec![0u8; 10];
                                    slice.read_exact(&mut skip_buf)?;
                                }
                                let thresholds_a = read_currents(&mut slice)?;
                                let active_alarms_a = slice.read_u16::<byteorder::BE>()?;

                                let thresholds_b = read_currents(&mut slice)?;
                                let active_alarms_b = slice.read_u16::<byteorder::BE>()?;

                                let alarm_delay = slice.read_u16::<byteorder::BE>()?;

                                let update = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|r| r.as_secs())
                                    .map_err(|e| {
                                        std::io::Error::new(std::io::ErrorKind::Other, e)
                                    })?;
                                Ok::<_, std::io::Error>(DctrParams {
                                    update,
                                    validity,
                                    f_i,
                                    raised_alarm_a: AlarmBitField::from_int(alarm_a),
                                    raised_alarm_b: AlarmBitField::from_int(alarm_b),
                                    thresholds_a,
                                    activated_alarms_a: AlarmBitField::from_int(active_alarms_a),
                                    thresholds_b,
                                    activated_alarms_b: AlarmBitField::from_int(active_alarms_b),
                                    alarm_delay,
                                })
                            };
                            match fetch() {
                                Ok(dctrparams) => {
                                    if let Ok(mut p) = params_clone.lock() {
                                        *p = Some(dctrparams);
                                    } else {
                                        eprintln!(
                                            "Cannot update params, cannot acquire mutex lock"
                                        );
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Error while reading from the RCM system: {:?}", e);
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
                        eprintln!("Error while connecting to RCM system: {:?}", e);
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(WAIT_AFTER_ERROR));
            }
            ()
        });

        Ok(Dctr {
            handler,
            do_run,
            params,
        })
    }

    pub fn get_current_params(&self) -> Option<DctrParams> {
        if let Ok(mut l) = self.params.lock() {
            l.take()
        } else {
            eprintln!("Unable to acquire mutex lock when fetching params!");
            None
        }
    }
}

impl Drop for Dctr {
    fn drop(&mut self) {
        self.do_run.store(false, Ordering::Relaxed);
    }
}
