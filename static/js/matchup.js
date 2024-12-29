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

let characterMask;
if (!localStorage.getItem("characterMask")) {
    characterMask = new Array(26).fill(true);
    localStorage.setItem("characterMask", JSON.stringify(characterMask));
} else {
    characterMask = JSON.parse(localStorage.getItem("characterMask"));
}

let renderTypeIndex = 0;

let matchupData = null;

const socket = io.connect("http://127.0.0.1:5000");
socket.on("matchup_update", function (data) {
    console.log(data);
    matchupData = data;
    populateDropdown();
    reRender(data);
});

function reRender(data) {
    const { matchups, winner } = data;

    const namedMatchups = mapCharacterNames(matchups);

    const maskIndices = characterMask.map((x, i) => (!x ? i : -1)).filter((i) => i != -1);
    const filteredMatchups = remove(namedMatchups, maskIndices, maskIndices);

    console.log(filteredMatchups);

    const rowOrder = calculateSortedIndices(filteredMatchups);
    const colOrder = calculateSortedIndices(
        flipMatchupChart(JSON.parse(JSON.stringify(filteredMatchups)))
    );

    const sortedMatchups = reorder(filteredMatchups, rowOrder, colOrder);

    renderMatchupChart(sortedMatchups, "p1-matchup-chart");
    chooseMatchupPairRender(filteredMatchups, winner);
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

function chooseMatchupPairRender(matchups, winner) {
    const p1Won = winner === "P1";
    document.querySelector(".winner-text").innerText = `${p1Won ? "P2" : "P1"} Chooses`;
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
    const matchupPairs = matchups.map((row, i) => {
        const weights = row.map((col) => {
            return col.data.matches < 5 ? 1 : 1 - (Math.abs(col.data.win_rate - 0.5) * 2) ** 4;
        });
        const weightsSum = weights.reduce((sum, w) => sum + w, 0);
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

function renderMatchupChart(matchups, id) {
    const table = document.getElementById(id);
    const body = table.querySelector(".matchup-chart-body");
    body.innerHTML = "";

    const headerRow = document.createElement("tr");
    const td = document.createElement("td");
    headerRow.appendChild(td);

    matchups[0].forEach((col) => {
        const td = document.createElement("td");
        const img = document.createElement("img");
        img.src = `/static/images/${col.against}.png`;
        td.appendChild(img);
        headerRow.appendChild(td);
    });

    body.appendChild(headerRow);

    matchups.forEach((row) => {
        const tableRow = document.createElement("tr");
        const td = document.createElement("td");
        const img = document.createElement("img");
        img.src = `/static/images/${row[0].with}.png`;
        td.appendChild(img);
        tableRow.appendChild(td);

        row.forEach((x) => {
            const td = document.createElement("td");
            const winRate = x.data.win_rate;
            td.innerText = Math.round(winRate * 100) / 100;
            const color = isNaN(winRate) ? "#000000" : hsv2rgb(Math.floor(winRate * 120), 0.6, 1);
            td.style.backgroundColor = color;

            const h5 = document.createElement("h5");
            h5.innerText = `(${x.data.matches})`;
            h5.classList.add("match-number");
            td.appendChild(h5);

            tableRow.appendChild(td);
        });

        table.querySelector(".matchup-chart-body").appendChild(tableRow);
    });
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
    console.log(`Toggled item ${index}: ${CHARACTERS[index]}`);
    element.classList.toggle("toggled");
    characterMask[index] = element.classList.contains("toggled");
    console.log(characterMask);
    localStorage.setItem("characterMask", JSON.stringify(characterMask));
    if (matchupData) reRender(matchupData);
}
