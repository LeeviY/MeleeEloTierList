import pandas as pd

columns = [
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
    "ignore",
]


def date_exists(datetime, df):
    return pd.to_datetime(datetime) not in df.index
