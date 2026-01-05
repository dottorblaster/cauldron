use gtk::prelude::{
    BoxExt, ButtonExt, EditableExt, GtkApplicationExt, ListBoxRowExt, OrientableExt, WidgetExt,
};
use relm4::{
    adw,
    adw::prelude::{AdwDialogExt, PreferencesGroupExt, PreferencesRowExt},
    gtk, Component, ComponentParts, ComponentSender, RelmWidgetExt,
};

use gettextrs::gettext;

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

#[derive(Debug, Clone)]
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
            set_title: &gettext("Add Bookmark"),
            set_content_width: 450,
            set_content_height: 250,

            #[wrap(Some)]
            set_child = &adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    #[wrap(Some)]
                    set_title_widget = &adw::WindowTitle {
                        set_title: &gettext("Add Bookmark"),
                    },
                },

                #[wrap(Some)]
                set_content = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 24,
                    set_spacing: 16,

                    adw::PreferencesGroup {
                        set_title: &gettext("URL"),
                        set_description: Some(&gettext("Enter the URL of the article you want to save")),

                        adw::EntryRow {
                            set_title: &gettext("Article URL"),
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
                            set_label: &gettext("Cancel"),
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
                                set_label: &gettext("Add"),
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

        // Only present the dialog if we're not in a test environment
        if !cfg!(test) {
            root.present(Some(&relm4::main_application().windows()[0]));
        }

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
                    self.error_message = Some(gettext("Please enter a URL"));
                    return;
                }

                // Basic URL validation
                if !self.url.starts_with("http://") && !self.url.starts_with("https://") {
                    self.error_message = Some(gettext("URL must start with http:// or https://"));
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
                            AddBookmarkCommandOutput::AddFailed(gettext(
                                "Invalid credentials. Please log in again",
                            ))
                        }
                        Err(instapaper::InstapaperError::RateLimited) => {
                            AddBookmarkCommandOutput::AddFailed(gettext(
                                "Rate limited. Please try again later",
                            ))
                        }
                        Err(e) => AddBookmarkCommandOutput::AddFailed(format!(
                            "{}: {:?}",
                            gettext("Failed to add bookmark"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::ComponentTester;

    fn mock_tokens() -> TokenPair {
        TokenPair {
            oauth_token: "test_token".to_string(),
            oauth_token_secret: "test_secret".to_string(),
        }
    }

    #[gtk::test]
    fn test_init_component() {
        let tokens = mock_tokens();
        let tester = ComponentTester::<AddBookmarkDialog>::launch(tokens.clone());
        tester.process_events();

        // Component should initialize with empty values
        assert_eq!(tester.model().url, "");
        assert_eq!(tester.model().error_message, None);
        assert_eq!(tester.model().is_loading, false);
        assert_eq!(tester.model().tokens.oauth_token, tokens.oauth_token);
        assert_eq!(
            tester.model().tokens.oauth_token_secret,
            tokens.oauth_token_secret
        );
    }

    #[gtk::test]
    fn test_set_url() {
        let tester = ComponentTester::<AddBookmarkDialog>::launch(mock_tokens());
        tester.send_input(AddBookmarkInput::SetUrl(
            "https://example.com/article".to_string(),
        ));
        tester.process_events();

        assert_eq!(tester.model().url, "https://example.com/article");
        assert_eq!(tester.model().error_message, None);
    }

    #[gtk::test]
    fn test_submit_with_empty_url() {
        let tester = ComponentTester::<AddBookmarkDialog>::launch(mock_tokens());
        tester.send_input(AddBookmarkInput::Submit);
        tester.process_events();

        assert_eq!(
            tester.model().error_message,
            Some(gettext("Please enter a URL"))
        );
        assert_eq!(tester.model().is_loading, false);
    }

    #[gtk::test]
    fn test_submit_with_invalid_url_no_protocol() {
        let tester = ComponentTester::<AddBookmarkDialog>::launch(mock_tokens());
        tester.send_input(AddBookmarkInput::SetUrl("example.com/article".to_string()));
        tester.send_input(AddBookmarkInput::Submit);
        tester.process_events();

        assert_eq!(
            tester.model().error_message,
            Some(gettext("URL must start with http:// or https://"))
        );
        assert_eq!(tester.model().is_loading, false);
    }

    #[gtk::test]
    fn test_submit_with_invalid_url_ftp_protocol() {
        let tester = ComponentTester::<AddBookmarkDialog>::launch(mock_tokens());
        tester.send_input(AddBookmarkInput::SetUrl(
            "ftp://example.com/file".to_string(),
        ));
        tester.send_input(AddBookmarkInput::Submit);
        tester.process_events();

        assert_eq!(
            tester.model().error_message,
            Some(gettext("URL must start with http:// or https://"))
        );
        assert_eq!(tester.model().is_loading, false);
    }

    #[gtk::test]
    fn test_submit_with_valid_http_url() {
        let tester = ComponentTester::<AddBookmarkDialog>::launch(mock_tokens());
        tester.send_input(AddBookmarkInput::SetUrl(
            "http://example.com/article".to_string(),
        ));
        tester.send_input(AddBookmarkInput::Submit);
        tester.process_events();

        // Should set loading state for async operation
        assert_eq!(tester.model().is_loading, true);
        assert_eq!(tester.model().error_message, None);
    }

    #[gtk::test]
    fn test_submit_with_valid_https_url() {
        let tester = ComponentTester::<AddBookmarkDialog>::launch(mock_tokens());
        tester.send_input(AddBookmarkInput::SetUrl(
            "https://example.com/article".to_string(),
        ));
        tester.send_input(AddBookmarkInput::Submit);
        tester.process_events();

        // Should set loading state for async operation
        assert_eq!(tester.model().is_loading, true);
        assert_eq!(tester.model().error_message, None);
    }

    #[gtk::test]
    fn test_cancel_sends_output() {
        let tester = ComponentTester::<AddBookmarkDialog>::launch(mock_tokens());
        tester.send_input(AddBookmarkInput::Cancel);
        tester.process_events();

        // Check that Cancelled output was sent
        let output = tester.try_recv_output();
        assert!(matches!(output, Some(AddBookmarkOutput::Cancelled)));
    }

    #[gtk::test]
    fn test_error_clears_on_url_change() {
        let tester = ComponentTester::<AddBookmarkDialog>::launch(mock_tokens());

        // Trigger an error
        tester.send_input(AddBookmarkInput::Submit);
        tester.process_events();
        assert!(tester.model().error_message.is_some());

        // Change URL should clear error
        tester.send_input(AddBookmarkInput::SetUrl("https://example.com".to_string()));
        tester.process_events();
        assert_eq!(tester.model().error_message, None);
    }

    // Note: Testing update_cmd with async operations requires actually running the async code
    // or mocking the network layer, which is beyond the scope of basic component testing.
    // The async behavior is better tested through integration tests.

    // Note: Full widget introspection tests require a fully rendered view hierarchy.
    // In test mode without presenting the dialog, some GTK widgets may not be fully initialized.
    // Widget behavior is better tested through integration or UI tests.

    #[gtk::test]
    fn test_multiple_validation_errors() {
        let tester = ComponentTester::<AddBookmarkDialog>::launch(mock_tokens());

        // Test empty URL error
        tester.send_input(AddBookmarkInput::Submit);
        tester.process_events();
        assert_eq!(
            tester.model().error_message,
            Some(gettext("Please enter a URL"))
        );

        // Clear error by setting a URL (but invalid protocol)
        tester.send_input(AddBookmarkInput::SetUrl("example.com".to_string()));
        tester.process_events();
        assert_eq!(tester.model().error_message, None);

        // Test protocol error
        tester.send_input(AddBookmarkInput::Submit);
        tester.process_events();
        assert_eq!(
            tester.model().error_message,
            Some(gettext("URL must start with http:// or https://"))
        );

        // Fix the URL
        tester.send_input(AddBookmarkInput::SetUrl("https://example.com".to_string()));
        tester.process_events();
        assert_eq!(tester.model().error_message, None);

        // Should now trigger async operation
        tester.send_input(AddBookmarkInput::Submit);
        tester.process_events();
        assert_eq!(tester.model().is_loading, true);
        assert_eq!(tester.model().error_message, None);
    }
}
