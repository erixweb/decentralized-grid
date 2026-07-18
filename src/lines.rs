const MATERIAL_RESISTIVITY: f64 = 1.68e-8;

pub struct Line {
    pub id: usize,
    pub from: usize,
    pub to: usize,
    pub distance_km: f64,
    pub cross_section_mm2: f64,
}

impl Line {
    pub fn power_loss_kw(&self, power_kw: f64, voltage_kv: f64) -> f64 {
        if power_kw.abs() < 1e-9 {
            return 0.0;
        }

        // Units using International System
        let power_w = power_kw.abs() * 1000.0;
        let voltage_v = voltage_kv * 1000.0;
        let distance_m = self.distance_km * 1000.0;
        let area_m2 = self.cross_section_mm2 * 1e-6;

        // I = P / (sqrt(3) · V · cos(\hi))
        let intensity = power_w / (3f64.sqrt() * voltage_v * 0.95);
        // R = p · L / A
        let resistance = MATERIAL_RESISTIVITY * distance_m / area_m2;

        // P_loss = 3 · R · I^2
        let power_loss_w = 3.0 * resistance * intensity * intensity;

        // Power losses in kW.
        power_loss_w / 1000.0
    }
}
