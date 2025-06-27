use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use bincode::{Decode, Encode};
use chrono::{DateTime, Utc};
use fs2::FileExt;
use num_enum::TryFromPrimitive;
use peppi::game::Game;
use peppi::io::slippi;
use regex::Regex;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use walkdir::WalkDir;

use crate::settings;

#[derive(
    Encode,
    Decode,
    Debug,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    TryFromPrimitive,
    ToSchema,
)]
#[repr(u16)]
pub enum Stage {
    Dummy,
    Test,
    FountainOfDreams,
    PokemonStadium,
    PrincessPeachsCastle,
    KongoJungle,
    Brinstar,
    Corneria,
    YoshisStory,
    Onett,
    MuteCity,
    RainbowCruise,
    JungleJapes,
    GreatBay,
    HyruleTemple,
    BrinstarDepths,
    YoshisIsland,
    GreenGreens,
    Fourside,
    MushroomKingdomI,
    MushroomKingdomII,
    Akaneia,
    Venom,
    PokeFloats,
    BigBlue,
    IcicleMountain,
    IceTop,
    FlatZone,
    DreamLand64,
    YoshisIslandN64,
    KongoJungleN64,
    BattleField,
    FinalDestination,
}

#[derive(Encode, Decode, Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub enum CSSCharacter {
    CaptainFalcon,
    DonkeyKong,
    Fox,
    GameAndWatch,
    Kirby,
    Bowser,
    Link,
    Luigi,
    Mario,
    Marth,
    Mewtwo,
    Ness,
    Peach,
    Pikachu,
    IceClimbers,
    Jigglypuff,
    Samus,
    Yoshi,
    Zelda,
    Sheik,
    Falco,
    YoungLink,
    DrMario,
    Roy,
    Pichu,
    Ganondorf,
    Empty,
}

impl From<InGameCharacter> for CSSCharacter {
    fn from(value: InGameCharacter) -> Self {
        match value {
            InGameCharacter::CaptainFalcon => CSSCharacter::CaptainFalcon,
            InGameCharacter::DonkeyKong => CSSCharacter::DonkeyKong,
            InGameCharacter::Fox => CSSCharacter::Fox,
            InGameCharacter::GameAndWatch => CSSCharacter::GameAndWatch,
            InGameCharacter::Kirby => CSSCharacter::Kirby,
            InGameCharacter::Bowser => CSSCharacter::Bowser,
            InGameCharacter::Link => CSSCharacter::Link,
            InGameCharacter::Luigi => CSSCharacter::Luigi,
            InGameCharacter::Mario => CSSCharacter::Mario,
            InGameCharacter::Marth => CSSCharacter::Marth,
            InGameCharacter::Mewtwo => CSSCharacter::Mewtwo,
            InGameCharacter::Ness => CSSCharacter::Ness,
            InGameCharacter::Peach => CSSCharacter::Peach,
            InGameCharacter::Pikachu => CSSCharacter::Pikachu,
            InGameCharacter::Popo | InGameCharacter::Nana => CSSCharacter::IceClimbers,
            InGameCharacter::Jigglypuff => CSSCharacter::Jigglypuff,
            InGameCharacter::Samus => CSSCharacter::Samus,
            InGameCharacter::Yoshi => CSSCharacter::Yoshi,
            InGameCharacter::Zelda => CSSCharacter::Zelda,
            InGameCharacter::Sheik => CSSCharacter::Sheik,
            InGameCharacter::Falco => CSSCharacter::Falco,
            InGameCharacter::YoungLink => CSSCharacter::YoungLink,
            InGameCharacter::DrMario => CSSCharacter::DrMario,
            InGameCharacter::Roy => CSSCharacter::Roy,
            InGameCharacter::Pichu => CSSCharacter::Pichu,
            InGameCharacter::Ganondorf => CSSCharacter::Ganondorf,
        }
    }
}

#[derive(Encode, Decode, Debug, Clone, TryFromPrimitive)]
#[repr(i32)]
pub enum InGameCharacter {
    Mario,
    Fox,
    CaptainFalcon,
    DonkeyKong,
    Kirby,
    Bowser,
    Link,
    Sheik,
    Ness,
    Peach,
    Popo,
    Nana,
    Pikachu,
    Samus,
    Yoshi,
    Jigglypuff,
    Mewtwo,
    Luigi,
    Marth,
    Zelda,
    YoungLink,
    DrMario,
    Falco,
    Pichu,
    GameAndWatch,
    Ganondorf,
    Roy,
}

// impl InGameCharacter {
//     pub fn from<i32>(value: i32) -> Option<Self> {}
// }

#[derive(Encode, Decode, Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Match {
    hash: String,
    pub datetime: i64,
    pub frames: usize,
    pub stage: Stage,
    pub players: (PlayerInfo, PlayerInfo),
    pub is_online: bool,
    pub end_type: u8,
    pub lras_initiator: Option<u8>,
    pub ignore: bool,
}

impl Default for Match {
    fn default() -> Self {
        Match {
            hash: String::new(),
            datetime: Utc::now().timestamp(),
            frames: 0,
            stage: Stage::Dummy,
            players: (PlayerInfo::default(), PlayerInfo::default()),
            is_online: false,
            end_type: peppi::game::EndMethod::Unresolved as u8,
            lras_initiator: None,
            ignore: true,
        }
    }
}

#[derive(Encode, Decode, Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PlayerInfo {
    pub code: String,
    port: u8,
    pub character: CSSCharacter,
    pub stocks: u8,
    pub won: bool,
}

impl Default for PlayerInfo {
    fn default() -> Self {
        PlayerInfo {
            code: String::new(),
            port: 0,
            character: CSSCharacter::Empty,
            stocks: 0,
            won: false,
        }
    }
}

pub fn read_replay(file_path: &str) -> Result<peppi::game::immutable::Game> {
    let file = fs::File::open(file_path)
        .with_context(|| format!("Failed to open file '{}'", file_path))?;

    slippi::read(
        &mut io::BufReader::new(file),
        Some(&peppi::io::slippi::de::Opts {
            skip_frames: false,
            compute_hash: true,
            debug: None,
        }),
    )
    .with_context(|| format!("Failed to parse '{}'", file_path))
}

pub fn parse_replay(game: peppi::game::immutable::Game) -> Result<Match> {
    let metadata = game
        .metadata()
        .as_ref()
        .context("Replay metadata is missing")?;

    let hash = game
        .hash
        .as_ref()
        .context("Game hash is missing")?
        .to_string();

    let datetime = metadata
        .get("startAt")
        .and_then(|v| v.as_str())
        .context("startAt is missing or not a string in replay metadata")?
        .parse::<DateTime<Utc>>()
        .context("Failed to parse startAt time")?
        .timestamp();

    let is_online = game
        .start()
        .r#match
        .as_ref()
        .is_some_and(|m| !m.id.is_empty());

    let game_end = game.end().as_ref().context("Match end data is missing")?;

    let r_presses = count_r_presses(&game).context("Failed to count R presses")?;

    let max_index = r_presses
        .iter()
        .enumerate()
        .max_by_key(|&(_, val)| val)
        .map(|(idx, _)| idx)
        .context("R press vector is empty")?;

    let players = game
        .start
        .players
        .iter()
        .enumerate()
        .map(|(i, player)| -> Result<PlayerInfo> {
            if player.r#type != peppi::game::PlayerType::Human {
                bail!("Replay has a non-human player");
            }

            let netplay_code = if is_online {
                let code = player
                    .netplay
                    .as_ref()
                    .context("Player netplay info is missing")?
                    .code
                    .as_str();

                if !settings::match_player_code(code) {
                    bail!("Player with netplay code '{}' is not recognized", code);
                }
                code
            } else {
                settings::r_presser(i == max_index)
            };

            let character = metadata
                .get("players")
                .and_then(|p| p.as_object())
                .and_then(|obj| obj.get(&(player.port as u8).to_string()))
                .and_then(|p| p.get("characters"))
                .and_then(|c| c.as_object())
                .context("Missing or invalid player/character metadata")?
                .iter()
                .max_by_key(|&(_, v)| v.as_i64().unwrap_or(0))
                .context("No characters found")?
                .0
                .parse::<i32>()
                .context("Failed to parse character")?;

            let stocks = game
                .frame(game.len() - 1)
                .ports
                .get(i)
                .context("Missing port data")?
                .leader
                .post
                .stocks;

            let placement = game_end
                .players
                .as_ref()
                .context("Players data is missing in game end")?
                .get(i)
                .with_context(|| format!("Missing player data for index {}", i))?
                .placement;

            Ok(PlayerInfo {
                code: netplay_code.to_string(),
                port: player.port as u8,
                character: CSSCharacter::from(
                    InGameCharacter::try_from(character)
                        .context("Invalid InGameCharacter value")?,
                ),
                stocks,
                won: placement == 0,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let players_tuple: (PlayerInfo, PlayerInfo) = match &players[..] {
        [a, b] => {
            if settings::is_player1(&a.code) {
                (a.clone(), b.clone())
            } else {
                (b.clone(), a.clone())
            }
        }
        _ => bail!("Vector length is not 2"),
    };

    Ok(Match {
        hash,
        stage: Stage::try_from(game.start().stage)?,
        datetime,
        players: players_tuple,
        end_type: game_end.method as u8,
        lras_initiator: game_end.lras_initiator.flatten().map(|port| port as u8),
        frames: game.len(),
        ignore: false,
        is_online,
    })
}

fn count_r_presses(game: &peppi::game::immutable::Game) -> Result<Vec<i32>> {
    if game.len() == 0 {
        return Err(anyhow!("Game has no frames".to_string()));
    }

    let mut counts = vec![0; 2];

    for frame_idx in 0..game.len() {
        let frame = game.frame(frame_idx);

        for (port_idx, port_data) in frame.ports.iter().enumerate() {
            let pressed = (port_data.leader.pre.buttons_physical >> 5) & 1;
            counts[port_idx] += pressed as i32;
        }
    }

    Ok(counts)
}

pub fn find_slippi_directory() -> Option<PathBuf> {
    let base_path = Path::new("C:\\Users");

    let user_dirs: Vec<PathBuf> = fs::read_dir(base_path)
        .unwrap_or_else(|_| panic!("Cannot read directory: {:?}", base_path))
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.is_dir() { Some(path) } else { None }
        })
        .collect();

    for user_dir in user_dirs {
        let slippi_path = user_dir.join("Documents").join("Slippi");
        if slippi_path.exists() && slippi_path.is_dir() {
            return Some(slippi_path);
        }
    }

    None
}

pub fn find_replay_directory() -> Option<PathBuf> {
    let date_pattern = Regex::new(r"^\d{4}-\d{2}$").unwrap();
    let mut latest_dir = PathBuf::new();

    let slippi_path = find_slippi_directory()?;

    if let Ok(entries) = fs::read_dir(&slippi_path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let subdir_osstr = entry.file_name();
            let subdir_name_lossy = subdir_osstr.to_string_lossy();
            let subdir_name = subdir_name_lossy.as_ref();

            if date_pattern.is_match(subdir_name) {
                latest_dir = slippi_path.join(subdir_name);
            }
        }
    } else {
        eprintln!("Failed to read directory: {:?}", slippi_path);
    }

    latest_dir.to_str()?;

    Some(latest_dir)
}

pub fn detect_new_files(games_set: &HashSet<String>, directory: &PathBuf) -> Option<String> {
    if let Ok(entries) = fs::read_dir(directory) {
        for entry in entries.filter_map(|e| e.ok()) {
            let filename = entry.file_name().to_string_lossy().to_string();

            let file_path = directory.join(&filename).to_str().unwrap_or("").to_string();

            if !games_set.contains(&file_path)
                && !file_path.is_empty()
                && !is_file_locked(&file_path)
            {
                return Some(file_path);
            }
        }
    }

    None
}

pub fn find_slp_files(directory: &str) -> Vec<String> {
    WalkDir::new(directory)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "slp"))
        .map(|e| e.path().to_string_lossy().to_string())
        .collect()
}

pub fn is_file_locked<P: AsRef<Path>>(file_path: P) -> bool {
    let file = match OpenOptions::new().read(true).write(true).open(file_path) {
        Ok(f) => f,
        Err(_) => return true,
    };

    match file.try_lock_exclusive() {
        Ok(_) => {
            let _ = fs2::FileExt::unlock(&file);
            false
        }
        Err(_) => true,
    }
}
