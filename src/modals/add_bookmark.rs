use gtk::prelude::{
    BoxExt, ButtonExt, EditableExt, GtkApplicationExt, ListBoxRowExt, OrientableExt, WidgetExt,
};
use relm4::{
    adw,
    adw::prelude::{AdwDialogExt, PreferencesGroupExt, PreferencesRowExt},
    gtk, Component, ComponentParts, ComponentSender, RelmWidgetExt,
};

use crate::network::instapaper;
use crate::persistence::token::TokenPair;

pub struct AddBookmarkDialog {
    url: String,
    error_message: Option<String>,
    is_loading: bool,
    tokens: TokenPair,
}

#[derive(Debug)]
pub enum AddBookmarkInput {
    SetUrl(String),
    Submit,
    Cancel,
}

#[derive(Debug)]
pub enum AddBookmarkOutput {
    BookmarkAdded(String),
    Cancelled,
}

#[derive(Debug)]
pub enum AddBookmarkCommandOutput {
    AddSuccess,
    AddFailed(String),
}

#[relm4::component(pub)]
impl Component for AddBookmarkDialog {
    type Init = TokenPair;
    type Input = AddBookmarkInput;
    type Output = AddBookmarkOutput;
    type CommandOutput = AddBookmarkCommandOutput;

    view! {
        adw::Dialog {
            set_title: "Add Bookmark",
            set_content_width: 450,
            set_content_height: 250,

            #[wrap(Some)]
            set_child = &adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    #[wrap(Some)]
                    set_title_widget = &adw::WindowTitle {
                        set_title: "Add Bookmark",
                    },
                },

                #[wrap(Some)]
                set_content = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 24,
                    set_spacing: 16,

                    adw::PreferencesGroup {
                        set_title: "URL",
                        set_description: Some("Enter the URL of the article you want to save"),

                        adw::EntryRow {
                            set_title: "Article URL",
                            set_sensitive: !model.is_loading,
                            connect_changed[sender] => move |entry| {
                                sender.input(AddBookmarkInput::SetUrl(entry.text().to_string()));
                            },
                            connect_activate => AddBookmarkInput::Submit,
                        },
                    },

                    gtk::Label {
                        #[watch]
                        set_visible: model.error_message.is_some(),
                        #[watch]
                        set_label: model.error_message.as_deref().unwrap_or(""),
                        add_css_class: "error",
                        set_wrap: true,
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 12,
                        set_halign: gtk::Align::End,
                        set_valign: gtk::Align::End,
                        set_vexpand: true,

                        gtk::Button {
                            set_label: "Cancel",
                            set_sensitive: !model.is_loading,
                            connect_clicked => AddBookmarkInput::Cancel,
                        },

                        if model.is_loading {
                            adw::Spinner {
                                set_halign: gtk::Align::Center,
                                set_valign: gtk::Align::Center,
                            }
                        } else {
                            gtk::Button {
                                set_label: "Add",
                                add_css_class: "suggested-action",
                                connect_clicked => AddBookmarkInput::Submit,
                            }
                        },
                    },
                },
            },

            connect_closed[sender] => move |_| {
                sender.input(AddBookmarkInput::Cancel);
            },
        }
    }

    fn init(
        tokens: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {
            url: String::new(),
            error_message: None,
            is_loading: false,
            tokens,
        };

        let widgets = view_output!();

        root.present(Some(&relm4::main_application().windows()[0]));

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>, root: &Self::Root) {
        match message {
            AddBookmarkInput::SetUrl(url) => {
                self.url = url;
                self.error_message = None;
            }
            AddBookmarkInput::Submit => {
                if self.url.is_empty() {
                    self.error_message = Some("Please enter a URL".to_string());
                    return;
                }

                // Basic URL validation
                if !self.url.starts_with("http://") && !self.url.starts_with("https://") {
                    self.error_message =
                        Some("URL must start with http:// or https://".to_string());
                    return;
                }

                self.is_loading = true;
                self.error_message = None;

                let url = self.url.clone();
                let tokens = self.tokens.clone();

                sender.oneshot_command(async move {
                    let client = instapaper::client();

                    match instapaper::add_bookmark(&client, &tokens, &url).await {
                        Ok(_) => AddBookmarkCommandOutput::AddSuccess,
                        Err(instapaper::InstapaperError::InvalidCredentials) => {
                            AddBookmarkCommandOutput::AddFailed(
                                "Invalid credentials. Please log in again.".to_string(),
                            )
                        }
                        Err(instapaper::InstapaperError::RateLimited) => {
                            AddBookmarkCommandOutput::AddFailed(
                                "Rate limited. Please try again later.".to_string(),
                            )
                        }
                        Err(e) => AddBookmarkCommandOutput::AddFailed(format!(
                            "Failed to add bookmark: {:?}",
                            e
                        )),
                    }
                });
            }
            AddBookmarkInput::Cancel => {
                root.close();
                let _ = sender.output(AddBookmarkOutput::Cancelled);
            }
        }
    }

    fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        sender: ComponentSender<Self>,
        root: &Self::Root,
    ) {
        match message {
            AddBookmarkCommandOutput::AddSuccess => {
                self.is_loading = false;
                let url = self.url.clone();
                root.close();
                let _ = sender.output(AddBookmarkOutput::BookmarkAdded(url));
            }
            AddBookmarkCommandOutput::AddFailed(error) => {
                self.is_loading = false;
                self.error_message = Some(error);
            }
        }
    }
}
