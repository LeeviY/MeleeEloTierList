use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use anyhow::{Context, Result};
use rayon::prelude::*;

use crate::files;

#[allow(dead_code)]
pub fn batch_process_replays(replay_dir: &str, matches: &mut HashMap<String, files::Match>) {
    println!("\nProcessing replays...");
    let start = Instant::now();
    let slp_files = files::find_slp_files(replay_dir);

    let total = slp_files.len().max(1);
    let step = (total / 100).max(1);

    let mut skipped = 0;

    for (i, file) in slp_files.iter().enumerate() {
        match process_replay(file.to_string(), matches) {
            Some(Ok(m)) => {
                matches.insert(file.to_string(), m);
            }
            Some(Err(_)) => {
                matches.insert(file.to_string(), files::Match::default());
            }
            None => {
                skipped += 1;
            }
        }

        if i % step == 0 {
            print!("\rProgress: {}%", (i * 100) / total);
            io::stdout().flush().unwrap();
        }
    }

    println!("\rProgress: 100% - Complete!");
    println!(
        "\nProcessed {} (Skipped {:?}) replays in {:?}",
        slp_files.len(),
        skipped,
        start.elapsed()
    );
}

pub fn batch_process_replays_threaded(
    replay_dir: &str,
    matches: &mut HashMap<String, files::Match>,
) {
    println!("Processing replays...");
    let start = Instant::now();
    let slp_files = files::find_slp_files(replay_dir);
    let skipped = AtomicUsize::new(0);

    let combined: HashMap<_, _> = slp_files
        .par_chunks(32)
        .flat_map(|chunk| {
            chunk
                .iter()
                .filter_map(|file| match process_replay(file.to_string(), matches) {
                    Some(Ok(m)) => Some((file.to_string(), m)),
                    Some(Err(_)) => Some((file.to_string(), files::Match::default())),
                    None => {
                        skipped.fetch_add(1, Ordering::Relaxed);
                        None
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect();

    matches.extend(combined);

    println!(
        "Processed {} (Skipped {:?}) replays in {:?}",
        slp_files.len(),
        skipped,
        start.elapsed()
    );
}

pub fn process_replay(
    new_replay_file: String,
    matches: &HashMap<String, files::Match>,
) -> Option<Result<files::Match>> {
    if matches.contains_key(&new_replay_file) {
        return None;
    }

    let new_replay = match files::read_replay(&new_replay_file)
        .with_context(|| format!("Failed to read replay '{}'", new_replay_file))
    {
        Ok(game) => game,
        Err(e) => return Some(Err(e)),
    };

    Some(
        files::parse_replay(new_replay)
            .with_context(|| format!("Failed to parse replay '{}'", new_replay_file)),
    )
}
