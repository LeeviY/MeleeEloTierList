import os
import json
import re
import eventlet

eventlet.monkey_patch()

from datetime import datetime
from typing import List, Tuple, Dict
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

tiers = {}
with open(TIER_FILE, "r") as file:
    tiers = json.load(file)

player_ports = {"P1": 2, "P2": 4}


### Routing
@app.route("/")
def tier_list():
    global tiers
    return render_template("index.html", tiers=elo_to_tiers(tiers))


@app.route("/reset", methods=["POST"])
def reset_tier_list():
    global tiers
    with open(TIER_FILE_BASE, "r") as file:
        tiers = json.load(file)

    with open(TIER_FILE, "w") as file:
        json.dump(tiers, file)

    return jsonify(elo_to_tiers(tiers)), 200


@app.route("/recalculate", methods=["POST"])
def recalculate_tier_list():
    global tiers
    with open(TIER_FILE_BASE, "r") as file:
        tiers = json.load(file)

    for file in set(os.listdir(slippi_directory)):
        try:
            result = parse_replay(os.path.join(slippi_directory, file))
            if not result:
                continue
            # TODO: fixme
            update_tiers(
                result[player_ports["P1"] - 1],
                result[player_ports["P2"] - 1],
            )
        except Exception as e:
            print(f"Failed to parse file {file} in recalculation: {e}")

    with app.app_context():
        return jsonify(elo_to_tiers(tiers)), 200


@app.route("/port", methods=["GET"])
def get_port():
    global player_ports
    print(player_ports)
    return jsonify(player_ports), 200


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


@socketio.on("connect")
def handle_connect():
    emit("tier_update", elo_to_tiers(tiers))


### Logic
def update_tiers(p1: dict[id.CSSCharacter, bool], p2: dict[id.CSSCharacter, bool]):
    global tiers, socketio
    p1_char = tiers["P1"][p1["character"]]
    p2_char = tiers["P2"][p2["character"]]
    p1_rating = p1_char["elo"]
    p2_rating = p2_char["elo"]

    E_p = lambda R_a, R_b: 1.0 / (1.0 + pow(10, ((R_b - R_a) / 400)))
    R_n = lambda R, K, S, E: R + K * (S - E)
    K = lambda m: max(1000 / 1.25**m, 100)

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


def elo_to_tiers(tiers):
    tier_list = {}
    for player, characters in tiers.items():
        tier_list[player] = {"S": [], "A": [], "B": [], "C": [], "D": [], "F": []}
        for i, character in enumerate(characters):
            rating = character["elo"]
            tier = ""
            if rating >= 2500:
                tier = "S"
            elif rating >= 2000:
                tier = "A"
            elif rating >= 1500:
                tier = "B"
            elif rating >= 1000:
                tier = "C"
            elif rating >= 500:
                tier = "D"
            else:
                tier = "F"

            tier_list[player][tier].append(
                {"name": id.CSSCharacter(i).name, "rating": rating}
            )

    return tier_list


def find_replay_directory():
    base_path = "C:\\Users"
    user_dirs = [
        os.path.join(base_path, user)
        for user in os.listdir(base_path)
        if os.path.isdir(os.path.join(base_path, user))
    ]

    date_pattern = re.compile(r"^\d{4}-(0[1-9]|1[0-2])$")
    latest_date = None
    latest_dir = None

    for user_dir in user_dirs:
        slippi_path = os.path.join(user_dir, "Documents", "Slippi")
        if os.path.exists(slippi_path) and os.path.isdir(slippi_path):
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

    return latest_dir


slippi_directory = find_replay_directory()
previous_files = set(os.listdir(slippi_directory))


def detect_new_files(directory) -> str:
    global previous_files

    current_files = set(os.listdir(directory))
    new_files = current_files - previous_files
    previous_files = current_files

    if new_files:
        for file in new_files:
            return file

    return ""


def parse_replay(file_path: str) -> List[Tuple[id.CSSCharacter, int]]:
    try:
        game = read_slippi(file_path, skip_frames=True)
        print(game)
        if game.metadata["lastFrame"] / 60 < MIN_GAME_DURATION_SECONDS:
            raise Exception("Game too short")

        players = {}
        for player in game.start.players:
            if player.type != 0:
                raise Exception("Non human player")

            players[player.port.value] = {
                "character": id.CSSCharacter(player.character),
            }

        # If zelda or sheik get the character from the last frame.
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

        # Quitter loses
        for player in game.end.players:
            if game.end.lras_initiator != None:
                players[player.port]["won"] = player.port != game.end.lras_initiator
            else:
                players[player.port]["won"] = player.placement == 0

        return players

    except Exception as e:
        print(f"An error occurred while parsing the file: {e}")
        return None


def is_file_locked(file_path):
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


def background_task():
    print(f"Watching directory: {slippi_directory}")
    while True:
        new_file = detect_new_files(slippi_directory)
        if new_file != "":
            print(f"Found new replay: {new_file}")
            path = os.path.join(slippi_directory, new_file)
            while is_file_locked(path):
                eventlet.sleep(0.5)
            result = parse_replay(path)
            if not result:
                continue
            # TODO: fixme
            update_tiers(
                result[player_ports["P1"] - 1],
                result[player_ports["P2"] - 1],
            )
            with open(TIER_FILE, "w") as file:
                json.dump(tiers, file)

            socketio.emit("tier_update", elo_to_tiers(tiers))

        eventlet.sleep(0.5)


if __name__ == "__main__":
    recalculate_tier_list()
    socketio.start_background_task(target=background_task)
    socketio.run(app, debug=True, use_reloader=False)
