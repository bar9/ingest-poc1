use serde::{Deserialize, Serialize};
use serde_json::{json};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Charging {
    pub start: u128,
    pub end: Option<u128>,
    pub nominal_max_power: f64,
    pub estimated_power: Option<f64>,
    pub charger: String,
    pub zip: String,
    pub location: [f64;2],
    pub energy: Option<f64>,
}

impl Charging {
    pub fn set_end(&mut self, end: u128) {
        self.end = Some(end);
        let duration_millis = self.end.unwrap() - self.start;
        let duration_seconds = duration_millis / 1000;
        let duration_hours: f64 = (duration_seconds as f64) / 3600_f64;
        self.energy = Some (duration_hours * self.estimated_power.unwrap());
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Realtime {
    pub last_update: u128,
    pub occupied: bool,
    pub nominal_max_power: f64, //grösste
    pub estimated_power: Option<f64>, //immer min. 11
    pub zip: String,
    pub location: [f64;2]
}