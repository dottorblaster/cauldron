use crate::config::APP_ID;
use anyhow::Result;
use relm4::gtk::glib;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistedArticle {
    pub title: String,
    pub uri: String,
    pub item_id: String,
    pub description: String,
    pub time: f64,
}

pub fn save_articles(articles: &[PersistedArticle]) -> Result<()> {
    let mut path = glib::user_data_dir();
    path.push(APP_ID);
    std::fs::create_dir_all(&path)?;
    path.push("articles.json");

    let json = serde_json::to_string(articles)?;
    let mut file = File::create(path)?;
    file.write_all(json.as_bytes())?;
    Ok(())
}

pub fn read_articles() -> Result<Vec<PersistedArticle>> {
    let mut path = glib::user_data_dir();
    path.push(APP_ID);
    path.push("articles.json");

    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let articles: Vec<PersistedArticle> = serde_json::from_str(&contents)?;
    Ok(articles)
}

pub fn clear_articles() -> Result<()> {
    let mut path = glib::user_data_dir();
    path.push(APP_ID);
    path.push("articles.json");

    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}
