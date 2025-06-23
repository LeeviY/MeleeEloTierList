mod files;
mod settings;

use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, Json},
    routing::{get, post},
};
use chrono::{DateTime, NaiveDate, Utc};
use peppi::io::slippi::read;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    sync::MutexGuard,
};
use std::{fs, io};
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep};
use tower_http::services::ServeDir;
use uuid::Uuid;

use crate::files::{find_slippi_directory, parse_replay};

#[derive(Debug)]
pub struct AppState {
    matches: Arc<tokio::sync::Mutex<HashMap<String, files::Match>>>,
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            matches: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }
}

fn batch_process_replays(
    replay_dir: &str,
    matches: &mut HashMap<String, files::Match>,
) -> Result<(), String> {
    let slp_files = files::find_slp_files(replay_dir);

    let total = slp_files.len().max(1);
    let step = (total / 100).max(1);

    for (i, file) in slp_files.iter().enumerate() {
        process_replay(file.to_string(), matches);

        if i % step == 0 {
            let percent = (i * 100) / total;
            println!("Progress: {}%", percent);
            println!("{:#?}", matches);
        }
    }

    Ok(())
}

fn process_replay(
    new_replay_file: String,
    matches: &mut HashMap<String, files::Match>,
) -> Result<(), String> {
    let new_replay = files::read_replay(&new_replay_file)
        .map_err(|e| format!("Error reading replay '{}': {}", new_replay_file, e))?;

    match files::parse_replay(new_replay) {
        Ok(r#match) => {
            matches.insert(new_replay_file, r#match);
        }
        Err(err) => {
            matches.insert(new_replay_file.clone(), files::Match::default());
            return Err(format!(
                "Error parsing replay '{}': {}",
                new_replay_file, err
            ));
        }
    }

    Ok(())
}

async fn background_task(app_state: AppState) -> Result<(), String> {
    println!("Starting background task...");

    let mut counter = 0;
    let spinner = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

    loop {
        // print!(
        //     "\rWatching for new replays... {}",
        //     spinner[counter % spinner.len()]
        // );
        // use std::io::{self, Write};
        // io::stdout().flush().unwrap();
        // counter += 1;
        // println!("{:#?}", app_state);

        let latest_directory =
            files::find_replay_directory().ok_or("Failed to find latest replay directory")?;
        let mut matches = app_state.matches.lock().await;

        let seen_replays: std::collections::HashSet<String> = matches.keys().cloned().collect();

        if let Some(new_replay_file) = files::detect_new_files(&seen_replays, &latest_directory) {
            println!("New replay detected: {}", new_replay_file);

            process_replay(new_replay_file.clone(), &mut matches);
        }

        // drop(matches);

        sleep(Duration::from_millis(500)).await;
    }
}

#[tokio::main]
async fn main() {
    // parse_replay("test_replays/Game_20241019T013605.slp", true).unwrap();
    // println!(
    //     "{:#?}",
    //     parse_replay(files::read_replay("test_replays/Game_20250619T235953.slp").unwrap()).unwrap()
    // );
    // println!(
    //     "{:#?}",
    //     parse_replay(files::read_replay("test_replays/Game_20250202T203150.slp").unwrap()).unwrap()
    // );
    // println!(
    //     "{:#?}",
    //     parse_replay(files::read_replay("test_replays/Game_20241208T180113.slp").unwrap()).unwrap()
    // );

    let replay_directory = files::find_replay_directory().unwrap();
    println!("{:#?}", replay_directory);

    // println!(
    //     "{:#?}",
    //     files::find_slp_files(find_slippi_directory().unwrap().to_str().unwrap())
    // );

    let state = AppState::new();

    {
        let mut matches = state.matches.lock().await;

        batch_process_replays(
            find_slippi_directory().unwrap().to_str().unwrap(),
            &mut matches,
        );
    }

    tokio::spawn(async move {
        background_task(state).await;
    });

    loop {}

    // let app = Router::new()
    //     .nest_service("/static", ServeDir::new("static"))
    //     .with_state(state);

    // let listener = tokio::net::TcpListener::bind("127.0.0.1:5000")
    //     .await
    //     .unwrap();

    // println!("Server running on http://127.0.0.1:5000");
    // axum::serve(listener, app).await.unwrap();
}
