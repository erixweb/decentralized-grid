type DataT = {
    x: number,
    y: number
}

export function chart(input: DataT[], canvas: HTMLCanvasElement) {
    const ctx = canvas.getContext("2d")
    if (!ctx) {
        panic("Could not get context")
    }
    
    if (input.length === 0) return;

    const padding = 50;
    const chartWidth = canvas.width - padding * 2;
    const chartHeight = canvas.height - padding * 2;

    const sortedInput = [...input].sort((a, b) => a.x - b.x);

    // X-Axis scaling 
    const maxX = Math.max(...sortedInput.map(d => d.x));
    const minX = Math.min(...sortedInput.map(d => d.x));
    const xRange = (maxX - minX) || 1; 

    // Y-Axis "Exaggeration" Scaling
    const maxY = Math.max(...sortedInput.map(d => d.y));
    const minY = Math.min(...sortedInput.map(d => d.y));
    
    // 1. ZOOM FACTOR: Change this number to exaggerate the line more!
    // Lower number (e.g., 0.1) = HUGE exaggeration. 
    // Higher number (e.g., 1.0) = Normal mathematical scale.
    const yAxisPadding = 5; 

    // Calculate the visual baseline. 
    const range = (maxY - minY) || 1; 
    const baselineY = minY - (range * yAxisPadding);
    const yRange = (maxY - baselineY) || 1;

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

    // Draw the Line
    ctx.beginPath();
    ctx.strokeStyle = "#4CAF50";
    ctx.lineWidth = 3;
    ctx.lineJoin = "round";

    sortedInput.forEach((point, index) => {
        const canvasX = mapX(point.x);
        const canvasY = mapY(point.y);

        if (index === 0) {
            ctx.moveTo(canvasX, canvasY);
        } else {
            ctx.lineTo(canvasX, canvasY);
        }
    });
    ctx.stroke();

    // Draw Points and Labels
    sortedInput.forEach((point) => {
        const canvasX = mapX(point.x);
        const canvasY = mapY(point.y);

        // Draw dot
        ctx.beginPath();
        ctx.arc(canvasX, canvasY, 5, 0, Math.PI * 2);
        ctx.fillStyle = "#000";
        ctx.fill();

        // Draw Y value
        ctx.fillStyle = "#333";
        ctx.font = "bold 12px sans-serif";
        ctx.textAlign = "center";
        ctx.fillText(point.y.toFixed(4), canvasX, canvasY - 15);

        // Draw X value
        ctx.fillText(point.x.toString(), canvasX, canvas.height - padding + 20);
    });
    

    ctx.textAlign = "right";
    ctx.fillText(baselineY.toFixed(4), padding - 10, canvas.height - padding);
    ctx.fillText(maxY.toFixed(4), padding - 10, padding + 5);
}

function panic(err: string) {
    throw new Error(err);
}
