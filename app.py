import os
import json
import eventlet
import re

eventlet.monkey_patch()

from datetime import datetime
from typing import List, Tuple
from slippi import Game, id
from flask import Flask, render_template, request, jsonify
from flask_socketio import SocketIO, emit


app = Flask(__name__)
socketio = SocketIO(app)


tiers = {}
with open("tier_list.json", "r") as file:
    tiers = json.load(file)

player_ports = {"P1": 2, "P2": 3}


### Routing
@app.route("/")
def tier_list():
    global tiers
    return render_template("index.html", tiers=elo_to_tiers(tiers))


@app.route("/reset", methods=["POST"])
def reset_tier_list():
    global tiers
    with open("tier_list.json.base", "r") as file:
        tiers = json.load(file)

    with open("tier_list.json", "w") as file:
        json.dump(tiers, file)


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
def update_tiers(p1: Tuple[id.CSSCharacter, int], p2: Tuple[id.CSSCharacter, int]):
    global tiers, socketio
    p1_character, p1_stocks = p1
    p2_character, p2_stocks = p2
    p1_rating = tiers["P1"][p1_character]
    p2_rating = tiers["P2"][p2_character]
    p1_expected = 1.0 / (1.0 + pow(10, ((p1_rating - p2_rating) / 400)))
    p2_expected = 1.0 / (1.0 + pow(10, ((p2_rating - p1_rating) / 400)))

    if p1_stocks - p2_stocks == 0:
        return

    tiers["P1"][p1_character] = p1_rating + 100 * (
        int(p1_stocks - p2_stocks > 0) - p1_expected
    )
    tiers["P2"][p2_character] = p2_rating + 100 * (
        int(p2_stocks - p1_stocks > 0) - p2_expected
    )

    with open("tier_list.json", "w") as file:
        json.dump(tiers, file)

    print(tiers)
    socketio.emit("tier_update", elo_to_tiers(tiers))


def elo_to_tiers(tiers):
    tier_list = {}
    for player, characters in tiers.items():
        tier_list[player] = {"S": [], "A": [], "B": [], "C": [], "D": [], "F": []}
        for i, rating in enumerate(characters):
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

    print(tier_list)
    return tier_list


def find_replay_directory():
    # return "C:\\Users\\Lahela\\Documents\\Slippi\\test"
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
        game = Game(file_path)

        print(game)
        if game.metadata.duration / 60 < 30:
            print("Game too short")
            return None

        chars = [p.character if p else None for p in game.start.players]
        # If zelda or sheik get the character from the last frame.
        for i, p in enumerate(chars):
            if p == id.CSSCharacter.ZELDA or p == id.CSSCharacter.SHEIK:
                chars[i] = id.CSSCharacter[
                    game.frames[-1].ports[i].leader.post.character.name
                ]

        stocks = [100] * 4
        for frame in game.frames:
            for i, port in enumerate(frame.ports):
                if port:
                    stocks[i] = min(stocks[i], port.leader.post.stocks)

        return list(zip(chars, stocks))

    except Exception as e:
        print(f"An error occurred while parsing the file: {e}")
        return None


def background_task():
    print(f"Watching directory: {slippi_directory}")
    while True:
        new_file = detect_new_files(slippi_directory)
        if new_file != "":
            print(f"Found new replay: {new_file}")
            result = parse_replay(os.path.join(slippi_directory, new_file))
            if result:
                update_tiers(
                    result[player_ports["P1"] - 1], result[player_ports["P2"] - 1]
                )

        eventlet.sleep(0.1)


if __name__ == "__main__":
    socketio.start_background_task(target=background_task)
    socketio.run(app, debug=True, use_reloader=False)
