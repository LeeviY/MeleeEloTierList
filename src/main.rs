mod db;
mod files;
mod glicko;
mod replays;
mod routes;
mod settings;

use axum::extract::ws::Message;
use chrono::{DateTime, NaiveDate};
use std::{
    collections::HashMap,
    io::{self, Write},
    sync::Arc,
};
use tokio::{
    sync::{Mutex, mpsc::UnboundedSender},
    time::{Duration, sleep},
};

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Matchup {
    win_rate: Option<f64>,
    matches: i32,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct LastResult {
    pub character: u8,
    pub rating_diff: f64,
    pub rd_diff: f64,
    pub volatility_diff: f64,
    pub win_probability: f64,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub matches: HashMap<String, files::Match>,
    pub last_match: Option<files::Match>,
    pub clients: Vec<UnboundedSender<Message>>,
    pub ratings: (Vec<glicko::Player>, Vec<glicko::Player>),
}

impl AppState {
    pub fn new(matches: HashMap<String, files::Match>) -> Self {
        Self {
            matches,
            last_match: None,
            clients: vec![],
            ratings: (vec![], vec![]),
        }
    }

    pub fn broadcast_updates(&mut self) {
        let start = std::time::Instant::now();

        let new_ratings = self.get_character_ratings_data();
        let last_result = self.get_last_result(&new_ratings);
        let new_matchups = self.get_matchup_update_data();

        self.ratings = new_ratings.clone();

        println!("{:?}", std::time::Instant::now().duration_since(start));

        let tier_msg = Message::Text(routes::create_tier_update_message(new_ratings));
        let result_msg = Message::Text(routes::create_results_update_message(last_result));
        let matchup_msg = Message::Text(routes::create_matchup_update_message(
            new_matchups,
            self.last_match.clone(),
        ));

        self.clients.retain(|client| {
            client.send(tier_msg.clone()).is_ok()
                && client.send(result_msg.clone()).is_ok()
                && client.send(matchup_msg.clone()).is_ok()
        });
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

    fn get_last_result(
        &self,
        new_ratings: &(Vec<glicko::Player>, Vec<glicko::Player>),
    ) -> (LastResult, LastResult) {
        let players = self.last_match.clone().unwrap().players;
        let p0_character = players.0.character as usize;
        let p1_character = players.1.character as usize;

        (
            LastResult {
                character: p0_character as u8,
                rating_diff: new_ratings.0[p0_character].rating
                    - self.ratings.0[p0_character].rating,
                rd_diff: new_ratings.0[p0_character].rd - self.ratings.0[p0_character].rd,
                volatility_diff: new_ratings.0[p0_character].volatility
                    - self.ratings.0[p0_character].volatility,
                win_probability: glicko::win_probability(
                    self.ratings.0[p0_character].rating,
                    self.ratings.1[p1_character].rating,
                    self.ratings.1[p1_character].rd,
                ),
            },
            LastResult {
                character: p1_character as u8,
                rating_diff: new_ratings.1[p1_character].rating
                    - self.ratings.1[p1_character].rating,
                rd_diff: new_ratings.1[p1_character].rd - self.ratings.1[p1_character].rd,
                volatility_diff: new_ratings.1[p1_character].volatility
                    - self.ratings.1[p1_character].volatility,
                win_probability: glicko::win_probability(
                    self.ratings.1[p1_character].rating,
                    self.ratings.0[p0_character].rating,
                    self.ratings.0[p0_character].rd,
                ),
            },
        )
    }
}

async fn background_task(state: Arc<Mutex<AppState>>) {
    let mut counter = 0;
    loop {
        let Some(latest_directory) = files::find_replay_directory() else {
            eprintln!("\rFailed to find latest replay directory");
            return;
        };

        print!(
            "\rWatching for new replays in {} {:<3}",
            latest_directory.to_str().unwrap_or("Unknown"),
            ".".repeat(counter % 4)
        );
        io::stdout().flush().unwrap();
        counter += 1;

        let mut state_guard = state.lock().await;
        let matches = &mut state_guard.matches;
        let seen_replays: std::collections::HashSet<String> = matches.keys().cloned().collect();

        if let Some(new_replay_file) = files::detect_new_files(&seen_replays, &latest_directory) {
            println!("\rNew replay detected: {}", new_replay_file);
            match replays::process_replay(new_replay_file.clone(), matches) {
                Some(Ok(m)) => {
                    println!("{:#?}", m);
                    matches.insert(new_replay_file, m.clone());
                    state_guard.last_match = Some(m);
                    state_guard.broadcast_updates();
                }
                Some(Err(e)) => {
                    matches.insert(new_replay_file.clone(), files::Match::default());
                    println!("Error processing replay: {} {}", new_replay_file, e);
                }
                None => {
                    println!("Skipped replay: {}", new_replay_file);
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

    let state = Arc::new(Mutex::new(AppState::new(
        db::read_from_file("db.bc").await.unwrap_or_else(|err| {
            eprintln!("Failed to load match database: {err}");
            HashMap::new()
        }),
    )));

    println!("{:?}", state.lock().await.matches.len());

    replays::batch_process_replays_threaded(
        files::find_slippi_directory().unwrap().to_str().unwrap(),
        &mut state.lock().await.matches,
    );

    {
        let mut state_guard = state.lock().await;
        state_guard.ratings = state_guard.get_character_ratings_data();
    }

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
