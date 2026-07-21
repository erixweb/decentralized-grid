use models::PvResponse;
use std::fs::File;
use std::io::BufReader;
use lines::Line;
use transformers::Transformer;
use buses::{Bus, BusKind};

mod lines; 
mod models;
mod transformers;
mod buses;


struct Grid {
    nodes: Vec<Bus>,
    lines: Vec<Line>,
    transformers: Vec<Transformer>,
    mv_voltage_kv: f64,
}
struct HourReport {
    hour: usize,
    total_demand_kw: f64,
    total_solar_kw: f64,
    total_battery_kw: f64, // Positive -> discharging | Negative -> charging
    grid_import_kw: f64,   // Positive -> Import | Negative -> Export
    losses_kw: f64,
    frequency_hz: f64,
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
        let mut total_solar_kw = 0.0;
        let mut total_battery_kw = 0.0;
        let mut grid_import_kw = 0.0;
        let mut losses_kw = 0.0;

        for bus_idx in 0..self.nodes.len() {
            if self.nodes[bus_idx].kind == BusKind::Substation {
                continue;
            }

            let demand = self.nodes[bus_idx].demand_at(hour);
            let solar = self.nodes[bus_idx].solar_generation_at(irradiation_w_m2);

            // Electrical balance
            // Positive -> Surplus
            // Negative -> Deficit
            let net_value = solar - demand;

            total_demand_kw += demand;
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
                Some(tr) => tr.refer_to_primary(-net_after_battery), // >0
                // importing, <0 exporting
                None => -net_after_battery,
            };

            let line_losses = match self.line_for(self.nodes[bus_idx].id) {
                Some(line) => line.power_loss_kw(mv_power_kw, self.mv_voltage_kv),
                None => 0.0,
            };

            losses_kw += line_losses;

            // Have line losses in mind to supply enough energy.
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
            frequency_hz: frequency_hz,
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
    // --- Buses ---
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
            120.0, 110.0, 100.0, 95.0, 95.0, 100.0, 120.0, 160.0, 220.0, 260.0, 290.0, 300.0,
            280.0, 260.0, 220.0, 230.0, 250.0, 280.0, 310.0, 320.0, 300.0, 260.0, 200.0, 150.0,
        ],
        40.0,
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
            250.0, 220.0, 200.0, 190.0, 190.0, 200.0, 230.0, 280.0, 330.0, 300.0, 280.0, 270.0,
            290.0, 340.0, 400.0, 380.0, 350.0, 340.0, 360.0, 400.0, 480.0, 560.0, 520.0, 380.0,
        ],
        180.0,
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
            380.0, 340.0, 300.0, 280.0, 270.0, 270.0, 290.0, 330.0, 400.0, 480.0, 560.0, 650.0,
            750.0, 820.0, 850.0, 830.0, 800.0, 820.0, 900.0, 1050.0, 1200.0, 1300.0, 1150.0, 700.0,
        ],
        260.0,
        300.0,
        100.0,
        100.0,
        Some((0, 6)),
        90.0,
        Some((18, 23)),
    );

    // Bus 4: Industrial Polygon 
    let industrial = Bus::new(
        4,
        BusKind::Industrial.label(),
        BusKind::Industrial,
        0.4,
        [
            650.0, 640.0, 630.0, 630.0, 630.0, 640.0, 700.0, 820.0, 900.0, 920.0, 930.0, 930.0,
            900.0, 850.0, 900.0, 920.0, 910.0, 880.0, 820.0, 750.0, 700.0, 680.0, 660.0, 650.0,
        ],
        520.0,
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
            15.0, 15.0, 15.0, 15.0, 15.0, 15.0, 20.0, 35.0, 55.0, 60.0, 55.0, 55.0, 55.0, 60.0,
            65.0, 60.0, 45.0, 30.0, 20.0, 15.0, 15.0, 15.0, 15.0, 15.0,
        ],
        95.0,
        40.0,
        15.0,
        15.0,
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

fn main() {
    let json_file = File::open("avg-irradiation-july.json").expect("File not found");
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

    println!("=== Simulació horària (dia feiner tipus de juliol) ===");
    for hour in 0..simulation_duration.min(data.outputs.daily_profile.len()) {
        let irradiation = data.outputs.daily_profile[hour].g_i;
        let report = grid.tick(hour, irradiation);

        daily_demand_kwh += report.total_demand_kw; // 1h -> kW == kWh
        daily_solar_kwh += report.total_solar_kw;
        daily_losses_kwh += report.losses_kw;
        daily_import_kwh += report.grid_import_kw;
        peak_import_kw = peak_import_kw.max(report.grid_import_kw);
        peak_export_kw = peak_export_kw.max(-report.grid_import_kw);

        print_hour_report(&report);
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
}
