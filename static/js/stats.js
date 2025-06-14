document.addEventListener("DOMContentLoaded", async () => {
    const response = await fetch("/character_ratings", { method: "GET" });
    const data = await response.json();
    console.log(data);

    const ratings = data.P1.map((obj) => obj.rating).concat(data.P2.map((obj) => obj.rating));
    const maxRaiting = Math.max(...ratings);
    const minRaiting = Math.min(...ratings);

    console.log(minRaiting, maxRaiting);

    const p1Characters = data.P1.map((obj, i) => {
        return { src: `/static/images/${CHARACTERS[i]}.png`, value: obj.rating };
    });
    p1Characters.sort((a, b) => a.value - b.value);

    const p2Characters = data.P2.map((obj, i) => {
        return { src: `/static/images/${CHARACTERS[i]}.png`, value: obj.rating };
    });
    p2Characters.sort((a, b) => a.value - b.value);

    addImagesToLine("line1", p1Characters, minRaiting, maxRaiting);
    addImagesToLine("line2", p2Characters, minRaiting, maxRaiting);
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
