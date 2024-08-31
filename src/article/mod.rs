use gtk::prelude::ButtonExt;
use relm4::factory::{DynamicIndex, FactoryComponent, FactorySender};
use relm4::gtk;

#[derive(Debug)]
pub struct Article {
    pub title: String,
    pub uri: String,
}

#[derive(Debug)]
pub enum ArticleOutput {
    ArticleSelected(String),
}

#[derive(Debug)]
pub enum ArticleInput {
    ArticleSelected,
}

#[relm4::factory(pub)]
impl FactoryComponent for Article {
    type Init = (String, String);
    type Input = ArticleInput;
    type Output = ArticleOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        #[root]
        gtk::Button::with_label(&self.title) {
            connect_clicked => ArticleInput::ArticleSelected,
        }
    }

    fn init_model(init: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        let (title, uri) = init;
        Self { title, uri }
    }

    fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
        match msg {
            ArticleInput::ArticleSelected => {
                sender
                    .output(ArticleOutput::ArticleSelected(self.uri.clone()))
                    .unwrap();
            }
        }
    }
}
