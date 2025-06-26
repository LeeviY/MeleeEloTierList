use crate::files;
use anyhow::{Context, Result, anyhow};
use rayon::prelude::*;
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

pub fn batch_process_replays(replay_dir: &str, matches: &mut HashMap<String, files::Match>) {
    println!("\nProcessing replays...");
    let start = Instant::now();
    let slp_files = files::find_slp_files(replay_dir);

    let total = slp_files.len().max(1);
    let step = (total / 100).max(1);

    let mut skipped = 0;

    for (i, file) in slp_files.iter().enumerate() {
        if process_replay(file.to_string(), matches).is_err() {
            skipped += 1;
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
    println!("\nProcessing replays...");
    let start = Instant::now();
    let slp_files = files::find_slp_files(replay_dir);

    let skipped = AtomicUsize::new(0);

    let thread_results: Vec<HashMap<String, files::Match>> = slp_files
        .par_chunks(32)
        .map(|chunk| {
            let mut local_matches = HashMap::new();
            for file in chunk {
                if process_replay(file.to_string(), &mut local_matches).is_err() {
                    skipped.fetch_add(1, Ordering::Relaxed);
                }
            }
            local_matches
        })
        .collect();

    for map in thread_results {
        matches.extend(map);
    }

    println!(
        "\nProcessed {} (Skipped {:?}) replays in {:?}",
        slp_files.len(),
        skipped,
        start.elapsed()
    );
}

pub fn process_replay(
    new_replay_file: String,
    matches: &mut HashMap<String, files::Match>,
) -> Result<files::Match> {
    if matches.contains_key(&new_replay_file) {
        return Err(anyhow!("File already exists in db"));
    }

    let new_replay = files::read_replay(&new_replay_file)
        .with_context(|| format!("Failed to read replay '{}'", new_replay_file))?;

    let r#match = files::parse_replay(new_replay)
        .inspect_err(|_| {
            matches.insert(new_replay_file.clone(), files::Match::default());
        })
        .with_context(|| format!("Failed to parse replay '{}'", new_replay_file))?;

    Ok(r#match)
}
