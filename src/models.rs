use serde::{Deserialize, Serialize};

// --- Top Level ---
pub const STEADY_FREQUENCY: f64 = 50.0;

#[derive(Deserialize, Debug)]
pub struct PvResponse {
    pub inputs: Inputs,
    pub outputs: Outputs,
}

// --- Inputs Section ---
#[derive(Deserialize, Debug)]
pub struct Inputs {
    pub location: Location,
    pub meteo_data: MeteoData,
    pub plane: Plane,
    pub time_format: String,
}

#[derive(Deserialize, Debug)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
    pub elevation: f64,
}

#[derive(Deserialize, Debug)]
pub struct MeteoData {
    pub radiation_db: String,
    pub meteo_db: String,
    pub year_min: u32,
    pub year_max: u32,
    pub use_horizon: bool,
    pub horizon_db: String,
}

#[derive(Deserialize, Debug)]
pub struct Plane {
    pub fixed: FixedPlane,
}

#[derive(Deserialize, Debug)]
pub struct FixedPlane {
    pub slope: AngleSetting,
    pub azimuth: AngleSetting,
}

#[derive(Deserialize, Debug)]
pub struct AngleSetting {
    pub value: f64,
    pub optimal: bool,
}

// --- Outputs Section ---
#[derive(Deserialize, Debug)]
pub struct Outputs {
    pub daily_profile: Vec<DailyProfile>,
}

#[derive(Deserialize, Debug)]
pub struct DailyProfile {
    pub month: u8,
    pub time: String,
    #[serde(rename = "G(i)")]
    pub g_i: f64,
    #[serde(rename = "Gb(i)")]
    pub gb_i: f64,
    #[serde(rename = "Gd(i)")]
    pub gd_i: f64,
}

#[derive(Serialize, Debug)]
pub struct HourReport {
    pub hour: usize,
    pub total_demand_kw: f64,
    pub total_solar_kw: f64,
    pub total_generation_kw: f64,
    pub total_battery_kw: f64,
    pub local_exchange_kw: f64,
    pub grid_import_kw: f64,
    pub losses_kw: f64,
    pub unserved_demand_kw: f64,
    pub curtailed_solar_kw: f64,
    pub transformer_overload_kw: f64,
    pub frequency_hz: f64,
}

#[derive(Serialize, Debug)]
pub struct ChartPoint {
    pub x: f64,
    pub y: f64,
}
