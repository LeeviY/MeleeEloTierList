const CHARACTERS = [
    "CAPTAIN_FALCON",
    "DONKEY_KONG",
    "FOX",
    "GAME_AND_WATCH",
    "KIRBY",
    "BOWSER",
    "LINK",
    "LUIGI",
    "MARIO",
    "MARTH",
    "MEWTWO",
    "NESS",
    "PEACH",
    "PIKACHU",
    "ICE_CLIMBERS",
    "JIGGLYPUFF",
    "SAMUS",
    "YOSHI",
    "ZELDA",
    "SHEIK",
    "FALCO",
    "YOUNG_LINK",
    "DR_MARIO",
    "ROY",
    "PICHU",
    "GANONDORF",
];

const colors = [
    "#a20016",
    "#340e00",
    "#7db5ff",
    "#d8f0b8",
    "#b1ceff",
    "#634b09",
    "#ffc374",
    "#d8d7ff",
    "#480007",
    "#17355b",
    "#cab4e6",
    "#db8234",
    "#906a00",
    "#e0ac00",
    "#dfddf2",
    "#e6acba",
    "#ab0600",
    "#f5faff",
    "#bc9a7c",
    "#5d4d3e",
    "#366a46",
    "#b46600",
    "#330040",
    "#571700",
    "#3d2300",
    "#584628",
    "#ffffff",
];

document.addEventListener("DOMContentLoaded", async () => {
    const response = await fetch("/ratings", { method: "GET" });
    const data = await response.json();
    console.log(data);
    console.log(data.ratings.at(-1));

    const latest_ratings = data.ratings.at(-1);

    const ratings = latest_ratings.P1.map((obj) => obj.rating).concat(
        latest_ratings.P2.map((obj) => obj.rating)
    );
    const maxRating = Math.max(...ratings);
    const minRating = Math.min(...ratings);

    console.log(minRating, maxRating);

    const p1Characters = latest_ratings.P1.map((obj, i) => {
        return { src: `/static/images/${CHARACTERS[i]}.png`, value: obj.rating };
    });
    p1Characters.sort((a, b) => a.value - b.value);

    const p2Characters = latest_ratings.P2.map((obj, i) => {
        return { src: `/static/images/${CHARACTERS[i]}.png`, value: obj.rating };
    });
    p2Characters.sort((a, b) => a.value - b.value);

    addImagesToLine("line1", p1Characters, minRating, maxRating);
    addImagesToLine("line2", p2Characters, minRating, maxRating);

    renderRatingProgression(data.ratings, "P1");
    renderRatingProgression(data.ratings, "P2");
});

function addImagesToLine(lineId, images, min, max) {
    const line = document.getElementById(lineId);
    images.forEach((imgData, i) => {
        const img = document.createElement("img");
        img.src = imgData.src;

        const percentage = ((imgData.value - min) / (max - min)) * 100;
        img.style.left = `${percentage}%`;
        img.style.top = i % 2 === 0 ? "-20px" : "-10px";

        line.appendChild(img);
    });
}

function renderRatingProgression(ratings, playerId) {
    const svg = d3.select(`#${playerId.toLowerCase()}-rating-progression-chart`);
    svg.selectAll("*").remove();

    const margin = { top: 20, right: 170, bottom: 30, left: 50 };
    const width = 1000 - margin.left - margin.right;
    const height = 500 - margin.top - margin.bottom;

    const g = svg.append("g").attr("transform", `translate(${margin.left},${margin.top})`);

    const ratingPeriods = ratings.length;
    const playerLines = Array.from({ length: CHARACTERS.length }, (_, i) => ({
        id: i,
        values: ratings.map((entry, roundIndex) => ({
            ratingPeriod: roundIndex,
            rating: entry[playerId][i].rating,
        })),
    }));

    // Add average line
    const averageLine = {
        id: CHARACTERS.length,
        values: ratings.map((entry, roundIndex) => {
            const avgRating =
                entry[playerId].reduce((sum, obj) => sum + obj.rating, 0) / entry[playerId].length;
            return {
                ratingPeriod: roundIndex,
                rating: avgRating,
            };
        }),
    };
    playerLines.push(averageLine);

    // Calculate trend line for average
    const n = averageLine.values.length;
    const sumX = averageLine.values.reduce((sum, d) => sum + d.ratingPeriod, 0);
    const sumY = averageLine.values.reduce((sum, d) => sum + d.rating, 0);
    const sumXY = averageLine.values.reduce((sum, d) => sum + d.ratingPeriod * d.rating, 0);
    const sumX2 = averageLine.values.reduce((sum, d) => sum + d.ratingPeriod ** 2, 0);

    const slope = (n * sumXY - sumX * sumY) / (n * sumX2 - sumX ** 2);
    const intercept = (sumY - slope * sumX) / n;

    const x = d3
        .scaleLinear()
        .domain([0, ratingPeriods - 1])
        .range([0, width]);

    const y = d3
        .scaleLinear()
        .domain([
            d3.min(playerLines, (p) => d3.min(p.values, (d) => d.rating)) - 50,
            d3.max(playerLines, (p) => d3.max(p.values, (d) => d.rating)) + 50,
        ])
        .range([height, 0]);

    g.append("g")
        .attr("transform", `translate(0,${height})`)
        .call(d3.axisBottom(x).ticks(Math.min(ratingPeriods, 20)));

    g.append("g").call(d3.axisLeft(y));

    const line = d3
        .line()
        .x((d) => x(d.ratingPeriod))
        .y((d) => y(d.rating));

    const maskKey = `${playerId}CharacterMask`;
    let characterMask = JSON.parse(localStorage.getItem(maskKey)) || Array(27).fill(true);

    g.selectAll(".player-line")
        .data(playerLines)
        .enter()
        .append("path")
        .attr("class", "player-line")
        .attr("id", (d) => `player-line-${d.id}`)
        .attr("fill", "none")
        .attr("stroke", (d, i) => colors[i % colors.length])
        .attr("stroke-width", 1.5)
        .attr("d", (d) => line(d.values))
        .style("display", (d) => (characterMask[d.id] === false ? "none" : null));

    g.append("line")
        .attr("class", "trend-line")
        .attr("x1", x(0))
        .attr("y1", y(intercept))
        .attr("x2", x(ratingPeriods - 1))
        .attr("y2", y(slope * (ratingPeriods - 1) + intercept))
        .attr("stroke", colors[CHARACTERS.length])
        .attr("stroke-width", 2)
        .attr("stroke-dasharray", "5,5")
        .style("opacity", 0.8)
        .style("display", characterMask[CHARACTERS.length] === false ? "none" : null);

    const legend = svg
        .append("g")
        .attr("class", "legend")
        .attr("transform", `translate(${width + margin.left + 20}, ${margin.top})`);

    const legendItemHeight = 14;

    const legendItems = legend
        .selectAll(".legend-item")
        .data(playerLines)
        .enter()
        .append("g")
        .attr("class", "legend-item")
        .attr("transform", (d, i) => `translate(0, ${i * legendItemHeight})`)
        .style("cursor", "pointer")
        .on("click", function (event, d) {
            const chartGroup = d3.select(this.parentNode.parentNode);
            const line = chartGroup.select(`#player-line-${d.id}`);

            const currentlyVisible = line.style("display") !== "none";
            line.style("display", currentlyVisible ? "none" : null);

            characterMask[d.id] = !currentlyVisible;
            localStorage.setItem(maskKey, JSON.stringify(characterMask));

            d3.select(this)
                .select("rect")
                .style("opacity", currentlyVisible ? 0.3 : 1);
            d3.select(this)
                .select("text")
                .style("opacity", currentlyVisible ? 0.3 : 1);
        });

    legendItems
        .append("rect")
        .attr("width", 4)
        .attr("height", 2)
        .attr("fill", (d, i) => colors[i % colors.length])
        .style("opacity", (d) => (characterMask[d.id] === false ? 0.3 : 1));

    legendItems
        .append("text")
        .attr("x", 8)
        .attr("y", 2)
        .attr("dy", "0.3em")
        .style("font-size", "8px")
        .style("fill", "white")
        .style("opacity", (d) => (characterMask[d.id] === false ? 0.3 : 1))
        .text((d) => CHARACTERS[d.id] || "AVERAGE");
}
