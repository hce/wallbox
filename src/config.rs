use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub pac2200: Option<ModbusConnection>,
    pub e3dc: ModbusConnection,
    pub wallbox: ModbusConnection,
    pub initial_connection_timeout: u64,
    pub phases: PhasesConfig,
    pub phase_voltage: u16,
    pub default_amps: u16,
    pub initial_phase_duration: u32,
    pub hysteresis_watts: i32,
    pub rfid: HashMap<String, ConfigRfid>,
    pub bind_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigRfid {
    pub name: String,
    pub pv_only: bool,
    pub min_amp: u16,
    pub max_amp: u16,
    pub max_charge: Option<u32>,
    pub minimum_charging_power: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PhasesConfig {
    OnePhase,
    ThreePhase,
}

impl PhasesConfig {
    pub fn number(&self) -> u16 {
        match self {
            PhasesConfig::OnePhase => 1,
            PhasesConfig::ThreePhase => 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusConnection {
    pub host: String,
    pub port: Option<u16>,
}
