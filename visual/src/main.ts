import { chart } from "./chart.ts"

document.querySelector<HTMLDivElement>('#app')!.innerHTML = `
<h1>Hello world</h1>
<canvas width="800" height="800" id="chart"></canvas>
`

// Chart data
const data = [{x: 0, y: 49.81}, {x: 1, y: 49.9}, {x: 2, y: 49.89}, {x: 3, y: 50}, {x: 4, y: 50}]
const chart_canvas = document.querySelector("#chart") as HTMLCanvasElement

chart(data, chart_canvas)
