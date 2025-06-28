mod db;
mod files;
mod glicko;
mod replays;
mod routes;
mod settings;

use std::cmp::min;
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::sync::Arc;

use axum::extract::ws::Message;
use chrono::{DateTime, NaiveDate};
use tokio::sync::Mutex;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::{Duration, sleep};

// TODO:
// Add extra dirs section.
// Investigate other database file formats.

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Matchup {
    win_rate: Option<f64>,
    matches: usize,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct LastResult {
    pub character: u8,
    pub rating_diff: f64,
    pub rd_diff: f64,
    pub volatility_diff: f64,
    pub win_probability: f64,
}

impl LastResult {
    fn new(
        character: usize,
        old_player: &glicko::Player,
        new_player: &glicko::Player,
        old_opponent: &glicko::Player,
    ) -> Self {
        Self {
            character: character as u8,
            rating_diff: new_player.rating - old_player.rating,
            rd_diff: new_player.rd - old_player.rd,
            volatility_diff: new_player.volatility - old_player.volatility,
            win_probability: glicko::win_probability(
                old_player.rating,
                old_opponent.rating,
                old_opponent.rd,
            ),
        }
    }
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

        let tier_msg = Message::Text(routes::create_tier_update_message(new_ratings).into());
        let result_msg = Message::Text(routes::create_results_update_message(last_result).into());
        let matchup_msg = Message::Text(
            routes::create_matchup_update_message(new_matchups, self.last_match.clone()).into(),
        );

        self.clients.retain(|client| {
            client.send(tier_msg.clone()).is_ok()
                && client.send(result_msg.clone()).is_ok()
                && client.send(matchup_msg.clone()).is_ok()
        });
    }

    pub fn get_values_filtered(&self) -> Vec<files::Match> {
        self.matches
            .values()
            .filter(|m| {
                !m.ignore
                    && m.frames > settings::MIN_FRAMES
                    && !matches!(
                        m.end_method,
                        files::EndMethod::Unresolved | files::EndMethod::NoContest
                    )
            })
            .cloned()
            .collect()
    }

    fn update_character_ratings(
        character_ratings: &(Vec<glicko::Player>, Vec<glicko::Player>),
        matches: &Vec<files::Match>,
    ) -> (Vec<glicko::Player>, Vec<glicko::Player>) {
        let mut games_per_character = (
            vec![Vec::new(); files::CHARACTER_COUNT],
            vec![Vec::new(); files::CHARACTER_COUNT],
        );

        for m in matches {
            let (p0_char, p1_char) = (
                m.players.0.character as usize,
                m.players.1.character as usize,
            );

            games_per_character.0[p0_char].push(glicko::Opponent {
                rating: character_ratings.1[p1_char].rating,
                rd: character_ratings.1[p1_char].rd,
                score: if m.players.0.won { 1.0 } else { 0.0 },
            });

            games_per_character.1[p1_char].push(glicko::Opponent {
                rating: character_ratings.0[p0_char].rating,
                rd: character_ratings.0[p0_char].rd,
                score: if m.players.1.won { 1.0 } else { 0.0 },
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
            files::CHARACTER_COUNT
        ];
        let mut character_ratings = (default_ratings.clone(), default_ratings);

        let grouped_matches = self
            .get_values_filtered()
            .into_iter()
            .map(|m| {
                let date = DateTime::from_timestamp(m.datetime, 0)
                    .unwrap()
                    .format("%Y-%m-%d")
                    .to_string();
                (date, m)
            })
            .fold(
                HashMap::<String, Vec<files::Match>>::new(),
                |mut acc, (date, match_data)| {
                    acc.entry(date).or_default().push(match_data);
                    acc
                },
            );

        let dates = grouped_matches
            .keys()
            .filter_map(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

        let Some((start_date, end_date)) = dates.clone().min().zip(dates.max()) else {
            println!("Grouped map is empty or contains no valid dates.");
            return character_ratings;
        };

        let date_iter = std::iter::successors(Some(start_date), |d| {
            let next = *d + chrono::Duration::days(1);
            (next <= end_date).then_some(next)
        });

        for date in date_iter {
            let empty_vec = Vec::new();
            let matches = grouped_matches
                .get(&date.format("%Y-%m-%d").to_string())
                .unwrap_or(&empty_vec);

            character_ratings = Self::update_character_ratings(&character_ratings, matches);
        }

        character_ratings
    }

    fn get_matchup_update_data(&self) -> Vec<Vec<Matchup>> {
        let mut matchup_chart =
            vec![vec![Matchup::default(); files::CHARACTER_COUNT]; files::CHARACTER_COUNT];

        let mut matches: Vec<_> = self.get_values_filtered();
        matches.sort_by_key(|m| -m.datetime);

        for m in matches {
            let matchup =
                &mut matchup_chart[m.players.0.character as usize][m.players.1.character as usize];

            if matchup.matches < settings::RATING_WINDOW_SIZE {
                *matchup.win_rate.get_or_insert(0.0) += if m.players.0.won { 1.0 } else { 0.0 };
            }
            matchup.matches += 1;
        }

        matchup_chart
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|mut matchup| {
                        if let Some(total_wins) = matchup.win_rate {
                            matchup.win_rate = Some(
                                total_wins
                                    / min(matchup.matches, settings::RATING_WINDOW_SIZE) as f64,
                            );
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
        let match_players = &self.last_match.as_ref().unwrap().players;
        let p0_character = match_players.0.character as usize;
        let p1_character = match_players.1.character as usize;

        (
            LastResult::new(
                p0_character,
                &self.ratings.0[p0_character],
                &new_ratings.0[p0_character],
                &self.ratings.1[p1_character],
            ),
            LastResult::new(
                p1_character,
                &self.ratings.1[p1_character],
                &new_ratings.1[p1_character],
                &self.ratings.0[p0_character],
            ),
        )
    }
}

async fn background_task(state: Arc<Mutex<AppState>>, write_to_db: bool) {
    let mut counter = 0;

    loop {
        let Some(replay_dir) = files::find_replay_directory() else {
            eprintln!("\rFailed to find latest replay directory");
            return;
        };

        print!(
            "\rWatching for new replays in {} {:<3}",
            replay_dir.to_str().unwrap_or("Unknown"),
            ".".repeat(counter % 4),
        );
        io::stdout().flush().ok();
        counter += 1;

        let mut state_guard = state.lock().await;
        let seen: HashSet<_> = state_guard.matches.keys().cloned().collect();

        // TODO: Get all new files instead of just one and batch process them.
        if let Some(replay_file) = files::detect_new_file(&seen, &replay_dir) {
            println!("\rNew replay detected: {}", replay_file);

            match replays::process_replay(replay_file.clone(), &state_guard.matches) {
                Some(Ok(parsed)) => {
                    println!("{:#?}", parsed);
                    state_guard
                        .matches
                        .insert(replay_file.clone(), parsed.clone());
                    state_guard.last_match = Some(parsed);
                    state_guard.broadcast_updates();

                    if write_to_db {
                        db::write_to_file(&state_guard.matches, settings::DB_FILENAME)
                            .await
                            .unwrap_or_else(|e| println!("Error writing to db: {}", e));
                    }
                }
                Some(Err(e)) => {
                    state_guard
                        .matches
                        .insert(replay_file.clone(), files::Match::default());
                    println!("Error processing replay: {} {}", replay_file, e);
                }
                None => {
                    println!("Skipped replay: {}", replay_file);
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
    let args: Vec<String> = std::env::args().collect();
    let mut write_to_db = true;
    if args.contains(&"nowrite".to_string()) {
        println!("Not writing replays to the database");
        write_to_db = false;
    }

    let state = Arc::new(Mutex::new(AppState::new(
        db::read_from_file("db.bc").await.unwrap_or_else(|err| {
            eprintln!("Failed to load match database: {err}");
            HashMap::new()
        }),
    )));

    let mut state_guard = state.lock().await;
    println!("Matches in db: {:?}", state_guard.matches.len());

    replays::batch_process_replays_threaded(
        files::find_slippi_directory().unwrap().to_str().unwrap(),
        &mut state_guard.matches,
    );
    if write_to_db {
        db::write_to_file(&state_guard.matches, settings::DB_FILENAME)
            .await
            .unwrap_or_else(|e| println!("Error writing to db: {}", e));
    }

    state_guard.ratings = state_guard.get_character_ratings_data();
    drop(state_guard);

    tokio::spawn(background_task(state.clone(), write_to_db));

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
