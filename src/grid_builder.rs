use crate::buses::{Bus, BusKind};
use crate::grid::Grid;
use crate::lines::Line;
use crate::transformers::Transformer;

pub fn create_grid() -> Grid {
    const SOLAR_COVERAGE: f64 = 0.3;
    const USEFUL_ROOF_PERCENTAGE: f64 = 0.5;

    // Based on Mapa-urbanístic-catalunya.csv for Malgrat de Mar (2025):
    // 13_Qual_SUC_R (Residential) sum of R1,R2,R3,R4,R5,R6:
    let residential_ha = 88.7441 * USEFUL_ROOF_PERCENTAGE * SOLAR_COVERAGE;

    // 12_A1_SUC (Industrial): 36.0394 ha
    let industrial_ha = 36.0394 * USEFUL_ROOF_PERCENTAGE * SOLAR_COVERAGE;

    // 12_A2_SUC (Touristic/Business): 8.6101 ha
    let touristic_ha = 8.6101 * USEFUL_ROOF_PERCENTAGE * SOLAR_COVERAGE;

    // 15_SE_SUC (Schools and local buildings): 16.1947 ha
    let school_ha = 16.1947 * USEFUL_ROOF_PERCENTAGE * SOLAR_COVERAGE;

    // Calc: (ha * 10000 m2/ha * 60%) / 5 m2/kWp = ha * 1200 kWp/ha
    let kwp_residential = residential_ha * 1200.0;
    let kwp_industrial = industrial_ha * 1200.0;
    let kwp_touristic = touristic_ha * 1200.0;
    let kwp_school = school_ha * 1200.0;

    // Bus 0: Substation
    let substation = Bus::new(
        0,
        BusKind::Substation.label(),
        BusKind::Substation,
        25.0,
        [0.0; 24],
        0.0,
        0.0,
        0.0,
        0.0,
        None,
        0.0,
        None,
    );

    // Bus 1: Historic Center
    let historic = Bus::new(
        1,
        BusKind::HistoricCenter.label(),
        BusKind::HistoricCenter,
        0.4,
        [
            800.0, 750.0, 700.0, 700.0, 750.0, 900.0, 1100.0, 1400.0, 1500.0, 1400.0, 1250.0,
            1100.0, 1000.0, 1000.0, 1050.0, 1150.0, 1300.0, 1600.0, 1800.0, 1900.0, 1750.0, 1500.0,
            1250.0, 1000.0,
        ],
        kwp_residential / 2.0,
        200.0, // Moderate storage for historic center
        50.0,
        50.0,
        None,
        0.0,
        None,
    );

    // Bus 2: Residential Zone
    let residential = Bus::new(
        2,
        BusKind::Residential.label(),
        BusKind::Residential,
        0.4,
        [
            800.0, 750.0, 700.0, 700.0, 750.0, 900.0, 1100.0, 1400.0, 1500.0, 1400.0, 1250.0,
            1100.0, 1000.0, 1000.0, 1050.0, 1150.0, 1300.0, 1600.0, 1800.0, 1900.0, 1750.0, 1500.0,
            1250.0, 1000.0,
        ],
        kwp_residential / 2.0,
        1500.0, // Distributed home storage
        300.0,
        300.0,
        None,
        0.0,
        None,
    );

    // Bus 3: Touristic zone
    let touristic = Bus::new(
        3,
        BusKind::Touristic.label(),
        BusKind::Touristic,
        0.4,
        [
            1200.0, 1100.0, 1000.0, 1000.0, 1100.0, 1500.0, 2000.0, 3000.0, 4000.0, 4500.0, 4800.0,
            5000.0, 5000.0, 4800.0, 4500.0, 4000.0, 3800.0, 3500.0, 3000.0, 2500.0, 2000.0, 1800.0,
            1500.0, 1300.0,
        ],
        kwp_touristic,
        2000.0, // Commercial storage for hotels/businesses
        500.0,
        500.0,
        Some((0, 6)),
        90.0,
        None,
    );

    // Bus 4: Industrial Polygon
    let industrial = Bus::new(
        4,
        BusKind::Industrial.label(),
        BusKind::Industrial,
        0.4,
        [
            1200.0, 1100.0, 1000.0, 1000.0, 1100.0, 1500.0, 2500.0, 3500.0, 4000.0, 4000.0, 4000.0,
            4000.0, 3500.0, 4000.0, 4000.0, 4000.0, 3500.0, 3000.0, 2500.0, 2000.0, 1500.0, 1200.0,
            1100.0, 1200.0,
        ],
        kwp_industrial,
        3000.0, // Large-scale BESS for industrial zone
        1000.0,
        1000.0,
        None,
        0.0,
        None,
    );

    // Bus 5: School
    let schools = Bus::new(
        5,
        BusKind::School.label(),
        BusKind::School,
        0.4,
        [
            50.0, 50.0, 50.0, 50.0, 100.0, 200.0, 500.0, 1500.0, 2000.0, 2000.0, 2000.0, 1800.0,
            1500.0, 1500.0, 1500.0, 1200.0, 800.0, 500.0, 200.0, 100.0, 100.0, 100.0, 100.0, 100.0,
        ],
        kwp_school,
        500.0, // Facility storage
        100.0,
        100.0,
        None,
        0.0,
        None,
    );

    let nodes = vec![
        substation,
        historic,
        residential,
        touristic,
        industrial,
        schools,
    ];

    // --- Medium tension lines (25kV) from substation to each ST ---
    let lines = vec![
        Line {
            id: 1,
            from: 0,
            to: 1,
            distance_km: 1.4,
            cross_section_mm2: 150.0,
        },
        Line {
            id: 2,
            from: 0,
            to: 2,
            distance_km: 1.1,
            cross_section_mm2: 150.0,
        },
        Line {
            id: 3,
            from: 0,
            to: 3,
            distance_km: 1.6,
            cross_section_mm2: 240.0,
        },
        Line {
            id: 4,
            from: 0,
            to: 4,
            distance_km: 2.4,
            cross_section_mm2: 240.0,
        },
        Line {
            id: 5,
            from: 0,
            to: 5,
            distance_km: 1.0,
            cross_section_mm2: 95.0,
        },
    ];

    // --- Transformation centers 24 kV / 0.4kV on each bus ---
    let transformers = vec![
        Transformer {
            id: 1,
            from: 0,
            to: 1,
            rated_power_kva: 400.0,
            efficiency: 0.98,
        },
        Transformer {
            id: 2,
            from: 0,
            to: 2,
            rated_power_kva: 630.0,
            efficiency: 0.98,
        },
        Transformer {
            id: 3,
            from: 0,
            to: 3,
            rated_power_kva: 1600.0,
            efficiency: 0.985,
        },
        Transformer {
            id: 4,
            from: 0,
            to: 4,
            rated_power_kva: 1250.0,
            efficiency: 0.99,
        },
        Transformer {
            id: 5,
            from: 0,
            to: 5,
            rated_power_kva: 250.0,
            efficiency: 0.97,
        },
    ];

    Grid::new(nodes, lines, transformers, 25.0)
}
