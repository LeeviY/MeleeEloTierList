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

    document.getElementById("toggle-filter-button").checked = _filterEnabled;
    document.getElementById("toggle-trendline-button").checked = _showTrendline;
    document.getElementById("toggle-difference-button").checked = _abosluteDifferenceMode;
    document.getElementById("toggle-sort-button").checked = _sortCharacters;
});

window.addEventListener("resize", updateFontSize);

let _characterMask = new Array(26).fill(true);
let _matchThreshold = 0;
let _showTrendline = false;
let _filterEnabled = false;
let _abosluteDifferenceMode = false;
let _sortCharacters = false;

loadLocalStorage();

function loadLocalStorage() {
    _characterMask = JSON.parse(localStorage.getItem("characterMask")) || new Array(26).fill(true);
    localStorage.setItem("characterMask", JSON.stringify(_characterMask));
    _matchThreshold = JSON.parse(localStorage.getItem("matchThreshold")) || 5;
    localStorage.setItem("matchThreshold", JSON.stringify(_matchThreshold));

    _filterEnabled = JSON.parse(localStorage.getItem("filterEnabled")) || false;
    localStorage.setItem("filterEnabled", JSON.stringify(_filterEnabled));

    _showTrendline = JSON.parse(localStorage.getItem("showTrendline")) || false;
    localStorage.setItem("showTrendline", JSON.stringify(_showTrendline));

    _abosluteDifferenceMode = JSON.parse(localStorage.getItem("abosluteDifferenceMode")) || false;
    localStorage.setItem("abosluteDifferenceMode", JSON.stringify(_abosluteDifferenceMode));

    _sortCharacters = JSON.parse(localStorage.getItem("sortCharacters")) || false;
    localStorage.setItem("sortCharacters", JSON.stringify(_sortCharacters));
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
    const filteredMatchups = _filterEnabled
        ? remove(namedMatchups, maskIndices, maskIndices)
        : namedMatchups;
    _characterCount = filteredMatchups.length;

    let rowOrder;
    let colOrder;
    if (_sortCharacters) {
        rowOrder = calculateSortedIndices(filteredMatchups);
        colOrder = calculateSortedIndices(
            flipMatchupChart(JSON.parse(JSON.stringify(filteredMatchups)))
        );
    } else {
        const tierlistOrder = [
            2, 9, 15, 20, 19, 0, 12, 14, 13, 17, 16, 7, 22, 25, 8, 1, 21, 6, 3, 10, 23, 24, 11, 18,
            4, 5,
        ];
        rowOrder = tierlistOrder;
        colOrder = tierlistOrder;
    }

    const sortedMatchups = reorder(filteredMatchups, rowOrder, colOrder);
    // const sortedMatchups = filteredMatchups;
    renderMatchupChart(sortedMatchups);

    if (renderMatchupPairs) {
        renderMatchups(filteredMatchups, winner);
    }

    if (_showTrendline) {
        const [k, d] = pcaBestFitLine(calculateMidPoints(sortedMatchups));
        console.log("k:", k, "d:", d);
        drawLine(k, d);
    }
}

function calculateMidPoints(matchups) {
    const points = matchups
        .map((row, y) => {
            const weightedSum = row.reduce((acc, col, x) => {
                if (isNaN(col.data.win_rate) || col.data.matches < _matchThreshold) return acc;
                const weight = Math.abs(col.data.win_rate - 0.5) * (x / (_characterCount - 1));
                return acc + weight;
            }, 0);

            const totalWeight = row.reduce((acc, col) => {
                if (isNaN(col.data.win_rate) || col.data.matches < _matchThreshold) return acc;
                return acc + Math.abs(col.data.win_rate - 0.5);
            }, 0);

            const middle = weightedSum / totalWeight;
            return [middle || -1, (matchups.length - 1 - y) / (_characterCount - 1)];
        })
        .filter(([middle]) => middle !== -1);

    return points;
}

function leastSquaresFit(points) {
    const [sumX, sumY, sumXY, sumX2] = points.reduce(
        ([sx, sy, sxy, sx2], [x, y]) => [sx + x, sy + y, sxy + x * y, sx2 + x * x],
        [0, 0, 0, 0]
    );
    const k = (points.length * sumXY - sumX * sumY) / (points.length * sumX2 - sumX ** 2);
    const d = (sumY - k * sumX) / points.length;
    return [k, d];
}

function pcaBestFitLine(points) {
    const [meanX, meanY] = points
        .reduce(([sumX, sumY], [x, y]) => [sumX + x, sumY + y], [0, 0])
        .map((sum) => sum / points.length);

    let [covXX, covXY, covYY] = [0, 0, 0];
    points.forEach(([x, y]) => {
        const dx = x - meanX,
            dy = y - meanY;
        covXX += dx * dx;
        covXY += dx * dy;
        covYY += dy * dy;
    });

    const trace = covXX + covYY,
        det = covXX * covYY - covXY ** 2;
    const eig1 = trace / 2 + Math.sqrt(trace ** 2 / 4 - det);
    const [vx, vy] = [1, (eig1 - covXX) / covXY];

    const k = vy / vx;
    const d = meanY - k * meanX;

    return [k, d];
}

function calculateSplitDifference(weights, k, d) {
    let over = 0;
    let under = 0;
    for (var i = 0; i < weights.length; i++) {
        const weight = weights[i];
        const x = weight[0];
        const y = weight[1];
        const val = weight[2];
        const ly = x * k + d;
        if (Math.abs(y - ly) < 1e-9) {
            continue;
        } else if (y > ly) {
            over += val;
        } else {
            under += val;
        }
    }

    return Math.abs(over - under);
}

function searchMinDifference(matchups) {
    let minDifference = Infinity,
        minK = 0,
        minD = 0;

    const weights = matchups.flatMap((row, y) =>
        row
            .map((col, x) => {
                return [
                    x / (_characterCount - 1),
                    (matchups.length - 1 - y) / (_characterCount - 1),
                    isNaN(col.data.win_rate) || col.data.matches < _matchThreshold
                        ? 0
                        : Math.abs(col.data.win_rate - 0.5) * Math.sqrt(col.data.matches),
                ];
            })
            .filter((x) => x[2] > 0)
    );

    for (let k = -30; k <= 0; k += 0.1) {
        for (let d = 0; d <= -k + 1; d += 0.05) {
            const difference = calculateSplitDifference(weights, k, d);
            if (difference < minDifference) {
                minDifference = difference;
                minK = k;
                minD = d;
            }
        }
    }

    return [minK, minD, minDifference];
}

function calculateSortedIndices(matchups) {
    // return matchups
    //     .map((row, i) => {
    //         const { wins, matches } = row.reduce(
    //             (acc, x) => {
    //                 if (!isNaN(x.data.win_rate)) {
    //                     acc.wins += x.data.win_rate * x.data.matches;
    //                     acc.matches += x.data.matches;
    //                 }
    //                 return acc;
    //             },
    //             { wins: 0, matches: 0 }
    //         );
    //         return [i, wins / Math.sqrt(matches)];
    //     })
    //     .sort((a, b) => b[1] - a[1])
    //     .map((x) => x[0]);
    const matchThreshold = 5;
    return matchups
        .map((row, i) => [i, row])
        .sort((a, b) => {
            let total = 0;
            let totalMatchesA = 0;
            let totalMatchesB = 0;
            for (let i = 0; i < a[1].length; i++) {
                const { win_rate: winA, matches: matchesA } = a[1][i].data;
                const { win_rate: winB, matches: matchesB } = b[1][i].data;

                totalMatchesA += matchesA;
                totalMatchesB += matchesB;

                if (
                    matchesA < matchThreshold ||
                    matchesB < matchThreshold ||
                    isNaN(winA) ||
                    isNaN(winB)
                ) {
                    continue;
                }

                if (winB > winA) {
                    total++;
                } else if (winB < winA) {
                    total--;
                }
            }

            return total != 0 ? total : totalMatchesB - totalMatchesA;
        })
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
        cell.style.background = `url(/static/images/${col.against}.png) center/cover no-repeat`;

        headerRow.appendChild(cell);
    });

    chart.appendChild(headerRow);

    matchups.forEach((row, y) => {
        const rowDiv = document.createElement("div");
        rowDiv.classList.add("row");

        const firstCell = document.createElement("div");
        firstCell.classList.add("cell");
        firstCell.style.background = `url(/static/images/${row[0].with}.png) center/cover no-repeat`;

        rowDiv.appendChild(firstCell);

        row.forEach((col, x) => {
            const cell = document.createElement("div");
            cell.classList.add("cell");

            const { win_rate, matches } = col.data;

            const winRateText = document.createElement("div");
            winRateText.innerText =
                Math.round((_abosluteDifferenceMode ? win_rate - 0.5 : win_rate) * 100) / 100;
            winRateText.classList.add("win-number");
            cell.appendChild(winRateText);

            const matchesText = document.createElement("div");
            matchesText.innerText = matches;
            matchesText.classList.add("match-number");
            cell.appendChild(matchesText);

            // if (_visualizeLine) {
            //     cell.style.backgroundColor =
            //         isNaN(win_rate) || matches < _matchThreshold
            //             ? "#000000"
            //             : hsv2rgb(
            //                   Math.floor(
            //                       ((checkLine(_k, _d, x, matchups.length - 1 - y) + 1) / 2) * 120
            //                   ),
            //                   1,
            //                   1
            //               );
            // } else {
            const hue = Math.floor(
                _abosluteDifferenceMode ? (0.5 - Math.abs(win_rate - 0.5)) * 240 : win_rate * 120
            );
            cell.style.backgroundColor = isNaN(win_rate)
                ? "#000000"
                : hsv2rgb(
                      hue,
                      matches < _matchThreshold ? 0.4 : 0.6,
                      matches < _matchThreshold ? 0.3 : 1
                  );
            // }

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
    canvas.width = chart.clientWidth - cellSize * 2;
    canvas.height = chart.clientWidth - cellSize * 2;
    canvas.style.top = cellSize * 1.5 + "px";
    canvas.style.left = cellSize * 1.5 + "px";
}

function drawLine(k, d) {
    const canvas = document.getElementById("grid-canvas");
    const ctx = canvas.getContext("2d");
    const { width, height } = canvas;

    let y0;
    let y1;
    if (k === 0) {
        const y = height - d * height;
        y0 = y;
        y1 = y;
    } else {
        y0 = height - d * height;
        y1 = height - (k * (width / height) + d) * height;
    }

    ctx.beginPath();
    ctx.moveTo(0, y0);
    ctx.lineTo(width, y1);
    ctx.strokeStyle = "#4ea9ffcc";
    ctx.lineWidth = 5;
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

function populateDropdown() {
    const characterMask = JSON.parse(localStorage.getItem("characterMask"));
    const dropdownMenu = document.getElementById("dropdown-menu");
    dropdownMenu.innerHTML = "";

    CHARACTERS.forEach((item, index) => {
        const div = document.createElement("div");
        div.textContent = item;
        div.onclick = () => toggleItem(index, div);
        div.classList.add("dropdown-item");
        if (characterMask[index]) div.classList.add("selected");
        dropdownMenu.appendChild(div);
    });
}

function toggleDropdown() {
    const dropdownMenu = document.getElementById("dropdown-menu");
    dropdownMenu.classList.toggle("visible");
}

function toggleItem(index, element) {
    element.classList.toggle("selected");
    _characterMask = JSON.parse(localStorage.getItem("characterMask"));
    _characterMask[index] = element.classList.contains("selected");
    localStorage.setItem("characterMask", JSON.stringify(_characterMask));
    if (_matchupData) reRender(_matchupData);
}

function toggleFilter(checked) {
    localStorage.setItem("filterEnabled", JSON.stringify(checked));

    _characterMask = checked
        ? JSON.parse(localStorage.getItem("characterMask"))
        : new Array(26).fill(true);
    reRender(_matchupData);
}

function toggleTrendline(checked) {
    localStorage.setItem("showTrendline", JSON.stringify(checked));
    _showTrendline = checked;
    reRender(_matchupData, false);
}

function handleSliderChange(value) {
    _matchThreshold = value;
    localStorage.setItem("matchThreshold", JSON.stringify(_matchThreshold));
    document.getElementById("match-threshold-slider-value").textContent = value;
    reRender(_matchupData, false);
}

function toggleDifference(checked) {
    _abosluteDifferenceMode = checked;
    localStorage.setItem("abosluteDifferenceMode", JSON.stringify(_abosluteDifferenceMode));
    reRender(_matchupData, false);
}

function toggleSort(checked) {
    _sortCharacters = checked;
    localStorage.setItem("sortCharacters", JSON.stringify(_sortCharacters));
    reRender(_matchupData, false);
}
