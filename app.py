import eventlet

eventlet.monkey_patch()
import json
import os
import sys
from datetime import datetime, timedelta
from pprint import pprint
from typing import Dict, List, Tuple, Union
from time import time

import numpy as np
import pandas as pd
from flask import Flask, jsonify, render_template, request
from flask_socketio import SocketIO, emit
from slippi import id

# from sklearn.cluster import DBSCAN

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


@app.route("/stats", methods=["GET"])
def stats():
    return render_template("stats.html")


@app.route("/matchups", methods=["GET"])
def matchups():
    global matchup_chart
    return jsonify(matchup_chart)


@app.route("/character_ratings", methods=["GET"])
def get_character_ratings():
    global character_ratings
    return jsonify(character_ratings)


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
    rating_period_games: pd.DataFrame,
) -> Dict[str, List[Dict[str, float]]]:
    games_per_character = {"P1": [[] for _ in range(26)], "P2": [[] for _ in range(26)]}
    for _, game in rating_period_games.iterrows():
        p1_character = game["p1_character"]
        p2_character = game["p2_character"]

        games_per_character["P1"][p1_character].append(
            {
                "opponent_rating": previous_ratings["P2"][p2_character]["rating"],
                "opponent_rd": previous_ratings["P2"][p2_character]["rd"],
                "score": int(game["p1_won"]),
            }
        )

        games_per_character["P2"][p2_character].append(
            {
                "opponent_rating": previous_ratings["P1"][p1_character]["rating"],
                "opponent_rd": previous_ratings["P1"][p1_character]["rd"],
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

    return new_ratings


def update_matchups(
    subset: pd.DataFrame,
    p1_character: id.CSSCharacter,
    p2_character: id.CSSCharacter,
    weighted: bool = False,
):
    # Matchup chart is stored from the perspective of P1.
    global matchup_chart

    if subset.empty:
        matchup_chart[p1_character][p2_character] = {
            "win_rate": "nan",
            "matches": 0,
        }
        return

    subset = subset[-100:]
    matches = len(subset)

    if weighted:
        weights = np.exp(-0.1 * np.arange(matches - 1, -1, -1))
        winrate1 = np.average(subset["p1_won"], weights=weights)
    else:
        winrate1 = subset["p1_won"].mean()

    winrate = winrate1 if not np.isnan(winrate1) else "nan"

    matchup_chart[p1_character][p2_character] = {
        "win_rate": winrate,
        "matches": matches,
    }


def process_new_replay(path: str):
    global games_df, last_results, character_ratings
    data = parse_replay(path, player_ports, True)
    date = pd.to_datetime(data["datetime"])
    if date not in games_df.index:
        games_df.loc[date] = data
        # games_df.to_pickle("db.pkl")

    p1_character_rating = character_ratings["P1"][data["p1_character"]]
    p2_character_rating = character_ratings["P2"][data["p2_character"]]

    p1_old_rating = p1_character_rating["rating"]
    p2_old_rating = p2_character_rating["rating"]

    reload_tier_list(games_df)

    last_results.append(
        {
            "P1": {
                "character": data["p1_character"],
                "delta": p1_character_rating["rating"] - p1_old_rating,
                "probability": glicko.win_probability(
                    p1_character_rating["rating"],
                    p2_character_rating["rating"],
                    p2_character_rating["rd"],
                ),
            },
            "P2": {
                "character": data["p2_character"],
                "delta": p2_character_rating["rating"] - p2_old_rating,
                "probability": glicko.win_probability(
                    p2_character_rating["rating"],
                    p1_character_rating["rating"],
                    p1_character_rating["rd"],
                ),
            },
        }
    )
    last_results = last_results[1:]


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

    return filtered[
        ["p1_code", "p1_character", "p2_character", "p1_won", "p2_won", "end_type"]
    ]


def reload_tier_list(df: pd.DataFrame):
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
    filtered_games_df = filter_relevant_games(df.copy())
    print(f"Game filtering done: {round(time() - start, 2)}s")

    # Recalculate matchups.
    start = time()
    # Matchup chart is stored from the perspective of P1.
    matchup_pairs_df = filtered_games_df[
        (filtered_games_df["p1_code"] == settings.PLAYER_CODES["P1"])
        & (filtered_games_df["end_type"] != 7)
    ].groupby(["p1_character", "p2_character"])

    for p1_character in range(0, 26):
        for p2_character in range(0, 26):
            key = (p1_character, p2_character)
            matchup_pair = (
                matchup_pairs_df.get_group(key)
                if key in matchup_pairs_df.groups
                else pd.DataFrame()
            )
            update_matchups(matchup_pair, p1_character, p2_character)
    print(f"Matchup recalculation done: {round(time() - start, 2)}s")

    # Recalculate tiers.
    start = time()
    filtered_games_df.index = pd.to_datetime(filtered_games_df.index, utc=True)
    rating_periods = filtered_games_df.groupby(filtered_games_df.index.date)
    print("rating periods:", len(rating_periods))
    min_date = filtered_games_df.index.min().date()
    max_date = filtered_games_df.index.max().date()

    columns = ["p1_character", "p2_character", "p1_won", "p2_won"]
    current_date = min_date
    while current_date <= max_date:
        if current_date in rating_periods.groups:
            daily_games = rating_periods.get_group(current_date)[columns]
        else:
            daily_games = pd.DataFrame(columns=columns)

        character_ratings = update_tiers(character_ratings, daily_games)

        current_date += timedelta(days=1)

    print(f"Tier list recalculation done: {round(time() - start, 2)}s")

    # ratings = []
    # for player, characters in character_ratings.items():
    #     for i, character in enumerate(characters):
    #         ratings.append((f"{player}_{id.CSSCharacter(i).name}", character["rating"]))

    # ratings = sorted(ratings, key=lambda x: x[1])
    # max_rating = ratings[-1][1]
    # min_rating = ratings[0][1]
    # ratings = [(n, (x - min_rating) / max_rating) for n, x in ratings]
    # values = np.array([v for _, v in ratings]).reshape(-1, 1)
    # clustering = DBSCAN(eps=0.01, min_samples=2).fit(values)

    # for (label, value), cluster_label in zip(ratings, clustering.labels_):
    #     print(f"{label}: value={value}, cluster={cluster_label}")


def background_task() -> None:
    global character_ratings, games_df

    reload_tier_list(games_df)

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
    socketio.start_background_task(target=background_task)
    socketio.run(app, debug=True, use_reloader=False)
