use buses::{Bus, BusKind};
use lines::Line;
use models::PvResponse;
use serde::Serialize;
use std::fs::File;
use std::io::{BufReader, Write};
use transformers::Transformer;

mod buses;
mod lines;
mod models;
mod transformers;

struct Grid {
    nodes: Vec<Bus>,
    lines: Vec<Line>,
    transformers: Vec<Transformer>,
    mv_voltage_kv: f64,
}
#[derive(Serialize)]
struct HourReport {
    hour: usize,
    total_demand_kw: f64,
    total_solar_kw: f64,
    total_battery_kw: f64, // Positive -> discharging | Negative -> charging
    grid_import_kw: f64,   // Positive -> Import | Negative -> Export
    losses_kw: f64,
    frequency_hz: f64,
}

/// Matches the `DataT` type in chart.ts: `{ x: number, y: number }`
#[derive(Serialize)]
struct ChartPoint {
    x: f64,
    y: f64,
}

// Normal frequency of the grid.
const STEADY_FREQUENCY: f64 = 50.01;

impl Grid {
    fn new(
        nodes: Vec<Bus>,
        lines: Vec<Line>,
        transformers: Vec<Transformer>,
        mv_voltage_kv: f64,
    ) -> Self {
        Self {
            nodes,
            lines,
            transformers,
            mv_voltage_kv,
        }
    }

    // Check which transformer supplies energy to a certain bus.
    fn transformer_for(&self, bus_id: usize) -> Option<&Transformer> {
        self.transformers.iter().find(|t| t.to == bus_id)
    }

    // Check which line is assigned to a certain bus.
    fn line_for(&self, bus_id: usize) -> Option<&Line> {
        self.lines.iter().find(|l| l.to == bus_id)
    }

    fn tick(&mut self, hour: usize, irradiation_w_m2: f64) -> HourReport {
        let mut total_demand_kw = 0.0;
        let mut potential_solar_kw = 0.0;
        let mut total_battery_kw = 0.0;
        let mut grid_import_kw = 0.0;
        let mut losses_kw = 0.0;

        // First pass: Calculate total demand and potential solar generation
        for bus_idx in 0..self.nodes.len() {
            if self.nodes[bus_idx].kind == BusKind::Substation {
                continue;
            }
            total_demand_kw += self.nodes[bus_idx].demand_at(hour);
            potential_solar_kw += self.nodes[bus_idx].solar_generation_at(irradiation_w_m2);
        }

        // Max absorption capacity = Demand + Battery Charge Capacity + Max Export Limit (e.g., 5000 kW)
        let max_export_limit = 5000.0;
        let mut battery_charge_capacity = 0.0;
        for bus in &self.nodes {
            if bus.kind != BusKind::Substation {
                // Only consider charging if it's not a night charge window (which uses grid)
                // and if the battery has space.
                battery_charge_capacity += bus.max_charge_kw;
            }
        }

        let max_absorption = total_demand_kw + battery_charge_capacity + max_export_limit;

        // Solar Curtailment: Limit generation to what the grid can handle to stabilize frequency
        let actual_solar_kw = potential_solar_kw.min(max_absorption);
        let curtailment_ratio = if potential_solar_kw > 0.0 {
            actual_solar_kw / potential_solar_kw
        } else {
            1.0
        };

        let mut total_solar_kw = 0.0;

        for bus_idx in 0..self.nodes.len() {
            if self.nodes[bus_idx].kind == BusKind::Substation {
                continue;
            }

            let demand = self.nodes[bus_idx].demand_at(hour);
            // Apply curtailment ratio to individual bus generation
            let solar =
                self.nodes[bus_idx].solar_generation_at(irradiation_w_m2) * curtailment_ratio;

            // Electrical balance
            let net_value = solar - demand;
            total_solar_kw += solar;

            let in_night_charge_window = matches!(
                self.nodes[bus_idx].night_charge_window,
                Some((start, end)) if hour >= start && hour < end
            );

            let net_after_battery = if in_night_charge_window {
                net_value
            } else if net_value > 0.0 {
                let (charged, remaining_surplus) = self.nodes[bus_idx].try_charge(net_value);

                total_battery_kw -= charged;
                remaining_surplus
            } else if net_value < 0.0 {
                let can_discharge = match self.nodes[bus_idx].discharge_window {
                    Some((start, end)) => hour >= start && hour < end,
                    None => true,
                };

                if can_discharge {
                    let (discharged, remaining_deficit) =
                        self.nodes[bus_idx].try_discharge(-net_value);

                    total_battery_kw += discharged;

                    -remaining_deficit
                } else {
                    net_value
                }
            } else {
                0.0
            };

            let night_charge = self.nodes[bus_idx].try_night_grid_charge(hour);

            total_battery_kw -= night_charge;
            let net_after_battery = net_after_battery - night_charge;

            // Power exchanged with the Medium Tension grid.
            let mv_power_kw = match self.transformer_for(self.nodes[bus_idx].id) {
                Some(tr) => tr.refer_to_primary(-net_after_battery),
                None => -net_after_battery,
            };

            let line_losses = match self.line_for(self.nodes[bus_idx].id) {
                Some(line) => line.power_loss_kw(mv_power_kw, self.mv_voltage_kv),
                None => 0.0,
            };

            losses_kw += line_losses;
            grid_import_kw += mv_power_kw + line_losses;
        }

        let imbalance_ratio = if total_demand_kw > 0.0 {
            (total_solar_kw + total_battery_kw - total_demand_kw) / total_demand_kw
        } else {
            0.0
        };

        let frequency_hz = STEADY_FREQUENCY + imbalance_ratio * 0.02;

        HourReport {
            hour,
            total_demand_kw,
            total_solar_kw,
            total_battery_kw,
            grid_import_kw,
            losses_kw,
            frequency_hz,
        }
    }

    pub fn dbg(&self) {
        println!("=== Results ===");
        println!(
            "{:<3} {:<45} {:<10} {:<10} {:<13}",
            "Id", "Bus", "kV", "kWp", "Battery kWh"
        );

        for bus in &self.nodes {
            println!(
                "{:<3} {:<45} {:>10.2} {:>10.1} {:>13.1}",
                bus.id, bus.name, bus.voltage_kv, bus.solar_kwp, bus.storable_energy_kwh
            )
        }
        println!();
        println!(
            "{:<3} {:<6} {:<6} {:>12} {:>13}",
            "Id", "From", "To", "Dist. (km)", "X-Section (mm²)"
        );
        for line in &self.lines {
            println!(
                "{:<3} {:<6} {:<6} {:>12.2} {:>13.0}",
                line.id, line.from, line.to, line.distance_km, line.cross_section_mm2
            );
        }
        println!();
    }
}

fn create_grid() -> Grid {
    const SOLAR_COVERAGE: f64 = 0.5;
    const USEFUL_ROOF_PERCENTAGE: f64 = 0.6;

    // Based on Mapa-urbanístic-catalunya.csv for Malgrat de Mar (2025):
    // 13_Qual_SUC_R (Residential) sum of R1,R2,R3,R4,R5,R6:
    let residential_ha = 88.7441 * USEFUL_ROOF_PERCENTAGE * SOLAR_COVERAGE;

    // 12_A1_SUC (Industrial): 36.0394 ha
    let industrial_ha = 36.0394 * USEFUL_ROOF_PERCENTAGE * SOLAR_COVERAGE;

    // 12_A2_SUC (Touristic/Business): 8.6101 ha
    let touristic_ha = 8.6101 * USEFUL_ROOF_PERCENTAGE * SOLAR_COVERAGE;

    // 15_SE_SUC (Schools and local buildings): 16.1947 ha
    let school_ha = 16.1947 * USEFUL_ROOF_PERCENTAGE * SOLAR_COVERAGE;

    // Calc: (ha * 10000 m2/ha * 60%) / 5 m2/kWp = ha * 1200 kWp/ha
    let kwp_residential = residential_ha * 1200.0;
    let kwp_industrial = industrial_ha * 1200.0;
    let kwp_touristic = touristic_ha * 1200.0;
    let kwp_school = school_ha * 1200.0;

    // Bus 0: Substation
    let substation = Bus::new(
        0,
        BusKind::Substation.label(),
        BusKind::Substation,
        25.0,
        [0.0; 24],
        0.0,
        0.0,
        0.0,
        0.0,
        None,
        0.0,
        None,
    );

    // Bus 1: Historic Center
    let historic = Bus::new(
        1,
        BusKind::HistoricCenter.label(),
        BusKind::HistoricCenter,
        0.4,
        [
            800.0, 750.0, 700.0, 700.0, 750.0, 900.0, 1100.0, 1400.0, 1500.0, 1400.0, 1250.0,
            1100.0, 1000.0, 1000.0, 1050.0, 1150.0, 1300.0, 1600.0, 1800.0, 1900.0, 1750.0, 1500.0,
            1250.0, 1000.0,
        ],
        kwp_residential / 2.0,
        0.0,
        0.0,
        0.0,
        None,
        0.0,
        None,
    );

    // Bus 2: Residential Zone
    let residential = Bus::new(
        2,
        BusKind::Residential.label(),
        BusKind::Residential,
        0.4,
        [
            800.0, 750.0, 700.0, 700.0, 750.0, 900.0, 1100.0, 1400.0, 1500.0, 1400.0, 1250.0,
            1100.0, 1000.0, 1000.0, 1050.0, 1150.0, 1300.0, 1600.0, 1800.0, 1900.0, 1750.0, 1500.0,
            1250.0, 1000.0,
        ],
        kwp_residential / 2.0,
        0.0,
        0.0,
        0.0,
        None,
        0.0,
        None,
    );

    // Bus 3: Touristic zone
    let touristic = Bus::new(
        3,
        BusKind::Touristic.label(),
        BusKind::Touristic,
        0.4,
        [
            1200.0, 1100.0, 1000.0, 1000.0, 1100.0, 1500.0, 2000.0, 3000.0, 4000.0, 4500.0, 4800.0,
            5000.0, 5000.0, 4800.0, 4500.0, 4000.0, 3800.0, 3500.0, 3000.0, 2500.0, 2000.0, 1800.0,
            1500.0, 1300.0,
        ],
        kwp_touristic,
        1500.0,
        500.0,
        500.0,
        Some((0, 6)),
        90.0,
        None,
    );

    // Bus 4: Industrial Polygon
    let industrial = Bus::new(
        4,
        BusKind::Industrial.label(),
        BusKind::Industrial,
        0.4,
        [
            1200.0, 1100.0, 1000.0, 1000.0, 1100.0, 1500.0, 2500.0, 3500.0, 4000.0, 4000.0, 4000.0,
            4000.0, 3500.0, 4000.0, 4000.0, 4000.0, 3500.0, 3000.0, 2500.0, 2000.0, 1500.0, 1200.0,
            1100.0, 1200.0,
        ],
        kwp_industrial,
        0.0,
        0.0,
        0.0,
        None,
        0.0,
        None,
    );

    // Bus 5: School
    let schools = Bus::new(
        5,
        BusKind::School.label(),
        BusKind::School,
        0.4,
        [
            50.0, 50.0, 50.0, 50.0, 100.0, 200.0, 500.0, 1500.0, 2000.0, 2000.0, 2000.0, 1800.0,
            1500.0, 1500.0, 1500.0, 1200.0, 800.0, 500.0, 200.0, 100.0, 100.0, 100.0, 100.0, 100.0,
        ],
        kwp_school,
        200.0,
        75.0,
        75.0,
        None,
        0.0,
        None,
    );

    let nodes = vec![
        substation,
        historic,
        residential,
        touristic,
        industrial,
        schools,
    ];

    // --- Medium tension lines (25kV) from substation to each ST ---
    let lines = vec![
        Line {
            id: 1,
            from: 0,
            to: 1,
            distance_km: 0.6,
            cross_section_mm2: 150.0,
        },
        Line {
            id: 2,
            from: 0,
            to: 2,
            distance_km: 1.1,
            cross_section_mm2: 150.0,
        },
        Line {
            id: 3,
            from: 0,
            to: 3,
            distance_km: 1.6,
            cross_section_mm2: 240.0,
        },
        Line {
            id: 4,
            from: 0,
            to: 4,
            distance_km: 2.4,
            cross_section_mm2: 240.0,
        },
        Line {
            id: 5,
            from: 0,
            to: 5,
            distance_km: 1.0,
            cross_section_mm2: 95.0,
        },
    ];

    // --- Transformation centers 24 kV / 0.4kV on each bus ---
    let transformers = vec![
        Transformer {
            id: 1,
            from: 0,
            to: 1,
            rated_power_kva: 400.0,
            efficiency: 0.98,
        },
        Transformer {
            id: 2,
            from: 0,
            to: 2,
            rated_power_kva: 630.0,
            efficiency: 0.98,
        },
        Transformer {
            id: 3,
            from: 0,
            to: 3,
            rated_power_kva: 1600.0,
            efficiency: 0.985,
        },
        Transformer {
            id: 4,
            from: 0,
            to: 4,
            rated_power_kva: 1250.0,
            efficiency: 0.99,
        },
        Transformer {
            id: 5,
            from: 0,
            to: 5,
            rated_power_kva: 250.0,
            efficiency: 0.97,
        },
    ];

    Grid::new(nodes, lines, transformers, 25.0)
}

fn print_hour_report(report: &HourReport) {
    let battery_label = if report.total_battery_kw >= 0.0 {
        format!("+{:.1} (discharge)", report.total_battery_kw)
    } else {
        format!("{:.1} (charge)", report.total_battery_kw)
    };
    let import_label = if report.grid_import_kw >= 0.0 {
        format!("{:>8.1} kW imported from transport", report.grid_import_kw)
    } else {
        format!("{:>8.1} kW exported to transport", -report.grid_import_kw)
    };

    println!(
        "{:02}:00 | Demand {:>7.1} kW | FV {:>7.1} kW | Battery {:>20} | {} | Line losses {:>5.2} kW | f={:.3} Hz",
        report.hour,
        report.total_demand_kw,
        report.total_solar_kw,
        battery_label,
        import_label,
        report.losses_kw,
        report.frequency_hz
    );
}

enum Month {
    January,
    February,
    March,
    April,
    May,
    June,
    July,
    August,
    September,
    October,
    November,
    December,
}

fn main() {
    const MONTH: Month = Month::August;
    let month_label = match MONTH {
        Month::January => "january",
        Month::February => "february",
        Month::March => "march",
        Month::April => "april",
        Month::May => "may",
        Month::June => "june",
        Month::July => "july",
        Month::August => "august",
        Month::September => "september",
        Month::October => "october",
        Month::November => "november",
        Month::December => "december",
    };
    let json_file =
        File::open(format!("pvgis/irradiation-{}.json", month_label)).expect("File not found");
    let reader = BufReader::new(json_file);

    let data: PvResponse =
        serde_json::from_reader(reader).expect("Failed to parse JSON. Check file's formatting.");

    let mut grid = create_grid();
    grid.dbg();

    let simulation_duration = 24usize;
    let mut daily_demand_kwh = 0.0;
    let mut daily_solar_kwh = 0.0;
    let mut daily_losses_kwh = 0.0;
    let mut daily_import_kwh = 0.0;
    let mut peak_import_kw = f64::MIN;
    let mut peak_export_kw = f64::MIN;
    let mut reports: Vec<HourReport> = Vec::new();

    println!("=== Simulació horària (dia feiner tipus de juliol) ===");
    for hour in 0..simulation_duration.min(data.outputs.daily_profile.len()) {
        println!("=== {} ===", month_label);
        let irradiation = data.outputs.daily_profile[hour].g_i;
        let report = grid.tick(hour, irradiation);

        daily_demand_kwh += report.total_demand_kw; // 1h -> kW == kWh
        daily_solar_kwh += report.total_solar_kw;
        daily_losses_kwh += report.losses_kw;
        daily_import_kwh += report.grid_import_kw;
        peak_import_kw = peak_import_kw.max(report.grid_import_kw);
        peak_export_kw = peak_export_kw.max(-report.grid_import_kw);

        print_hour_report(&report);
        reports.push(report);
    }

    println!("\n=== Resum diari ===");
    println!(
        "Demanda total consumida:        {:>9.1} kWh",
        daily_demand_kwh
    );
    println!(
        "Generació fotovoltaica total:   {:>9.1} kWh",
        daily_solar_kwh
    );
    println!(
        "Autoconsum solar (% demanda):    {:>8.1} %",
        100.0 * daily_solar_kwh / daily_demand_kwh
    );
    println!(
        "Pèrdues tècniques a les línies:  {:>9.2} kWh",
        daily_losses_kwh
    );
    println!(
        "Balanç net amb la xarxa de transport: {:>9.1} kWh ({})",
        daily_import_kwh,
        if daily_import_kwh >= 0.0 {
            "importador net"
        } else {
            "exportador net"
        }
    );
    println!("Punta d'importació:  {:>7.1} kW", peak_import_kw.max(0.0));
    println!("Punta d'exportació:  {:>7.1} kW", peak_export_kw.max(0.0));

    println!("\nEstat final de bateries:");
    for bus in &grid.nodes {
        if bus.storable_energy_kwh > 0.0 {
            println!(
                "  {:<45} SOC {:>5.1}% ({:.1}/{:.1} kWh)",
                bus.name,
                bus.soc_percent(),
                bus.stored_energy_kwh,
                bus.storable_energy_kwh
            );
        }
    }

    // Simulation data as JSON
    let json = serde_json::to_string_pretty(&reports).expect("Failed to serialize simulation data");
    let mut file =
        File::create("simulation_data.json").expect("Failed to create simulation_data.json");
    file.write_all(json.as_bytes())
        .expect("Failed to write simulation_data.json");
    println!("\nSimulation data written to simulation_data.json");
}
