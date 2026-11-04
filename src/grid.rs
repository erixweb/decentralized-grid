use crate::buses::{Bus, BusKind};
use crate::lines::Line;
use crate::models::{HourReport, STEADY_FREQUENCY};
use crate::transformers::Transformer;

const MAX_SUBSTATION_IMPORT_KW: f64 = 15_000.0;
const MAX_SUBSTATION_EXPORT_KW: f64 = 5_000.0;
const POWER_FACTOR: f64 = 0.95;
const LIMIT_SEARCH_ITERATIONS: usize = 60;
const POWER_EPSILON_KW: f64 = 1e-9;

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
        Self { nodes, lines, transformers, mv_voltage_kv }
    }

    fn transformer_for(&self, bus_id: usize) -> Option<&Transformer> {
        self.transformers.iter().find(|t| t.to == bus_id)
    }

    fn line_for(&self, bus_id: usize) -> Option<&Line> {
        self.lines.iter().find(|l| l.to == bus_id)
    }

    // Returns (net substation import, total transformer + feeder losses).
    // Positive power means import; negative power means export.
    fn calculate_grid_exchange(&self, secondary_power_by_bus: &[f64]) -> (f64, f64) {
        let mut grid_import_kw = 0.0;
        let mut losses_kw = 0.0;

        for (bus_idx, bus) in self.nodes.iter().enumerate() {
            if bus.kind == BusKind::Substation {
                continue;
            }

            let secondary_power_kw = secondary_power_by_bus[bus_idx];
            let primary_power_kw = match self.transformer_for(bus.id) {
                Some(transformer) => transformer.refer_to_primary(secondary_power_kw),
                None => secondary_power_kw,
            };
            let transformer_loss_kw = (primary_power_kw - secondary_power_kw).abs();
            let line_loss_kw = self.line_for(bus.id)
                .map(|line| line.power_loss_kw(primary_power_kw, self.mv_voltage_kv))
                .unwrap_or(0.0);

            losses_kw += transformer_loss_kw + line_loss_kw;
            // Feeder losses are consumed power, and therefore reduce net export too.
            grid_import_kw += primary_power_kw + line_loss_kw;
        }

        (grid_import_kw, losses_kw)
    }

    // Enforce the aggregate substation connection limit by proportionally reducing
    // only the direction that is over its limit. Returns curtailed import/export kW.
    fn enforce_substation_limits(
        &self,
        secondary_power_by_bus: &mut [f64],
    ) -> (f64, f64) {
        let (requested_exchange_kw, _) = self.calculate_grid_exchange(secondary_power_by_bus);

        if requested_exchange_kw > MAX_SUBSTATION_IMPORT_KW {
            let original_imports: f64 = secondary_power_by_bus.iter()
                .filter(|power| **power > 0.0).sum();
            let mut low = 0.0;
            let mut high = 1.0;

            for _ in 0..LIMIT_SEARCH_ITERATIONS {
                let factor = (low + high) / 2.0;
                let candidate: Vec<f64> = secondary_power_by_bus.iter()
                    .map(|power| if *power > 0.0 { *power * factor } else { *power })
                    .collect();
                if self.calculate_grid_exchange(&candidate).0 <= MAX_SUBSTATION_IMPORT_KW {
                    low = factor;
                } else {
                    high = factor;
                }
            }

            for power in secondary_power_by_bus.iter_mut() {
                if *power > 0.0 { *power *= low; }
            }
            let delivered_imports: f64 = secondary_power_by_bus.iter()
                .filter(|power| **power > 0.0).sum();
            return ((original_imports - delivered_imports).max(0.0), 0.0);
        }

        if requested_exchange_kw < -MAX_SUBSTATION_EXPORT_KW {
            let original_exports: f64 = secondary_power_by_bus.iter()
                .filter(|power| **power < 0.0).map(|power| -power).sum();
            let mut low = 0.0;
            let mut high = 1.0;

            for _ in 0..LIMIT_SEARCH_ITERATIONS {
                let factor = (low + high) / 2.0;
                let candidate: Vec<f64> = secondary_power_by_bus.iter()
                    .map(|power| if *power < 0.0 { *power * factor } else { *power })
                    .collect();
                if self.calculate_grid_exchange(&candidate).0 >= -MAX_SUBSTATION_EXPORT_KW {
                    low = factor;
                } else {
                    high = factor;
                }
            }

            for power in secondary_power_by_bus.iter_mut() {
                if *power < 0.0 { *power *= low; }
            }
            let delivered_exports: f64 = secondary_power_by_bus.iter()
                .filter(|power| **power < 0.0).map(|power| -power).sum();
            return (0.0, (original_exports - delivered_exports).max(0.0));
        }

        (0.0, 0.0)
    }

    // Idealized surplus/deficit matching for the local-exchange indicator.
    // Positive means surplus; negative means deficit.
    fn clear_local_exchange(net_power: &mut [f64]) -> f64 {
        let mut exchanged_kw = 0.0;
        for seller in 0..net_power.len() {
            if net_power[seller] <= 0.0 { continue; }
            for buyer in 0..net_power.len() {
                if net_power[buyer] >= 0.0 { continue; }
                let traded_kw = net_power[seller].min(-net_power[buyer]);
                net_power[seller] -= traded_kw;
                net_power[buyer] += traded_kw;
                exchanged_kw += traded_kw;
                if net_power[seller] <= POWER_EPSILON_KW { break; }
            }
        }
        exchanged_kw
    }

    pub fn tick(&mut self, hour: usize, irradiation_w_m2: f64) -> HourReport {
        let mut total_demand_kw = 0.0;
        let mut potential_solar_kw = 0.0;
        let mut potential_generator_kw = 0.0;
        let mut total_battery_kw = 0.0;

        for bus in &self.nodes {
            if bus.kind == BusKind::Substation { continue; }
            total_demand_kw += bus.demand_at(hour);
            potential_solar_kw += bus.solar_generation_at(irradiation_w_m2);
            potential_generator_kw += bus.generation_profile_kw[hour];
        }

        let battery_charge_capacity: f64 = self.nodes.iter()
            .filter(|bus| bus.kind != BusKind::Substation)
            .map(|bus| bus.max_charge_kw).sum();
        let max_absorption = (total_demand_kw + battery_charge_capacity + MAX_SUBSTATION_EXPORT_KW
            - potential_generator_kw).max(0.0);
        let actual_solar_kw = potential_solar_kw.min(max_absorption);
        let curtailment_ratio = if potential_solar_kw > 0.0 {
            actual_solar_kw / potential_solar_kw
        } else { 1.0 };
        let mut total_solar_kw = 0.0;
        let mut total_generation_kw = 0.0;
        let mut curtailed_solar_kw = (potential_solar_kw - actual_solar_kw).max(0.0);
        let mut curtailed_generation_kw = curtailed_solar_kw;
        let mut unserved_demand_kw = 0.0;
        let mut secondary_power_by_bus = vec![0.0; self.nodes.len()];
        let mut night_grid_charge_by_bus = vec![0.0; self.nodes.len()];

        for bus_idx in 0..self.nodes.len() {
            if self.nodes[bus_idx].kind == BusKind::Substation { continue; }

            let demand = self.nodes[bus_idx].demand_at(hour);
            let solar = self.nodes[bus_idx].solar_generation_at(irradiation_w_m2) * curtailment_ratio;
            total_solar_kw += solar;
            let generator = self.nodes[bus_idx].generation_profile_kw[hour];
            total_generation_kw += solar + generator;
            let net_value = solar + generator - demand;
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
                    let (discharged, remaining_deficit) = self.nodes[bus_idx].try_discharge(-net_value);
                    total_battery_kw += discharged;
                    -remaining_deficit
                } else { net_value }
            } else { 0.0 };

            let night_charge = self.nodes[bus_idx].try_night_grid_charge(hour);
            total_battery_kw -= night_charge;
            night_grid_charge_by_bus[bus_idx] = night_charge;
            // Positive means power imported at the bus; negative means exported.
            secondary_power_by_bus[bus_idx] = -net_after_battery + night_charge;
        }

        // Enforce each transformer's active-power capacity (kVA × assumed power factor).
        // Import above the rating becomes unserved demand; export above it is curtailed.
        let mut transformer_overload_kw = 0.0;
        for bus_idx in 0..self.nodes.len() {
            if self.nodes[bus_idx].kind == BusKind::Substation { continue; }
            if let Some(transformer) = self.transformer_for(self.nodes[bus_idx].id) {
                let active_limit_kw = transformer.rated_power_kva * POWER_FACTOR;
                let requested_kw = secondary_power_by_bus[bus_idx];
                if requested_kw > active_limit_kw {
                    let excess = requested_kw - active_limit_kw;
                    secondary_power_by_bus[bus_idx] = active_limit_kw;
                    transformer_overload_kw += excess;
                    let curtailed_battery_charge = excess.min(night_grid_charge_by_bus[bus_idx]);
                    if curtailed_battery_charge > 0.0 {
                        self.nodes[bus_idx].stored_energy_kwh -= curtailed_battery_charge;
                        night_grid_charge_by_bus[bus_idx] -= curtailed_battery_charge;
                        total_battery_kw += curtailed_battery_charge;
                    }
                    unserved_demand_kw += excess - curtailed_battery_charge;
                } else if requested_kw < -active_limit_kw {
                    let excess = -active_limit_kw - requested_kw;
                    secondary_power_by_bus[bus_idx] = -active_limit_kw;
                    transformer_overload_kw += excess;
                    curtailed_generation_kw += excess;
                }
            }
        }

        // The connection limit is enforced after transformer limits and feeder losses.
        let requested_secondary_power_by_bus = secondary_power_by_bus.clone();
        let (unserved_at_substation_kw, curtailed_at_substation_kw) =
            self.enforce_substation_limits(&mut secondary_power_by_bus);
        let mut curtailed_battery_charge_kw = 0.0;
        for bus_idx in 0..self.nodes.len() {
            let requested_export = (-requested_secondary_power_by_bus[bus_idx]).max(0.0);
            let delivered_export = (-secondary_power_by_bus[bus_idx]).max(0.0);
            let curtailed_export = (requested_export - delivered_export).max(0.0);
            total_solar_kw = (total_solar_kw - curtailed_export).max(0.0);
            total_generation_kw = (total_generation_kw - curtailed_export).max(0.0);

            let requested_import = requested_secondary_power_by_bus[bus_idx].max(0.0);
            let delivered_import = secondary_power_by_bus[bus_idx].max(0.0);
            let import_reduction = (requested_import - delivered_import).max(0.0);
            let curtailed_charge = import_reduction.min(night_grid_charge_by_bus[bus_idx]);
            if curtailed_charge > 0.0 {
                self.nodes[bus_idx].stored_energy_kwh -= curtailed_charge;
                total_battery_kw += curtailed_charge;
                curtailed_battery_charge_kw += curtailed_charge;
            }
        }
        unserved_demand_kw += (unserved_at_substation_kw - curtailed_battery_charge_kw).max(0.0);
        curtailed_generation_kw += curtailed_at_substation_kw;
        curtailed_solar_kw = curtailed_generation_kw;

        let (grid_import_kw, losses_kw) = self.calculate_grid_exchange(&secondary_power_by_bus);

        // The upstream transmission system is modelled as a stiff 50 Hz source.
        // A static power-flow calculation cannot predict frequency dynamics.
        let frequency_hz = STEADY_FREQUENCY;

        let mut exchange_balances: Vec<f64> = secondary_power_by_bus.iter()
            .map(|power| -*power).collect();
        let local_exchange_kw = Self::clear_local_exchange(&mut exchange_balances);

        HourReport {
            hour,
            total_demand_kw,
            total_solar_kw,
            total_generation_kw,
            total_battery_kw,
            local_exchange_kw,
            grid_import_kw,
            losses_kw,
            unserved_demand_kw,
            curtailed_solar_kw: curtailed_generation_kw,
            curtailed_generation_kw,
            transformer_overload_kw,
            frequency_hz,
        }
    }

    pub fn dbg(&self) {
        println!("=== Results ===");
        println!("{:<3} {:<45} {:<10} {:<10} {:<13}", "Id", "Bus", "kV", "kWp", "Battery kWh");
        for bus in &self.nodes {
            println!("{:<3} {:<45} {:>10.2} {:>10.1} {:>13.1}",
                bus.id, bus.name, bus.voltage_kv, bus.solar_kwp, bus.storable_energy_kwh)
        }
        println!();
        println!("{:<3} {:<6} {:<6} {:>12} {:>13}", "Id", "From", "To", "Dist. (km)", "X-Section (mm²)");
        for line in &self.lines {
            println!("{:<3} {:<6} {:<6} {:>12} {:>13.0}",
                line.id, line.from, line.to, line.distance_km, line.cross_section_mm2);
        }
        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::{Grid, MAX_SUBSTATION_EXPORT_KW, MAX_SUBSTATION_IMPORT_KW};
    use crate::buses::{Bus, BusKind};
    use crate::lines::Line;
    use crate::transformers::Transformer;

    fn test_grid() -> Grid {
        let mut demand = [0.0; 24];
        demand[0] = 2_000.0;
        Grid::new(
            vec![
                Bus::new(0, "Substation", BusKind::Substation, 25.0, [0.0; 24], 0.0, 0.0, 0.0, 0.0, None, 0.0, None),
                Bus::new(1, "Load", BusKind::Industrial, 0.4, demand, 0.0, 0.0, 0.0, 0.0, None, 0.0, None),
            ],
            vec![Line { id: 0, from: 0, to: 1, distance_km: 0.0, cross_section_mm2: 240.0 }],
            vec![Transformer { id: 0, from: 0, to: 1, rated_power_kva: 1_000.0, efficiency: 1.0 }],
            25.0,
        )
    }

    #[test]
    fn transformer_rating_limits_import_and_reports_unserved_load() {
        let mut grid = test_grid();
        let report = grid.tick(0, 0.0);
        assert!((report.grid_import_kw - 950.0).abs() < 1e-6);
        assert!((report.unserved_demand_kw - 1_050.0).abs() < 1e-6);
        assert!((report.transformer_overload_kw - 1_050.0).abs() < 1e-6);
    }

    #[test]
    fn substation_import_limit_is_physically_enforced() {
        let mut grid = test_grid();
        grid.transformers[0].rated_power_kva = 30_000.0;
        grid.nodes[1].demand_profile[0] = 20_000.0;
        let report = grid.tick(0, 0.0);
        assert!(report.grid_import_kw <= MAX_SUBSTATION_IMPORT_KW + 1e-6);
        assert!(report.unserved_demand_kw > 0.0);
    }

    #[test]
    fn substation_export_limit_is_physically_enforced() {
        let mut grid = test_grid();
        grid.transformers[0].rated_power_kva = 30_000.0;
        let mut feeder_power = vec![0.0, -6_000.0];
        let (unserved, curtailed) = grid.enforce_substation_limits(&mut feeder_power);
        let final_exchange = grid.calculate_grid_exchange(&feeder_power).0;
        assert_eq!(unserved, 0.0);
        assert!(curtailed > 999.0 && curtailed < 1_001.0);
        assert!(final_exchange >= -MAX_SUBSTATION_EXPORT_KW - 1e-6);
    }

    #[test]
    fn grid_frequency_is_stiff_upstream_reference() {
        let mut grid = test_grid();
        let report = grid.tick(0, 0.0);
        assert_eq!(report.frequency_hz, 50.0);
    }
}
