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
    "type",
]

types = {
    "stage": "Int64",
    "p1_code": "string",
    "p1_port": "Int64",
    "p1_character": "Int64",
    "p1_stocks": "Int64",
    "p2_code": "string",
    "p2_port": "Int64",
    "p2_character": "Int64",
    "p2_stocks": "Int64",
    "end_type": "Int64",
    "lras_initiator": "Int64",
    "p1_won": "boolean",
    "p2_won": "boolean",
    "frames": "Int64",
    "ignore": "bool",
    "type": "string",
    # "datetime": "datetime64[ns]",
}

empty = {
    "stage": -1,
    "p1_code": "",
    "p1_port": -1,
    "p1_character": -1,
    "p1_stocks": -1,
    "p2_code": "",
    "p2_port": -1,
    "p2_character": -1,
    "p2_stocks": -1,
    "end_type": -1,
    "lras_initiator": -1,
    "p1_won": False,
    "p2_won": False,
    "frames": -1,
    "ignore": True,
    "type": "",
}


def date_exists(datetime, df):
    return pd.to_datetime(datetime) not in df.index
