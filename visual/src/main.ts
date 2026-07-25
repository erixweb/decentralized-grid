import { chart } from "./chart.ts"

type DataT = {
    x: number,
    y: number
}

interface HourReport {
    hour: number;
    total_demand_kw: number;
    total_solar_kw: number;
    total_battery_kw: number;
    grid_import_kw: number;
    losses_kw: number;
    frequency_hz: number;
}

const METRICS: Record<string, { label: string, key: keyof HourReport }> = {
    demand: { label: "Total Demand (kW)", key: "total_demand_kw" },
    solar: { label: "Total Solar (kW)", key: "total_solar_kw" },
    battery: { label: "Battery (kW)", key: "total_battery_kw" },
    import: { label: "Grid Import (kW)", key: "grid_import_kw" },
    losses: { label: "Line Losses (kW)", key: "losses_kw" },
    frequency: { label: "Frequency (Hz)", key: "frequency_hz" },
};

document.querySelector<HTMLDivElement>('#app')!.innerHTML = `
<h1 style="font-family: sans-serif; margin-bottom: 20px;">Simulation Results</h1>
<div id="legend" style="font-family: sans-serif; margin-bottom: 20px; display: flex; gap: 15px; flex-wrap: wrap;"></div>
<canvas width="800" height="800" id="chart"></canvas>
`

const chart_canvas = document.querySelector("#chart") as HTMLCanvasElement
const legend_div = document.querySelector("#legend") as HTMLDivElement

async function init() {
    try {
        const response = await fetch("./simulation_data.json");
        const reports: HourReport[] = await response.json();

        const seriesData: Record<string, DataT[]> = {};
        const visibility: Record<string, boolean> = {};

        Object.entries(METRICS).forEach(([id, metric]) => {
            seriesData[id] = reports.map(r => ({ x: r.hour, y: r[metric.key] }));
            visibility[id] = id === 'frequency'; // Default only frequency visible
        });

        // Create legend
        Object.entries(METRICS).forEach(([id, metric]) => {
            const label = document.createElement('label');
            label.style.cursor = 'pointer';
            label.innerHTML = `
                <input type="checkbox" ${visibility[id] ? 'checked' : ''} data-id="${id}">
                <span style="margin-left: 5px;">${metric.label}</span>
            `;
            legend_div.appendChild(label);

            label.querySelector('input')?.addEventListener('change', (e) => {
                const checkbox = e.target as HTMLInputElement;
                visibility[id] = checkbox.checked;
                chart(seriesData, visibility, chart_canvas);
            });
        });

        // Initial render
        chart(seriesData, visibility, chart_canvas);

    } catch (e) {
        console.error("Failed to load simulation data:", e);
    }
}

init();
