use models::PvResponse;
use std::fs::File;
use std::io::BufReader;

mod buses;
mod grid;
mod grid_builder;
mod lines;
mod models;
mod reporting;
mod transformers;

fn main() {
    println!("Enter the month (1-12):");
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .expect("Failed to read line");

    let month_num: usize = input
        .trim()
        .parse()
        .expect("Please enter a valid number between 1 and 12");

    let month_label = match month_num {
        1 => "january",
        2 => "february",
        3 => "march",
        4 => "april",
        5 => "may",
        6 => "june",
        7 => "july",
        8 => "august",
        9 => "september",
        10 => "october",
        11 => "november",
        12 => "december",
        _ => panic!("Month must be between 1 and 12"),
    };

    let json_file =
        File::open(format!("pvgis/irradiation-{}.json", month_label)).expect("File not found");
    let reader = BufReader::new(json_file);

    let data: PvResponse =
        serde_json::from_reader(reader).expect("Failed to parse JSON. Check file's formatting.");

    let mut grid = grid_builder::create_grid("malgrat");
    grid.dbg();

    let simulation_duration = 24usize;
    let mut daily_demand_kwh = 0.0;
    let mut daily_solar_kwh = 0.0;
    let mut daily_losses_kwh = 0.0;
    let mut daily_import_kwh = 0.0;
    let mut peak_import_kw = f64::MIN;
    let mut peak_export_kw = f64::MIN;
    let mut reports: Vec<models::HourReport> = Vec::new();

    println!("=== Simulació horària ===");
    for hour in 0..simulation_duration.min(data.outputs.daily_profile.len()) {
        println!("=== {} ===", month_label);
        let irradiation = data.outputs.daily_profile[hour].g_i;
        let report = grid.tick(hour, irradiation);

        daily_demand_kwh += report.total_demand_kw; // 1h -> kW == kWh
        daily_solar_kwh += report.total_solar_kw;
        daily_losses_kwh += report.losses_kw;
        daily_import_kwh += report.grid_import_kw;
        peak_import_kw = peak_import_kw.max(report.grid_import_kw);
        peak_export_kw = peak_export_kw.max(-report.grid_import_kw);

        reporting::print_hour_report(&report);
        reports.push(report);
    }

    println!("\n=== Resum diari ===");
    println!(
        "Demanda total consumida:        {:>9.1} kWh",
        daily_demand_kwh
    );
    println!(
        "Generació fotovoltaica total:   {:>9.1} kWh",
        daily_solar_kwh
    );
    println!(
        "Autoconsum solar (% demanda):    {:>8.1} %",
        100.0 * daily_solar_kwh / daily_demand_kwh
    );
    println!(
        "Pèrdues tècniques a les línies:  {:>9.2} kWh",
        daily_losses_kwh
    );
    println!(
        "Balanç net amb la xarxa de transport: {:>9.1} kWh ({})",
        daily_import_kwh,
        if daily_import_kwh >= 0.0 {
            "importador net"
        } else {
            "exportador net"
        }
    );
    println!("Punta d'importació:  {:>7.1} kW", peak_import_kw.max(0.0));
    println!("Punta d'exportació:  {:>7.1} kW", peak_export_kw.max(0.0));

    println!("\nEstat final de bateries:");
    for bus in &grid.nodes {
        if bus.storable_energy_kwh > 0.0 {
            println!(
                "  {:<45} SOC {:>5.1}% ({:.1}/{:.1} kWh)",
                bus.name,
                bus.soc_percent(),
                bus.stored_energy_kwh,
                bus.storable_energy_kwh
            );
        }
    }

    reporting::write_simulation_data("./visual/public/simulation_data.json", &reports);
}
