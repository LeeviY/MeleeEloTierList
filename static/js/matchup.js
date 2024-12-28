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

let renderTypeIndex = 0;
const renderFunctions = [renderClosestMatchups, renderRandomMatchups];
let matchupData = null;

const socket = io.connect("http://127.0.0.1:5000");
socket.on("matchup_update", function (data) {
    console.log(data);
    matchupData = data;
    const { chart, winner } = data;
    const flippedChart = flipDiagonally(chart).map((row) =>
        row.map(({ win_rate, matches }) => ({ win_rate: 1 - win_rate, matches }))
    );
    renderMatchupChart(chart, "p1-matchup-chart");
    renderMatchupChart(flippedChart, "p2-matchup-chart");
    renderFunctions[renderTypeIndex](winner === "P1" || !renderTypeIndex ? chart : flippedChart, winner);
});

function renderClosestMatchups(matchups) {
    const matchupPairs = matchups.flatMap((row, i) =>
        row.map((data, j) => [CHARACTERS[j], CHARACTERS[i], data["win_rate"] - 0.5, data["matches"]])
    );

    matchupPairs.sort((a, b) => {
        const p2 = Math.abs(a[2]);
        const p1 = Math.abs(b[2]);

        return isNaN(p2) - isNaN(p1) || p2 - p1 || b[3] - a[3];
    });

    renderMatchupPairs(matchupPairs.filter((a) => a[3] > 0));
}

function weightedRandom(items, weights) {
    const cumulativeWeights = weights.reduce((acc, weight) => {
        acc.push((acc.at(-1) || 0) + weight);
        return acc;
    }, []);
    const random = Math.random() * cumulativeWeights.at(-1);
    return cumulativeWeights.findIndex((cw) => random < cw);
}

function renderRandomMatchups(matchups, winner) {
    const matchupPairs = matchups.map((row, i) => {
        const weights = row.map(({ win_rate: winRate, matches }) =>
            matches < 10 ? 1 : 1 - Math.abs(winRate - 0.5) * 2
        );
        const weightSum = weights.reduce((sum, w) => sum + w, 0);
        const chosen = weightedRandom(
            row,
            weights.map((w) => w / weightSum)
        );
        return [CHARACTERS[i], CHARACTERS[chosen], 0.5 - row[chosen]["win_rate"], row[chosen]["matches"]];
    });

    document.querySelector(".winner-text").innerText = `${winner === "P1" ? "P2" : "P1"} Chooses`;
    renderMatchupPairs(matchupPairs);
}

function renderMatchupPairs(pairs) {
    const container = document.querySelector(".closest-matchup-items");
    container.innerHTML = "";
    const imgPair = document.getElementById("matchup-pair");

    pairs.forEach((matchup) => {
        const newPair = imgPair.content.cloneNode(true);
        const text = newPair.querySelector(".matchup-pair-text");
        text.innerText = `${Number((matchup[2] * 100).toPrecision(3))}% (${matchup[3]})`;
        const images = newPair.querySelectorAll(".matchup-pair-image");
        images[0].src = `/static/images/${matchup[0]}.png`;
        images[1].src = `/static/images/${matchup[1]}.png`;
        container.appendChild(newPair);
    });
}

function renderMatchupChart(matchups, id) {
    const table = document.getElementById(id);
    const body = table.querySelector(".matchup-chart-body");
    body.innerHTML = "";

    // Create header.
    const headerRow = document.createElement("tr");
    const td = document.createElement("td");
    headerRow.appendChild(td);

    CHARACTERS.forEach((character) => {
        const td = document.createElement("td");
        const img = document.createElement("img");
        img.src = `/static/images/${character}.png`;
        td.appendChild(img);
        headerRow.appendChild(td);
    });

    body.appendChild(headerRow);

    // Create table rows dynamically
    for (const [index, element] of matchups.entries()) {
        const row = document.createElement("tr");
        const td = document.createElement("td");
        const img = document.createElement("img");
        img.src = `/static/images/${CHARACTERS[index]}.png`;
        td.appendChild(img);
        row.appendChild(td);

        element.forEach((x) => {
            const td = document.createElement("td");
            const winRate = x.win_rate;
            td.innerText = Math.round(winRate * 100) / 100;
            if (winRate < 0.2) {
                td.className = "low";
            } else if (winRate >= 0.2 && winRate < 0.4) {
                td.className = "medium-low";
            } else if (winRate >= 0.4 && winRate < 0.6) {
                td.className = "medium";
            } else if (winRate >= 0.6 && winRate < 0.8) {
                td.className = "medium-high";
            } else if (winRate >= 0.8 && winRate <= 1) {
                td.className = "high";
            } else {
                td.className = "nan";
            }

            const h5 = document.createElement("h5");
            h5.innerText = `(${x.matches})`;
            h5.classList.add("match-number");
            td.appendChild(h5);

            row.appendChild(td);
        });

        table.querySelector(".matchup-chart-body").appendChild(row);
    }
}

function flipDiagonally(matrix) {
    for (let i = 0; i < matrix.length; i++) {
        for (let j = i + 1; j < matrix.length; j++) {
            [matrix[i][j], matrix[j][i]] = [matrix[j][i], matrix[i][j]];
        }
    }
    return matrix;
}

async function fetchMatchups() {
    try {
        const response = await fetch("/matchups", {
            method: "GET",
        });
        const data = await response.json();
        renderMatchupChart(data, "p1-matchup-chart");
        renderMatchupChart(
            flipDiagonally(data).map((x) =>
                x.map((y) => {
                    return {
                        win_rate: 1 - y.win_rate,
                        matches: y.matches,
                    };
                })
            ),
            "p2-matchup-chart"
        );
    } catch (error) {
        console.error("Error fetching matchups:", error);
    }
}

function toggleActionMenu() {
    document.querySelector(".action-buttons").classList.toggle("collapsed");
}

function incrementMatchupPairRenderFunction() {
    renderTypeIndex++;
    renderTypeIndex %= 2;
    if (!matchupData) return;
    const { chart, winner } = matchupData;
    renderFunctions[renderTypeIndex](winner === "P1" || !renderTypeIndex ? chart : flippedChart, winner);
}
