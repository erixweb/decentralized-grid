<h1 align="center">Decentralized Electrical Grid Simulation</h1>

<br />

A simulation based on **real-world data**, solar global irradiation from PV_GIS, energy consumption data and line losses.

<h2 align="left">Setup</h2>

To install the repository locally run this command using Git:

```bash
git clone https://github.com/erixweb/decentralized-grid
```

To modify nodes, go to function create_grid() in `src/main.rs` and change the values there.

<h2>Local energy exchange</h2>

At each simulated hour, every node first uses its own photovoltaic generation to serve its demand and, where configured, charges or discharges its local battery. The remaining surpluses and deficits are matched to calculate the reported `local_exchange_kw` indicator.

The exchange matching is an idealized, deterministic surplus-first allocation. It does not change the physical net power at each node. This distinction is important for the current city configurations: each node is connected to the MV substation by its own radial feeder. A transfer between two nodes therefore still loads the seller's feeder (export) and the buyer's feeder (import); it must not be removed before calculating their feeder flows and losses.

For each node, the simulator calculates:
- Transformer primary-side power using the configured transformer efficiency. The difference between primary- and secondary-side power is counted as transformer loss, for both import and export.
- Three-phase line losses using `P_loss = 3 R I²`, with current estimated from the absolute primary-side power, MV voltage and a fixed power factor of 0.95.
- Net exchange with the transport grid after feeder line losses.

**Model assumptions and limitations:**
- The network is currently represented as independent radial feeders connected to a common MV substation; actual peer-to-peer routes and meshed power flows are not modelled.
- Local exchange is an energy-allocation indicator, not a physical direct transfer or a financial market. Network congestion and voltage constraints are not represented.
- Line losses assume a balanced three-phase system, fixed power factor, copper resistivity at the reference temperature, and nominal voltage.
- Transformer losses are approximated from the configured constant efficiency; no-load and load-dependent loss components are not separated, and rated-power limits are not enforced.

This is an educational energy-balance approximation, not a full AC load-flow simulation.
