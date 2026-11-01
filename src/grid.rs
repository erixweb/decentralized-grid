use crate::buses::{Bus, BusKind};
use crate::lines::Line;
use crate::models::{HourReport, STEADY_FREQUENCY};
use crate::transformers::Transformer;

// Simplified grid-connection limits and frequency-response sensitivity.
const MAX_SUBSTATION_IMPORT_KW: f64 = 15_000.0;
const MAX_SUBSTATION_EXPORT_KW: f64 = 5_000.0;
const FREQUENCY_DROOP_HZ_PER_PU: f64 = 2.0;

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

    // Estimate frequency from power that cannot be exchanged with the upstream grid.
    // Positive grid_import_kw means import; negative means export. Within the
    // connection limits, the upstream grid is treated as a stiff 50 Hz reference.
    // Outside those limits, unmet import lowers frequency and excess export raises it.
    fn estimate_frequency(grid_import_kw: f64, total_demand_kw: f64) -> f64 {
        let power_imbalance_kw = if grid_import_kw > MAX_SUBSTATION_IMPORT_KW {
            -(grid_import_kw - MAX_SUBSTATION_IMPORT_KW)
        } else if grid_import_kw < -MAX_SUBSTATION_EXPORT_KW {
            -MAX_SUBSTATION_EXPORT_KW - grid_import_kw
        } else {
            0.0
        };

        let imbalance_ratio = if total_demand_kw > 0.0 {
            power_imbalance_kw / total_demand_kw
        } else {
            0.0
        };

        STEADY_FREQUENCY + imbalance_ratio * FREQUENCY_DROOP_HZ_PER_PU
    }

    // Match local PV/battery surpluses with deficits for the peer-exchange indicator.
    // Values are bus-terminal kW: positive means surplus, negative means deficit.
    // This matching is an energy-allocation step; it must not erase physical feeder
    // flows. In the current radial/star topology, a peer trade flows out through the
    // seller's feeder and back through the buyer's feeder via the MV busbar.
    fn clear_local_exchange(net_power: &mut [f64]) -> f64 {
        let mut exchanged_kw = 0.0;

        for seller in 0..net_power.len() {
            if net_power[seller] <= 0.0 {
                continue;
            }

            for buyer in 0..net_power.len() {
                if net_power[buyer] >= 0.0 {
                    continue;
                }

                let traded_kw = net_power[seller].min(-net_power[buyer]);
                net_power[seller] -= traded_kw;
                net_power[buyer] += traded_kw;
                exchanged_kw += traded_kw;

                if net_power[seller] <= 1e-9 {
                    break;
                }
            }
        }

        exchanged_kw
    }

    pub fn tick(&mut self, hour: usize, irradiation_w_m2: f64) -> HourReport {
        let mut total_demand_kw = 0.0;
        let mut potential_solar_kw = 0.0;
        let mut total_battery_kw = 0.0;
        let mut grid_import_kw = 0.0;
        let mut losses_kw = 0.0;

        // First pass: Calculate total demand and potential solar generation.
        for bus_idx in 0..self.nodes.len() {
            if self.nodes[bus_idx].kind == BusKind::Substation {
                continue;
            }
            total_demand_kw += self.nodes[bus_idx].demand_at(hour);
            potential_solar_kw += self.nodes[bus_idx].solar_generation_at(irradiation_w_m2);
        }

        // Maximum solar absorption includes local demand, battery charging and
        // the permitted export capacity of the substation.
        let mut battery_charge_capacity = 0.0;
        for bus in &self.nodes {
            if bus.kind != BusKind::Substation {
                battery_charge_capacity += bus.max_charge_kw;
            }
        }

        let max_absorption =
            total_demand_kw + battery_charge_capacity + MAX_SUBSTATION_EXPORT_KW;

        // Solar Curtailment: Limit generation to what the grid can handle.
        let actual_solar_kw = potential_solar_kw.min(max_absorption);
        let curtailment_ratio = if potential_solar_kw > 0.0 {
            actual_solar_kw / potential_solar_kw
        } else {
            1.0
        };

        let mut total_solar_kw = 0.0;
        let mut net_power_by_bus = vec![0.0; self.nodes.len()];

        // Each node first serves its own demand and uses its own battery.
        for bus_idx in 0..self.nodes.len() {
            if self.nodes[bus_idx].kind == BusKind::Substation {
                continue;
            }

            let demand = self.nodes[bus_idx].demand_at(hour);
            let solar =
                self.nodes[bus_idx].solar_generation_at(irradiation_w_m2) * curtailment_ratio;
            total_solar_kw += solar;

            let net_value = solar - demand;
            let in_night_charge_window = matches!(
                self.nodes[bus_idx].night_charge_window,
                Some((start, end)) if hour >= start && hour < end
            );

            let mut net_after_battery = if in_night_charge_window {
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
            net_after_battery -= night_charge;
            net_power_by_bus[bus_idx] = net_after_battery;
        }

        // Calculate the amount matched between peers on a copy. Keep the original
        // nodal balances for the physical feeder-flow and loss calculations below.
        let mut exchange_balances = net_power_by_bus.clone();
        let local_exchange_kw = Self::clear_local_exchange(&mut exchange_balances);

        // In the current star topology every node has its own feeder to the MV busbar.
        // A peer transfer therefore loads both the exporting and importing feeders.
        // Calculate transformer and line losses from each node's full pre-trade balance.
        for bus_idx in 0..self.nodes.len() {
            if self.nodes[bus_idx].kind == BusKind::Substation {
                continue;
            }

            let bus_power_kw = -net_power_by_bus[bus_idx];
            let (mv_power_kw, transformer_losses_kw) =
                match self.transformer_for(self.nodes[bus_idx].id) {
                    Some(tr) => {
                        let primary_power_kw = tr.refer_to_primary(bus_power_kw);
                        let transformer_loss_kw = (primary_power_kw - bus_power_kw).abs();
                        (primary_power_kw, transformer_loss_kw)
                    }
                    None => (bus_power_kw, 0.0),
                };

            let line_losses_kw = match self.line_for(self.nodes[bus_idx].id) {
                Some(line) => line.power_loss_kw(mv_power_kw, self.mv_voltage_kv),
                None => 0.0,
            };

            losses_kw += transformer_losses_kw + line_losses_kw;
            grid_import_kw += mv_power_kw + line_losses_kw;
        }

        // Do not use grid_import_kw to close the energy balance before estimating
        // frequency: that would make the residual zero by construction. Frequency
        // only deviates when the requested exchange exceeds the substation limits.
        let frequency_hz = Self::estimate_frequency(grid_import_kw, total_demand_kw);

        HourReport {
            hour,
            total_demand_kw,
            total_solar_kw,
            total_battery_kw,
            local_exchange_kw,
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

#[cfg(test)]
mod tests {
    use super::Grid;
    use super::{MAX_SUBSTATION_EXPORT_KW, MAX_SUBSTATION_IMPORT_KW};
    use crate::models::STEADY_FREQUENCY;

    #[test]
    fn local_surplus_covers_other_nodes_deficit() {
        let mut net = [100.0, -80.0, -50.0];
        let exchanged = Grid::clear_local_exchange(&mut net);

        assert!((exchanged - 100.0).abs() < 1e-9);
        assert!(net.iter().all(|value| *value >= -1e-9));
        assert!((net.iter().sum::<f64>() + 30.0).abs() < 1e-9);
    }

    #[test]
    fn local_exchange_preserves_total_power_balance() {
        let mut net = [75.0, -20.0, 10.0, -90.0];
        let initial_balance = net.iter().sum::<f64>();
        Grid::clear_local_exchange(&mut net);

        assert!((net.iter().sum::<f64>() - initial_balance).abs() < 1e-9);
        assert!((net.iter().filter(|value| **value > 0.0).sum::<f64>() - 0.0).abs() < 1e-9);
    }

    #[test]
    fn no_trade_when_all_nodes_have_same_balance_sign() {
        let mut net = [10.0, 20.0, 0.0];
        let exchanged = Grid::clear_local_exchange(&mut net);

        assert_eq!(exchanged, 0.0);
        assert_eq!(net, [10.0, 20.0, 0.0]);
    }

    #[test]
    fn frequency_is_nominal_within_substation_limits() {
        assert_eq!(
            Grid::estimate_frequency(10_000.0, 10_000.0),
            STEADY_FREQUENCY
        );
        assert_eq!(
            Grid::estimate_frequency(-MAX_SUBSTATION_EXPORT_KW, 10_000.0),
            STEADY_FREQUENCY
        );
    }

    #[test]
    fn frequency_falls_when_import_capacity_is_exceeded() {
        let frequency = Grid::estimate_frequency(
            MAX_SUBSTATION_IMPORT_KW + 1_000.0,
            10_000.0,
        );

        assert!((frequency - 49.8).abs() < 1e-9);
    }

    #[test]
    fn frequency_rises_when_export_capacity_is_exceeded() {
        let frequency = Grid::estimate_frequency(
            -(MAX_SUBSTATION_EXPORT_KW + 1_000.0),
            10_000.0,
        );

        assert!((frequency - 50.2).abs() < 1e-9);
    }
}
