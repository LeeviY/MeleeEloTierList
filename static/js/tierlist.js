document.addEventListener("DOMContentLoaded", () => {
    fetchPorts();
});

const socket = io.connect("http://127.0.0.1:5000");
socket.on("tier_update", function (data) {
    console.log("tier_update", data);
    updateTierList(data);
});

socket.on("results_update", function (data) {
    console.log("results_update", data);
    renderLastResults(data);
});

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

let _tierList = null;

function updateTierList(data) {
    _tierList = data;
    console.log("data:", data);
    renderPlayerTierList("P1", data.P1);
    renderPlayerTierList("P2", data.P2);
    const ratings = data.P1.map((obj) => obj.rating).concat(data.P2.map((obj) => obj.rating));
    const maxRaiting = Math.max(...ratings);
    const minRaiting = Math.min(...ratings);
    console.log("max:", maxRaiting, "min:", minRaiting);
    alignTiers(
        Object.keys(eloToTiers(data.P1)).map(Function.prototype.call, String.prototype.toLowerCase)
    );
}

function eloToTiers(characters) {
    const tierList = { S: [], A: [], B: [], C: [], D: [], F: [] };
    for (const [i, character] of characters.entries()) {
        const rating = character["rating"];
        let tier = "";
        if (rating >= 1800) tier = "S";
        else if (rating >= 1650) tier = "A";
        else if (rating >= 1500) tier = "B";
        else if (rating >= 1350) tier = "C";
        else if (rating >= 1200) tier = "D";
        else tier = "F";

        tierList[tier].push({
            id: i,
            name: CHARACTERS[i],
            rating: rating,
            matches: character["matches"],
            volatility: character["volatility"],
        });
    }
    return tierList;
}

function renderPlayerTierList(playerId, ratings) {
    const tierListContainer = document.getElementById(`tier-list-${playerId.toLowerCase()}`);
    tierListContainer.innerHTML = "";

    for (const [tier, items] of Object.entries(eloToTiers(ratings))) {
        const tierDiv = document.createElement("div");
        tierDiv.classList.add("tier", `tier-${tier.toLowerCase()}`);

        const tierTitle = document.createElement("h2");
        tierTitle.textContent = `${tier} Tier`;
        tierDiv.appendChild(tierTitle);

        const tierItemsContainer = document.createElement("div");
        tierItemsContainer.classList.add("tier-items");

        const sortedItems = items.sort((a, b) => {
            return b.rating - a.rating;
        });
        sortedItems.forEach((item) => {
            const itemDiv = document.createElement("div");
            itemDiv.classList.add("item");
            const img = document.createElement("img");
            img.src = `/static/images/${item.name}.png`;
            img.dataset.name = item.name;
            img.classList.add(playerId.toLowerCase());
            img.onmouseover = () => highlightImages(playerId, item.id);

            itemDiv.appendChild(img);
            const ratingText = document.createElement("p");
            ratingText.classList.add("item-rating");
            ratingText.textContent = `${Math.round(item.rating)}(${item.matches})`;
            itemDiv.appendChild(ratingText);

            const volatilityText = document.createElement("p");
            volatilityText.classList.add("item-rating");
            volatilityText.textContent = `${Math.round(item.volatility * 10000) / 10000}`;
            itemDiv.appendChild(volatilityText);

            tierItemsContainer.appendChild(itemDiv);
        });

        tierDiv.appendChild(tierItemsContainer);
        tierListContainer.appendChild(tierDiv);
    }
}

function highlightImages(playerId, id) {
    document.querySelectorAll(".highlight").forEach((el) => el.classList.remove("highlight"));

    const otherPlayerId = playerId == "P1" ? "P2" : "P1";

    const referenceCharacter = _tierList[playerId][id];
    const closestCharacter =
        CHARACTERS[
            Object.entries(_tierList[otherPlayerId]).reduce(
                (closest, [index, character]) => {
                    const delta = Math.abs(character.rating - referenceCharacter.rating);
                    return delta < closest.delta ? { index, delta } : closest;
                },
                { index: null, delta: Infinity }
            ).index
        ];

    document
        .querySelectorAll(
            `img[data-name="${
                CHARACTERS[id]
            }"].${playerId.toLowerCase()}, img[data-name="${closestCharacter}"].${otherPlayerId.toLowerCase()}`
        )
        .forEach((img) => img.classList.add("highlight"));
}

function alignTiers(tierNames) {
    if (!tierNames) {
        console.error("missing tier names for alignment");
    }
    tierNames.forEach((tierName) => {
        const tiers = document.querySelectorAll(`.tier.tier-${tierName}`);
        let maxHeight = 0;
        tiers.forEach((tier) => {
            const height = tier.querySelector(".tier-items").scrollHeight;
            if (height > maxHeight) {
                maxHeight = height;
            }
        });

        tiers.forEach((tier) => {
            tier.querySelector(".tier-items").style.minHeight = `${maxHeight}px`;
        });
    });
}

function renderLastResults(results) {
    const resultsContainer = document.querySelector(".result-items");
    resultsContainer.innerHTML = "";

    results.forEach((result) => {
        const resultDiv = document.createElement("div");
        resultDiv.classList.add("result");
        for (const [player, items] of Object.entries(result)) {
            const itemDiv = document.createElement("div");
            itemDiv.classList.add("item");
            itemDiv.style.float = player == "P1" ? "left" : "right";
            const img = document.createElement("img");
            img.src = `/static/images/${items.character}.png`;
            itemDiv.appendChild(img);
            const ratingText = document.createElement("p");
            ratingText.classList.add("item-rating");
            ratingText.textContent = `${Math.round(items.delta)}`;
            itemDiv.appendChild(ratingText);
            resultDiv.appendChild(itemDiv);
        }
        const clear = document.createElement("div");
        clear.style.clear = "both";
        resultDiv.appendChild(clear);
        resultsContainer.appendChild(resultDiv);
    });
}

let selectedPorts = { P1: null, P2: null };

function highlightActivePort(player, port) {
    const buttonsContainer = document.getElementById(`port-buttons-${player.toLowerCase()}`);
    const buttons = buttonsContainer.querySelectorAll("button");

    buttons.forEach((button) => {
        button.classList.remove("active");
        if (parseInt(button.innerText.replace("Port ", ""), 10) === port) {
            button.classList.add("active");
        }
    });

    selectedPorts[player] = port;
}

async function sendPortToServer(player, port) {
    try {
        const response = await fetch("/port", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ player, port }),
        });

        const data = await response.json();
        if (!data.message) {
            alert(data.error || "Failed to update port.");
        }
    } catch (error) {
        console.error("Error setting port:", error);
        alert("Error setting port.");
    }
}

async function handlePortClick(player, port, btn) {
    highlightActivePort(player, port);
    await sendPortToServer(player, port);
}

async function fetchPorts() {
    try {
        const response = await fetch("/port", { method: "GET" });
        const data = await response.json();

        if (data.P1) {
            highlightActivePort("P1", data.P1);
        }

        if (data.P2) {
            highlightActivePort("P2", data.P2);
        }
    } catch (error) {
        console.error("Error fetching ports:", error);
        alert("Error fetching ports.");
    }
}

let ALLOW_EXIT = false;
function toggleAllowExit() {
    fetch("/allow_exit", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ value: !ALLOW_EXIT }),
    })
        .then((response) => response.json())
        .then((data) => {
            ALLOW_EXIT = !ALLOW_EXIT;
            document.getElementById(
                "allow-exit-button"
            ).innerText = `Allow Quitting: ${ALLOW_EXIT}`;
            console.log(data);
        })
        .catch((err) => {
            console.error(err);
        });
}

function resetTierList() {
    if (confirm("Are you sure you want to reset the tier list?")) {
        fetch("/reset", {
            method: "POST",
        })
            .then((response) => response.json())
            .then((data) => {
                updateTierList(data);
                console.log("Reset successful:", data);
            })
            .catch((err) => {
                console.error("Error resetting tier list:", err);
            });
    }
}

function recalculateTierList() {
    if (confirm("Are you sure you want to recalculate the tier list?")) {
        fetch("/recalculate", {
            method: "POST",
        })
            .then((response) => response.json())
            .then((data) => {
                updateTierList(data);
                console.log("Recalculation successful:", data);
            })
            .catch((err) => {
                console.error("Error recalculating tier list:", err);
            });
    }
}

function toggleActionMenu() {
    document.querySelector(".action-buttons").classList.toggle("collapsed");
}

async function handleStartDateChange() {
    try {
        const response = await fetch("/date_range", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ player, port }),
        });

        const data = await response.json();
        if (!data.message) {
            alert(data.error || "Failed to update date.");
        }
    } catch (error) {
        console.error("Error setting date:", error);
    }
}

async function handleEndDateChange() {
    try {
        const response = await fetch("/port", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ player, port }),
        });

        const data = await response.json();
        if (!data.message) {
            alert(data.error || "Failed to update date.");
        }
    } catch (error) {
        console.error("Error setting date:", error);
    }
}
