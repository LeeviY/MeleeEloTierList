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

let renderTypeIndex = 1;
const renderFunctions = [renderClosestMatchups, renderRandomMatchups];
let matchupData = null;

const socket = io.connect("http://127.0.0.1:5000");
socket.on("matchup_update", function (data) {
    console.log(data);
    matchupData = data;
    const { matchups, winner } = data;
    renderMatchupChart(matchups, "p1-matchup-chart");
    renderMatchupChart(flipMatchupChart(matchups), "p2-matchup-chart");
    chooseMatchupPairRender(matchups, winner);
});

function flipMatchupChart(matchupChart) {
    return matchupChart
        .map((row, i) => row.map((_, j) => (j > i ? matchupChart[j][i] : matchupChart[i][j]))) // Invert diagonally
        .map((row) => row.map(({ win_rate, matches }) => ({ win_rate: 1 - win_rate, matches }))); // Invert win ratio
}

function chooseMatchupPairRender(matchups, winner) {
    const p1Won = winner === "P1";
    document.querySelector(".winner-text").innerText = `${p1Won ? "P2" : "P1"} Chooses`;
    renderFunctions[renderTypeIndex](p1Won || !renderTypeIndex ? matchups : flipMatchupChart(matchups));
}

function incrementMatchupPairRenderFunction() {
    renderTypeIndex++;
    renderTypeIndex %= 2;
    if (!matchupData) return;
    const { matchups, winner } = matchupData;
    chooseMatchupPairRender(matchups, winner);
}

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

function renderRandomMatchups(matchups) {
    const matchupPairs = matchups.map((row, i) => {
        const weights = row.map(({ win_rate, matches }) => (matches < 10 ? 1 : 1 - Math.abs(win_rate - 0.5) * 2));
        const weightsSum = weights.reduce((sum, w) => sum + w, 0);
        const random = Math.random();
        const chosen = weights
            .reduce((acc, weight) => {
                acc.push((acc.at(-1) || 0) + weight / weightsSum);
                return acc;
            }, [])
            .findIndex((cw) => random < cw);
        return [CHARACTERS[i], CHARACTERS[chosen], 0.5 - row[chosen]["win_rate"], row[chosen]["matches"]];
    });

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
            // const classMap = [
            //     { threshold: 0.2, className: "low" },
            //     { threshold: 0.4, className: "medium-low" },
            //     { threshold: 0.6, className: "medium" },
            //     { threshold: 0.8, className: "medium-high" },
            //     { threshold: 1, className: "high" },
            // ];

            // td.className = classMap.find(({ threshold }) => winRate <= threshold)?.className || "nan";
            const color = isNaN(winRate) ? "#000000" : hsv2rgb(Math.floor(winRate * 120), 0.6, 1);
            td.style.backgroundColor = color;

            const h5 = document.createElement("h5");
            h5.innerText = `(${x.matches})`;
            h5.classList.add("match-number");
            td.appendChild(h5);

            row.appendChild(td);
        });

        table.querySelector(".matchup-chart-body").appendChild(row);
    }
}

const hsv2rgb = (h, s, v) => {
    if (s === 0) {
        return `#${Math.round(v * 255)
            .toString(16)
            .padStart(2, "0")
            .repeat(3)}`;
    }

    h /= 60;
    const i = Math.floor(h);
    const f = h - i;
    const p = v * (1 - s);
    const q = v * (1 - s * f);
    const t = v * (1 - s * (1 - f));

    let rgb;
    switch (i) {
        case 0:
            rgb = [v, t, p];
            break;
        case 1:
            rgb = [q, v, p];
            break;
        case 2:
            rgb = [p, v, t];
            break;
        case 3:
            rgb = [p, q, v];
            break;
        case 4:
            rgb = [t, p, v];
            break;
        default:
            rgb = [v, p, q];
            break;
    }

    return `#${rgb
        .map((x) =>
            Math.round(x * 255)
                .toString(16)
                .padStart(2, "0")
        )
        .join("")}`;
};

function toggleActionMenu() {
    document.querySelector(".action-buttons").classList.toggle("collapsed");
}
