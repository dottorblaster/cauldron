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

pub struct LoginDialog {
    username: String,
    password: String,
    error_message: Option<String>,
    is_loading: bool,
}

#[derive(Debug)]
pub enum LoginInput {
    SetUsername(String),
    SetPassword(String),
    Submit,
    Cancel,
}

#[derive(Debug, Clone)]
pub enum LoginOutput {
    LoggedIn(TokenPair, String),
    Cancelled,
}

#[derive(Debug)]
pub enum LoginCommandOutput {
    LoginSuccess(TokenPair, String),
    LoginFailed(String),
}

#[relm4::component(pub)]
impl Component for LoginDialog {
    type Init = ();
    type Input = LoginInput;
    type Output = LoginOutput;
    type CommandOutput = LoginCommandOutput;

    view! {
        adw::Dialog {
            set_title: &gettext("Login to Instapaper"),
            set_content_width: 400,
            set_content_height: 300,

            #[wrap(Some)]
            set_child = &adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    #[wrap(Some)]
                    set_title_widget = &adw::WindowTitle {
                        set_title: &gettext("Login to Instapaper"),
                    },
                },

                #[wrap(Some)]
                set_content = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 24,
                    set_spacing: 16,

                    adw::PreferencesGroup {
                        set_title: &gettext("Credentials"),

                        adw::EntryRow {
                            set_title: &gettext("Email or Username"),
                            set_sensitive: !model.is_loading,
                            connect_changed[sender] => move |entry| {
                                sender.input(LoginInput::SetUsername(entry.text().to_string()));
                            },
                        },

                        adw::PasswordEntryRow {
                            set_title: &gettext("Password"),
                            set_sensitive: !model.is_loading,
                            connect_changed[sender] => move |entry| {
                                sender.input(LoginInput::SetPassword(entry.text().to_string()));
                            },
                            connect_activate => LoginInput::Submit,
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
                            connect_clicked => LoginInput::Cancel,
                        },

                        if model.is_loading {
                            adw::Spinner {
                                set_halign: gtk::Align::Center,
                                set_valign: gtk::Align::Center,
                            }
                        } else {
                            gtk::Button {
                                set_label: &gettext("Login"),
                                add_css_class: "suggested-action",
                                connect_clicked => LoginInput::Submit,
                            }
                        },
                    },
                },
            },

            connect_closed[sender] => move |_| {
                sender.input(LoginInput::Cancel);
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {
            username: String::new(),
            password: String::new(),
            error_message: None,
            is_loading: false,
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
            LoginInput::SetUsername(username) => {
                self.username = username;
                self.error_message = None;
            }
            LoginInput::SetPassword(password) => {
                self.password = password;
                self.error_message = None;
            }
            LoginInput::Submit => {
                if self.username.is_empty() || self.password.is_empty() {
                    self.error_message = Some(gettext("Please enter both username and password"));
                    return;
                }

                self.is_loading = true;
                self.error_message = None;

                let username = self.username.clone();
                let password = self.password.clone();

                sender.oneshot_command(async move {
                    let client = instapaper::client();

                    match instapaper::authenticate(&client, &username, &password).await {
                        Ok(tokens) => {
                            // Verify credentials and get username
                            match instapaper::verify_credentials(&client, &tokens).await {
                                Ok(user) => LoginCommandOutput::LoginSuccess(tokens, user.username),
                                Err(_) => LoginCommandOutput::LoginSuccess(tokens, username),
                            }
                        }
                        Err(instapaper::InstapaperError::InvalidCredentials) => {
                            LoginCommandOutput::LoginFailed(gettext("Invalid username or password"))
                        }
                        Err(instapaper::InstapaperError::RateLimited) => {
                            LoginCommandOutput::LoginFailed(gettext(
                                "Rate limited. Please try again later",
                            ))
                        }
                        Err(instapaper::InstapaperError::ServiceUnavailable) => {
                            LoginCommandOutput::LoginFailed(gettext(
                                "Instapaper is currently unavailable",
                            ))
                        }
                        Err(e) => LoginCommandOutput::LoginFailed(format!(
                            "{}: {:?}",
                            gettext("Login failed"),
                            e
                        )),
                    }
                });
            }
            LoginInput::Cancel => {
                root.close();
                let _ = sender.output(LoginOutput::Cancelled);
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
            LoginCommandOutput::LoginSuccess(tokens, username) => {
                self.is_loading = false;
                root.close();
                let _ = sender.output(LoginOutput::LoggedIn(tokens, username));
            }
            LoginCommandOutput::LoginFailed(error) => {
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

    #[gtk::test]
    fn test_init_component() {
        let tester = ComponentTester::<LoginDialog>::launch(());
        tester.process_events();

        // Component should initialize with empty values
        assert_eq!(tester.model().username, "");
        assert_eq!(tester.model().password, "");
        assert_eq!(tester.model().error_message, None);
        assert_eq!(tester.model().is_loading, false);
    }

    #[gtk::test]
    fn test_set_username() {
        let tester = ComponentTester::<LoginDialog>::launch(());
        tester.send_input(LoginInput::SetUsername("test@example.com".to_string()));
        tester.process_events();

        assert_eq!(tester.model().username, "test@example.com");
        assert_eq!(tester.model().error_message, None);
    }

    #[gtk::test]
    fn test_set_password() {
        let tester = ComponentTester::<LoginDialog>::launch(());
        tester.send_input(LoginInput::SetPassword("password123".to_string()));
        tester.process_events();

        assert_eq!(tester.model().password, "password123");
        assert_eq!(tester.model().error_message, None);
    }

    #[gtk::test]
    fn test_submit_with_empty_username() {
        let tester = ComponentTester::<LoginDialog>::launch(());
        tester.send_input(LoginInput::SetPassword("password123".to_string()));
        tester.send_input(LoginInput::Submit);
        tester.process_events();

        assert_eq!(
            tester.model().error_message,
            Some(gettext("Please enter both username and password"))
        );
        assert_eq!(tester.model().is_loading, false);
    }

    #[gtk::test]
    fn test_submit_with_empty_password() {
        let tester = ComponentTester::<LoginDialog>::launch(());
        tester.send_input(LoginInput::SetUsername("test@example.com".to_string()));
        tester.send_input(LoginInput::Submit);
        tester.process_events();

        assert_eq!(
            tester.model().error_message,
            Some(gettext("Please enter both username and password"))
        );
        assert_eq!(tester.model().is_loading, false);
    }

    #[gtk::test]
    fn test_submit_with_both_empty() {
        let tester = ComponentTester::<LoginDialog>::launch(());
        tester.send_input(LoginInput::Submit);
        tester.process_events();

        assert_eq!(
            tester.model().error_message,
            Some(gettext("Please enter both username and password"))
        );
        assert_eq!(tester.model().is_loading, false);
    }

    #[gtk::test]
    fn test_submit_with_valid_credentials_sets_loading() {
        let tester = ComponentTester::<LoginDialog>::launch(());
        tester.send_input(LoginInput::SetUsername("test@example.com".to_string()));
        tester.send_input(LoginInput::SetPassword("password123".to_string()));
        tester.send_input(LoginInput::Submit);
        tester.process_events();

        // After submit with valid credentials, loading should be true
        // (async operation will be in progress)
        assert_eq!(tester.model().is_loading, true);
        assert_eq!(tester.model().error_message, None);
    }

    #[gtk::test]
    fn test_cancel_sends_output() {
        let tester = ComponentTester::<LoginDialog>::launch(());
        tester.send_input(LoginInput::Cancel);
        tester.process_events();

        // Check that Cancelled output was sent
        let output = tester.try_recv_output();
        assert!(matches!(output, Some(LoginOutput::Cancelled)));
    }

    #[gtk::test]
    fn test_error_clears_on_username_change() {
        let tester = ComponentTester::<LoginDialog>::launch(());

        // Trigger an error
        tester.send_input(LoginInput::Submit);
        tester.process_events();
        assert!(tester.model().error_message.is_some());

        // Change username should clear error
        tester.send_input(LoginInput::SetUsername("test@example.com".to_string()));
        tester.process_events();
        assert_eq!(tester.model().error_message, None);
    }

    #[gtk::test]
    fn test_error_clears_on_password_change() {
        let tester = ComponentTester::<LoginDialog>::launch(());

        // Trigger an error
        tester.send_input(LoginInput::Submit);
        tester.process_events();
        assert!(tester.model().error_message.is_some());

        // Change password should clear error
        tester.send_input(LoginInput::SetPassword("password123".to_string()));
        tester.process_events();
        assert_eq!(tester.model().error_message, None);
    }

    // Note: Testing update_cmd with async operations requires actually running the async code
    // or mocking the network layer, which is beyond the scope of basic component testing.
    // The async behavior is better tested through integration tests.

    // Note: Full widget introspection tests require a fully rendered view hierarchy.
    // In test mode without presenting the dialog, some GTK widgets may not be fully initialized.
    // Widget behavior is better tested through integration or UI tests.
}
