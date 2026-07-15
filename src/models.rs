use serde::Deserialize;

// --- Top Level ---
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

    // Serde rename handles the parentheses in the JSON keys
    #[serde(rename = "G(i)")]
    pub g_i: f64,

    #[serde(rename = "Gb(i)")]
    pub gb_i: f64,

    #[serde(rename = "Gd(i)")]
    pub gd_i: f64,
}
