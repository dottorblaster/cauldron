pub mod renderer;

use relm4::adw::{prelude::ActionRowExt, ActionRow};
use relm4::factory::{DynamicIndex, FactoryComponent, FactorySender};
use relm4::gtk;
use relm4::gtk::glib;

use crate::network::instapaper::InstapaperBookmark;

pub use renderer::{ArticleRenderer, ArticleRendererInput};

#[derive(Debug, Clone)]
pub struct ArticleInit {
    pub title: String,
    pub uri: String,
    pub item_id: String,
    pub description: String,
    pub time: f64,
}

#[derive(Debug)]
pub struct Article {
    pub title: String,
    pub uri: String,
    pub item_id: String,
    pub description: String,
    pub time: f64,
}

impl Article {
    fn format_date(&self) -> String {
        if self.time == 0.0 {
            return String::from("Unknown date");
        }

        let timestamp = self.time as i64;
        let datetime = chrono::DateTime::from_timestamp(timestamp, 0);

        match datetime {
            Some(dt) => {
                let now = chrono::Utc::now();
                let duration = now.signed_duration_since(dt);

                if duration.num_days() == 0 {
                    String::from("Today")
                } else if duration.num_days() == 1 {
                    String::from("Yesterday")
                } else if duration.num_days() < 7 {
                    format!("{} days ago", duration.num_days())
                } else if duration.num_weeks() < 4 {
                    let weeks = duration.num_weeks();
                    if weeks == 1 {
                        String::from("1 week ago")
                    } else {
                        format!("{} weeks ago", weeks)
                    }
                } else {
                    dt.format("%b %d, %Y").to_string()
                }
            }
            None => String::from("Unknown date"),
        }
    }

    fn calculate_reading_time(&self) -> String {
        let text = format!("{} {}", self.title, self.description);
        let word_count = text.split_whitespace().count();
        let minutes = (word_count as f32 / 200.0).ceil() as usize;

        if minutes < 1 {
            String::from("< 1 min read")
        } else if minutes == 1 {
            String::from("1 min read")
        } else {
            format!("{} min read", minutes)
        }
    }
}

#[derive(Debug)]
pub enum ArticleOutput {
    ArticleSelected(String, String, String, String, f64),
}

#[derive(Debug)]
pub enum ArticleInput {
    ArticleSelected,
}

#[relm4::factory(pub)]
impl FactoryComponent for Article {
    type Init = ArticleInit;
    type Input = ArticleInput;
    type Output = ArticleOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        #[root]
        ActionRow::builder()
            .activatable(true)
            .selectable(true)
            .title(&self.title)
            .subtitle({
                let mut parts = Vec::new();

                if !self.description.is_empty() {
                    let truncated_desc = if self.description.len() > 100 {
                        format!("{}...", &self.description[..100])
                    } else {
                        self.description.clone()
                    };
                    parts.push(truncated_desc);
                }

                let metadata = format!("{} · {}", self.format_date(), self.calculate_reading_time());
                parts.push(metadata);

                glib::markup_escape_text(&parts.join("\n"))
            })
            .build() {
            connect_activated => ArticleInput::ArticleSelected
        }
    }

    fn init_model(init: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self {
            title: init.title,
            uri: init.uri,
            item_id: init.item_id,
            description: init.description,
            time: init.time,
        }
    }

    fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
        match msg {
            ArticleInput::ArticleSelected => {
                sender
                    .output(ArticleOutput::ArticleSelected(
                        self.title.clone(),
                        self.uri.clone(),
                        self.item_id.clone(),
                        self.description.clone(),
                        self.time,
                    ))
                    .unwrap();
            }
        }
    }
}

pub fn parse_instapaper_response(bookmarks: Vec<InstapaperBookmark>) -> Vec<Article> {
    let mut parsed_articles: Vec<Article> = bookmarks
        .iter()
        .map(|bookmark| Article {
            item_id: bookmark.bookmark_id.to_string(),
            title: if bookmark.title.is_empty() {
                bookmark.url.clone()
            } else {
                bookmark.title.clone()
            },
            uri: bookmark.url.clone(),
            description: bookmark.description.clone(),
            time: bookmark.time,
        })
        .collect();

    // Sort by bookmark_id descending (newest first)
    parsed_articles
        .sort_by_key(|element| std::cmp::Reverse(element.item_id.parse::<i64>().unwrap_or(0)));

    parsed_articles
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use flume;
    use relm4::factory::FactoryVecDeque;

    #[test]
    fn test_parse_instapaper_response() {
        let bookmarks = vec![InstapaperBookmark {
            description: "A sample description".to_owned(),
            starred: "false".to_owned(),
            extra: HashMap::new(),
            bookmark_id: 12345,
            title: "Test Article Title".to_owned(),
            url: "https://example.com/article".to_owned(),
            progress: 0.0,
            time: 1234567890.0,
            hash: "abc123".to_owned(),
        }];

        let articles = parse_instapaper_response(bookmarks);
        assert_eq!(articles[0].item_id, "12345");
        assert_eq!(articles[0].title, "Test Article Title");
        assert_eq!(articles[0].uri, "https://example.com/article");
        assert_eq!(articles[0].description, "A sample description");
        assert_eq!(articles[0].time, 1234567890.0);
    }

    #[test]
    fn test_parse_instapaper_response_empty_title() {
        let bookmarks = vec![InstapaperBookmark {
            description: "A sample description".to_owned(),
            starred: "false".to_owned(),
            extra: HashMap::new(),
            bookmark_id: 12345,
            title: "".to_owned(),
            url: "https://example.com/article".to_owned(),
            progress: 0.0,
            time: 1234567890.0,
            hash: "abc123".to_owned(),
        }];

        let articles = parse_instapaper_response(bookmarks);
        // When title is empty, should use URL as title
        assert_eq!(articles[0].title, "https://example.com/article");
    }

    #[gtk::test]
    fn test_init_model() {
        let (sender, _) = flume::unbounded();
        let test_sender: relm4::Sender<()> = sender.into();
        let mut articles: FactoryVecDeque<Article> = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(&test_sender, |_| {});
        articles.guard().push_back(ArticleInit {
            title: "".to_owned(),
            uri: "".to_owned(),
            item_id: "".to_owned(),
            description: "".to_owned(),
            time: 0.0,
        });
    }
}
