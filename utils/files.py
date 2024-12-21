import os
import re
import traceback
from datetime import datetime
from pprint import pprint
from typing import Dict, Union

import pandas as pd
import pytz
import win32file
from peppi_py import read_slippi
from slippi import id

import database
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


def date_from_replay_name(filename: str) -> str:
    date = filename.split("_")[1].split(".")[0]
    return (
        datetime.strptime(date, "%Y%m%dT%H%M%S")
        .astimezone(pytz.timezone("Europe/Helsinki"))
        .isoformat()
    )


def detect_new_files(games_df: pd.DataFrame, directory: str) -> str:
    for file in os.listdir(directory):
        if pd.to_datetime(date_from_replay_name(file)) not in games_df.index:
            return file

    return ""


def parse_replay(
    file_path: str, ports: Dict[str, int] = None, debug_print: bool = False
) -> Dict[int, Dict[str, Union[id.CSSCharacter, bool]]]:
    try:
        game = read_slippi(file_path, skip_frames=False)
        if debug_print:
            # print(game)
            pass

        datetime = game.metadata["startAt"]

        # Check and ignore CPU games.
        for player in game.start.players:
            if player.type != 0:
                if debug_print:
                    print("Non human player")
                empty = database.empty.copy()
                empty["datetime"] = datetime
                return empty

        # Check that both players are known if not local game.
        game_player_codes = [
            x.netplay.code for x in game.start.players if x.netplay.code != ""
        ]
        if (
            (not settings.PLAYER_CODES["P1"] in game_player_codes)
            or (not settings.PLAYER_CODES["P2"] in game_player_codes)
        ) and len(game_player_codes) > 0:
            if debug_print:
                print("Unknown player")
            empty = database.empty.copy()
            empty["datetime"] = datetime
            return empty

        # If zelda or sheik, use the character with more frames.
        for player in game.start.players:
            if player.character in {id.CSSCharacter.ZELDA, id.CSSCharacter.SHEIK}:
                chars = game.metadata["players"][str(player.port)]["characters"]
                player.character = id.CSSCharacter[
                    id.InGameCharacter(int(max(chars, key=chars.get))).name
                ].value

        # Notice: py-slippi has indexes depending on port and empty players in rest of the ports,
        #         but peppi-py just lists the non empty players in port order.
        # TODO: check if player order is consistant between lists
        p1_index = 0
        p2_index = 1

        game_p1 = {
            "code": game.start.players[p1_index].netplay.code,
            "port": game.start.players[p1_index].port.value
            + 1,  # replay ports start from 0
            "character": id.CSSCharacter(game.start.players[p1_index].character),
            "stocks": game.frames.ports[p1_index].leader.post.stocks[-1].as_py(),
            "won": game.end.players[p1_index].placement == 0,
        }

        game_p2 = {
            "code": game.start.players[p2_index].netplay.code,
            "port": game.start.players[p2_index].port.value
            + 1,  # replay ports start from 0
            "character": id.CSSCharacter(game.start.players[p2_index].character),
            "stocks": game.frames.ports[p2_index].leader.post.stocks[-1].as_py(),
            "won": game.end.players[p2_index].placement == 0,
        }

        # Map players code based on ports.
        if ports and not game_p1["code"] and not game_p2["code"]:
            if game_p1["port"] == ports["P1"] and game_p2["port"] == ports["P2"]:
                game_p1["code"] = settings.PLAYER_CODES["P1"]
                game_p2["code"] = settings.PLAYER_CODES["P2"]
            elif game_p1["port"] == ports["P2"] and game_p2["port"] == ports["P1"]:
                game_p1["code"] = settings.PLAYER_CODES["P2"]
                game_p2["code"] = settings.PLAYER_CODES["P1"]
            else:
                if debug_print:
                    print("Port mapping defined but no matching player found.")
                empty = database.empty.copy()
                empty["datetime"] = datetime
                return empty

        p1 = game_p1
        p2 = game_p2
        # Swap players so that game_p1 matches database P1.
        if not p1["code"] == settings.PLAYER_CODES["P1"]:
            temp = p1
            p1 = p2
            p2 = temp

        # if p1["won"] == 1.0 or p1["won"] == 0.0 or p2["won"] == 1.0 or p2["won"] == 0.0:
        #     print(p1, p2)

        # replay ports start from 0
        lras_initiator = game.end.lras_initiator
        lras_initiator = lras_initiator + 1 if lras_initiator else lras_initiator

        data = {
            "stage": game.start.stage,
            "p1_code": p1["code"],
            "p1_port": p1["port"],
            "p1_character": p1["character"],
            "p1_stocks": p1["stocks"],
            "p2_code": p2["code"],
            "p2_port": p2["port"],
            "p2_character": p2["character"],
            "p2_stocks": p2["stocks"],
            "end_type": game.end.method.value,
            "lras_initiator": lras_initiator,
            "p1_won": p1["won"],
            "p2_won": p2["won"],
            "datetime": datetime,
            "frames": game.metadata["lastFrame"],
            "ignore": False,
            "type": "netplay" if len(game_player_codes) else "local",
        }

        if debug_print:
            pprint(data)

        return data

    except Exception as e:
        print(f"An error occurred while parsing the file {file_path}: {e}")
        traceback.print_exc()
        empty = database.empty.copy()
        empty["datetime"] = date_from_replay_name(file_path.split("\\")[-1])
        return empty
