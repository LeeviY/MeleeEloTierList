import os
import re
import traceback
from datetime import datetime
from pprint import pprint
from typing import Dict, List, Tuple, Union

import win32file
from peppi_py import read_slippi
from slippi import id

import settings


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


def find_slippi_replay_directory() -> str:
    base_path = "C:\\Users"
    user_dirs = [
        os.path.join(base_path, user)
        for user in os.listdir(base_path)
        if os.path.isdir(os.path.join(base_path, user))
    ]

    slippi_dirs = []
    for user_dir in user_dirs:
        slippi_path = os.path.join(user_dir, "Documents", "Slippi")
        if os.path.exists(slippi_path) and os.path.isdir(slippi_path):
            slippi_dirs.append(slippi_path)

    if len(slippi_dirs) == 0:
        print("No slippi replay directory found.")
        return ""
    elif len(slippi_dirs) > 1:
        print(f"Found multiple slippi replay directories: {slippi_dirs}")

    return slippi_dirs[0]


def find_replay_directory() -> str:
    base_path = "C:\\Users"
    user_dirs = [
        os.path.join(base_path, user)
        for user in os.listdir(base_path)
        if os.path.isdir(os.path.join(base_path, user))
    ]

    date_pattern = re.compile(r"^\d{4}-(0[1-9]|1[0-2])$")
    latest_date = None
    latest_dir = ""

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


previous_files = set(os.listdir(find_replay_directory()))


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
        game_player_codes = [x["code"] for x in players.values() if x["code"] != ""]
        if (
            (not settings.PLAYER_CODES["P1"] in game_player_codes)
            or (not settings.PLAYER_CODES["P2"] in game_player_codes)
        ) and len(game_player_codes) > 0:
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
