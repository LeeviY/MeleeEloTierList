use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub players: Players,
    pub directory: Directory,
    pub database: Database,
    pub rating: Rating,
    pub debug: Debug,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Players {
    pub p1_id: String,
    pub p2_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Directory {
    pub slippi: String,
    pub extra: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Database {
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Rating {
    pub min_frames: usize,
    pub rating_window: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Debug {
    pub update_db: bool,
}

const CONFIG_FILE: &str = "config.toml";

pub static CONFIG: Lazy<Arc<Config>> = Lazy::new(|| {
    let content = fs::read_to_string(CONFIG_FILE).expect("Failed to read config file");
    let config: Config = toml::from_str(&content).expect("Invalid config format");
    Arc::new(config)
});

pub fn match_player_code(code: &str) -> bool {
    code == CONFIG.players.p1_id || code == CONFIG.players.p2_id
}

pub fn r_presser(is_max: bool) -> &'static str {
    if is_max {
        CONFIG.players.p1_id.as_str()
    } else {
        CONFIG.players.p2_id.as_str()
    }
}

pub fn is_player1(id: &str) -> bool {
    id == CONFIG.players.p1_id.as_str()
}
