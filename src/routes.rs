use axum::{
    Router,
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::{Html, IntoResponse, Json},
    routing::get,
};
use futures::StreamExt;
use serde_json::json;
use std::{fs, sync::Arc};
use tokio::sync::Mutex;
use tower_http::services::ServeDir;

use crate::{AppState, Matchup, files::Match, glicko::Player};

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
        "ratings": {
            "P1": character_ratings.0,
            "P2": character_ratings.1,
        },
    }))
}
//

fn create_tier_update_message(character_ratings: (Vec<Player>, Vec<Player>)) -> String {
    serde_json::json!({
        "event": "tier_update",
        "data": {
            "P1": character_ratings.0,
            "P2": character_ratings.1
        }
    })
    .to_string()
}

fn create_results_update_message(last_match: Option<Match>) -> String {
    serde_json::json!({
        "event": "results_update",
        "data": match last_match {
            Some(m) => serde_json::json!({
                "P1": {
                    "character": m.players.0.character,
                    "won": m.players.0.won
                },
                "P2": {
                    "character": m.players.1.character,
                    "won": m.players.1.won
                }
            }),
            None => serde_json::Value::Null,
        }
    })
    .to_string()
}

fn create_matchup_update_message(matchup_chart: Vec<Vec<Matchup>>) -> String {
    serde_json::json!({
        "event": "matchup_update",
        "data": {
            "matchups": matchup_chart,
            "winner": "P2", // TODO: fixme
        },
    })
    .to_string()
}

pub async fn handle_socket(mut socket: WebSocket, state: Arc<Mutex<AppState>>) {
    let state_guard = state.lock().await;
    let character_ratings = state_guard.get_character_ratings_data();
    let matchup_chart = state_guard.get_matchup_update_data();
    let last_match = state_guard.last_match.clone();
    drop(state_guard);

    let _ = socket
        .send(Message::Text(create_tier_update_message(character_ratings)))
        .await;
    let _ = socket
        .send(Message::Text(create_results_update_message(last_match)))
        .await;
    let _ = socket
        .send(Message::Text(create_matchup_update_message(matchup_chart)))
        .await;

    while let Some(Ok(msg)) = socket.next().await {
        if matches!(msg, Message::Close(_)) {
            break;
        }
    }
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
