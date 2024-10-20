use relm4::{
    actions::{RelmAction, RelmActionGroup},
    adw,
    factory::FactoryVecDeque,
    gtk, main_application, Component, ComponentController, ComponentParts, ComponentSender,
    Controller,
};

use gio::prelude::{ApplicationExtManual, FileExt};
use gtk::prelude::{
    ApplicationExt, ApplicationWindowExt, ButtonExt, GtkWindowExt, OrientableExt, SettingsExt,
    WidgetExt,
};
use gtk::{gio, glib};
use webkit6::{
    prelude::WebViewExt, UserContentInjectedFrames, UserStyleLevel, UserStyleSheet, WebView,
};

use crate::article::{Article, ArticleOutput};
use crate::config::{APP_ID, PROFILE, RESOURCES_FILE};
use crate::modals::about::AboutDialog;
use crate::network::pocket;
use crate::persistence::token;
use article_scraper::{FtrConfigEntry, FullTextParser, Readability};
use reqwest::Client;
use url::Url;

pub(super) struct App {
    about_dialog: Controller<AboutDialog>,
    loading: bool,
    auth_code: String,
    access_token: String,
    username: String,
    articles: FactoryVecDeque<Article>,
    article_html: Option<String>,
    article_uri: Option<String>,
    article_item_id: Option<String>,
}

#[derive(Debug)]
pub(super) enum AppMsg {
    Quit,
    StartLogin,
    Open(String),
    ArticleSelected(String, String),
    RefreshArticles,
    ArchiveArticle,
}

#[derive(Debug)]
pub(super) enum CommandMsg {
    RefreshedArticles(Vec<Article>),
    ScrapedArticle(String),
    SetToken((String, String)),
    SetAuthCode(String),
    ArticleArchived(String),
}

relm4::new_action_group!(pub(super) WindowActionGroup, "win");
relm4::new_stateless_action!(PreferencesAction, WindowActionGroup, "preferences");
relm4::new_stateless_action!(pub(super) ShortcutsAction, WindowActionGroup, "show-help-overlay");
relm4::new_stateless_action!(AboutAction, WindowActionGroup, "about");

#[relm4::component(pub)]
impl Component for App {
    type Init = ();
    type Input = AppMsg;
    type Output = ();
    type CommandOutput = CommandMsg;
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
                    set_width_request: 350,
                    set_orientation: gtk::Orientation::Vertical,

                    adw::HeaderBar {
                        set_show_end_title_buttons: false,
                        set_show_title: false,
                        pack_start = if model.loading {
                            &gtk::Spinner {
                                set_spinning: true,
                            }
                        } else {
                            &gtk::Button {
                                set_icon_name: "view-refresh-symbolic",
                                connect_clicked => AppMsg::RefreshArticles
                            }
                        },

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
                    gtk::ScrolledWindow {
                        add_css_class: "navigation-sidebar",
                        set_propagate_natural_height: true,

                        gtk::Box {
                            set_margin_end: 12,
                            set_margin_top: 12,
                            set_margin_start: 12,
                            set_margin_bottom: 12,
                            set_orientation: gtk::Orientation::Vertical,

                            #[local_ref]
                            articles_list_box -> gtk::ListBox {
                                #[watch]
                                set_visible: !model.access_token.is_empty(),
                                set_selection_mode: gtk::SelectionMode::Single,
                                add_css_class: "navigation-sidebar",
                            }
                        }
                    }
                },

                append = &gtk::Separator {
                    set_orientation: gtk::Orientation::Vertical,
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_hexpand: true,

                    #[name = "content_header"]
                    adw::HeaderBar {
                        #[name = "back_button"]
                        pack_start = &gtk::Button {
                            set_icon_name: "shoe-box-symbolic",
                            connect_clicked => AppMsg::ArchiveArticle
                        },

                        #[wrap(Some)]
                        set_title_widget = &adw::WindowTitle {
                            set_title: "Cauldron",
                        }
                    },
                    gtk::Label {
                        #[watch]
                        set_visible: model.article_html.is_none(),
                        add_css_class: "title-1",
                        set_vexpand: true,
                        set_text: "Select an article",
                    },
                    gtk::ScrolledWindow {
                        #[watch]
                        set_visible: model.article_html.is_some(),
                        set_propagate_natural_height: true,
                        set_vexpand: true,

                        WebView {
                            set_widget_name: "browser",
                            connect_resource_load_started: |webview, _, _| {
                                let res = gio::Resource::load(RESOURCES_FILE).expect("Could not load gresource file");
                                gio::resources_register(&res);

                                let data = res
                                    .lookup_data(
                                        "/it/dottorblaster/cauldron/article_view/style.css",
                                        gio::ResourceLookupFlags::NONE,
                                    )
                                    .unwrap();
                                let css_string = &glib::GString::from_utf8_checked(data.to_vec()).unwrap();

                                let user_style_sheet = UserStyleSheet::new(
                                    css_string,
                                    UserContentInjectedFrames::TopFrame,
                                    UserStyleLevel::User,
                                    &[],
                                    &[],
                                );

                                match webview.user_content_manager() {
                                    Some(content_manager) => {
                                        content_manager.add_style_sheet(&user_style_sheet);
                                    },
                                    None => {}
                                }
                            },
                            #[watch]
                            load_html: (&model.article_html.clone().unwrap_or_default(), Some("https://dottorblaster.it"))
                        },
                    },
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

        let access_token = match token::read_token() {
            Ok(token) => token,
            Err(_) => String::new(),
        };

        let username = String::new();
        let articles = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(sender.input_sender(), |output| match output {
                ArticleOutput::ArticleSelected(uri, item_id) => {
                    AppMsg::ArticleSelected(uri, item_id)
                }
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
            article_html: None,
            article_uri: None,
            article_item_id: None,
            loading: false,
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

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>, _: &Self::Root) {
        match message {
            AppMsg::Quit => main_application().quit(),
            AppMsg::ArticleSelected(uri, item_id) => {
                self.article_uri = Some(uri.clone());
                self.article_item_id = Some(item_id);

                sender.oneshot_command(async move {
                    let article = get_html(Some(uri)).await;
                    let html = Readability::extract(&article, None).await;
                    CommandMsg::ScrapedArticle(html.unwrap())
                });
            }
            AppMsg::StartLogin => {
                sender.oneshot_command(async {
                    let client = pocket::client();
                    let code_response = pocket::initiate_login(&client).await;
                    CommandMsg::SetAuthCode(code_response.code)
                });
            }
            AppMsg::Open(_uri) => {
                let auth_code = self.auth_code.clone();

                sender.oneshot_command(async move {
                    let client = pocket::client();

                    let authorization_response = pocket::authorize(&client, &auth_code).await;

                    CommandMsg::SetToken((
                        authorization_response.username,
                        authorization_response.access_token,
                    ))
                });
            }
            AppMsg::RefreshArticles => {
                let access_token = self.access_token.clone();
                self.loading = true;

                sender.oneshot_command(async move {
                    let client = pocket::client();
                    let entries = pocket::get_entries(&client, &access_token).await;

                    let parsed_entries = crate::article::parse_json_response(entries);

                    CommandMsg::RefreshedArticles(parsed_entries)
                });
            }
            AppMsg::ArchiveArticle => {
                let access_token = self.access_token.clone();
                let item_id = self.article_item_id.clone().unwrap();

                sender.oneshot_command(async move {
                    let client = pocket::client();
                    let _ = pocket::archive(&client, &access_token, &item_id).await;

                    CommandMsg::ArticleArchived(item_id)
                });
            }
        }
    }

    fn update_cmd(
        &mut self,
        message: Self::CommandOutput,
        sender: ComponentSender<Self>,
        _: &Self::Root,
    ) {
        match message {
            CommandMsg::RefreshedArticles(entries) => {
                self.loading = false;
                self.articles.guard().clear();
                entries.iter().for_each(
                    |Article {
                         title,
                         uri,
                         item_id,
                     }| {
                        self.articles.guard().push_back((
                            title.to_owned(),
                            uri.to_owned(),
                            item_id.to_owned(),
                        ));
                    },
                );
            }
            CommandMsg::ScrapedArticle(html) => self.article_html = Some(html),
            CommandMsg::SetAuthCode(auth_code) => {
                let pocket_uri = pocket::encode_pocket_uri(&auth_code);

                open::that(pocket_uri).expect("Could not open the browser");
                auth_code.clone_into(&mut self.auth_code)
            }
            CommandMsg::SetToken((username, access_token)) => {
                self.username = username;
                self.access_token = access_token;

                let _ = token::save_token(&self.access_token);
                sender.input(AppMsg::RefreshArticles);
            }
            CommandMsg::ArticleArchived(_item_id) => {
                self.article_html = None;
                self.article_uri = None;
                self.article_item_id = None;
                sender.input(AppMsg::RefreshArticles);
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

async fn get_html(source_url: Option<String>) -> String {
    let source_url = source_url.map(|url| Url::parse(&url).expect("invalid source url"));

    if let Some(source_url) = source_url {
        match FullTextParser::download(
            &source_url,
            &Client::new(),
            None,
            &FtrConfigEntry::default(),
        )
        .await
        {
            Ok(html) => html,
            Err(_err) => "".to_owned(),
        }
    } else {
        unreachable!()
    }
}
