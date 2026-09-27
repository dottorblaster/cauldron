pub mod renderer;

use relm4::adw;
use relm4::gtk;

use gtk::prelude::*;

use crate::network::instapaper::InstapaperBookmark;

pub use renderer::{ArticleRenderer, ArticleRendererInput};

#[derive(Debug, Clone)]
pub struct Article {
    pub title: String,
    pub uri: String,
    pub item_id: String,
    pub description: String,
    pub time: f64,
    pub tags: Vec<String>,
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

    fn truncated_description(&self) -> String {
        if self.description.is_empty() {
            return String::new();
        }
        if self.description.len() > 100 {
            // Find the last char boundary at or before byte index 100
            let mut end = 100;
            while !self.description.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}...", &self.description[..end])
        } else {
            self.description.clone()
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

    /// Subtitle shown in the sidebar row: truncated description (when
    /// present) plus date and reading time, one per line.
    fn sidebar_subtitle(&self) -> String {
        let mut parts = Vec::new();

        let truncated_desc = self.truncated_description();
        if !truncated_desc.is_empty() {
            parts.push(truncated_desc);
        }

        let metadata = format!("{} · {}", self.format_date(), self.calculate_reading_time());
        parts.push(metadata);

        parts.join("\n")
    }

    /// One `.pill` label per tag, rendered in the row suffix.
    fn sidebar_tag_pills(&self) -> Vec<gtk::Label> {
        self.tags
            .iter()
            .map(|tag| {
                let pill = gtk::Label::new(Some(&format!("#{}", tag)));
                pill.add_css_class("pill");
                pill.set_halign(gtk::Align::Start);
                pill
            })
            .collect()
    }

    /// Builds the `AdwSidebarItem` used for this article in the sidebar.
    pub fn to_sidebar_item(&self) -> adw::SidebarItem {
        let tags_box = adw::WrapBox::new();
        tags_box.set_valign(gtk::Align::Center);
        tags_box.set_halign(gtk::Align::Start);
        for pill in self.sidebar_tag_pills() {
            tags_box.append(&pill);
        }

        adw::SidebarItem::builder()
            .title(&self.title)
            .subtitle(self.sidebar_subtitle())
            .suffix(&tags_box)
            .build()
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
            tags: bookmark.tags.iter().map(|t| t.name.clone()).collect(),
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
    use crate::testing::widget_inspection;

    fn make_article(title: &str, id: &str, tags: Vec<String>) -> Article {
        Article {
            title: title.to_string(),
            uri: format!("https://example.com/{}", id),
            item_id: id.to_string(),
            description: format!("About {}", title),
            time: 1234567890.0,
            tags,
        }
    }

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
            tags: vec![],
        }];

        let articles = parse_instapaper_response(bookmarks);
        assert_eq!(articles[0].item_id, "12345");
        assert_eq!(articles[0].title, "Test Article Title");
        assert_eq!(articles[0].uri, "https://example.com/article");
        assert_eq!(articles[0].description, "A sample description");
        assert_eq!(articles[0].time, 1234567890.0);
        assert!(articles[0].tags.is_empty());
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
            tags: vec![],
        }];

        let articles = parse_instapaper_response(bookmarks);
        // When title is empty, should use URL as title
        assert_eq!(articles[0].title, "https://example.com/article");
    }

    #[test]
    fn test_parse_instapaper_response_with_tags() {
        use crate::network::instapaper::InstapaperTag;

        let bookmarks = vec![InstapaperBookmark {
            description: "".to_owned(),
            starred: "false".to_owned(),
            extra: HashMap::new(),
            bookmark_id: 100,
            title: "Tagged Article".to_owned(),
            url: "https://example.com/tagged".to_owned(),
            progress: 0.0,
            time: 0.0,
            hash: "".to_owned(),
            tags: vec![
                InstapaperTag {
                    id: 1,
                    name: "Rust".to_owned(),
                },
                InstapaperTag {
                    id: 2,
                    name: "Programming".to_owned(),
                },
            ],
        }];

        let articles = parse_instapaper_response(bookmarks);
        assert_eq!(articles[0].tags, vec!["Rust", "Programming"]);
    }

    #[test]
    fn test_format_date() {
        let unknown = Article {
            title: "Test".to_string(),
            uri: "https://example.com/1".to_string(),
            item_id: "1".to_string(),
            description: "".to_string(),
            time: 0.0,
            tags: vec![],
        };
        assert_eq!(unknown.format_date(), "Unknown date");

        let recent_time = chrono::Utc::now().timestamp() as f64;
        let recent = Article {
            title: "Test".to_string(),
            uri: "https://example.com/1".to_string(),
            item_id: "1".to_string(),
            description: "".to_string(),
            time: recent_time,
            tags: vec![],
        };
        assert_eq!(recent.format_date(), "Today");
    }

    #[test]
    fn test_calculate_reading_time() {
        // Zero words (empty title and description) -> "< 1 min read"
        let empty = Article {
            title: "".to_owned(),
            uri: "https://example.com/1".to_owned(),
            item_id: "1".to_owned(),
            description: "".to_owned(),
            time: 0.0,
            tags: vec![],
        };
        assert_eq!(empty.calculate_reading_time(), "< 1 min read");

        // Exactly one word -> "1 min read"
        let one_word = Article {
            title: "Word".to_owned(),
            uri: "https://example.com/2".to_owned(),
            item_id: "2".to_owned(),
            description: "".to_owned(),
            time: 0.0,
            tags: vec![],
        };
        assert_eq!(one_word.calculate_reading_time(), "1 min read");

        // ~200 words still rounds to "1 min read"
        let long = Article {
            title: "".to_owned(),
            uri: "https://example.com/3".to_owned(),
            item_id: "3".to_owned(),
            description: "word ".repeat(199),
            time: 0.0,
            tags: vec![],
        };
        assert_eq!(long.calculate_reading_time(), "1 min read");
    }

    #[test]
    fn test_truncated_description_with_multibyte_char_at_boundary() {
        // Build a description where a multi-byte character spans the 100-byte boundary.
        // '\u{a0}' (non-breaking space) is 2 bytes in UTF-8 (0xC2 0xA0).
        // Place it so bytes 99..101 contain '\u{a0}', making byte index 100 not a char boundary.
        let mut desc = "a".repeat(99); // 99 ASCII bytes
        desc.push('\u{a0}'); // bytes 99..101
        desc.push_str(&"b".repeat(10)); // pad to exceed 100 bytes total

        let article = Article {
            title: "Test".to_owned(),
            uri: "https://example.com/1".to_owned(),
            item_id: "1".to_owned(),
            description: desc.clone(),
            time: 0.0,
            tags: vec![],
        };

        // This should not panic and should produce a valid truncated string
        let result = article.truncated_description();
        assert!(result.ends_with("..."));
        assert!(result.len() <= 103); // at most 100 bytes of content + "..."

        // Verify it ends at a valid char boundary
        assert!(result.is_char_boundary(result.len() - 3));
    }

    #[test]
    fn test_sidebar_subtitle_contains_description_and_metadata() {
        let article = make_article("Test Article", "1", vec![]);

        let subtitle = article.sidebar_subtitle();
        assert!(subtitle.contains("About Test Article"));
        assert!(subtitle.contains('·'));
        assert!(subtitle.contains("min read"));
    }

    #[test]
    fn test_sidebar_subtitle_without_description_only_has_metadata() {
        let article = Article {
            title: "No Description".to_owned(),
            uri: "https://example.com/1".to_owned(),
            item_id: "1".to_owned(),
            description: "".to_owned(),
            time: 0.0,
            tags: vec![],
        };

        let subtitle = article.sidebar_subtitle();
        assert!(!subtitle.contains("No Description"));
        assert!(subtitle.contains("Unknown date"));
    }

    #[gtk::test]
    fn test_to_sidebar_item_title_and_subtitle() {
        let article = make_article("Test Article", "1", vec![]);
        let item = article.to_sidebar_item();

        let title: String = item.property_value("title").get().unwrap();
        assert_eq!(title, "Test Article");

        let subtitle: String = item.property_value("subtitle").get().unwrap();
        assert!(subtitle.contains("About Test Article"));
        assert!(subtitle.contains("min read"));
    }

    #[gtk::test]
    fn test_to_sidebar_item_renders_tag_pills() {
        let article = make_article(
            "Tagged Article",
            "1",
            vec!["Rust".to_owned(), "UI".to_owned()],
        );
        let item = article.to_sidebar_item();

        let suffix: gtk::Widget = item.property_value("suffix").get().unwrap();
        let pills = widget_inspection::find_all_descendants_by_css_class(&suffix, "pill");
        assert_eq!(pills.len(), 2, "Should render one pill per tag");

        let pill_texts: Vec<String> = pills
            .iter()
            .filter_map(|w| w.clone().dynamic_cast::<gtk::Label>().ok())
            .map(|label| label.text().to_string())
            .collect();
        assert!(pill_texts.contains(&"#Rust".to_string()));
        assert!(pill_texts.contains(&"#UI".to_string()));
    }

    #[gtk::test]
    fn test_to_sidebar_item_without_tags_has_no_pills() {
        let article = make_article("Untagged Article", "2", vec![]);
        let item = article.to_sidebar_item();

        let suffix: gtk::Widget = item.property_value("suffix").get().unwrap();
        assert!(
            widget_inspection::find_all_descendants_by_css_class(&suffix, "pill").is_empty(),
            "Articles without tags should not render any pills"
        );
    }
}
