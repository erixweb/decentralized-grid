use crate::models::HourReport;
use std::fs::File;
use std::io::Write;

pub fn print_hour_report(report: &HourReport) {
    let battery_label = if report.total_battery_kw >= 0.0 {
        format!("+{:.1} (discharge)", report.total_battery_kw)
    } else {
        format!("{:.1} (charge)", report.total_battery_kw)
    };
    let import_label = if report.grid_import_kw >= 0.0 {
        format!("{:>8.1} kW imported from transport", report.grid_import_kw)
    } else {
        format!("{:>8.1} kW exported to transport", -report.grid_import_kw)
    };

    println!(
        "{:02}:00 | Demand {:>7.1} kW | FV {:>7.1} kW | Battery {:>20} | Local exchange {:>7.1} kW | {} | Technical losses {:>5.2} kW | f={:.3} Hz",
        report.hour,
        report.total_demand_kw,
        report.total_solar_kw,
        battery_label,
        report.local_exchange_kw,
        import_label,
        report.losses_kw,
        report.frequency_hz
    );
}

pub fn write_simulation_data(path: &str, reports: &[HourReport]) {
    let json = serde_json::to_string_pretty(&reports).expect("Failed to serialize simulation data");
    let mut file = File::create(path).expect("Failed to create simulation_data.json");
    file.write_all(json.as_bytes())
        .expect("Failed to write simulation_data.json");
    println!("\nSimulation data written to simulation_data.json");
}
