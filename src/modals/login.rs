use gtk::prelude::{BoxExt, ButtonExt, EditableExt, OrientableExt, WidgetExt};
use relm4::{
    adw,
    adw::prelude::{AdwDialogExt, PreferencesGroupExt, PreferencesRowExt},
    gtk, Component, ComponentParts, ComponentSender, RelmWidgetExt,
};
use webkit6::prelude::{GtkApplicationExt, ListBoxRowExt};

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

#[derive(Debug)]
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
            set_title: "Login to Instapaper",
            set_content_width: 400,
            set_content_height: 300,

            #[wrap(Some)]
            set_child = &adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    #[wrap(Some)]
                    set_title_widget = &adw::WindowTitle {
                        set_title: "Login to Instapaper",
                    },
                },

                #[wrap(Some)]
                set_content = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_margin_all: 24,
                    set_spacing: 16,

                    adw::PreferencesGroup {
                        set_title: "Credentials",

                        adw::EntryRow {
                            set_title: "Email or Username",
                            set_sensitive: !model.is_loading,
                            connect_changed[sender] => move |entry| {
                                sender.input(LoginInput::SetUsername(entry.text().to_string()));
                            },
                        },

                        adw::PasswordEntryRow {
                            set_title: "Password",
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
                            set_label: "Cancel",
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
                                set_label: "Login",
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

        root.present(Some(&relm4::main_application().windows()[0]));

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
                    self.error_message =
                        Some("Please enter both username and password".to_string());
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
                            LoginCommandOutput::LoginFailed(
                                "Invalid username or password".to_string(),
                            )
                        }
                        Err(instapaper::InstapaperError::RateLimited) => {
                            LoginCommandOutput::LoginFailed(
                                "Rate limited. Please try again later.".to_string(),
                            )
                        }
                        Err(instapaper::InstapaperError::ServiceUnavailable) => {
                            LoginCommandOutput::LoginFailed(
                                "Instapaper is currently unavailable".to_string(),
                            )
                        }
                        Err(e) => LoginCommandOutput::LoginFailed(format!("Login failed: {:?}", e)),
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
