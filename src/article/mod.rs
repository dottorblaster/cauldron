use relm4::adw::{prelude::ActionRowExt, ActionRow};
use relm4::factory::{DynamicIndex, FactoryComponent, FactorySender};
use relm4::gtk;

use crate::network::instapaper::InstapaperBookmark;

#[derive(Debug)]
pub struct Article {
    pub title: String,
    pub uri: String,
    pub item_id: String,
}

#[derive(Debug)]
pub enum ArticleOutput {
    ArticleSelected(String, String),
}

#[derive(Debug)]
pub enum ArticleInput {
    ArticleSelected,
}

#[relm4::factory(pub)]
impl FactoryComponent for Article {
    type Init = (String, String, String);
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
            .build() {
            connect_activated => ArticleInput::ArticleSelected
        }
    }

    fn init_model(init: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        let (title, uri, item_id) = init;
        Self {
            title,
            uri,
            item_id,
        }
    }

    fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
        match msg {
            ArticleInput::ArticleSelected => {
                sender
                    .output(ArticleOutput::ArticleSelected(
                        self.uri.clone(),
                        self.item_id.clone(),
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
        })
        .collect();

    // Sort by bookmark_id descending (newest first)
    parsed_articles
        .sort_by_key(|element| std::cmp::Reverse(element.item_id.parse::<i64>().unwrap_or(0)));

    parsed_articles
}

#[cfg(test)]
mod tests {
    use super::*;
    use flume;
    use relm4::factory::FactoryVecDeque;

    #[test]
    fn test_parse_instapaper_response() {
        let bookmarks = vec![InstapaperBookmark {
            type_field: "bookmark".to_owned(),
            bookmark_id: 12345,
            title: "Test Article Title".to_owned(),
            url: "https://example.com/article".to_owned(),
            progress: 0.0,
            time: 1234567890,
            hash: "abc123".to_owned(),
        }];

        let articles = parse_instapaper_response(bookmarks);
        assert_eq!(articles[0].item_id, "12345");
        assert_eq!(articles[0].title, "Test Article Title");
        assert_eq!(articles[0].uri, "https://example.com/article");
    }

    #[test]
    fn test_parse_instapaper_response_empty_title() {
        let bookmarks = vec![InstapaperBookmark {
            type_field: "bookmark".to_owned(),
            bookmark_id: 12345,
            title: "".to_owned(),
            url: "https://example.com/article".to_owned(),
            progress: 0.0,
            time: 1234567890,
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
        articles
            .guard()
            .push_back(("".to_owned(), "".to_owned(), "".to_owned()));
    }
}
