use crate::config::APP_ID;
use anyhow::Result;
use relm4::gtk::glib;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::io::Write;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TokenPair {
    pub oauth_token: String,
    pub oauth_token_secret: String,
}

pub fn save_tokens(tokens: &TokenPair) -> Result<()> {
    let mut path = glib::user_data_dir();
    path.push(APP_ID);
    std::fs::create_dir_all(&path).expect("Could not create directory.");
    path.push("tokens.json");

    let json = serde_json::to_string(tokens)?;
    let mut file = File::create(path)?;
    file.write_all(json.as_bytes())?;
    Ok(())
}

pub fn read_tokens() -> Result<TokenPair> {
    let mut path = glib::user_data_dir();
    path.push(APP_ID);
    path.push("tokens.json");

    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let tokens: TokenPair = serde_json::from_str(&contents)?;
    Ok(tokens)
}

pub fn clear_tokens() -> Result<()> {
    let mut path = glib::user_data_dir();
    path.push(APP_ID);
    path.push("tokens.json");

    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}
