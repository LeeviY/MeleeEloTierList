import argparse
import os
import re

import pandas as pd

import database
import settings
from utils.files import find_slippi_replay_directory, parse_replay


def recalculate_database(overwrite: bool = False):
    if not overwrite and os.path.exists("db.pkl"):
        games_df = pd.read_pickle("db.pkl")
    else:
        games_df = pd.DataFrame(columns=database.columns)
        games_df = games_df.set_index("datetime")

    date_pattern = re.compile(r"^\d{4}-(0[1-9]|1[0-2])$")

    slippi_directory = find_slippi_replay_directory()

    replay_dirs = [
        os.path.join(slippi_directory, month)
        for month in os.listdir(slippi_directory)
        if os.path.isdir(os.path.join(slippi_directory, month))
        and date_pattern.match(month)
    ]

    replay_dirs += settings.EXTRA_DIRS

    for dir in replay_dirs:
        for file in os.listdir(dir):
            try:
                data = parse_replay(os.path.join(dir, file))
                if pd.to_datetime(data["datetime"]) not in games_df.index:
                    games_df.loc[pd.to_datetime(data["datetime"])] = data
            except Exception as e:
                print(f"Failed to parse file {file} in reprocessing: {e}")

    games_df.to_pickle("db.pkl")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Recalculate the game database.")
    parser.add_argument(
        "--overwrite",
        action="store_true",
        help="If set, overwrite the existing database instead of appending to it.",
    )
    args = parser.parse_args()

    recalculate_database(overwrite=args.overwrite)
