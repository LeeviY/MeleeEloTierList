use crate::files::Match;

use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn write_to_file(
    data: &Arc<Mutex<HashMap<String, Match>>>,
    filename: &str,
) -> Result<()> {
    let guard = data.lock().await;
    let matches_ref = &*guard;

    let encoded = bincode::encode_to_vec(matches_ref, bincode::config::standard())?;
    fs::write(filename, encoded)?;

    Ok(())
}

pub async fn read_from_file(filename: &str) -> Result<Arc<Mutex<HashMap<String, Match>>>> {
    let encoded = fs::read(filename)?;
    let (matches, _): (HashMap<String, Match>, usize) =
        bincode::decode_from_slice(&encoded, bincode::config::standard())?;

    Ok(Arc::new(Mutex::new(matches)))
}
