pub struct Transformer {
    pub id: usize,
    pub from: usize,
    pub to: usize,
    pub rated_power_kva: f64,
    pub efficiency: f64,
}

impl Transformer {
    pub fn refer_to_primary(&self, power_kw: f64) -> f64 {
        if power_kw >= 0.0 {
            power_kw / self.efficiency
        } else {
            power_kw * self.efficiency
        }
    }
}
