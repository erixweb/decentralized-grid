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

At each simulated hour, every node first uses its own photovoltaic generation to serve its demand and, where configured, charges or discharges its local battery. The remaining surpluses and deficits are then matched between nodes before the residual power is exchanged with the transport grid.

The matching is a simple deterministic surplus-first algorithm: each node with excess power supplies nodes with a deficit until either its surplus or the deficit is exhausted. The hourly report exposes `local_exchange_kw`, and the daily summary reports the total energy exchanged locally in kWh.

**Model assumptions and limitations:**
- Local trades are treated as ideal transfers at bus terminals. The model does not yet calculate the physical route between a seller and a buyer, network congestion, voltage constraints, or losses specifically attributable to peer-to-peer transfers.
- The algorithm matches aggregate local surpluses and deficits; it is not a financial market and does not model prices, bids, or settlement.
- Any surplus or deficit left after local matching is handled by the existing transformer, line-loss, and transport-grid calculations.

This is an educational energy-balance approximation, not a full AC load-flow simulation.
