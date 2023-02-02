#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub pac2200: Option<ModbusConnection>,
    pub e3dc: ModbusConnection,
    pub wallbox: ModbusConnection,
    pub initial_connection_timeout: u64,
    pub phases: PhasesConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PhasesConfig {
    OnePhase,
    ThreePhase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusConnection {
    pub host: String,
    pub port: Option<u16>,
}
