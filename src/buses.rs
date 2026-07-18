#[derive(Debug)]
pub struct Bus {
    pub id: usize,
    pub name: String,
    pub kind: BusKind,
    pub demand_profile: [f64; 24],
    pub voltage_kv: f64,
    pub stored_energy_kwh: f64,
    pub storable_energy_kwh: f64,
    pub max_charge_kw: f64,
    pub max_discharge_kw: f64,
    pub solar_kwp: f64,
    pub night_charge_window: Option<(usize, usize)>,
    pub night_charge_target_percent: f64,
    pub discharge_window: Option<(usize, usize)>,
    pub frequency: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BusKind {
    Substation,
    HistoricCenter,
    Residential,
    Touristic,
    Industrial,
    School,
}

impl BusKind {
    pub fn label(&self) -> &'static str {
        match self {
            BusKind::Substation => "Malgrat's substation ST",
            BusKind::HistoricCenter => "Historic Center",
            BusKind::Residential => "Residential zone",
            BusKind::Touristic => "Touristic zone",
            BusKind::Industrial => "Industrial polygon",
            BusKind::School => "School",
        }
    }
}

// Normal frequency of the grid.
const STEADY_FREQUENCY: f64 = 50.0;

// Typical loses in a photovoltaic system due to temperature, inverters or cable losses.
const PV_PERFORMANCE_RATIO: f64 = 0.82;

impl Bus {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: usize,
        name: &str,
        kind: BusKind,
        voltage_kv: f64,
        demand_profile: [f64; 24],
        solar_kwp: f64,
        storable_energy_kwh: f64,
        max_charge_kw: f64,
        max_discharge_kw: f64,
        night_charge_window: Option<(usize, usize)>,
        night_charge_target_percent: f64,
        discharge_window: Option<(usize, usize)>,
    ) -> Self {
        Self {
            id,
            name: name.to_string(),
            kind,
            demand_profile,
            voltage_kv,
            stored_energy_kwh: 0.0,
            storable_energy_kwh,
            max_charge_kw,
            max_discharge_kw,
            solar_kwp,
            night_charge_window,
            night_charge_target_percent,
            discharge_window,
            frequency: STEADY_FREQUENCY,
        }
    }

    // Charge the batteries taking energy from the grid.
    pub fn try_night_grid_charge(&mut self, hour: usize) -> f64 {
        let Some((start, end)) = self.night_charge_window else {
            return 0.0;
        };

        if hour < start || hour >= end {
            return 0.0;
        }

        // Kwh needed from the grid
        let target_kwh = self.storable_energy_kwh * self.night_charge_target_percent / 100.0;

        // CHECK: Target energy is already satisfied
        if self.stored_energy_kwh >= target_kwh {
            return 0.0;
        }

        // Batteries capacity to charge
        let charge_kw = (target_kwh - self.stored_energy_kwh)
            .min(self.max_charge_kw)
            .max(0.0);

        self.stored_energy_kwh += charge_kw;

        charge_kw
    }

    // Demand kWh at a certain hour.
    pub fn demand_at(&self, hour: usize) -> f64 {
        self.demand_profile[hour]
    }

    // Solar generation in kW based on global irradiance.
    pub fn solar_generation_at(&self, irradiation_w_m2: f64) -> f64 {
        self.solar_kwp * (irradiation_w_m2 / 1000.0) * PV_PERFORMANCE_RATIO
    }

    // Charge batteries using PV surplus.
    pub fn try_charge(&mut self, surplus_kw: f64) -> (f64, f64) {
        if surplus_kw <= 0.0 || self.storable_energy_kwh <= 0.0 {
            return (0.0, surplus_kw);
        }

        let required_kwh = self.storable_energy_kwh - self.stored_energy_kwh;

        let chargeable_kw = surplus_kw
            .min(self.max_charge_kw)
            .min(required_kwh)
            .max(0.0);

        self.stored_energy_kwh += chargeable_kw;

        (chargeable_kw, surplus_kw - chargeable_kw)
    }

    // Cover local grid deficit.
    pub fn try_discharge(&mut self, deficit_kw: f64) -> (f64, f64) {
        if deficit_kw <= 0.0 || self.stored_energy_kwh <= 0.0 {
            return (0.0, deficit_kw);
        }
        let dischargable_kw = deficit_kw
            .min(self.max_discharge_kw)
            .min(self.stored_energy_kwh)
            .max(0.0);

        self.stored_energy_kwh -= dischargable_kw;

        (dischargable_kw, deficit_kw - dischargable_kw)
    }

    // State of Charge percentage
    pub fn soc_percent(&self) -> f64 {
        if self.storable_energy_kwh <= 0.0 {
            0.0
        } else {
            100.0 * self.stored_energy_kwh / self.storable_energy_kwh
        }
    }
}

