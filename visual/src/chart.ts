type DataT = {
    x: number,
    y: number
}

const COLORS: Record<string, string> = {
    demand: "#4CAF50",   // Green
    solar: "#FFEB3B",    // Yellow
    battery: "#2196F3",  // Blue
    import: "#F44336",   // Red
    losses: "#9C27B0",   // Purple
    frequency: "#FF9800", // Orange
};

export function chart(
    series: Record<string, DataT[]>,
    visibility: Record<string, boolean>,
    canvas: HTMLCanvasElement,
    hoverX?: number
) {
    const ctx = canvas.getContext("2d")
    if (!ctx) {
        panic("Could not get context")
    }

    const visibleIds = Object.keys(series).filter(id => visibility[id]);
    if (visibleIds.length === 0) {
        ctx.clearRect(0, 0, canvas.width, canvas.height);
        return;
    }

    const padding = 50;
    const chartWidth = canvas.width - padding * 2;
    const chartHeight = canvas.height - padding * 2;

    // X-Axis scaling (shared across all series as they all use 0-23 hours)
    const allPoints = visibleIds.flatMap(id => series[id]);
    const minX = Math.min(...allPoints.map(d => d.x));
    const maxX = Math.max(...allPoints.map(d => d.x));
    const xRange = (maxX - minX) || 1;

    // Y-Axis Scaling - Dynamic based on visible series
    const minY = Math.min(...allPoints.map(d => d.y));
    const maxY = Math.max(...allPoints.map(d => d.y));

    const range = (maxY - minY) || 1;
    const yAxisPadding = 0.1; // 10% padding top/bottom
    const baselineY = minY - (range * yAxisPadding);
    const topY = maxY + (range * yAxisPadding);
    const yRange = (topY - baselineY) || 1;

    // Clear canvas
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    // Draw Axes
    ctx.beginPath();
    ctx.moveTo(padding, padding);
    ctx.lineTo(padding, canvas.height - padding);
    ctx.lineTo(canvas.width - padding, canvas.height - padding);
    ctx.strokeStyle = "#333";
    ctx.lineWidth = 2;
    ctx.stroke();

    // Helper to map data to canvas pixels
    const mapX = (dataX: number) => padding + ((dataX - minX) / xRange) * chartWidth;
    const mapY = (dataY: number) => (canvas.height - padding) - ((dataY - baselineY) / yRange) * chartHeight;

    // Draw each visible series
    visibleIds.forEach(id => {
        const data = [...series[id]].sort((a, b) => a.x - b.x);
        const color = COLORS[id] || "#000";

        ctx.beginPath();
        ctx.strokeStyle = color;
        ctx.lineWidth = 3;
        ctx.lineJoin = "round";

        data.forEach((point, index) => {
            const canvasX = mapX(point.x);
            const canvasY = mapY(point.y);

            if (index === 0) {
                ctx.moveTo(canvasX, canvasY);
            } else {
                ctx.lineTo(canvasX, canvasY);
            }
        });
        ctx.stroke();

        // Draw Points
        data.forEach((point) => {
            const canvasX = mapX(point.x);
            const canvasY = mapY(point.y);

            ctx.beginPath();
            ctx.arc(canvasX, canvasY, 4, 0, Math.PI * 2);
            ctx.fillStyle = color;
            ctx.fill();
        });
    });

    // X-Axis Labels (Hours)
    ctx.fillStyle = "#333";
    ctx.font = "12px sans-serif";
    ctx.textAlign = "center";
    for (let h = 0; h < 24; h++) {
        const canvasX = mapX(h);
        if (canvasX >= padding && canvasX <= canvas.width - padding) {
            ctx.fillText(h.toString(), canvasX, canvas.height - padding + 20);
        }
    }

    // Y-Axis Labels
    ctx.textAlign = "right";
    ctx.fillStyle = "#333";

    const numTicks = 5;
    for (let i = 0; i < numTicks; i++) {
        const tickY = baselineY + (i * (yRange / (numTicks - 1)));
        const canvasY = mapY(tickY);
        ctx.fillText(tickY.toFixed(2), padding - 10, canvasY);
    }

    // Draw Hover Tooltip
    if (hoverX !== undefined) {
        const canvasX = mapX(hoverX);
        
        // Vertical line at hover position
        ctx.beginPath();
        ctx.setLineDash([5, 5]);
        ctx.strokeStyle = "#999";
        ctx.lineWidth = 1;
        ctx.moveTo(canvasX, padding);
        ctx.lineTo(canvasX, canvas.height - padding);
        ctx.stroke();
        ctx.setLineDash([]);

        // Tooltip box
        let tooltipY = padding;
        const tooltipPadding = 5;
        ctx.font = "12px sans-serif";
        
        visibleIds.forEach(id => {
            const point = series[id].find(p => p.x === hoverX);
            if (point) {
                const text = `${id}: ${point.y.toFixed(2)}`;
                const textWidth = ctx.measureText(text).width;
                
                ctx.fillStyle = "rgba(255, 255, 255, 0.8)";
                ctx.fillRect(canvasX + 5, tooltipY, textWidth + 10, 20);
                
                ctx.fillStyle = COLORS[id] || "#000";
                ctx.textAlign = "left";
                ctx.fillText(text, canvasX + 10, tooltipY + 15);
                
                tooltipY += 25;
            }
        });
    }
}

function panic(err: string) {
    throw new Error(err);
}
