mod db;
mod files;
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
use futures::{SinkExt, StreamExt};
use serde_json::json;
use std::io::{self, Write};
use std::{collections::HashMap, sync::Arc};
use std::{fs, process::exit};
use tokio::{
    sync::Mutex,
    time::{Duration, sleep},
};
use tower_http::services::ServeDir;

#[derive(Debug, Clone)]
pub struct AppState {
    pub matches: Arc<Mutex<HashMap<String, files::Match>>>,
}

impl AppState {
    pub fn new(matches: Arc<Mutex<HashMap<String, files::Match>>>) -> Self {
        AppState { matches }
    }
}

// Route Handlers
async fn index() -> Html<String> {
    let html = fs::read_to_string("templates/index.html")
        .unwrap_or_else(|_| "<html><body>File not found</body></html>".to_string());

    Html(html)
}

async fn matchup_chart() -> Html<&'static str> {
    Html("<html><body>Matchup Chart Placeholder</body></html>")
}

async fn stats() -> Html<&'static str> {
    Html("<html><body>Stats Placeholder</body></html>")
}

async fn get_matchups(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({
        "matchups": [], // Placeholder
    }))
}

async fn get_character_ratings(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({
        "ratings": {

        }, // Placeholder
    }))
}

async fn get_character_ratings_data(state: &AppState) -> String {
    serde_json::json!({
        "event": "tier_update",
        "data": {
            "P1":{},
            "P2":{},
        }
    })
    .to_string()
}

async fn get_last_results_data(state: &AppState) -> String {
    unimplemented!();
}

async fn get_matchup_update_data(state: &AppState) -> String {
    unimplemented!();
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let character_ratings = get_character_ratings_data(&state).await;
    // let last_results = get_last_results_data(&state).await;
    // let matchup_update = get_matchup_update_data(&state).await;

    let _ = socket.send(Message::Text(character_ratings)).await;
    // let _ = socket.send(Message::Text(last_results)).await;
    // let _ = socket.send(Message::Text(matchup_update)).await;

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

    replays::batch_process_replays_threaded(
        files::find_slippi_directory().unwrap().to_str().unwrap(),
        &mut *(state.matches.lock().await),
    );

    println!("{}", state.matches.lock().await.len());

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
