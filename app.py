import eventlet

eventlet.monkey_patch()
import json
import os
import sys
from datetime import datetime
from pprint import pprint
from typing import Dict, List, Tuple, Union
from time import time

import numpy as np
import pandas as pd
from flask import Flask, jsonify, render_template, request
from flask_socketio import SocketIO, emit
from slippi import id

import database
import glicko
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

matchup_chart = []


### TODO:
# add stock count based tier list
# refactor tier list html to use templates
# use data classes


### Routing
@app.route("/")
def tier_list():
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


def update_tiers(
    previous_ratings: Dict[str, List[Dict[str, float]]],
    rating_period_games: List[Dict[str, Union[id.CSSCharacter, bool]]],
) -> Dict[str, List[Dict[str, float]]]:
    games_per_character = {"P1": [[] for _ in range(26)], "P2": [[] for _ in range(26)]}

    for game in rating_period_games:
        games_per_character["P1"][game["p1"]].append(
            {
                "opponent_rating": previous_ratings["P2"][game["p2"]]["rating"],
                "opponent_rd": previous_ratings["P2"][game["p2"]]["rd"],
                "score": int(game["p1_won"]),
            }
        )

        games_per_character["P2"][game["p2"]].append(
            {
                "opponent_rating": previous_ratings["P1"][game["p1"]]["rating"],
                "opponent_rd": previous_ratings["P1"][game["p1"]]["rd"],
                "score": int(game["p2_won"]),
            }
        )

    new_ratings = {
        "P1": [x for x in previous_ratings["P1"]],
        "P2": [x for x in previous_ratings["P2"]],
    }

    for player, characters_results in games_per_character.items():
        for i, character_results in enumerate(characters_results):
            new_rating = glicko.glicko2_rating_update(
                previous_ratings[player][i], character_results
            )
            new_rating["matches"] = new_ratings[player][i]["matches"] + len(
                character_results
            )

            new_ratings[player][i] = new_rating

    # last_results.append(
    #     {
    #         "P1": {
    #             "character": str(p1["character"]),
    #             "delta": mu1_new - mu1_old,
    #         },
    #         "P2": {
    #             "character": str(p2["character"]),
    #             "delta": mu2_new - mu2_old,
    #         },
    #     }
    # )
    # last_results = last_results[1:]

    return new_ratings


def update_matchups(
    p1_character: id.CSSCharacter,
    p2_character: id.CSSCharacter,
    weighted: bool = False,
):
    # Matchup chart is stored from the perspective of P1.
    global matchup_chart

    subset = games_df[
        (games_df["p1_character"] == p1_character)
        & (games_df["p2_character"] == p2_character)
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

    matchup_chart[p1_character][p2_character] = {
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
    reload_tier_list()


def filter_relevant_games(games: pd.DataFrame) -> List[dict]:
    global player_ports, games_list

    filtered = games[
        (~games["ignore"])
        & (games["frames"] / 60 >= settings.MIN_GAME_DURATION_SECONDS)
        & ((settings.ALLOW_EXIT) | (games["end_type"] != 7))
    ].copy()

    lras_mask = filtered["lras_initiator"].notna()
    filtered.loc[lras_mask, "p1_won"] = (
        filtered["p1_port"] != filtered["lras_initiator"]
    )
    filtered.loc[lras_mask, "p2_won"] = (
        filtered["p2_port"] != filtered["lras_initiator"]
    )

    return [
        {
            "p1": id.CSSCharacter(row["p1_character"]),
            "p1_won": row["p1_won"],
            "p2": id.CSSCharacter(row["p2_character"]),
            "p2_won": row["p2_won"],
        }
        for _, row in filtered.iterrows()
    ]


def reload_tier_list():
    global character_ratings, matchup_chart
    character_ratings = {
        "P1": [
            {"rating": 1500, "rd": 350, "volatility": 0.06, "matches": 0}
            for _ in range(26)
        ],
        "P2": [
            {"rating": 1500, "rd": 350, "volatility": 0.06, "matches": 0}
            for _ in range(26)
        ],
    }

    matchup_chart = [
        [{"win_rate": "nan", "matches": 0} for _ in range(26)] for _ in range(26)
    ]

    # Filter games.
    start = time()
    games_list = []
    # TODO: use games_df["ignored" == False]
    games_df.index = pd.to_datetime(games_df.index, utc=True)
    groups = games_df.groupby(games_df.index.date)
    print("rating periods:", len(groups))
    for _, group in groups:
        relevant_games = filter_relevant_games(group)
        if len(relevant_games) > 0:
            print(len(relevant_games))
            games_list.append(relevant_games)
    print(f"Game filtering done: {round(time() - start, 2)}s")

    # Recalculate matchups.
    start = time()
    for p1_character in range(0, 26):
        for p2_character in range(0, 26):
            update_matchups(p1_character, p2_character)
    print(f"Matchup recalculation done: {round(time() - start, 2)}s")

    # Recalculate tiers.
    start = time()
    for rating_period in games_list:
        character_ratings = update_tiers(character_ratings, rating_period)
    print(f"Tier list recalculation done: {round(time() - start, 2)}s")


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

    socketio.start_background_task(target=background_task)
    socketio.run(app, debug=True, use_reloader=False)
