use crate::buses::{Bus, BusKind};
use crate::grid::Grid;
use crate::lines::Line;
use crate::transformers::Transformer;
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;

#[derive(Deserialize)]
struct BusConfig {
    id: usize,
    name: String,
    kind: BusKind,
    voltage_kv: f64,
    demand_profile: [f64; 24],
    #[serde(default)]
    generation_profile_kw: Option<[f64; 24]>,
    roof_area_ha: f64,
    storable_energy_kwh: f64,
    max_charge_kw: f64,
    max_discharge_kw: f64,
    night_charge_window: Option<(usize, usize)>,
    night_charge_target_percent: f64,
    discharge_window: Option<(usize, usize)>,
}

#[derive(Deserialize)]
struct LineConfig {
    id: usize,
    from: usize,
    to: usize,
    distance_km: f64,
    cross_section_mm2: f64,
}

#[derive(Deserialize)]
struct TransformerConfig {
    id: usize,
    from: usize,
    to: usize,
    rated_power_kva: f64,
    efficiency: f64,
}

#[derive(Deserialize)]
struct CityConfig {
    mv_voltage_kv: f64,
    buses: Vec<BusConfig>,
    lines: Vec<LineConfig>,
    transformers: Vec<TransformerConfig>,
}

pub fn create_grid(city_name: &str) -> Grid {
    const SOLAR_COVERAGE: f64 = 0.3;
    const USEFUL_ROOF_PERCENTAGE: f64 = 0.5;
    const KWP_PER_HA: f64 = 1200.0;

    let path = format!("cities/{}.json", city_name);
    let file = File::open(&path).expect(&format!("Could not find config file at {}", path));
    let reader = BufReader::new(file);
    let config: CityConfig =
        serde_json::from_reader(reader).expect("Failed to parse city config JSON");

    let nodes = config
        .buses
        .into_iter()
        .map(|b| {
            let is_generator = b.kind == BusKind::Generator;
            let solar_kwp = if is_generator { 0.0 } else {
                b.roof_area_ha * USEFUL_ROOF_PERCENTAGE * SOLAR_COVERAGE * KWP_PER_HA
            };
            let mut bus = Bus::new(
                b.id,
                &b.name,
                b.kind,
                b.voltage_kv,
                if is_generator { [0.0; 24] } else { b.demand_profile },
                solar_kwp,
                if is_generator { 0.0 } else { b.storable_energy_kwh },
                if is_generator { 0.0 } else { b.max_charge_kw },
                if is_generator { 0.0 } else { b.max_discharge_kw },
                if is_generator { None } else { b.night_charge_window },
                if is_generator { 0.0 } else { b.night_charge_target_percent },
                if is_generator { None } else { b.discharge_window },
            );
            bus.generation_profile_kw = b.generation_profile_kw.unwrap_or([0.0; 24]);
            bus
        })
        .collect();

    let lines = config
        .lines
        .into_iter()
        .map(|l| Line {
            id: l.id,
            from: l.from,
            to: l.to,
            distance_km: l.distance_km,
            cross_section_mm2: l.cross_section_mm2,
        })
        .collect();

    let transformers = config
        .transformers
        .into_iter()
        .map(|t| Transformer {
            id: t.id,
            from: t.from,
            to: t.to,
            rated_power_kva: t.rated_power_kva,
            efficiency: t.efficiency,
        })
        .collect();

    Grid::new(nodes, lines, transformers, config.mv_voltage_kv)
}
