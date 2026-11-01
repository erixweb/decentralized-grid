# Decentralized Electrical Grid Simulation

A simplified educational simulation using real-world solar irradiation data from PVGIS, estimated electricity demand, photovoltaic generation, battery dispatch and network losses.

## Setup

Clone the repository:

```bash
git clone https://github.com/erixweb/decentralized-grid
```

To modify nodes, edit the city configuration in `cities/`.

## Local energy exchange

At each simulated hour, each node first uses its photovoltaic generation to serve local demand and, where configured, charges or discharges its local battery. Remaining surpluses and deficits are matched to calculate `local_exchange_kw`.

The matching is an idealized allocation indicator, not a financial market or a model of direct physical peer-to-peer routing. The current city configurations use independent radial feeders connected to a common MV substation.

## Transformer and substation capacity limits

Transformer active-power capacity is approximated as:

`P_limit = rated_power_kva × 0.95`

The 0.95 power factor is the same fixed assumption used in the line-current calculation. Import above a transformer's limit is recorded as `unserved_demand_kw`; export above the limit is recorded as `curtailed_solar_kw`. `transformer_overload_kw` reports the requested power in excess of transformer ratings before it is shed or curtailed.

The substation connection limits are 15,000 kW import and 5,000 kW export. They are enforced after transformer limits and feeder losses. If an aggregate limit is exceeded, the simulator proportionally reduces the relevant direction of power flow using a numerical search. The reduced import is counted as unserved demand; the reduced export is counted as curtailed generation. The final `grid_import_kw` therefore respects the configured connection limits (within numerical tolerance).

The simulation reports transformer and feeder losses separately as a combined `losses_kw` value. Transformer efficiency is constant and feeder losses use a balanced three-phase approximation:

`P_loss = 3 R I²`

with fixed power factor 0.95, nominal voltage and copper resistivity at a reference temperature.

## Frequency model

The upstream transmission system is treated as a stiff 50 Hz reference, so `frequency_hz` remains 50 Hz. This is intentional: a static hourly energy-balance model cannot calculate grid frequency deviations. A credible frequency model would require at least generator/inverter response, system inertia, control dynamics and time-dependent disturbances. The previous heuristic frequency deviation has therefore been removed rather than presenting it as a physical result.

## Model assumptions and limitations

- The network is represented as independent radial feeders connected to a common MV substation; meshed power flows and direct peer-to-peer routes are not modelled.
- Transformer limits are enforced using a fixed power factor approximation; transformer thermal dynamics and voltage regulation are not represented.
- Line losses assume a balanced three-phase system, fixed power factor, nominal voltage and constant conductor resistivity.
- No nodal voltage solution, reactive-power flow, line thermal ampacity, or AC load-flow solution is calculated.
- Battery charge/discharge efficiencies and degradation are not modelled; initial state of charge is zero.
- Hourly frequency is a fixed upstream reference, not a dynamic stability calculation.

This is an educational energy-balance approximation, not a full AC load-flow simulation.
