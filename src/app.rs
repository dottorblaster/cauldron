use relm4::{
    abstractions::Toaster,
    actions::{RelmAction, RelmActionGroup},
    adw,
    factory::FactoryVecDeque,
    gtk, main_application, Component, ComponentController, ComponentParts, ComponentSender,
    Controller,
};

use gtk::prelude::{
    ApplicationExt, ApplicationWindowExt, ButtonExt, GtkWindowExt, OrientableExt, SettingsExt,
    WidgetExt,
};
use gtk::{gio, glib};

use crate::article::{Article, ArticleOutput, ArticleRenderer, ArticleRendererInput};
use crate::config::{APP_ID, PROFILE};
use crate::modals::about::AboutDialog;
use crate::modals::login::{LoginDialog, LoginOutput};
use crate::network::instapaper;
use crate::persistence::token::{self, TokenPair};
use article_scraper::{FtrConfigEntry, FullTextParser, Readability};
use reqwest::Client;
use url::Url;

pub(super) struct App {
    loading: bool,
    tokens: Option<TokenPair>,
    username: String,
    articles: FactoryVecDeque<Article>,
    article_html: Option<String>,
    article_title: Option<String>,
    article_uri: Option<String>,
    article_item_id: Option<String>,
    toaster: Toaster,
    login_dialog: Option<Controller<LoginDialog>>,
    article_renderer: Controller<ArticleRenderer>,
}

#[derive(Debug)]
pub(super) enum AppMsg {
    Quit,
    StartLogin,
    LoginCompleted(TokenPair, String),
    LoginCancelled,
    Logout,
    ArticleSelected(String, String, String),
    RefreshArticles,
    ArchiveArticle,
    CopyArticleUrl,
    OpenArticle,
}

#[derive(Debug)]
pub(super) enum CommandMsg {
    RefreshedArticles(Vec<Article>),
    ScrapedArticle(String),
    ArticleArchived(String),
    OpenUrl(String),
}

relm4::new_action_group!(pub(super) WindowActionGroup, "win");
relm4::new_stateless_action!(PreferencesAction, WindowActionGroup, "preferences");
relm4::new_stateless_action!(pub(super) ShortcutsAction, WindowActionGroup, "show-help-overlay");
relm4::new_stateless_action!(AboutAction, WindowActionGroup, "about");
relm4::new_stateless_action!(LogoutAction, WindowActionGroup, "logout");

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
                "_Logout" => LogoutAction,
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
            adw::NavigationSplitView {
                #[wrap(Some)]
                set_sidebar = &adw::NavigationPage {
                    adw::ToolbarView {
                        set_top_bar_style: adw::ToolbarStyle::Raised,

                        add_top_bar = &adw::HeaderBar {
                            pack_start = if model.loading {
                                &adw::Spinner {
                                    set_halign: gtk::Align::Center,
                                    set_valign: gtk::Align::Center,
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

                        #[wrap(Some)]
                        set_content = &gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,

                            gtk::Button::with_label("Login") {
                                #[watch]
                                set_visible: model.tokens.is_none(),
                                connect_clicked => AppMsg::StartLogin,
                            },

                            gtk::ScrolledWindow {
                                #[watch]
                                set_visible: model.tokens.is_some(),
                                add_css_class: "navigation-sidebar",
                                set_propagate_natural_height: true,

                                gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,

                                    #[local_ref]
                                    articles_list_box -> gtk::ListBox {
                                        set_selection_mode: gtk::SelectionMode::Single,
                                        add_css_class: "navigation-sidebar",
                                    }
                                }
                            }
                        }
                    },
                },

                adw::NavigationPage {
                    #[local_ref]
                    toast_overlay -> adw::ToastOverlay {
                        set_vexpand: true,

                        adw::ToolbarView {
                          set_top_bar_style: adw::ToolbarStyle::Raised,

                          add_top_bar = &adw::HeaderBar {
                                #[name = "back_button"]
                                pack_start = &gtk::Box{
                                    gtk::Button {
                                        set_icon_name: "shoe-box-symbolic",
                                        connect_clicked => AppMsg::ArchiveArticle
                                    },
                                    gtk::Button {
                                        set_icon_name: "edit-copy-symbolic",
                                        connect_clicked => AppMsg::CopyArticleUrl
                                    },
                                    gtk::Button {
                                        set_icon_name: "compass-symbolic",
                                        connect_clicked => AppMsg::OpenArticle
                                    },
                                },

                                #[wrap(Some)]
                                set_title_widget = &adw::WindowTitle {
                                    set_title: "Cauldron",
                                }
                            },

                            #[wrap(Some)]
                            set_content = &gtk::Box {
                                set_hexpand: true,
                                 gtk::Label {
                                    #[watch]
                                    set_visible: model.article_html.is_none(),
                                    add_css_class: "title-1",
                                    set_hexpand: true,
                                    set_text: "Select an article",
                                },
                                #[local_ref]
                                article_renderer_widget -> gtk::ScrolledWindow {
                                    #[watch]
                                    set_visible: model.article_html.is_some(),
                                },
                            }
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
        let tokens = match token::read_tokens() {
            Ok(t) => Some(t),
            Err(_) => None,
        };

        let username = String::new();
        let articles = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(sender.input_sender(), |output| match output {
                ArticleOutput::ArticleSelected(title, uri, item_id) => {
                    AppMsg::ArticleSelected(title, uri, item_id)
                }
            });

        let article_renderer = ArticleRenderer::builder().launch(()).detach();

        let model = Self {
            tokens,
            username,
            articles,
            article_html: None,
            article_title: None,
            article_uri: None,
            article_item_id: None,
            loading: false,
            toaster: Toaster::default(),
            login_dialog: None,
            article_renderer,
        };

        let toast_overlay = model.toaster.overlay_widget();

        let articles_list_box = model.articles.widget();

        let article_renderer_widget = model.article_renderer.widget();

        let widgets = view_output!();

        let mut actions = RelmActionGroup::<WindowActionGroup>::new();

        let shortcuts_action = {
            let shortcuts = widgets.shortcuts.clone();
            RelmAction::<ShortcutsAction>::new_stateless(move |_| {
                shortcuts.present();
            })
        };

        let about_action = {
            RelmAction::<AboutAction>::new_stateless(move |_| {
                AboutDialog::builder().launch(()).detach();
            })
        };

        let logout_action = {
            let sender_clone = sender.clone();
            RelmAction::<LogoutAction>::new_stateless(move |_| {
                sender_clone.input(AppMsg::Logout);
            })
        };

        actions.add_action(shortcuts_action);
        actions.add_action(about_action);
        actions.add_action(logout_action);
        actions.register_for_widget(&widgets.main_window);

        widgets.load_window_size();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>, _: &Self::Root) {
        match message {
            AppMsg::Quit => main_application().quit(),
            AppMsg::ArticleSelected(title, uri, item_id) => {
                self.article_title = Some(title.clone());
                self.article_uri = Some(uri.clone());
                self.article_item_id = Some(item_id);

                self.article_renderer
                    .emit(ArticleRendererInput::SetTitle(title));

                sender.oneshot_command(async move {
                    let article = get_html(Some(uri)).await;
                    let html = Readability::extract(&article, None).await;
                    CommandMsg::ScrapedArticle(html.unwrap())
                });
            }
            AppMsg::StartLogin => {
                let login_dialog =
                    LoginDialog::builder()
                        .launch(())
                        .forward(sender.input_sender(), |output| match output {
                            LoginOutput::LoggedIn(tokens, username) => {
                                AppMsg::LoginCompleted(tokens, username)
                            }
                            LoginOutput::Cancelled => AppMsg::LoginCancelled,
                        });

                self.login_dialog = Some(login_dialog);
            }
            AppMsg::LoginCompleted(tokens, username) => {
                let _ = token::save_tokens(&tokens);
                self.tokens = Some(tokens);
                self.username = username;
                self.login_dialog = None;
                sender.input(AppMsg::RefreshArticles);
            }
            AppMsg::LoginCancelled => {
                self.login_dialog = None;
            }
            AppMsg::Logout => {
                println!("porco dio");
                let _ = token::clear_tokens();
                self.tokens = None;
                self.username = String::new();
                self.articles.guard().clear();
                self.article_html = None;
                self.article_uri = None;
                self.article_item_id = None;
            }
            AppMsg::RefreshArticles => {
                if let Some(tokens) = self.tokens.clone() {
                    self.loading = true;

                    sender.oneshot_command(async move {
                        let client = instapaper::client();
                        let entries = instapaper::get_bookmarks(&client, &tokens).await;

                        match entries {
                            Ok(bookmarks) => {
                                let parsed_entries =
                                    crate::article::parse_instapaper_response(bookmarks);
                                CommandMsg::RefreshedArticles(parsed_entries)
                            }
                            Err(_) => CommandMsg::RefreshedArticles(vec![]),
                        }
                    });
                }
            }
            AppMsg::ArchiveArticle => {
                if let (Some(tokens), Some(item_id)) =
                    (self.tokens.clone(), self.article_item_id.clone())
                {
                    sender.oneshot_command(async move {
                        let client = instapaper::client();
                        let bookmark_id: i64 = item_id.parse().unwrap_or(0);
                        let _ = instapaper::archive_bookmark(&client, &tokens, bookmark_id).await;

                        CommandMsg::ArticleArchived(item_id)
                    });
                }
            }
            AppMsg::CopyArticleUrl => match self.article_uri.clone() {
                Some(uri) => {
                    let _ = crate::persistence::clipboard::copy(&uri);
                    let toast = adw::Toast::builder()
                        .title("URL copied to clipboard.")
                        .timeout(3000)
                        .build();
                    self.toaster.add_toast(toast);
                }
                None => {}
            },
            AppMsg::OpenArticle => {
                if let Some(uri) = self.article_uri.clone() {
                    sender.oneshot_command(async move { CommandMsg::OpenUrl(uri.to_owned()) });
                }
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
            CommandMsg::ScrapedArticle(html) => {
                self.article_html = Some(html.clone());
                self.article_renderer
                    .emit(ArticleRendererInput::SetContent(html));
            }
            CommandMsg::ArticleArchived(_item_id) => {
                self.article_html = None;
                self.article_title = None;
                self.article_uri = None;
                self.article_item_id = None;
                sender.input(AppMsg::RefreshArticles);
            }
            CommandMsg::OpenUrl(url) => {
                open::that(url).expect("Could not open the browser");
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
