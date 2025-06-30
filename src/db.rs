use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::str::FromStr;

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};

use crate::files::{self, Match, PlayerInfo};

pub async fn write_to_file(data: &HashMap<String, Match>, filename: &str) -> Result<()> {
    let encoded = bincode::encode_to_vec(data, bincode::config::standard())?;
    fs::write(filename, encoded)?;

    Ok(())
}

pub async fn read_from_file(filename: &str) -> Result<HashMap<String, Match>> {
    let encoded = fs::read(filename)?;
    let (matches, _): (HashMap<String, Match>, usize) =
        bincode::decode_from_slice(&encoded, bincode::config::standard())?;

    Ok(matches)
}

pub fn read_matches_from_csv(filename: &str) -> Result<HashMap<String, Match>> {
    let file = File::open(filename)?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    lines.next();
    let mut matches = HashMap::new();

    for line in lines {
        let line = line?;
        let fields: Vec<&str> = line.split(',').collect();

        let get = |i| {
            fields
                .get(i)
                .ok_or_else(|| anyhow!("Missing field {} in CSV line", i))
        };

        if get(15)? == &"True" || get(16)? == &"netplay" {
            continue;
        }

        let m = Match {
            hash: String::new(),
            datetime: DateTime::<Utc>::from_str(get(0)?)?.timestamp(),
            frames: get(14)?.parse()?,
            stage: files::Stage::try_from(get(1)?.parse::<u16>()?)?,
            players: (
                PlayerInfo {
                    code: get(2)?.to_string(),
                    port: get(3)?.parse()?,
                    character: files::CSSCharacter::try_from(get(4)?.parse::<u16>()?)?,
                    stocks: get(5)?.parse()?,
                    won: get(12)? == &"True",
                },
                PlayerInfo {
                    code: get(6)?.to_string(),
                    port: get(7)?.parse()?,
                    character: files::CSSCharacter::try_from(get(8)?.parse::<u16>()?)?,
                    stocks: get(9)?.parse()?,
                    won: get(13)? == &"True",
                },
            ),
            is_online: false,
            end_method: files::EndMethod::try_from(get(10)?.parse::<u8>()?)?,
            lras_initiator: get(11).ok().and_then(|s| s.parse::<u8>().ok()),
            ignore: false,
        };

        let key = format!("csv_{}", get(0)?);
        matches.insert(key, m);
    }

    Ok(matches)
}
