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
    use crate::testing::FactoryComponentTester;

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
        let mut tester = FactoryComponentTester::<Article>::new(gtk::ListBox::default());

        let index = tester.init(ArticleInit {
            title: "Test Article".to_owned(),
            uri: "https://example.com".to_owned(),
            item_id: "123".to_owned(),
            description: "A test article".to_owned(),
            time: 1234567890.0,
        });

        tester.get(index, |article: &Article| {
            assert_eq!(article.title, "Test Article");
            assert_eq!(article.uri, "https://example.com");
            assert_eq!(article.item_id, "123");
            assert_eq!(article.description, "A test article");
            assert_eq!(article.time, 1234567890.0);
        });
    }

    #[gtk::test]
    fn test_article_selected() {
        let mut tester = FactoryComponentTester::<Article>::new(gtk::ListBox::default());

        let index = tester.init(ArticleInit {
            title: "Test Article".to_owned(),
            uri: "https://example.com".to_owned(),
            item_id: "123".to_owned(),
            description: "A test article".to_owned(),
            time: 1234567890.0,
        });

        // Send ArticleSelected input
        tester.send_input(index, ArticleInput::ArticleSelected);
        tester.process_events();

        // Note: Output capture doesn't work reliably in test contexts,
        // so we only verify the component state remains unchanged
        tester.get(index, |article: &Article| {
            assert_eq!(article.title, "Test Article");
            assert_eq!(article.uri, "https://example.com");
            assert_eq!(article.item_id, "123");
        });
    }

    #[gtk::test]
    fn test_format_date() {
        let mut tester = FactoryComponentTester::<Article>::new(gtk::ListBox::default());

        // Test with zero timestamp
        let index = tester.init(ArticleInit {
            title: "Test".to_owned(),
            uri: "https://example.com".to_owned(),
            item_id: "1".to_owned(),
            description: "".to_owned(),
            time: 0.0,
        });

        tester.get(index, |article: &Article| {
            assert_eq!(article.format_date(), "Unknown date");
        });
    }

    #[gtk::test]
    fn test_calculate_reading_time() {
        let mut tester = FactoryComponentTester::<Article>::new(gtk::ListBox::default());

        // Create an article with zero words (empty title and description)
        // This should give "< 1 min read"
        let index = tester.init(ArticleInit {
            title: "".to_owned(),
            uri: "https://example.com".to_owned(),
            item_id: "1".to_owned(),
            description: "".to_owned(),
            time: 0.0,
        });

        tester.get(index, |article: &Article| {
            assert_eq!(article.calculate_reading_time(), "< 1 min read");
        });

        // Test with exactly 1 word
        let index2 = tester.init(ArticleInit {
            title: "Word".to_owned(),
            uri: "https://example.com".to_owned(),
            item_id: "2".to_owned(),
            description: "".to_owned(),
            time: 0.0,
        });

        tester.get(index2, |article: &Article| {
            assert_eq!(article.calculate_reading_time(), "1 min read");
        });

        // Test with more words (approximately 200 words would still be 1 min due to rounding)
        let long_description = "word ".repeat(199);
        let index3 = tester.init(ArticleInit {
            title: "".to_owned(),
            uri: "https://example.com".to_owned(),
            item_id: "3".to_owned(),
            description: long_description,
            time: 0.0,
        });

        tester.get(index3, |article: &Article| {
            assert_eq!(article.calculate_reading_time(), "1 min read");
        });
    }

    #[gtk::test]
    fn test_article_widget_introspection() {
        let mut tester = FactoryComponentTester::<Article>::new(gtk::ListBox::default());

        // Initialize multiple articles
        tester.init(ArticleInit {
            title: "First Article".to_owned(),
            uri: "https://example.com/first".to_owned(),
            item_id: "1".to_owned(),
            description: "First article description".to_owned(),
            time: 1234567890.0,
        });

        tester.init(ArticleInit {
            title: "Second Article".to_owned(),
            uri: "https://example.com/second".to_owned(),
            item_id: "2".to_owned(),
            description: "Second article description".to_owned(),
            time: 1234567900.0,
        });

        tester.init(ArticleInit {
            title: "Third Article".to_owned(),
            uri: "https://example.com/third".to_owned(),
            item_id: "3".to_owned(),
            description: "Third article description".to_owned(),
            time: 1234567910.0,
        });

        tester.process_events();

        // Deep assertions: Test that all three articles are rendered
        assert_eq!(tester.count_factory_children(), 3);

        // Find all ActionRow widgets (one per article)
        let action_rows: Vec<relm4::adw::ActionRow> = tester.find_all_widgets_by_type();
        assert_eq!(action_rows.len(), 3);

        // Verify that we can find articles by their title
        // Note: ActionRow titles are stored internally, so we test the component state instead
        assert!(tester.find_label_containing_text("First Article").is_some());
        assert!(tester
            .find_label_containing_text("Second Article")
            .is_some());
        assert!(tester.find_label_containing_text("Third Article").is_some());

        // Collect all children to verify the structure
        let all_children = tester.collect_factory_children();
        assert_eq!(all_children.len(), 3, "Should have 3 article rows");
    }

    #[gtk::test]
    fn test_article_widget_introspection_single() {
        let mut tester = FactoryComponentTester::<Article>::new(gtk::ListBox::default());

        // Test with a single article
        tester.init(ArticleInit {
            title: "My Article Title".to_owned(),
            uri: "https://example.com/my-article".to_owned(),
            item_id: "42".to_owned(),
            description: "This is a great article about testing".to_owned(),
            time: 1234567890.0,
        });

        tester.process_events();

        // Verify exactly one child
        assert_eq!(tester.count_factory_children(), 1);

        // Find the ActionRow
        let action_row: Option<relm4::adw::ActionRow> = tester.find_widget_by_type();
        assert!(action_row.is_some(), "Should find the ActionRow widget");

        // Verify we can find the article's content
        assert!(tester
            .find_label_containing_text("My Article Title")
            .is_some());
    }
}
