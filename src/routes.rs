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
use std::{fs, sync::Arc};
use tokio::sync::{Mutex, mpsc};
use tower_http::services::ServeDir;

use crate::{AppState, LastResult, Matchup, files, glicko::Player, settings};

// Route Handlers
pub async fn index() -> Html<String> {
    Html(
        fs::read_to_string("templates/index.html")
            .unwrap_or_else(|_| "<html><body>File not found</body></html>".to_string()),
    )
}

pub async fn matchup_chart() -> Html<String> {
    Html(
        fs::read_to_string("templates/matchup.html")
            .unwrap_or_else(|_| "<html><body>File not found</body></html>".to_string()),
    )
}

pub async fn stats() -> Html<String> {
    Html(
        fs::read_to_string("templates/stats.html")
            .unwrap_or_else(|_| "<html><body>File not found</body></html>".to_string()),
    )
}

pub async fn get_character_ratings(
    State(state): State<Arc<Mutex<AppState>>>,
) -> Json<serde_json::Value> {
    let character_ratings = state.lock().await.get_character_ratings_data();
    Json(json!({
        "P1": character_ratings.0,
        "P2": character_ratings.1,
    }))
}
//

pub fn create_tier_update_message(character_ratings: (Vec<Player>, Vec<Player>)) -> String {
    serde_json::json!({
        "event": "tier_update",
        "data": {"P1": character_ratings.0, "P2": character_ratings.1}
    })
    .to_string()
}

pub fn create_results_update_message(last_results: (LastResult, LastResult)) -> String {
    serde_json::json!({
        "event": "results_update",
        "data": serde_json::json!({"P1": last_results.0, "P2": last_results.1})
    })
    .to_string()
}

pub fn create_matchup_update_message(
    matchup_chart: Vec<Vec<Matchup>>,
    last_match: Option<files::Match>,
) -> String {
    let winner = if let Some(m) = last_match {
        if m.players.0.won {
            settings::P1
        } else {
            settings::P2
        }
    } else {
        ""
    };

    serde_json::json!({
        "event": "matchup_update",
        "data": {
            "matchups": matchup_chart,
            "winner": winner,
        },
    })
    .to_string()
}

pub async fn handle_socket(mut socket: WebSocket, state: Arc<Mutex<AppState>>) {
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    let mut state_guard = state.lock().await;
    state_guard.clients.push(tx.clone());
    let character_ratings = state_guard.get_character_ratings_data();
    let matchup_chart = state_guard.get_matchup_update_data();
    let last_match = state_guard.last_match.clone();
    drop(state_guard);

    let _ = socket
        .send(Message::Text(create_tier_update_message(character_ratings)))
        .await;

    let _ = socket
        .send(Message::Text(create_matchup_update_message(
            matchup_chart,
            last_match,
        )))
        .await;

    let (mut sender, mut receiver) = socket.split();
    let write_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(msg)) = receiver.next().await {
        if matches!(msg, Message::Close(_)) {
            break;
        }
    }

    write_task.abort();
}

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<Mutex<AppState>>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

pub fn create_router(state: Arc<Mutex<AppState>>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/ws", get(websocket_handler))
        .route("/matchup_chart", get(matchup_chart))
        .route("/stats", get(stats))
        .route("/character_ratings", get(get_character_ratings))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state)
}
