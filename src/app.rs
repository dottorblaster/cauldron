use relm4::{
    abstractions::Toaster,
    actions::{RelmAction, RelmActionGroup},
    adw,
    factory::FactoryVecDeque,
    gtk, main_application, Component, ComponentController, ComponentParts, ComponentSender,
    Controller,
};

use gtk::prelude::{
    ApplicationExt, ApplicationWindowExt, ButtonExt, EditableExt, GtkWindowExt, OrientableExt,
    SettingsExt, WidgetExt,
};
use gtk::{gio, glib};

use crate::article::{Article, ArticleInit, ArticleOutput, ArticleRenderer, ArticleRendererInput};
use crate::config::{APP_ID, PROFILE};
use crate::modals::about::AboutDialog;
use crate::modals::add_bookmark::{AddBookmarkDialog, AddBookmarkOutput};
use crate::modals::login::{LoginDialog, LoginOutput};
use crate::network::instapaper;
use crate::persistence::articles::{self, PersistedArticle};
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
    add_bookmark_dialog: Option<Controller<AddBookmarkDialog>>,
    article_renderer: Controller<ArticleRenderer>,
    search_mode: bool,
    search_query: String,
    all_articles: Vec<Article>,
}

#[derive(Debug)]
pub(super) enum AppMsg {
    Quit,
    StartLogin,
    LoginCompleted(TokenPair, String),
    LoginCancelled,
    Logout,
    ArticleSelected(String, String, String, String, f64),
    RefreshArticles,
    ArchiveArticle,
    CopyArticleUrl,
    OpenArticle,
    ShowAddBookmarkDialog,
    AddBookmarkCompleted(String),
    AddBookmarkCancelled,
    ToggleSearchMode,
    UpdateSearchQuery(String),
    ClearSearch,
}

#[derive(Debug)]
pub(super) enum CommandMsg {
    RefreshedArticles(Vec<Article>),
    ScrapedArticle(String),
    ArticleArchived(String),
    OpenUrl(String),
    BookmarkAdded,
    Error(String),
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

                        add_top_bar = if model.search_mode {
                            &adw::HeaderBar {
                                #[wrap(Some)]
                                set_title_widget = &gtk::SearchEntry {
                                    set_placeholder_text: Some("Search articles..."),
                                    connect_search_changed[sender] => move |entry| {
                                        sender.input(AppMsg::UpdateSearchQuery(entry.text().to_string()));
                                    },
                                    grab_focus: (),
                                },

                                pack_end = &gtk::Button {
                                    set_icon_name: "window-close-symbolic",
                                    set_tooltip_text: Some("Close search"),
                                    connect_clicked => AppMsg::ClearSearch,
                                },
                            }
                        } else {
                            &adw::HeaderBar {
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

                                pack_end = &gtk::Box {
                                    gtk::Button {
                                        #[watch]
                                        set_visible: model.tokens.is_some(),
                                        set_icon_name: "system-search-symbolic",
                                        set_tooltip_text: Some("Search articles"),
                                        connect_clicked => AppMsg::ToggleSearchMode,
                                    },

                                    gtk::Button {
                                        #[watch]
                                        set_visible: model.tokens.is_some(),
                                        set_icon_name: "list-add-symbolic",
                                        set_tooltip_text: Some("Add bookmark"),
                                        connect_clicked => AppMsg::ShowAddBookmarkDialog,
                                    },

                                    gtk::MenuButton {
                                        set_icon_name: "open-menu-symbolic",
                                        set_menu_model: Some(&primary_menu),
                                    },
                                },
                            }
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
                                set_vscrollbar_policy: gtk::PolicyType::Automatic,
                                set_hscrollbar_policy: gtk::PolicyType::Never,

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
        let mut articles = FactoryVecDeque::builder()
            .launch(gtk::ListBox::default())
            .forward(sender.input_sender(), |output| match output {
                ArticleOutput::ArticleSelected(title, uri, item_id, description, time) => {
                    AppMsg::ArticleSelected(title, uri, item_id, description, time)
                }
            });

        let cached_articles = articles::read_articles().unwrap_or_default();

        let all_articles: Vec<Article> = cached_articles
            .iter()
            .map(|article| Article {
                title: article.title.clone(),
                uri: article.uri.clone(),
                item_id: article.item_id.clone(),
                description: article.description.clone(),
                time: article.time,
            })
            .collect();

        cached_articles.iter().for_each(|article| {
            articles.guard().push_back(ArticleInit {
                title: article.title.clone(),
                uri: article.uri.clone(),
                item_id: article.item_id.clone(),
                description: article.description.clone(),
                time: article.time,
            });
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
            add_bookmark_dialog: None,
            article_renderer,
            search_mode: false,
            search_query: String::new(),
            all_articles,
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
            AppMsg::ArticleSelected(title, uri, item_id, description, time) => {
                self.article_title = Some(title.clone());
                self.article_uri = Some(uri.clone());
                self.article_item_id = Some(item_id);

                self.article_renderer
                    .emit(ArticleRendererInput::SetTitle(title));

                self.article_renderer
                    .emit(ArticleRendererInput::SetMetadata {
                        url: uri.clone(),
                        description: description.clone(),
                        time,
                    });

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
                let _ = articles::clear_articles();
                self.tokens = None;
                self.username = String::new();
                self.articles.guard().clear();
                self.article_html = None;
                self.article_uri = None;
                self.article_item_id = None;
                self.all_articles.clear();
                self.search_query.clear();
                self.search_mode = false;
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
            AppMsg::ShowAddBookmarkDialog => {
                if let Some(tokens) = self.tokens.clone() {
                    let add_bookmark_dialog = AddBookmarkDialog::builder().launch(tokens).forward(
                        sender.input_sender(),
                        |output| match output {
                            AddBookmarkOutput::BookmarkAdded(url) => {
                                AppMsg::AddBookmarkCompleted(url)
                            }
                            AddBookmarkOutput::Cancelled => AppMsg::AddBookmarkCancelled,
                        },
                    );
                    self.add_bookmark_dialog = Some(add_bookmark_dialog);
                }
            }
            AppMsg::AddBookmarkCompleted(url) => {
                if let Some(tokens) = self.tokens.clone() {
                    sender.oneshot_command(async move {
                        let client = instapaper::client();
                        match instapaper::add_bookmark(&client, &tokens, &url).await {
                            Ok(_) => CommandMsg::BookmarkAdded,
                            Err(e) => CommandMsg::Error(format!("Failed to add bookmark: {:?}", e)),
                        }
                    });
                }
                self.add_bookmark_dialog = None;
            }
            AppMsg::AddBookmarkCancelled => {
                self.add_bookmark_dialog = None;
            }
            AppMsg::ToggleSearchMode => {
                self.search_mode = !self.search_mode;
                if !self.search_mode {
                    self.search_query.clear();
                    self.rebuild_article_list();
                }
            }
            AppMsg::UpdateSearchQuery(query) => {
                self.search_query = query;
                self.rebuild_article_list();
            }
            AppMsg::ClearSearch => {
                self.search_mode = false;
                self.search_query.clear();
                self.rebuild_article_list();
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

                self.all_articles = entries.clone();
                self.rebuild_article_list();

                let persisted: Vec<PersistedArticle> = entries
                    .iter()
                    .map(|a| PersistedArticle {
                        title: a.title.clone(),
                        uri: a.uri.clone(),
                        item_id: a.item_id.clone(),
                        description: a.description.clone(),
                        time: a.time,
                    })
                    .collect();

                if let Err(e) = articles::save_articles(&persisted) {
                    eprintln!("Failed to save articles cache: {}", e);
                }
            }
            CommandMsg::ScrapedArticle(html) => {
                self.article_html = Some(html.clone());
                self.article_renderer
                    .emit(ArticleRendererInput::SetContent(html));
            }
            CommandMsg::ArticleArchived(item_id) => {
                self.all_articles.retain(|a| a.item_id != item_id);

                self.article_html = None;
                self.article_title = None;
                self.article_uri = None;
                self.article_item_id = None;
                sender.input(AppMsg::RefreshArticles);
            }
            CommandMsg::OpenUrl(url) => {
                open::that(url).expect("Could not open the browser");
            }
            CommandMsg::BookmarkAdded => {
                let toast = adw::Toast::builder()
                    .title("Bookmark added successfully")
                    .timeout(3)
                    .build();
                self.toaster.add_toast(toast);
                sender.input(AppMsg::RefreshArticles);
            }
            CommandMsg::Error(error) => {
                let toast = adw::Toast::builder().title(&error).timeout(5).build();
                self.toaster.add_toast(toast);
            }
        }
    }

    fn shutdown(&mut self, widgets: &mut Self::Widgets, _output: relm4::Sender<Self::Output>) {
        let current_articles: Vec<PersistedArticle> = self
            .articles
            .iter()
            .map(|a| PersistedArticle {
                title: a.title.clone(),
                uri: a.uri.clone(),
                item_id: a.item_id.clone(),
                description: a.description.clone(),
                time: a.time,
            })
            .collect();
        let _ = articles::save_articles(&current_articles);

        widgets.save_window_size().unwrap();
    }
}

impl App {
    fn filter_articles(&self) -> Vec<ArticleInit> {
        if self.search_query.is_empty() {
            self.all_articles
                .iter()
                .map(|a| ArticleInit {
                    title: a.title.clone(),
                    uri: a.uri.clone(),
                    item_id: a.item_id.clone(),
                    description: a.description.clone(),
                    time: a.time,
                })
                .collect()
        } else {
            let query_lower = self.search_query.to_lowercase();
            self.all_articles
                .iter()
                .filter(|a| a.title.to_lowercase().contains(&query_lower))
                .map(|a| ArticleInit {
                    title: a.title.clone(),
                    uri: a.uri.clone(),
                    item_id: a.item_id.clone(),
                    description: a.description.clone(),
                    time: a.time,
                })
                .collect()
        }
    }

    fn rebuild_article_list(&mut self) {
        let filtered = self.filter_articles();
        self.articles.guard().clear();
        for article in filtered {
            self.articles.guard().push_back(article);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_articles_logic() {
        let article1 = Article {
            title: "Rust Programming Language".to_string(),
            uri: "https://example.com/rust".to_string(),
            item_id: "1".to_string(),
            description: "About Rust".to_string(),
            time: 0.0,
        };

        let article2 = Article {
            title: "Python Tutorial".to_string(),
            uri: "https://example.com/python".to_string(),
            item_id: "2".to_string(),
            description: "About Python".to_string(),
            time: 0.0,
        };

        let article3 = Article {
            title: "Advanced Rust Patterns".to_string(),
            uri: "https://example.com/rust-patterns".to_string(),
            item_id: "3".to_string(),
            description: "Rust patterns".to_string(),
            time: 0.0,
        };

        let all_articles = vec![article1.clone(), article2.clone(), article3.clone()];

        let filter_by_query = |query: &str| -> Vec<ArticleInit> {
            if query.is_empty() {
                all_articles
                    .iter()
                    .map(|a| ArticleInit {
                        title: a.title.clone(),
                        uri: a.uri.clone(),
                        item_id: a.item_id.clone(),
                        description: a.description.clone(),
                        time: a.time,
                    })
                    .collect()
            } else {
                let query_lower = query.to_lowercase();
                all_articles
                    .iter()
                    .filter(|a| a.title.to_lowercase().contains(&query_lower))
                    .map(|a| ArticleInit {
                        title: a.title.clone(),
                        uri: a.uri.clone(),
                        item_id: a.item_id.clone(),
                        description: a.description.clone(),
                        time: a.time,
                    })
                    .collect()
            }
        };

        let filtered = filter_by_query("rust");
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].title, "Rust Programming Language");
        assert_eq!(filtered[1].title, "Advanced Rust Patterns");

        let filtered_upper = filter_by_query("RUST");
        assert_eq!(filtered_upper.len(), 2);

        let filtered_none = filter_by_query("javascript");
        assert_eq!(filtered_none.len(), 0);

        let filtered_empty = filter_by_query("");
        assert_eq!(filtered_empty.len(), 3);

        let filtered_partial = filter_by_query("python");
        assert_eq!(filtered_partial.len(), 1);
        assert_eq!(filtered_partial[0].title, "Python Tutorial");
    }
}
