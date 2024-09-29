use relm4::adw::{prelude::ActionRowExt, ActionRow};
use relm4::factory::{DynamicIndex, FactoryComponent, FactorySender};
use relm4::gtk;

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

pub fn parse_json_response(response: serde_json::Value) -> Vec<Article> {
    let mut articles = vec![];

    for (_, value) in response["list"].as_object().unwrap() {
        let title = value["resolved_title"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let uri = value["resolved_url"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let item_id = value["item_id"].as_str().unwrap_or_default().to_owned();
        articles.push(Article {
            title,
            uri,
            item_id,
        })
    }

    articles
}
