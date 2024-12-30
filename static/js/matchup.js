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

document.addEventListener("DOMContentLoaded", () => {
    document.getElementById("match-threshold-slider-value").textContent = matchThreshold;
    document.getElementById("match-threshold-slider").setAttribute("value", matchThreshold);
});

let characterMask;
// let renderTypeIndex = 0;
let matchThreshold = 5;

loadLocalStorage();

function loadLocalStorage() {
    characterMask = JSON.parse(localStorage.getItem("characterMask")) || new Array(26).fill(true);
    localStorage.setItem("characterMask", JSON.stringify(characterMask));

    matchThreshold = JSON.parse(localStorage.getItem("matchThreshold")) || 5;
    localStorage.setItem("matchThreshold", JSON.stringify(matchThreshold));
}

let matchupData;
let characterCount = 0;

const socket = io.connect("http://127.0.0.1:5000");
socket.on("matchup_update", (data) => {
    console.log(data);
    matchupData = data;
    populateDropdown();
    reRender(data);
});

function reRender(data, reRenderMatchupPairs = true) {
    const { matchups, winner } = data;

    const namedMatchups = mapCharacterNames(matchups);

    const maskIndices = characterMask.flatMap((x, i) => (x ? [] : i));
    const filteredMatchups = remove(namedMatchups, maskIndices, maskIndices);
    characterCount = filteredMatchups.length;

    console.log(filteredMatchups);

    const rowOrder = calculateSortedIndices(filteredMatchups);
    const colOrder = calculateSortedIndices(
        flipMatchupChart(JSON.parse(JSON.stringify(filteredMatchups)))
    );

    const sortedMatchups = reorder(filteredMatchups, rowOrder, colOrder);

    renderMatchupChart(sortedMatchups);
    if (reRenderMatchupPairs) renderMatchups(filteredMatchups, winner);
}

function calculateSortedIndices(matchups) {
    return matchups
        .map((row, i) => {
            const { wins, matches } = row.reduce(
                (acc, x) =>
                    !isNaN(x.data.win_rate)
                        ? {
                              wins: acc.wins + x.data.win_rate * x.data.matches,
                              matches: acc.matches + x.data.matches,
                          }
                        : acc,
                { wins: 0, matches: 0 }
            );
            return [i, wins / matches];
        })
        .sort((a, b) => b[1] - a[1])
        .map((x) => x[0]);
}

function mapCharacterNames(matchups) {
    return matchups.map((row, i) => {
        return row.map((col, j) => {
            return { with: CHARACTERS[i], against: CHARACTERS[j], data: col };
        });
    });
}

function reorder(input, rowIndices, columnIndices) {
    return rowIndices
        .map((i) => input[i])
        .map((row) => {
            return columnIndices.map((i) => row[i]);
        });
}

function remove(input, rowIndices, colIndices) {
    input = input.filter((_, rowIndex) => !rowIndices.includes(rowIndex));
    return input.map((row) => {
        return row.filter((_, colIndex) => !colIndices.includes(colIndex));
    });
}

function flipMatchupChart(matchupChart) {
    return flipDiagonally(matchupChart).map((row) => {
        return row.map((x) => {
            return {
                with: x.against,
                against: x.with,
                data: { win_rate: 1 - x.data.win_rate, matches: x.data.matches },
            };
        });
    });
}

function flipDiagonally(matrix) {
    for (let i = 0; i < matrix.length; i++) {
        for (let j = i + 1; j < matrix.length; j++) {
            [matrix[i][j], matrix[j][i]] = [matrix[j][i], matrix[i][j]];
        }
    }
    return matrix;
}

function renderMatchups(matchups, winner) {
    const p1Won = winner === "P1";
    // p1Won = false;
    const container = document.getElementById("random-matchup-player-id-container");
    const textElements = container.querySelectorAll(".matchup-player-id");
    textElements[0].textContent = p1Won ? "P2" : "P1";
    textElements[1].textContent = !p1Won ? "P2" : "P1";

    renderClosestMatchups(matchups);
    renderRandomMatchups(!p1Won ? matchups : flipMatchupChart(matchups));
}

// function incrementMatchupPairRenderFunction() {
//     renderTypeIndex++;
//     renderTypeIndex %= 2;
//     if (!matchupData) return;
//     reRender(matchupData);
// }

function renderClosestMatchups(matchups) {
    const matchupPairs = matchups.flatMap((row) =>
        row.map((col) => [col.with, col.against, col.data.win_rate - 0.5, col.data.matches])
    );

    matchupPairs.sort((a, b) => {
        const p2 = Math.abs(a[2]);
        const p1 = Math.abs(b[2]);
        return isNaN(p2) - isNaN(p1) || p2 - p1 || b[3] - a[3];
    });

    renderMatchupPairs(
        matchupPairs.filter((a) => a[3] > 0),
        "closest-matchup-container"
    );
}

function renderRandomMatchups(matchups) {
    const calcWeight = (winRate, matches) => {
        if (matches < 5) return 1;
        return 0.5 - Math.abs(winRate - 0.5) + 0.5 / matches;
    };

    const matchupPairs = matchups.map((row) => {
        const weights = row.map((col) => calcWeight(col.data.win_rate, col.data.matches));
        const weightsSum = weights.reduce((sum, w) => sum + w, 0);
        // console.log(`${row[0].with}:`);
        // weights.forEach((x, i) => {
        //     console.log(
        //         "\t",
        //         row[i].against.slice(0, 3),
        //         Math.round((x / weightsSum) * 1000) / 1000
        //     );
        // });
        const random = Math.random();
        const opponent = weights
            .reduce((acc, weight) => {
                acc.push((acc.at(-1) || 0) + weight / weightsSum);
                return acc;
            }, [])
            .findIndex((cw) => random < cw);
        return [
            row[opponent].with,
            row[opponent].against,
            row[opponent].data["win_rate"] - 0.5,
            row[opponent].data["matches"],
        ];
    });

    renderMatchupPairs(matchupPairs, "random-matchup-container");
}

function renderMatchupPairs(pairs, id) {
    const container = document.getElementById(id).querySelector(".matchup-items");
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

function renderMatchupChart(matchups) {
    const chart = document.getElementById("matchup-chart");
    chart.innerHTML = "";

    console.log(chart.clientWidth);

    // Create header row
    const headerRow = document.createElement("div");
    headerRow.classList.add("row");

    const emptyCell = document.createElement("div");
    emptyCell.classList.add("cell");
    headerRow.appendChild(emptyCell);

    matchups[0].forEach((col) => {
        const cell = document.createElement("div");
        cell.classList.add("cell");

        const img = document.createElement("img");
        img.src = `/static/images/${col.against}.png`;
        cell.appendChild(img);

        headerRow.appendChild(cell);
    });

    chart.appendChild(headerRow);

    // Create body rows
    matchups.forEach((row) => {
        const rowDiv = document.createElement("div");
        rowDiv.classList.add("row");

        const firstCell = document.createElement("div");
        firstCell.classList.add("cell");

        const img = document.createElement("img");
        img.src = `/static/images/${row[0].with}.png`;
        firstCell.appendChild(img);
        rowDiv.appendChild(firstCell);

        row.forEach((x) => {
            const cell = document.createElement("div");
            cell.classList.add("cell");

            const { win_rate, matches } = x.data;

            const winRateText = document.createElement("div");
            winRateText.innerText = Math.round(win_rate * 100) / 100;
            winRateText.classList.add("win-number");
            cell.appendChild(winRateText);

            const matchesText = document.createElement("div");
            matchesText.innerText = `(${matches})`;
            matchesText.classList.add("match-number");
            cell.appendChild(matchesText);

            cell.style.backgroundColor = isNaN(win_rate)
                ? "#000000"
                : hsv2rgb(
                      Math.floor(win_rate * 120),
                      matches < matchThreshold ? 0.4 : 0.6,
                      matches < matchThreshold ? 0.3 : 1
                  );

            rowDiv.appendChild(cell);
        });

        chart.appendChild(rowDiv);
    });

    updateFontSize();
}

function updateFontSize() {
    const chart = document.getElementById("matchup-chart");
    if (!chart) return;

    const winRateTextSize = chart.clientWidth / 3 / characterCount;
    const winRateTexts = chart.querySelectorAll(".win-number");
    winRateTexts.forEach((winRateText) => (winRateText.style.fontSize = winRateTextSize + "px"));

    const matchesTextSize = chart.clientWidth / 4 / characterCount;
    const matchesTexts = chart.querySelectorAll(".match-number");
    matchesTexts.forEach((matchesText) => (matchesText.style.fontSize = matchesTextSize + "px"));
}

window.addEventListener("resize", updateFontSize);

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

function populateDropdown() {
    const characterMask = JSON.parse(localStorage.getItem("characterMask"));
    const dropdownMenu = document.getElementById("dropdown-menu");
    dropdownMenu.innerHTML = "";
    CHARACTERS.forEach((item, index) => {
        const li = document.createElement("li");
        li.textContent = item;
        li.onclick = () => toggleItem(index, li);
        li.classList.add("dropdown-item");
        if (characterMask[index]) li.classList.add("toggled");
        dropdownMenu.appendChild(li);
    });
}

function toggleDropdown() {
    const dropdownMenu = document.getElementById("dropdown-menu");
    dropdownMenu.classList.toggle("visible");
}

function toggleItem(index, element) {
    console.log(`Toggled item ${index}: ${CHARACTERS[index]}`);
    element.classList.toggle("toggled");
    characterMask = JSON.parse(localStorage.getItem("characterMask"));
    characterMask[index] = element.classList.contains("toggled");
    localStorage.setItem("characterMask", JSON.stringify(characterMask));
    if (matchupData) reRender(matchupData);
}

function toggleFilter() {
    const button = document.getElementById("toggle-filter-button");
    button.classList.toggle("toggled");
    if (!button.classList.contains("toggled")) {
        characterMask = new Array(26).fill(true);
        button.innerText = "Enable Filter";
    } else {
        characterMask = JSON.parse(localStorage.getItem("characterMask"));
        button.innerText = "Disable Filter";
    }
    reRender(matchupData);
}

function handleSliderChange(value) {
    matchThreshold = value;
    localStorage.setItem("matchThreshold", JSON.stringify(matchThreshold));
    document.getElementById("match-threshold-slider-value").textContent = value;
    reRender(matchupData, false);
}
