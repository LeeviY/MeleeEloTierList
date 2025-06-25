mod db;
mod files;
mod glicko;
mod replays;
mod settings;

use axum::{
    Router,
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::{Html, IntoResponse, Json},
    routing::get,
};
use chrono::{DateTime, NaiveDate};
use futures::StreamExt;
use serde_json::json;
use std::io::{self, Write};
use std::{collections::HashMap, sync::Arc};
use std::{fs, process::exit};
use tokio::{
    sync::Mutex,
    time::{Duration, sleep},
};
use tower_http::services::ServeDir;

// struct CharacterVec<T> {
//     data: [T; 26],
// }

// impl<T> CharacterVec<T> {
//     fn new(data: [T; 26]) -> Self {
//         Self { data }
//     }

//     fn get(&self, index: usize) -> Option<&T> {
//         self.data.get(index)
//     }

//     pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
//         self.data.get_mut(index)
//     }
// }

// impl<T> Index<usize> for CharacterVec<T> {
//     type Output = T;

//     fn index(&self, index: usize) -> &Self::Output {
//         &self.data[index]
//     }
// }

// impl<T> IndexMut<usize> for CharacterVec<T> {
//     fn index_mut(&mut self, index: usize) -> &mut Self::Output {
//         &mut self.data[index]
//     }
// }

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Matchup {
    win_rate: Option<f64>,
    matches: i32,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub matches: Arc<Mutex<HashMap<String, files::Match>>>,
}

impl AppState {
    pub fn new(matches: Arc<Mutex<HashMap<String, files::Match>>>) -> Self {
        AppState { matches }
    }

    pub async fn filter(self) -> HashMap<String, files::Match> {
        self.matches
            .lock()
            .await
            .iter()
            .filter(|(_, m)| !m.ignore && m.frames / 60 > 30 && m.end_type != 0 && m.end_type != 7)
            .map(|(k, m)| (k.clone(), m.clone()))
            .collect()
    }

    pub async fn filter_values(&self) -> Vec<files::Match> {
        self.matches
            .lock()
            .await
            .iter()
            .filter(|(_, m)| !m.ignore && m.frames / 60 > 30 && m.end_type != 0 && m.end_type != 7)
            .map(|(_, m)| (m.clone()))
            .collect()
    }

    pub async fn order_players(&self) -> Vec<files::Match> {
        self.matches
            .lock()
            .await
            .iter()
            .map(|(_, m)| (m.clone()))
            .collect()
    }
}

// Route Handlers
async fn index() -> Html<String> {
    Html(
        fs::read_to_string("templates/index.html")
            .unwrap_or_else(|_| "<html><body>File not found</body></html>".to_string()),
    )
}

async fn matchup_chart() -> Html<String> {
    Html(
        fs::read_to_string("templates/matchup.html")
            .unwrap_or_else(|_| "<html><body>File not found</body></html>".to_string()),
    )
}

async fn stats() -> Html<String> {
    Html(
        fs::read_to_string("templates/stats.html")
            .unwrap_or_else(|_| "<html><body>File not found</body></html>".to_string()),
    )
}

async fn get_matchups(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({
        "matchups": [], // Placeholder
    }))
}

async fn get_character_ratings(State(state): State<AppState>) -> Json<serde_json::Value> {
    let character_ratings = get_character_ratings_data(&state).await;
    Json(json!({
        "ratings": {
            "P1": character_ratings.0,
            "P2": character_ratings.1,
        },
    }))
}

fn update_character_ratings(
    character_ratings: &(Vec<glicko::Player>, Vec<glicko::Player>),
    matches: &[files::Match],
) -> (Vec<glicko::Player>, Vec<glicko::Player>) {
    let mut games_per_character = (vec![Vec::new(); 26], vec![Vec::new(); 26]);

    for m in matches {
        let (p0_char, p1_char) = (
            m.players.0.character as usize,
            m.players.1.character as usize,
        );

        games_per_character.0[p0_char].push(glicko::Opponent {
            rating: character_ratings.1[p1_char].rating,
            rd: character_ratings.1[p1_char].rd,
            score: m.players.0.won as i32 as f64,
        });

        games_per_character.1[p1_char].push(glicko::Opponent {
            rating: character_ratings.0[p0_char].rating,
            rd: character_ratings.0[p0_char].rd,
            score: m.players.1.won as i32 as f64,
        });
    }

    (
        games_per_character
            .0
            .iter()
            .zip(&character_ratings.0)
            .map(|(results, player)| player.update(results))
            .collect(),
        games_per_character
            .1
            .iter()
            .zip(&character_ratings.1)
            .map(|(results, player)| player.update(results))
            .collect(),
    )
}

async fn get_character_ratings_data(
    state: &AppState,
) -> (Vec<glicko::Player>, Vec<glicko::Player>) {
    let default_ratings = vec![
        glicko::Player {
            rating: 1500.0,
            rd: 350.0,
            volatility: 0.06,
            matches: 0,
        };
        26
    ];
    let mut character_ratings = (default_ratings.clone(), default_ratings.clone());

    let mut grouped: HashMap<String, Vec<files::Match>> = HashMap::new();
    let matches = state.filter_values().await;
    println!("filtered length {}", matches.len());
    for m in matches {
        let naive = DateTime::from_timestamp(m.datetime, 0).unwrap();
        let date = naive.format("%Y-%m-%d").to_string();

        grouped.entry(date).or_default().push(m.clone());
    }

    println!("ranking periods {}", grouped.len());

    let min_date = grouped
        .keys()
        .filter_map(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .min();

    let max_date = grouped
        .keys()
        .filter_map(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .max();

    if let (Some(start), Some(end)) = (min_date, max_date) {
        let mut current = start;
        while current <= end {
            let date_str = current.format("%Y-%m-%d").to_string();
            if let Some(matches) = grouped.get(&date_str) {
                character_ratings = update_character_ratings(&character_ratings, matches);
            } else {
                character_ratings = update_character_ratings(&character_ratings, &Vec::new());
            }

            current += chrono::Duration::days(1);
        }
    } else {
        println!("Grouped map is empty or contains no valid dates.");
    }

    character_ratings
}

async fn get_last_results_data(state: &AppState) -> String {
    unimplemented!();
}

async fn get_matchup_update_data(state: &AppState) -> Vec<Vec<Matchup>> {
    let mut matchup_chart = vec![vec![Matchup::default(); 26]; 26];

    let mut matches: Vec<_> = state.filter_values().await;
    matches.sort_by_key(|m| -m.datetime);

    for m in matches {
        let (p0_char, p1_char) = (
            m.players.0.character as usize,
            m.players.1.character as usize,
        );
        let matchup = &mut matchup_chart[p0_char][p1_char];

        if matchup.matches < 100 {
            matchup.matches += 1;
            matchup.win_rate =
                Some(matchup.win_rate.unwrap_or(0.0) + m.players.0.won as i32 as f64);
        }
    }

    matchup_chart
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|mut matchup| {
                    if let Some(total_wins) = matchup.win_rate {
                        matchup.win_rate = Some(total_wins / matchup.matches as f64);
                    }
                    matchup
                })
                .collect()
        })
        .collect()
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let character_ratings = get_character_ratings_data(&state).await;
    let matchup_chart = get_matchup_update_data(&state).await;

    // println!("{}", character_ratings);
    // let last_results = get_last_results_data(&state).await;
    let _ = socket
        .send(Message::Text(
            serde_json::json!({
                "event": "tier_update",
                "data": {
                    "P1": character_ratings.0,
                    "P2": character_ratings.1
                }
            })
            .to_string(),
        ))
        .await;
    // let _ = socket.send(Message::Text(last_results)).await;
    let _ = socket
        .send(Message::Text(
            serde_json::json!({
                "event": "matchup_update",
                "data": {
                    "matchups": matchup_chart,
                    "winner": "P2", // TODO: fixme
                },
            })
            .to_string(),
        ))
        .await;

    while let Some(Ok(msg)) = socket.next().await {
        if matches!(msg, Message::Close(_)) {
            break;
        }
    }
}

async fn background_task(app_state: AppState) {
    let mut counter = 0;
    loop {
        print!("\rWatching for new replays{:<3}", ".".repeat(counter % 4));
        io::stdout().flush().unwrap();
        counter += 1;

        let Some(latest_directory) = files::find_replay_directory() else {
            eprintln!("\rFailed to find latest replay directory");
            return;
        };

        let mut matches = app_state.matches.lock().await;
        let seen_replays: std::collections::HashSet<String> = matches.keys().cloned().collect();

        if let Some(new_replay_file) = files::detect_new_files(&seen_replays, &latest_directory) {
            println!("\rNew replay detected: {}", new_replay_file);
            replays::process_replay(new_replay_file.clone(), &mut matches)
                .unwrap_or_else(|e| println!("Error processing replay: {} {}", new_replay_file, e));
        }

        sleep(Duration::from_millis(500)).await;
    }
}

fn tests() {
    println!(
        "{:#?}",
        files::parse_replay(files::read_replay("test_replays/Game_20250619T235953.slp").unwrap())
            .unwrap()
    );
    println!(
        "{:#?}",
        files::parse_replay(files::read_replay("test_replays/Game_20250202T203150.slp").unwrap())
            .unwrap()
    );
    println!(
        "{:#?}",
        files::parse_replay(files::read_replay("test_replays/Game_20241208T180113.slp").unwrap())
            .unwrap()
    );

    println!(
        "{:#?}",
        files::parse_replay(files::read_replay("test_replays/Game_20250623T210148.slp").unwrap())
            .unwrap()
    );

    println!(
        "{:#?}",
        files::parse_replay(
            files::read_replay("test_replays/Game_20250623T210148_other.slp").unwrap()
        )
        .unwrap()
    );

    // println!(
    //     "{:#?}",
    //     files::find_slp_files(files::find_slippi_directory().unwrap().to_str().unwrap())
    // );
}

#[tokio::main]
async fn main() {
    // tests();

    let replay_directory = files::find_replay_directory().unwrap();
    println!("{:#?}", replay_directory);

    let state = AppState::new(db::read_from_file("db.bc").await.unwrap_or_else(|err| {
        eprintln!("Failed to load match database: {err}");
        Arc::new(Mutex::new(HashMap::new()))
    }));

    println!("{:?}", state.matches.lock().await.len());

    replays::batch_process_replays(
        files::find_slippi_directory().unwrap().to_str().unwrap(),
        &mut *(state.matches.lock().await),
    );

    db::write_to_file(&state.matches, "db.bc")
        .await
        .unwrap_or_else(|e| println!("Error writing to db: {}", e));

    tokio::spawn(background_task(state.clone()));

    let router = Router::new()
        .route("/", get(index))
        .route("/ws", get(websocket_handler))
        //
        .route("/matchup_chart", get(matchup_chart))
        .route("/stats", get(stats))
        .route("/character_ratings", get(get_character_ratings))
        //
        .nest_service("/static", ServeDir::new("static"))
        //
        .route("/matchups", get(get_matchups))
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:5000")
        .await
        .unwrap();
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}
