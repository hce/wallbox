use std::io::Result;
use std::ops::Add;

pub struct Timeouter {
    timeout_at: std::time::SystemTime,
}

impl Timeouter {
    pub fn new(secs: u64) -> Timeouter {
        let timeout_at = std::time::SystemTime::now().add(std::time::Duration::from_secs(secs));
        Timeouter { timeout_at }
    }

    pub fn ok(&self) -> bool {
        std::time::SystemTime::now() < self.timeout_at
    }
}
