# Decentralized Electrical Grid Simulation

A simulation based on **real-world data**, solar global irradiation from PV_GIS, energy consumption data and line losses.

## Setup

To install the repository locally run this command using Git:

```bash
git clone https://github.com/erixweb/decentralized-grid
```

To modify nodes, go to function create_grid() in `src/main.rs` and change the values there.

## Local energy exchange

At each simulated hour, every node first uses its own photovoltaic generation to serve its demand and, where configured, charges or discharges its local battery. The remaining surpluses and deficits are matched to calculate the reported `local_exchange_kw` indicator.

The exchange matching is an idealized, deterministic surplus-first allocation. It does not change the physical net power at each node. This distinction is important for the current city configurations: each node is connected to the MV substation by its own radial feeder. A transfer between two nodes therefore still loads the seller's feeder (export) and the buyer's feeder (import); it must not be removed before calculating their feeder flows and losses.

For each node, the simulator calculates:
- Transformer primary-side power using the configured transformer efficiency. The difference between primary- and secondary-side power is counted as transformer loss, for both import and export.
- Three-phase line losses using `P_loss = 3 R I²`, with current estimated from the absolute primary-side power, MV voltage and a fixed power factor of 0.95.
- Net exchange with the transport grid after feeder line losses.

## Frequency model

The upstream grid is treated as a stiff 50 Hz reference while the requested power exchange remains within the substation connection limits (15,000 kW import and 5,000 kW export). Frequency deviates only when the requested import or export exceeds those limits: an import shortfall lowers the estimated frequency, while excess generation that cannot be exported raises it. The deviation is a simplified proportional response, using 2 Hz per unit of unmet power relative to total demand.

This avoids calculating frequency from a residual that already includes the grid import needed to balance demand, which would force the residual to approximately zero every hour.

**This is an educational approximation, not a dynamic frequency-stability model.** It does not simulate generator inertia, governor response, primary/secondary frequency control, or frequency disturbances in the upstream transmission grid. Therefore a result of exactly 50 Hz during normal operation is expected and physically consistent with the assumed grid-connected model.

## Model assumptions and limitations

- The network is currently represented as independent radial feeders connected to a common MV substation; actual peer-to-peer routes and meshed power flows are not modelled.
- Local exchange is an energy-allocation indicator, not a physical direct transfer or a financial market. Network congestion and voltage constraints are not represented.
- Line losses assume a balanced three-phase system, fixed power factor, copper resistivity at the reference temperature, and nominal voltage.
- Transformer losses are approximated from the configured constant efficiency; no-load and load-dependent loss components are not separated, and rated-power limits are not enforced.

This is an educational energy-balance approximation, not a full AC load-flow simulation.
