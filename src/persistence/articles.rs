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
    #[serde(default)]
    pub tags: Vec<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_persisted_article_without_tags() {
        let json = r#"{
            "title": "Test",
            "uri": "https://example.com",
            "item_id": "1",
            "description": "desc",
            "time": 0.0
        }"#;

        let article: PersistedArticle = serde_json::from_str(json).unwrap();
        assert!(article.tags.is_empty());
    }

    #[test]
    fn test_roundtrip_persisted_article_with_tags() {
        let article = PersistedArticle {
            title: "Test".to_string(),
            uri: "https://example.com".to_string(),
            item_id: "1".to_string(),
            description: "desc".to_string(),
            time: 0.0,
            tags: vec!["Rust".to_string(), "Programming".to_string()],
        };

        let json = serde_json::to_string(&article).unwrap();
        let deserialized: PersistedArticle = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.tags, vec!["Rust", "Programming"]);
    }
}
