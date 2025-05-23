import eventlet

eventlet.monkey_patch()
import json
import os
import sys
from datetime import datetime
from pprint import pprint
from typing import Dict, List, Tuple, Union

import numpy as np
import pandas as pd
from flask import Flask, jsonify, render_template, request
from flask_socketio import SocketIO, emit
from slippi import id

import database
import settings
from utils.files import (
    detect_new_files,
    find_replay_directory,
    is_file_locked,
    parse_replay,
)

app = Flask(__name__)
app.json.sort_keys = False
socketio = SocketIO(app)


games_df = pd.DataFrame(columns=database.columns)
games_df = games_df.set_index("datetime")
games_df = pd.read_pickle("db.pkl")
games_df = games_df.sort_index()
print(games_df)
print(games_df.columns)
print(games_df.dtypes)

player_ports = settings.DEFAULT_PLAYER_PORTS
date_range = {"start": datetime(1, 1, 1, 0, 0, 0), "end": datetime.now()}

last_results = [None] * 10

character_ratings = {}

ignored_games = set()

matchup_chart = [[{"win_rate": "nan", "matches": 0}] * 26 for _ in range(26)]


### TODO:
# add stock count based tier list
# add matchup chart
# refactor tier list html to use templates


### Routing
@app.route("/")
def tier_list():
    global character_ratings
    return render_template("index.html")


@app.route("/matchup_chart")
def matchup_charts():
    return render_template("matchup.html")


@app.route("/matchups", methods=["GET"])
def matchups():
    global matchup_chart
    return jsonify(matchup_chart)


@app.route("/reset", methods=["POST"])
def reset_tier_list():
    global character_ratings
    with open(settings.TIER_FILE_BASE, "r") as file:
        character_ratings = json.load(file)

    return jsonify(character_ratings)


@app.route("/recalculate", methods=["POST"])
def recalculate_tier_list():
    global character_ratings
    reload_tier_list()

    socketio.emit("results_update", last_results)
    with app.app_context():
        return jsonify(character_ratings)


@app.route("/port", methods=["GET"])
def get_port():
    global player_ports
    return jsonify(player_ports)


@app.route("/port", methods=["POST"])
def set_port():
    global player_ports
    data = request.json
    player = data.get("player")
    port = data.get("port")

    if player in player_ports:
        try:
            port = int(port)
            player_ports[player] = port
            return jsonify({"message": f"Port for {player} set to {port}"})
        except ValueError:
            return jsonify({"error": "Invalid port number"}), 400
    else:
        return jsonify({"error": "Player not found"}), 404


@app.route("/allow_exit", methods=["POST"])
def set_qutting():
    data = request.json
    settings.ALLOW_EXIT = data.get("value")
    return jsonify({"message": f"Allowing exit set to {settings.ALLOW_EXIT}"})


@app.route("/date_range", methods=["GET"])
def get_date_range():
    global date_range
    return jsonify(date_range)


@app.route("/date_range", methods=["POST"])
def set_date_range():
    global date_range
    data = request.json
    start = data.get("start")
    end = data.get("end")

    if not start or not end:
        return jsonify({"error": "Missing values"}), 400

    date_range["start"] = start
    date_range["end"] = end

    return jsonify({"message": f"Date range set to {date_range}"})


@socketio.on("connect")
def emit_all():
    socketio.emit("tier_update", character_ratings)
    socketio.emit("results_update", last_results)
    socketio.emit(
        "matchup_update",
        {
            "matchups": matchup_chart,
            "winner": "P1" if games_df.tail(1).squeeze()["p1_won"] else "P2",
        },
    )


### Logic
def update_tiers(
    p1: Dict[str, Union[id.CSSCharacter, bool]],
    p2: Dict[str, Union[id.CSSCharacter, bool]],
) -> Dict[str, List[Dict[str, int]]]:
    global last_results, character_ratings, games_df
    if not p1 or not p2:
        return None

    p1_char = character_ratings["P1"][p1["character"]]
    p2_char = character_ratings["P2"][p2["character"]]
    p1_rating = p1_char["elo"]
    p2_rating = p2_char["elo"]

    p1_char["matches"] += 1
    p2_char["matches"] += 1

    E_p = lambda R_a, R_b: 1.0 / (1.0 + pow(10, ((R_b - R_a) / 400)))
    R_n = lambda R, K, S, E: R + K * (S - E)
    K = lambda m: max(800 / m, 100)

    p1_char["elo"] = R_n(
        p1_rating,
        K(p1_char["matches"]),
        int(p1["won"]),
        E_p(p1_rating, p2_rating),
    )
    p2_char["elo"] = R_n(
        p2_rating,
        K(p2_char["matches"]),
        int(p2["won"]),
        E_p(p2_rating, p1_rating),
    )

    last_results.append(
        {
            "P1": {
                "character": id.CSSCharacter(p1["character"]).name,
                "delta": p1_char["elo"] - p1_rating,
            },
            "P2": {
                "character": id.CSSCharacter(p2["character"]).name,
                "delta": p2_char["elo"] - p2_rating,
            },
        }
    )
    last_results = last_results[1:]


# def elo_to_tiers(
#     tiers: Dict[str, List[Dict[str, int]]]
# ) -> Dict[str, List[Dict[str, Union[str, int]]]]:
#     tier_list = {}
#     for player, characters in tiers.items():
#         tier_list[player] = {"S": [], "A": [], "B": [], "C": [], "D": [], "F": []}
#         for i, character in enumerate(characters):
#             rating = character["elo"]
#             tier = ""
#             if rating >= 2000:
#                 tier = "S"
#             elif rating >= 1800:
#                 tier = "A"
#             elif rating >= 1600:
#                 tier = "B"
#             elif rating >= 1400:
#                 tier = "C"
#             elif rating >= 1200:
#                 tier = "D"
#             else:
#                 tier = "F"

#             tier_list[player][tier].append(
#                 {
#                     "name": id.CSSCharacter(i).name,
#                     "rating": rating,
#                     "matches": character["matches"],
#                 }
#             )

#     return tier_list


def process_game(
    data: Dict[str, Union[int, str]], debug_print: bool = False, weighted: bool = True
) -> None:
    global player_ports
    if not data or data["ignore"]:
        return

    if data["frames"] / 60 < settings.MIN_GAME_DURATION_SECONDS:
        if debug_print:
            print("Game too short")
        return

    if not settings.ALLOW_EXIT and data["end_type"] == 7:
        if debug_print:
            print("Game exited")
        return

    # Redefine the winner as the non quitter.
    lras_initiator = data["lras_initiator"]
    if lras_initiator != None and not np.isnan(lras_initiator):
        data["p1_won"] = data["p1_port"] != lras_initiator
        data["p2_won"] = data["p2_port"] != lras_initiator

    if data["p1_won"] == data["p2_won"]:
        print(data)

    P1 = {"character": id.CSSCharacter(data["p1_character"]), "won": data["p1_won"]}
    P2 = {"character": id.CSSCharacter(data["p2_character"]), "won": data["p2_won"]}

    update_tiers(P1, P2)
    update_matchups(P1, P2, weighted)


def update_matchups(P1, P2, weighted: bool = True):
    # Matchup chart is stored from the perspective of P1.
    global matchup_chart

    subset = games_df[
        (games_df["p1_character"] == P1["character"])
        & (games_df["p2_character"] == P2["character"])
        & (games_df["p1_code"] == settings.PLAYER_CODES["P1"])
        & (games_df["end_type"] != 7)
    ]

    winrate = "nan"
    matches = len(subset)

    if not weighted:
        subset = subset[-100:]
        winrate1 = subset["p1_won"].mean()
        matches = len(subset)
        if not np.isnan(winrate1):
            winrate = winrate1
    else:
        # weight_func = lambda x: 1.1 ** (1 - x)
        weight_func = lambda x: np.e ** (-0.1 * (x - 1))
        subset = subset[-100:]
        matches = len(subset)

        if not subset.empty:
            weights = np.array([weight_func(i) for i in reversed(range(len(subset)))])
            winrate1 = (subset["p1_won"] * weights).sum() / weights.sum()

            if not np.isnan(winrate1):
                winrate = winrate1

    matchup_chart[P1["character"]][P2["character"]] = {
        "win_rate": winrate,
        "matches": matches,
    }


def process_new_replay(path: str):
    global games_df
    data = parse_replay(path, player_ports, True)
    date = pd.to_datetime(data["datetime"])
    if date not in games_df.index:
        games_df.loc[date] = data
        games_df.to_pickle("db.pkl")
    process_game(data, True, False)


def reload_tier_list():
    global character_ratings, matchup_chart
    with open(settings.TIER_FILE_BASE, "r") as file:
        character_ratings = json.load(file)

    matchup_chart = [[{"win_rate": "nan", "matches": 0}] * 26 for _ in range(26)]

    # TODO: use games_df["ignored" == False]
    for row in games_df.to_dict(orient="records"):
        process_game(row, False, False)

    print("Tier list recalculation done.")


def background_task() -> None:
    global character_ratings, games_df

    reload_tier_list()

    print("")
    spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
    spin_index = 0

    while True:
        latest_directory = find_replay_directory()

        sys.stdout.write(
            f"\rWatching directory: {latest_directory} {spinner[spin_index]}"
        )
        sys.stdout.flush()
        spin_index = (spin_index + 1) % len(spinner)

        new_file = detect_new_files(games_df, latest_directory)
        if new_file != "":
            print(f"\nFound new replay: {new_file}")

            path = os.path.join(latest_directory, new_file)
            while is_file_locked(path):
                eventlet.sleep(0.5)

            process_new_replay(path)
            print("Processing new replay done.")

            emit_all()

        eventlet.sleep(0.5)


if __name__ == "__main__":
    # parse_replay(
    #     r"C:\Users\Leevi\projects\Python\MeleeEloTierList\test_replays\Game_20241019T013605.slp",
    #     player_ports,
    #     True,
    # )

    # parse_replay(
    #     r"C:\Users\Leevi\projects\Python\MeleeEloTierList\test_replays\Game_20241208T180113.slp",
    #     player_ports,
    #     True,
    # )

    # games_df.to_csv("db.csv")

    socketio.start_background_task(target=background_task)
    socketio.run(app, debug=True, use_reloader=False)
