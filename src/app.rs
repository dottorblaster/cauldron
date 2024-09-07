use relm4::{
    actions::{RelmAction, RelmActionGroup},
    adw,
    factory::FactoryVecDeque,
    gtk, main_application, Component, ComponentController, ComponentParts, ComponentSender,
    Controller, SimpleComponent,
};

use gio::prelude::{ApplicationExtManual, FileExt};
use gtk::prelude::{
    ApplicationExt, ApplicationWindowExt, ButtonExt, GtkWindowExt, OrientableExt, SettingsExt,
    WidgetExt,
};
use gtk::{gio, glib};

use crate::article::{Article, ArticleOutput};
use crate::config::{APP_ID, PROFILE};
use crate::modals::about::AboutDialog;
use crate::network;

pub(super) struct App {
    about_dialog: Controller<AboutDialog>,
    auth_code: String,
    access_token: String,
    username: String,
    articles: FactoryVecDeque<Article>,
}

#[derive(Debug)]
pub(super) enum AppMsg {
    Quit,
    StartLogin,
    Open(String),
    ArticleSelected(String),
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

                    #[local_ref]
                    articles_list_box -> gtk::ListBox {
                        #[watch]
                        set_visible: !model.access_token.is_empty(),
                        set_selection_mode: gtk::SelectionMode::Single,
                        add_css_class: "navigation-sidebar",
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
        let articles = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(sender.input_sender(), |output| match output {
                ArticleOutput::ArticleSelected(uri) => AppMsg::ArticleSelected(uri),
            });

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

        let articles_list_box = model.articles.widget();

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
            AppMsg::ArticleSelected(uri) => {
                println!("{}", uri)
            }
            AppMsg::StartLogin => {
                let client = network::client();
                let code_response = network::initiate_login(&client);
                println!("{:?}", code_response.code);

                let pocket_uri = network::encode_pocket_uri(&code_response.code);

                open::that(pocket_uri).expect("Could not open the browser");
                code_response.code.clone_into(&mut self.auth_code);
            }
            AppMsg::Open(_uri) => {
                let client = network::client();

                let authorization_response = network::authorize(&client, &self.auth_code);

                self.username = authorization_response.username;
                self.access_token = authorization_response.access_token;

                let entries = network::get_entries(&client, &self.access_token);
                println!("{}", entries);

                self.articles
                    .guard()
                    .push_back(("kekw".to_owned(), "asd".to_owned()));

                self.articles
                    .guard()
                    .push_back(("LELW".to_owned(), "AAA".to_owned()));
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
