use std::collections::HashMap;
use std::fs;

use anyhow::Result;

use crate::files::Match;

pub async fn write_to_file(data: &HashMap<String, Match>, filename: &str) -> Result<()> {
    let encoded = bincode::encode_to_vec(data, bincode::config::standard())?;
    fs::write(filename, encoded)?;

    Ok(())
}

pub async fn read_from_file(filename: &str) -> Result<HashMap<String, Match>> {
    let encoded = fs::read(filename)?;
    let (matches, _): (HashMap<String, Match>, usize) =
        bincode::decode_from_slice(&encoded, bincode::config::standard())?;

    Ok(matches)
}
