use relm4::{
    actions::{RelmAction, RelmActionGroup},
    adw, gtk, main_application, Component, ComponentController, ComponentParts, ComponentSender,
    Controller, SimpleComponent,
};

use adw::prelude::PreferencesRowExt;
use gio::prelude::{ApplicationExtManual, FileExt};
use gtk::prelude::{
    ApplicationExt, ApplicationWindowExt, ButtonExt, GtkWindowExt, OrientableExt, SettingsExt,
    WidgetExt,
};
use gtk::{gio, glib};

use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};
use url::form_urlencoded;

use crate::config::{APP_ID, PROFILE};
use crate::modals::about::AboutDialog;
use crate::types::{
    PocketAccessTokenRequest, PocketAccessTokenResponse, PocketCodeResponse, PocketEntriesRequest,
};

pub(super) struct App {
    about_dialog: Controller<AboutDialog>,
    auth_code: String,
    access_token: String,
    username: String,
    articles: Vec<String>,
}

#[derive(Debug)]
pub(super) enum AppMsg {
    Quit,
    StartLogin,
    Open(String),
}

relm4::new_action_group!(pub(super) WindowActionGroup, "win");
relm4::new_stateless_action!(PreferencesAction, WindowActionGroup, "preferences");
relm4::new_stateless_action!(pub(super) ShortcutsAction, WindowActionGroup, "show-help-overlay");
relm4::new_stateless_action!(AboutAction, WindowActionGroup, "about");

#[relm4::component(pub)]
impl SimpleComponent for App {
    type Init = ();
    type Input = AppMsg;
    type Output = ();
    type Widgets = AppWidgets;

    menu! {
        primary_menu: {
            section! {
                "_Preferences" => PreferencesAction,
                "_Keyboard" => ShortcutsAction,
                "_About Cauldron" => AboutAction,
            }
        }
    }

    view! {
        main_window = adw::ApplicationWindow::new(&main_application()) {
            set_visible: true,

            connect_close_request[sender] => move |_| {
                sender.input(AppMsg::Quit);
                glib::Propagation::Stop
            },

            #[wrap(Some)]
            set_help_overlay: shortcuts = &gtk::Builder::from_resource(
                    "/it/dottorblaster/cauldron/gtk/help-overlay.ui"
                )
                .object::<gtk::ShortcutsWindow>("help_overlay")
                .unwrap() -> gtk::ShortcutsWindow {
                    set_transient_for: Some(&main_window),
                    set_application: Some(&main_application()),
            },

            add_css_class?: if PROFILE == "Devel" {
                    Some("devel")
                } else {
                    None
                },
            adw::Leaflet{
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,

                    adw::HeaderBar {
                        pack_end = &gtk::MenuButton {
                            set_icon_name: "open-menu-symbolic",
                            set_menu_model: Some(&primary_menu),
                        },

                    },

                    gtk::Button::with_label("Login") {
                        #[watch]
                        set_visible: model.access_token.is_empty(),
                        connect_clicked => AppMsg::StartLogin,
                    },

                    gtk::ListBox {
                        #[watch]
                        set_visible: !model.access_token.is_empty(),
                        set_selection_mode: gtk::SelectionMode::Single,
                        add_css_class: "navigation-sidebar",

                        adw::ActionRow {
                            set_title: "Section 1",
                        },

                        adw::ActionRow {
                            set_title: "Section 2",
                        },

                        adw::ActionRow {
                            set_title: "Section 3",
                        },


                        connect_row_selected[sender] => move |_, row| {

                        }
                    }

                },

                append = &gtk::Separator {
                    set_orientation: gtk::Orientation::Vertical,
                } -> {
                    set_navigatable: false,
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_hexpand: true,

                    #[name = "content_header"]
                    adw::HeaderBar {
                        #[name = "back_button"]
                        pack_start = &gtk::Button {
                            set_icon_name: "go-previous-symbolic",
                            connect_clicked => move |_| {
                            }
                        },

                        #[wrap(Some)]
                        set_title_widget = &adw::WindowTitle {
                            set_title: "Content",
                        }
                    },

                    gtk::Label {
                        add_css_class: "title-1",
                        set_vexpand: true,

                        #[watch]
                        set_text: "Kekw",
                    }
                },

            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let open_sender = sender.clone();
        main_application().connect_open(move |_, files, _| {
            if let Some(uri) = files.first().map(|f| f.uri()) {
                open_sender.input(AppMsg::Open(uri.to_string()));
            } else {
                println!("No URI to open");
            }
        });

        let auth_code = String::new();
        let access_token = String::new();
        let username = String::new();
        let articles = vec![];

        let about_dialog = AboutDialog::builder()
            .transient_for(&root)
            .launch(())
            .detach();

        let model = Self {
            about_dialog,
            auth_code,
            access_token,
            username,
            articles,
        };

        let widgets = view_output!();

        let mut actions = RelmActionGroup::<WindowActionGroup>::new();

        let shortcuts_action = {
            let shortcuts = widgets.shortcuts.clone();
            RelmAction::<ShortcutsAction>::new_stateless(move |_| {
                shortcuts.present();
            })
        };

        let about_action = {
            let sender = model.about_dialog.sender().clone();
            RelmAction::<AboutAction>::new_stateless(move |_| {
                sender.send(()).unwrap();
            })
        };

        actions.add_action(shortcuts_action);
        actions.add_action(about_action);
        actions.register_for_widget(&widgets.main_window);

        widgets.load_window_size();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            AppMsg::Quit => main_application().quit(),
            AppMsg::StartLogin => {
                let client = reqwest::blocking::Client::new();
                let mut map = std::collections::HashMap::new();

                let mut headers = HeaderMap::new();
                headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
                headers.insert(
                    HeaderName::from_static("x-accept"),
                    HeaderValue::from_static("application/json"),
                );

                map.insert("consumer_key", "99536-5a753dbe04d6ade99e80b4ab");
                map.insert("redirect_uri", "pocket://kekw");

                let res = client
                    .post("https://getpocket.com/v3/oauth/request")
                    .headers(headers)
                    .json(&map)
                    .send()
                    .expect("Unexpected error");

                let code_response: PocketCodeResponse =
                    res.json().expect("Could not decode the response");
                println!("{:?}", code_response.code);

                let encoded_pocket_params: String = form_urlencoded::Serializer::new(String::new())
                    .append_pair("request_token", &code_response.code)
                    .append_pair("redirect_uri", "pocket://kekw")
                    .finish();

                let pocket_uri = format!(
                    "https://getpocket.com/auth/authorize?{}",
                    encoded_pocket_params
                );
                open::that(pocket_uri).expect("Could not open the browser");
                code_response.code.clone_into(&mut self.auth_code);
            }
            AppMsg::Open(uri) => {
                println!("{}", uri);
                println!("auth code: {}", self.auth_code);

                let request_params = PocketAccessTokenRequest {
                    consumer_key: "99536-5a753dbe04d6ade99e80b4ab".to_owned(),
                    code: self.auth_code.clone(),
                };
                let mut headers = HeaderMap::new();
                headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
                headers.insert(
                    HeaderName::from_static("x-accept"),
                    HeaderValue::from_static("application/json"),
                );

                let client = reqwest::blocking::Client::new();

                let res = client
                    .post("https://getpocket.com/v3/oauth/authorize")
                    .headers(headers.clone())
                    .json(&request_params)
                    .send()
                    .expect("Unexpected error");

                let code_response: PocketAccessTokenResponse =
                    res.json().expect("Could not decode the response");

                self.username = code_response.username;
                self.access_token = code_response.access_token;

                let request_params = PocketEntriesRequest {
                    consumer_key: "99536-5a753dbe04d6ade99e80b4ab".to_owned(),
                    access_token: self.access_token.clone(),
                    count: "30".to_owned(),
                };

                let entries: serde_json::Value = client
                    .post("https://getpocket.com/v3/get")
                    .headers(headers)
                    .json(&request_params)
                    .send()
                    .expect("Unexpected error")
                    .json()
                    .expect("lmao");

                println!("{}", entries);
            }
        }
    }

    fn shutdown(&mut self, widgets: &mut Self::Widgets, _output: relm4::Sender<Self::Output>) {
        widgets.save_window_size().unwrap();
    }
}

impl AppWidgets {
    fn save_window_size(&self) -> Result<(), glib::BoolError> {
        let settings = gio::Settings::new(APP_ID);
        let (width, height) = self.main_window.default_size();

        settings.set_int("window-width", width)?;
        settings.set_int("window-height", height)?;

        settings.set_boolean("is-maximized", self.main_window.is_maximized())?;

        Ok(())
    }

    fn load_window_size(&self) {
        let settings = gio::Settings::new(APP_ID);

        let width = settings.int("window-width");
        let height = settings.int("window-height");
        let is_maximized = settings.boolean("is-maximized");

        self.main_window.set_default_size(width, height);

        if is_maximized {
            self.main_window.maximize();
        }
    }
}
