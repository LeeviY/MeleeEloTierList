import os
import json
import re
import eventlet
import sys
import contextlib
import pandas as pd
import numpy as np
import traceback

eventlet.monkey_patch()

from pprint import pprint
from datetime import datetime
from typing import List, Tuple, Dict, Union
from flask import Flask, render_template, request, jsonify
from flask_socketio import SocketIO, emit

import win32file
import peppi_py
from slippi import Game, id
from peppi_py import read_slippi, read_peppi


app = Flask(__name__)
app.json.sort_keys = False
socketio = SocketIO(app)

TIER_FILE = "tier_list.json"
TIER_FILE_BASE = "tier_list.json.base"
MIN_GAME_DURATION_SECONDS = 30
ALLOW_EXIT = False
EXTRA_DIRS = [f"{os.path.dirname(os.path.abspath(__file__))}\\2024-12"]

games_df = pd.DataFrame(
    columns=[
        "stage",
        "p1_code",
        "p1_port",
        "p1_character",
        "p1_stocks",
        "p2_code",
        "p2_port",
        "p2_character",
        "p2_stocks",
        "end_type",
        "lras_initiator",
        "p1_won",
        "p2_won",
        "datetime",
        "frames",
    ]
)
games_df = games_df.set_index("datetime")
games_df = pd.read_pickle("db.pkl")
games_df = games_df.sort_index()

print(games_df)
last_results = [None] * 10

character_ratings = {}
with open(TIER_FILE, "r") as file:
    character_ratings = json.load(file)

player_ports = {"P1": 2, "P2": 4}
players_codes = {"P1": "LY＃863", "P2": "KEKW＃849"}


### TODO:
# parse replays to db file
# add stock count based tier list
# add matchup chart
# refactor tier list html to use templates


### Routing
@app.route("/")
def tier_list():
    global character_ratings
    return render_template("index.html", tiers=elo_to_tiers(character_ratings))


@app.route("/reset", methods=["POST"])
def reset_tier_list():
    global character_ratings
    with open(TIER_FILE_BASE, "r") as file:
        character_ratings = json.load(file)

    with open(TIER_FILE, "w") as file:
        json.dump(character_ratings, file)

    return jsonify(elo_to_tiers(character_ratings)), 200


@app.route("/recalculate", methods=["POST"])
def recalculate_tier_list():
    global character_ratings, slippi_directory
    with open(TIER_FILE_BASE, "r") as file:
        character_ratings = json.load(file)

    for row in games_df.to_dict(orient="records"):
        process_replay(row)

    count_1 = 0
    for character in character_ratings["P1"]:
        count_1 += character["matches"]

    count_2 = 0
    for character in character_ratings["P2"]:
        count_2 += character["matches"]

    print(count_1, count_2)

    with open(TIER_FILE, "w") as file:
        json.dump(character_ratings, file)

    socketio.emit("results_update", last_results)
    with app.app_context():
        return jsonify(elo_to_tiers(character_ratings)), 200


@app.route("/port", methods=["GET"])
def get_port():
    global player_ports
    return jsonify(player_ports), 200


# @app.route("/last_results", methods=["GET"])
# def last_results():
#     global last_results
#     return jsonify(last_results), 200


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
            return jsonify({"message": f"Port for {player} set to {port}"}), 200
        except ValueError:
            return jsonify({"error": "Invalid port number"}), 400
    else:
        return jsonify({"error": "Player not found"}), 404


@app.route("/allow_exit", methods=["POST"])
def set_qutting():
    global ALLOW_EXIT
    data = request.json
    ALLOW_EXIT = data.get("value")
    return jsonify({"message": f"Allowing exit set to {ALLOW_EXIT}"}), 200


@socketio.on("connect")
def handle_connect():
    emit("tier_update", elo_to_tiers(character_ratings))
    emit("results_update", last_results)


### Logic
def update_tiers(
    tiers: Dict[str, List[Dict[str, int]]],
    p1: Dict[str, Union[id.CSSCharacter, bool]],
    p2: Dict[str, Union[id.CSSCharacter, bool]],
) -> Dict[str, List[Dict[str, int]]]:
    global last_results
    if not p1 or not p2:
        return None

    p1_char = tiers["P1"][p1["character"]]
    p2_char = tiers["P2"][p2["character"]]
    p1_rating = p1_char["elo"]
    p2_rating = p2_char["elo"]

    E_p = lambda R_a, R_b: 1.0 / (1.0 + pow(10, ((R_b - R_a) / 400)))
    R_n = lambda R, K, S, E: R + K * (S - E)
    K = lambda m: max(800 / (m + 1), 100)

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
    p1_char["matches"] += 1
    p2_char["matches"] += 1

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

    return tiers


def elo_to_tiers(
    tiers: Dict[str, List[Dict[str, int]]]
) -> Dict[str, List[Dict[str, Union[str, int]]]]:
    tier_list = {}
    for player, characters in tiers.items():
        tier_list[player] = {"S": [], "A": [], "B": [], "C": [], "D": [], "F": []}
        for i, character in enumerate(characters):
            rating = character["elo"]
            tier = ""
            if rating >= 2000:
                tier = "S"
            elif rating >= 1800:
                tier = "A"
            elif rating >= 1600:
                tier = "B"
            elif rating >= 1400:
                tier = "C"
            elif rating >= 1200:
                tier = "D"
            else:
                tier = "F"

            tier_list[player][tier].append(
                {
                    "name": id.CSSCharacter(i).name,
                    "rating": rating,
                    "matches": character["matches"],
                }
            )

    return tier_list


def find_replay_directory() -> Tuple[str, str]:
    base_path = "C:\\Users"
    user_dirs = [
        os.path.join(base_path, user)
        for user in os.listdir(base_path)
        if os.path.isdir(os.path.join(base_path, user))
    ]

    date_pattern = re.compile(r"^\d{4}-(0[1-9]|1[0-2])$")
    latest_date = None
    latest_dir = None

    slippi_dir = ""

    for user_dir in user_dirs:
        slippi_path = os.path.join(user_dir, "Documents", "Slippi")
        if os.path.exists(slippi_path) and os.path.isdir(slippi_path):
            slippi_dir = slippi_path
            for subdir in os.listdir(slippi_path):
                if date_pattern.match(subdir):
                    try:
                        current_date = datetime.strptime(subdir, "%Y-%m")
                        if latest_date is None or current_date > latest_date:
                            latest_date = current_date
                            latest_dir = os.path.join(slippi_path, subdir)
                    except ValueError:
                        pass

    if latest_dir:
        print(f"Found latest directory: {latest_dir}")
    else:
        print("No valid directories found.")

    return slippi_dir, latest_dir


slippi_directory, latest_directory = find_replay_directory()
previous_files = set(os.listdir(latest_directory))


def detect_new_files(directory: str) -> str:
    global previous_files

    current_files = set(os.listdir(directory))
    new_files = current_files - previous_files
    previous_files = current_files

    if new_files:
        for file in new_files:
            return file

    return ""


def parse_replay(
    file_path: str, debug_print: bool = False
) -> Dict[int, Dict[str, Union[id.CSSCharacter, bool]]]:
    try:
        game = read_slippi(file_path, skip_frames=False)
        if debug_print:
            print(game)

        players = {}
        for player in game.start.players:
            if player.type != 0:
                if debug_print:
                    print("Non human player")
                return

            players[player.port.value] = {
                "code": player.netplay.code,
                "character": id.CSSCharacter(player.character),
            }

        # Check that both players are known.
        player_codes = [x["code"] for x in players.values() if x["code"] != ""]
        if (
            (not players_codes["P1"] in player_codes)
            or (not players_codes["P2"] in player_codes)
        ) and len(player_codes) > 0:
            if debug_print:
                print("Unknown player")
            return

        # If zelda or sheik, use the character with more frames.
        for port, player in players.items():
            if (
                player["character"] == id.CSSCharacter.ZELDA
                or player == id.CSSCharacter.SHEIK
            ):
                chars = game.metadata["players"][str(port)]["characters"]
                most_played = ("", 0)
                for c, frames in chars.items():
                    if frames > most_played[1]:
                        most_played = (c, frames)
                player["character"] = id.CSSCharacter[
                    id.InGameCharacter(int(most_played[0])).name
                ]

        # TODO: check if player order is consistant between lists
        data = {
            "stage": game.start.stage,
            "p1_code": game.start.players[0].netplay.code,
            "p1_port": game.start.players[0].port.value,
            "p1_character": players[game.start.players[0].port]["character"],
            "p1_stocks": game.frames.ports[0].leader.post.stocks[-1].as_py(),
            "p2_code": game.start.players[1].netplay.code,
            "p2_port": game.start.players[1].port.value,
            "p2_character": players[game.start.players[1].port]["character"],
            "p2_stocks": game.frames.ports[1].leader.post.stocks[-1].as_py(),
            "end_type": game.end.method.value,
            "lras_initiator": game.end.lras_initiator,
            "p1_won": game.end.players[0].placement == 0,
            "p2_won": game.end.players[1].placement == 0,
            "datetime": game.metadata["startAt"],
            "frames": game.metadata["lastFrame"],
        }

        if debug_print:
            pprint(data)

        return data

    except Exception as e:
        print(f"An error occurred while parsing the file {file_path}: {e}")
        traceback.print_exc()
        return None


def is_file_locked(file_path: str) -> bool:
    try:
        handle = win32file.CreateFile(
            file_path,
            win32file.GENERIC_READ,
            win32file.FILE_SHARE_READ,
            None,
            win32file.OPEN_EXISTING,
            0,
            None,
        )
        win32file.CloseHandle(handle)
        return False
    except Exception:
        return True


def check_lras(entry):
    entry["p1_won"] = (
        True
        if entry["lras_initiator"] != None
        and entry["p1_port"] != entry["lras_initiator"]
        else entry["p1_won"]
    )
    entry["p2_won"] = (
        True
        if entry["lras_initiator"] != None
        and entry["p1_port"] != entry["lras_initiator"]
        else entry["p2_won"]
    )

    return entry


def process_replay(data: Dict, debug_print: bool = False) -> None:
    global character_ratings, games_df
    if not data:
        return

    if data["frames"] / 60 < MIN_GAME_DURATION_SECONDS:
        if debug_print:
            print("Game too short")
        return

    if not ALLOW_EXIT and data["end_type"] == 7:
        if debug_print:
            print("Game exited")
        return

    data = check_lras(data)

    # Map ports from the replays players to tier lists players.
    P1 = None
    if (
        data["p1_code"] == players_codes["P1"]
        or data["p1_port"] == player_ports["P1"] - 1
    ):
        P1 = {"character": id.CSSCharacter(data["p1_character"]), "won": data["p1_won"]}
    else:
        P1 = {"character": id.CSSCharacter(data["p2_character"]), "won": data["p2_won"]}

    P2 = None
    if (
        data["p2_code"] == players_codes["P2"]
        or data["p2_port"] == player_ports["P2"] - 1
    ):
        P2 = {"character": id.CSSCharacter(data["p2_character"]), "won": data["p2_won"]}
    else:
        P2 = {"character": id.CSSCharacter(data["p1_character"]), "won": data["p1_won"]}

    new_tiers = update_tiers(character_ratings, P1, P2)

    if new_tiers:
        character_ratings = new_tiers


def process_new_replay(path: str):
    data = parse_replay(path, True)
    if pd.to_datetime(data["datetime"]) not in games_df.index:
        games_df.loc[pd.to_datetime(data["datetime"])] = data
    process_replay(data, True)


def background_task() -> None:
    print(f"Watching directory: {latest_directory}")
    global character_ratings

    while True:
        new_file = detect_new_files(latest_directory)
        if new_file != "":
            print(f"Found new replay: {new_file}")

            path = os.path.join(latest_directory, new_file)
            while is_file_locked(path):
                eventlet.sleep(0.5)

            process_new_replay(path)

            with open(TIER_FILE, "w") as file:
                json.dump(character_ratings, file)

            socketio.emit("tier_update", elo_to_tiers(character_ratings))
            socketio.emit("results_update", last_results)

        eventlet.sleep(0.5)


def recalculate_database(dir):
    games_df = pd.DataFrame(
        columns=[
            "stage",
            "p1_code",
            "p1_port",
            "p1_character",
            "p1_stocks",
            "p2_code",
            "p2_port",
            "p2_character",
            "p2_stocks",
            "end_type",
            "lras_initiator",
            "p1_won",
            "p2_won",
            "datetime",
            "frames",
        ]
    )
    games_df = games_df.set_index("datetime")

    date_pattern = re.compile(r"^\d{4}-(0[1-9]|1[0-2])$")

    replay_dirs = [
        os.path.join(slippi_directory, month)
        for month in os.listdir(slippi_directory)
        if os.path.isdir(os.path.join(slippi_directory, month))
        and date_pattern.match(month)
    ]

    replay_dirs += EXTRA_DIRS

    for dir in replay_dirs:
        for file in os.listdir(dir):
            try:
                process_new_replay(os.path.join(dir, file))
            except Exception as e:
                print(f"Failed to parse file {file} in reprocessing: {e}")

    games_df.to_pickle("db.pkl")


if __name__ == "__main__":
    # process_replay(
    #     r"C:\Users\Leevi\projects\Python\MeleeEloTierList\test_replays\Game_20241019T013605.slp",
    #     True,
    # )

    # recalculate_database(slippi_directory)

    socketio.start_background_task(target=background_task)
    socketio.run(app, debug=True, use_reloader=False)
