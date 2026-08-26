use crate::buses::{Bus, BusKind};
use crate::lines::Line;
use crate::models::{HourReport, STEADY_FREQUENCY};
use crate::transformers::Transformer;

pub struct Grid {
    pub nodes: Vec<Bus>,
    pub lines: Vec<Line>,
    pub transformers: Vec<Transformer>,
    pub mv_voltage_kv: f64,
}

impl Grid {
    pub fn new(
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

    pub fn tick(&mut self, hour: usize, irradiation_w_m2: f64) -> HourReport {
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
        let max_substation_import_kw = 15000.0;

        // Potencia de respaldo efectivamente importada desde la red de transporte
        let effective_import_kw = grid_import_kw.min(max_substation_import_kw);

        // Balanza de potencia no compensada:
        // Al importar automáticamente la energía requerida por la subestación,
        // el déficit residual de la red local se reduce a cero.
        let uncompensated_imbalance_kw = (total_solar_kw + total_battery_kw + effective_import_kw)
            - (total_demand_kw + losses_kw);

        let imbalance_ratio = if total_demand_kw > 0.0 {
            uncompensated_imbalance_kw / total_demand_kw
        } else {
            0.0
        };

        let frequency_hz = STEADY_FREQUENCY + imbalance_ratio * 2.0;

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
