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
    handleSliderChange(_matchThreshold);
    document.getElementById("match-threshold-slider").setAttribute("value", _matchThreshold);

    if (_showTrendline) {
        toggleTrendline();
    }

    if (_filterEnabled) {
        toggleFilter();
    }
});

window.addEventListener("resize", updateFontSize);

let _characterMask = new Array(26).fill(true);
let _matchThreshold;
let _showTrendline;
let _filterEnabled;

loadLocalStorage();

function loadLocalStorage() {
    // _characterMask = JSON.parse(localStorage.getItem("characterMask")) || new Array(26).fill(true);
    // localStorage.setItem("characterMask", JSON.stringify(_characterMask));

    _matchThreshold = JSON.parse(localStorage.getItem("matchThreshold")) || 5;
    localStorage.setItem("matchThreshold", JSON.stringify(_matchThreshold));

    _filterEnabled = JSON.parse(localStorage.getItem("filterEnabled")) || false;
    localStorage.setItem("filterEnabled", JSON.stringify(_filterEnabled));

    _showTrendline = JSON.parse(localStorage.getItem("showTrendline")) || false;
    localStorage.setItem("showTrendline", JSON.stringify(_showTrendline));
}

let _matchupData;
let _characterCount = 0;

const socket = io.connect("http://127.0.0.1:5000");
socket.on("matchup_update", (data) => {
    console.log(data);
    _matchupData = data;
    populateDropdown();
    reRender(data);
});

let _k = 0;
let _d = 0;

let _visualizeLine = false;

function reRender(data, renderMatchupPairs = true) {
    console.log("reRender");
    if (!data) return;
    const { matchups, winner } = data;

    const namedMatchups = mapCharacterNames(matchups);

    const maskIndices = _characterMask.flatMap((x, i) => (x ? [] : i));
    const filteredMatchups = remove(namedMatchups, maskIndices, maskIndices);
    _characterCount = filteredMatchups.length;

    const rowOrder = calculateSortedIndices(filteredMatchups);
    const colOrder = calculateSortedIndices(
        flipMatchupChart(JSON.parse(JSON.stringify(filteredMatchups)))
    );

    const sortedMatchups = reorder(filteredMatchups, rowOrder, colOrder);

    if (_showTrendline) {
        const [min, k, d] = searchMinDifference(sortedMatchups);
        console.log("min:", min, "k:", k, "d:", d);
        _k = k;
        _d = d;
    }

    renderMatchupChart(sortedMatchups);

    if (renderMatchupPairs) {
        renderMatchups(filteredMatchups, winner);
    }

    if (_showTrendline) {
        // const [min, k, d] = searchMinDifference(sortedMatchups);
        // console.log("min:", min, "k:", k, "d:", d);
        // _k = k;
        // _d = d;
        drawTrendline(_k, _d);
    }
}

function calculateSplitDifference(weights, k, d) {
    let over = 0;
    let under = 0;
    for (let y = 0; y < weights.length; y++) {
        for (let x = 0; x < weights.length; x++) {
            const px = x / (_characterCount - 1);
            const py = y / (_characterCount - 1);
            const ly = px * k + d;
            if (Math.abs(py - ly) < 1e-9) {
                continue;
            } else if (py > ly) {
                over += weights[weights.length - 1 - y][x];
            } else {
                under += weights[weights.length - 1 - y][x];
            }
        }
    }

    return Math.abs(over - under);
}

function searchMinDifference(matchups) {
    let minDifference = Infinity,
        minK = 0,
        minD = 0;

    const weights = matchups.map((row) =>
        row.map((x) =>
            isNaN(x.data.win_rate) || x.data.matches < _matchThreshold
                ? 0
                : Math.abs(x.data.win_rate - 0.5) * Math.sqrt(x.data.matches)
        )
    );

    for (let k = -30; k <= 0; k += 0.1) {
        for (let d = 0; d <= -k + 1; d += 0.2) {
            const difference = calculateSplitDifference(weights, k, d);
            if (difference < minDifference) {
                minDifference = difference;
                minK = k;
                minD = d;
            }
        }
    }

    return [minDifference, minK, minD];
}

function calculateSortedIndices(matchups) {
    return matchups
        .map((row, i) => {
            const { wins, matches } = row.reduce(
                (acc, x) => {
                    if (!isNaN(x.data.win_rate)) {
                        acc.wins += x.data.win_rate * x.data.matches;
                        acc.matches += x.data.matches;
                    }
                    return acc;
                },
                { wins: 0, matches: 0 }
            );
            return [i, wins / Math.sqrt(matches)];
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

function checkLine(k, d, x, y) {
    const px = x / (_characterCount - 1);
    const py = y / (_characterCount - 1);
    const ly = px * k + d;
    console.log(py - ly);
    if (Math.abs(py - ly) < 1e-9) {
        return 0;
    } else if (py > ly) {
        return 1;
    } else {
        return -1;
    }
}

function renderMatchupChart(matchups) {
    const chart = document.getElementById("matchup-chart");
    chart.innerHTML = "";

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

    matchups.forEach((row, y) => {
        const rowDiv = document.createElement("div");
        rowDiv.classList.add("row");

        const firstCell = document.createElement("div");
        firstCell.classList.add("cell");

        const img = document.createElement("img");
        img.src = `/static/images/${row[0].with}.png`;
        firstCell.appendChild(img);
        rowDiv.appendChild(firstCell);

        row.forEach((col, x) => {
            const cell = document.createElement("div");
            cell.classList.add("cell");

            const { win_rate, matches } = col.data;

            const winRateText = document.createElement("div");
            winRateText.innerText = Math.round(win_rate * 100) / 100;
            winRateText.classList.add("win-number");
            cell.appendChild(winRateText);

            const matchesText = document.createElement("div");
            matchesText.innerText = `(${matches})`;
            matchesText.classList.add("match-number");
            cell.appendChild(matchesText);

            if (_visualizeLine) {
                cell.style.backgroundColor =
                    isNaN(win_rate) || matches < _matchThreshold
                        ? "#000000"
                        : hsv2rgb(
                              Math.floor(
                                  ((checkLine(_k, _d, x, matchups.length - 1 - y) + 1) / 2) * 120
                              ),
                              1,
                              1
                          );
            } else {
                cell.style.backgroundColor = isNaN(win_rate)
                    ? "#000000"
                    : hsv2rgb(
                          Math.floor(win_rate * 120),
                          matches < _matchThreshold ? 0.4 : 0.6,
                          matches < _matchThreshold ? 0.3 : 1
                      );
            }

            rowDiv.appendChild(cell);
        });

        chart.appendChild(rowDiv);
    });

    updateFontSize();
    updateCanvasSize(chart);
}

function updateCanvasSize(chart) {
    const canvas = document.getElementById("grid-canvas");
    const cellSize = chart.clientWidth / (_characterCount + 1);
    canvas.width = chart.clientWidth - cellSize;
    canvas.height = chart.clientWidth - cellSize;
    canvas.style.top = cellSize + "px";
    canvas.style.left = cellSize + "px";
}

function drawTrendline(k, d) {
    const canvas = document.getElementById("grid-canvas");

    const size = canvas.width;

    const pCoord = [Math.max((1 - d) / k, 0) * size, size - Math.min(d, 1) * size];
    const qCoord = [Math.max(-d / k, 0) * size, size];

    const ctx = canvas.getContext("2d");

    ctx.beginPath();
    ctx.moveTo(...pCoord);
    ctx.lineTo(...qCoord);
    ctx.strokeStyle = "#4ea9ffaa";
    ctx.lineWidth = 2;
    ctx.stroke();
}

function updateFontSize() {
    const chart = document.getElementById("matchup-chart");
    if (!chart) return;

    const winRateTextSize = chart.clientWidth / 3 / _characterCount;
    const winRateTexts = chart.querySelectorAll(".win-number");
    winRateTexts.forEach((winRateText) => (winRateText.style.fontSize = winRateTextSize + "px"));

    const matchesTextSize = chart.clientWidth / 4 / _characterCount;
    const matchesTexts = chart.querySelectorAll(".match-number");
    matchesTexts.forEach((matchesText) => (matchesText.style.fontSize = matchesTextSize + "px"));
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
    element.classList.toggle("toggled");
    _characterMask = JSON.parse(localStorage.getItem("characterMask"));
    _characterMask[index] = element.classList.contains("toggled");
    localStorage.setItem("characterMask", JSON.stringify(_characterMask));
    if (_matchupData) reRender(_matchupData);
}

function toggleFilter() {
    const button = document.getElementById("toggle-filter-button");
    button.classList.toggle("toggled");

    _filterEnabled = button.classList.contains("toggled");
    localStorage.setItem("filterEnabled", JSON.stringify(_filterEnabled));

    if (_filterEnabled) {
        _characterMask = JSON.parse(localStorage.getItem("characterMask"));
        button.innerText = "Disable Filter";
    } else {
        _characterMask = new Array(26).fill(true);
        button.innerText = "Enable Filter";
    }
    reRender(_matchupData);
}

function toggleTrendline() {
    const button = document.getElementById("toggle-line-button");
    button.classList.toggle("toggled");

    _showTrendline = button.classList.contains("toggled");
    localStorage.setItem("showTrendline", JSON.stringify(_showTrendline));

    button.innerText = _showTrendline ? "Hide Trendline" : "Show Trendline";

    reRender(_matchupData, false);
}

function handleSliderChange(value) {
    _matchThreshold = value;
    localStorage.setItem("matchThreshold", JSON.stringify(_matchThreshold));
    document.getElementById("match-threshold-slider-value").textContent = value;
    reRender(_matchupData, false);
}
