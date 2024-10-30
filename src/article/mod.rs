use relm4::adw::{prelude::ActionRowExt, ActionRow};
use relm4::factory::{DynamicIndex, FactoryComponent, FactorySender};
use relm4::gtk;

use crate::network::pocket::PocketArticle;

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

pub fn parse_json_response(downloaded_articles: Vec<PocketArticle>) -> Vec<Article> {
    downloaded_articles
        .iter()
        .map(
            |PocketArticle {
                 item_id,
                 resolved_title,
                 resolved_url,
             }| Article {
                item_id: item_id.to_owned(),
                title: resolved_title.to_owned(),
                uri: resolved_url.to_owned(),
            },
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_response() {
        let downloaded_articles = vec![PocketArticle {
            item_id: "item_id_one".to_owned(),
            resolved_title: "this_is_a_resolved_title".to_owned(),
            resolved_url: "this_is_a_resolved_url".to_owned(),
        }];

        let articles = parse_json_response(downloaded_articles);
        assert_eq!(articles[0].item_id, "item_id_one")
    }
}
