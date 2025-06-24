use crate::files;
use anyhow::{Context, Result, anyhow};
use std::collections::HashMap;
use std::io::{self, Write};

pub fn batch_process_replays(replay_dir: &str, matches: &mut HashMap<String, files::Match>) {
    let slp_files = files::find_slp_files(replay_dir);

    let total = slp_files.len().max(1);
    let step = (total / 100).max(1);

    for (i, file) in slp_files.iter().enumerate() {
        let _ = process_replay(file.to_string(), matches);

        if i % step == 0 {
            print!("\rProgress: {}%", (i * 100) / total);
            io::stdout().flush().unwrap();
        }
    }

    println!("\rProgress: 100% - Complete!");
}

pub fn process_replay(
    new_replay_file: String,
    matches: &mut HashMap<String, files::Match>,
) -> Result<()> {
    if matches.contains_key(&new_replay_file) {
        return Err(anyhow!("File already exists in db"));
    }

    let new_replay = files::read_replay(&new_replay_file)
        .with_context(|| format!("Failed to read replay '{}'", new_replay_file))?;

    let r#match = files::parse_replay(new_replay)
        .map_err(|err| {
            matches.insert(new_replay_file.clone(), files::Match::default());
            err
        })
        .with_context(|| format!("Failed to parse replay '{}'", new_replay_file))?;

    matches.insert(new_replay_file, r#match);
    Ok(())
}
