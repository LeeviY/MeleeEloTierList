document.addEventListener("DOMContentLoaded", fetchPorts);

const socket = io.connect("http://127.0.0.1:5000");
socket.on("tier_update", function (data) {
    console.log(data);
    updateData(data);
});

function updateData(data) {
    console.log(data);
    renderPlayerTierList("tier-list-p1", data.P1);
    renderPlayerTierList("tier-list-p2", data.P2);
}

function renderPlayerTierList(playerId, tiers) {
    const tierListContainer = document.getElementById(playerId);
    tierListContainer.innerHTML = "";

    for (const [tier, items] of Object.entries(tiers)) {
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
            itemDiv.appendChild(img);
            const ratingText = document.createElement("p");
            ratingText.classList.add("item-rating");
            ratingText.textContent = `${Math.round(item.rating)}(${
                item.matches
            })`;
            itemDiv.appendChild(ratingText);
            tierItemsContainer.appendChild(itemDiv);
        });

        tierDiv.appendChild(tierItemsContainer);
        tierListContainer.appendChild(tierDiv);
    }
}

let selectedPorts = { P1: null, P2: null };

function highlightActivePort(player, port) {
    const buttonsContainer = document.getElementById(
        `port-buttons-${player.toLowerCase()}`
    );
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
                updateData(data);
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
                updateData(data);
                console.log("Recalculation successful:", data);
            })
            .catch((err) => {
                console.error("Error recalculating tier list:", err);
            });
    }
}
