mod db;
mod files;
mod glicko;
mod replays;
mod routes;
mod settings;

use chrono::{DateTime, NaiveDate};
use std::io::{self, Write};
use std::{collections::HashMap, sync::Arc};
use tokio::{
    sync::Mutex,
    time::{Duration, sleep},
};

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Matchup {
    win_rate: Option<f64>,
    matches: i32,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub matches: HashMap<String, files::Match>,
    pub last_match: Option<files::Match>,
}

impl AppState {
    pub fn new(matches: HashMap<String, files::Match>) -> Self {
        AppState {
            matches,
            last_match: None,
        }
    }

    pub fn filter_values(&self) -> Vec<files::Match> {
        self.matches
            .values()
            .filter(|m| {
                !m.ignore && m.frames > settings::MIN_FRAMES && !matches!(m.end_type, 0 | 7)
            })
            .cloned()
            .collect()
    }

    fn update_character_ratings(
        character_ratings: &(Vec<glicko::Player>, Vec<glicko::Player>),
        matches: &Vec<files::Match>,
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

    fn get_character_ratings_data(&self) -> (Vec<glicko::Player>, Vec<glicko::Player>) {
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
        let matches = self.filter_values();
        for m in matches {
            let naive = DateTime::from_timestamp(m.datetime, 0).unwrap();
            let date = naive.format("%Y-%m-%d").to_string();

            grouped.entry(date).or_default().push(m.clone());
        }

        let dates: Vec<_> = grouped
            .keys()
            .filter_map(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
            .collect();
        let min_date = dates.iter().min().cloned();
        let max_date = dates.iter().max().cloned();

        if let (Some(start), Some(end)) = (min_date, max_date) {
            let mut date = start;
            while date <= end {
                let date_str = date.format("%Y-%m-%d").to_string();
                if let Some(matches) = grouped.get(&date_str) {
                    character_ratings = Self::update_character_ratings(&character_ratings, matches);
                } else {
                    character_ratings =
                        Self::update_character_ratings(&character_ratings, &Vec::new());
                }

                date += chrono::Duration::days(1);
            }
        } else {
            println!("Grouped map is empty or contains no valid dates.");
        }

        character_ratings
    }

    fn get_matchup_update_data(&self) -> Vec<Vec<Matchup>> {
        let mut matchup_chart = vec![vec![Matchup::default(); 26]; 26];

        let mut matches: Vec<_> = self.filter_values();
        matches.sort_by_key(|m| -m.datetime);

        for m in matches {
            let matchup =
                &mut matchup_chart[m.players.0.character as usize][m.players.1.character as usize];

            if matchup.matches < 100 {
                matchup.matches += 1;
                *matchup.win_rate.get_or_insert(0.0) += m.players.0.won as i32 as f64;
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

    pub fn get_newest_match(&self) -> files::Match {
        self.matches
            .values()
            .max_by_key(|m| m.datetime)
            .cloned()
            .expect("matches empty")
    }
}

async fn background_task(state: Arc<Mutex<AppState>>) {
    let mut counter = 0;
    loop {
        print!("\rWatching for new replays{:<3}", ".".repeat(counter % 4));
        io::stdout().flush().unwrap();
        counter += 1;

        let Some(latest_directory) = files::find_replay_directory() else {
            eprintln!("\rFailed to find latest replay directory");
            return;
        };

        let mut state_guard = state.lock().await;
        let matches = &mut state_guard.matches;
        let seen_replays: std::collections::HashSet<String> = matches.keys().cloned().collect();

        if let Some(new_replay_file) = files::detect_new_files(&seen_replays, &latest_directory) {
            println!("\rNew replay detected: {}", new_replay_file);
            match replays::process_replay(new_replay_file.clone(), matches) {
                Ok(r#match) => {
                    matches.insert(new_replay_file, r#match.clone());
                    state_guard.last_match = Some(r#match);
                }
                Err(e) => {
                    println!("Error processing replay: {} {}", new_replay_file, e);
                }
            }
        }

        drop(state_guard);

        sleep(Duration::from_millis(500)).await;
    }
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
}

#[tokio::main]
async fn main() {
    // tests();

    let replay_directory = files::find_replay_directory().unwrap();
    println!("{:#?}", replay_directory);

    let state = Arc::new(Mutex::new(AppState::new(
        db::read_from_file("db.bc").await.unwrap_or_else(|err| {
            eprintln!("Failed to load match database: {err}");
            HashMap::new()
        }),
    )));

    println!("{:?}", state.lock().await.matches.len());

    replays::batch_process_replays(
        files::find_slippi_directory().unwrap().to_str().unwrap(),
        &mut state.lock().await.matches,
    );

    db::write_to_file(&state.lock().await.matches, "db.bc")
        .await
        .unwrap_or_else(|e| println!("Error writing to db: {}", e));

    tokio::spawn(background_task(state.clone()));

    axum::serve(
        tokio::net::TcpListener::bind("127.0.0.1:5000")
            .await
            .unwrap(),
        routes::create_router(state.clone()),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .unwrap();
}

// fn tests() {
//     println!(
//         "{:#?}",
//         files::parse_replay(files::read_replay("test_replays/Game_20250619T235953.slp").unwrap())
//             .unwrap()
//     );
//     println!(
//         "{:#?}",
//         files::parse_replay(files::read_replay("test_replays/Game_20250202T203150.slp").unwrap())
//             .unwrap()
//     );
//     println!(
//         "{:#?}",
//         files::parse_replay(files::read_replay("test_replays/Game_20241208T180113.slp").unwrap())
//             .unwrap()
//     );

//     println!(
//         "{:#?}",
//         files::parse_replay(files::read_replay("test_replays/Game_20250623T210148.slp").unwrap())
//             .unwrap()
//     );

//     println!(
//         "{:#?}",
//         files::parse_replay(
//             files::read_replay("test_replays/Game_20250623T210148_other.slp").unwrap()
//         )
//         .unwrap()
//     );

//     // println!(
//     //     "{:#?}",
//     //     files::find_slp_files(files::find_slippi_directory().unwrap().to_str().unwrap())
//     // );
// }
