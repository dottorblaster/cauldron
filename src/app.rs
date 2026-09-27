use relm4::{
    abstractions::Toaster,
    actions::{RelmAction, RelmActionGroup},
    adw::{self, prelude::AdwDialogExt},
    gtk, main_application, Component, ComponentController, ComponentParts, ComponentSender,
    Controller,
};

use gtk::prelude::{
    ApplicationExt, ButtonExt, Cast, EditableExt, GtkApplicationExt, GtkWindowExt, IsA,
    ListModelExt, OrientableExt, SettingsExt, WidgetExt,
};
use gtk::{gio, glib};

use gettextrs::{gettext, pgettext};

use crate::article::{Article, ArticleRenderer, ArticleRendererInput};
use crate::config::{APP_ID, PROFILE};
use crate::modals::about::AboutDialog;
use crate::modals::add_bookmark::{AddBookmarkDialog, AddBookmarkOutput};
use crate::modals::login::{LoginDialog, LoginOutput};
use crate::network::instapaper;
use crate::persistence::articles::{self, PersistedArticle};
use crate::persistence::token::{self, TokenPair};
use article_scraper::{FtrConfigEntry, FullTextParser, Readability};
use reqwest::Client;
use std::collections::HashSet;
use url::Url;

pub(super) struct App {
    loading: bool,
    tokens: Option<TokenPair>,
    username: String,
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
    visible_articles: Vec<Article>,
    sidebar: adw::Sidebar,
    sidebar_section: adw::SidebarSection,
    selected_item_id: Option<String>,
    selected_tag: Option<String>,
    available_tags: Vec<String>,
    tag_model: gtk::StringList,
}

#[derive(Debug)]
pub(super) enum AppMsg {
    Quit,
    StartLogin,
    LoginCompleted(TokenPair, String),
    LoginCancelled,
    Logout,
    ArticleSelected(String, String, String, String, f64, Vec<String>),
    ArticleActivated(u32),
    RefreshArticles,
    ArchiveArticle,
    CopyArticleUrl,
    OpenArticle,
    ShowAddBookmarkDialog,
    AddBookmarkCompleted(String, Vec<String>),
    AddBookmarkCancelled,
    ToggleSearchMode,
    UpdateSearchQuery(String),
    ClearSearch,
    SetTagFilter(Option<String>),
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
                &gettext("_Preferences") => PreferencesAction,
                &gettext("_Keyboard") => ShortcutsAction,
                &gettext("_About Cauldron") => AboutAction,
                &gettext("_Logout") => LogoutAction,
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
                                    set_placeholder_text: Some(&gettext("Search articles...")),
                                    connect_search_changed[sender] => move |entry| {
                                        sender.input(AppMsg::UpdateSearchQuery(entry.text().to_string()));
                                    },
                                    grab_focus: (),
                                },

                                pack_end = &gtk::Button {
                                    set_icon_name: "window-close-symbolic",
                                    set_tooltip_text: Some(&gettext("Close search")),
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

                                #[wrap(Some)]
                                set_title_widget = &gtk::DropDown::new(Some(model.tag_model.clone()), gtk::Expression::NONE) {
                                    #[watch]
                                    set_visible: model.tokens.is_some() && !model.available_tags.is_empty(),
                                    connect_selected_notify[sender] => move |dropdown| {
                                        let selected = dropdown.selected();
                                        if selected == 0 || selected == gtk::INVALID_LIST_POSITION {
                                            sender.input(AppMsg::SetTagFilter(None));
                                        } else if let Some(item) = dropdown.model()
                                            .and_then(|m| m.item(selected))
                                            .and_then(|obj| obj.downcast::<gtk::StringObject>().ok())
                                        {
                                            sender.input(AppMsg::SetTagFilter(Some(item.string().to_string())));
                                        }
                                    },
                                },

                                pack_end = &gtk::Box {
                                    gtk::Button {
                                        #[watch]
                                        set_visible: model.tokens.is_some(),
                                        set_icon_name: "system-search-symbolic",
                                        set_tooltip_text: Some(&gettext("Search articles")),
                                        connect_clicked => AppMsg::ToggleSearchMode,
                                    },

                                    gtk::Button {
                                        #[watch]
                                        set_visible: model.tokens.is_some(),
                                        set_icon_name: "list-add-symbolic",
                                        set_tooltip_text: Some(&gettext("Add bookmark")),
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

                            gtk::Button::with_label(&gettext("Login")) {
                                #[watch]
                                set_visible: model.tokens.is_none(),
                                connect_clicked => AppMsg::StartLogin,
                            },

                            #[local_ref]
                            sidebar_widget -> adw::Sidebar {
                                set_vexpand: true,

                                #[watch]
                                set_visible: model.tokens.is_some(),
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
                                    set_text: &gettext("Select an article"),
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

        let sidebar_section = adw::SidebarSection::new();
        let sidebar = adw::Sidebar::new();
        sidebar.append(sidebar_section.clone());

        let sidebar_sender = sender.clone();
        sidebar.connect_activated(move |_, position| {
            sidebar_sender.input(AppMsg::ArticleActivated(position));
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
                tags: article.tags.clone(),
            })
            .collect();

        let article_renderer = ArticleRenderer::builder().launch(()).detach();

        let mut available_tags: Vec<String> = all_articles
            .iter()
            .flat_map(|a| a.tags.iter().cloned())
            .collect::<HashSet<String>>()
            .into_iter()
            .collect();
        available_tags.sort();

        let all_label = gettext("All");
        let mut tag_items: Vec<&str> = vec![&all_label];
        tag_items.extend(available_tags.iter().map(|s| s.as_str()));
        let tag_model = gtk::StringList::new(&tag_items);

        let mut model = Self {
            tokens,
            username,
            sidebar,
            sidebar_section,
            visible_articles: Vec::new(),
            selected_item_id: None,
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
            selected_tag: None,
            available_tags,
            tag_model,
        };

        model.rebuild_article_list();

        let toast_overlay = model.toaster.overlay_widget();

        let sidebar_widget = model.sidebar.clone();

        let article_renderer_widget = model.article_renderer.widget();

        let widgets = view_output!();

        let mut actions = RelmActionGroup::<WindowActionGroup>::new();

        let shortcuts_action = {
            let main_window = widgets.main_window.clone();
            RelmAction::<ShortcutsAction>::new_stateless(move |_| {
                build_shortcuts_dialog().present(Some(&main_window));
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

        main_application().set_accels_for_action("win.show-help-overlay", &["<Control>question"]);

        widgets.load_window_size();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>, _: &Self::Root) {
        match message {
            AppMsg::Quit => main_application().quit(),
            AppMsg::ArticleActivated(position) => {
                if let Some(article) = self.visible_articles.get(position as usize) {
                    self.selected_item_id = Some(article.item_id.clone());
                    sender.input(AppMsg::ArticleSelected(
                        article.title.clone(),
                        article.uri.clone(),
                        article.item_id.clone(),
                        article.description.clone(),
                        article.time,
                        article.tags.clone(),
                    ));
                }
            }
            AppMsg::ArticleSelected(title, uri, item_id, description, time, tags) => {
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
                        tags,
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
                let _ = token::clear_tokens();
                let _ = articles::clear_articles();
                self.tokens = None;
                self.username = String::new();
                self.sidebar_section.remove_all();
                self.visible_articles.clear();
                self.selected_item_id = None;
                self.sidebar.set_selected(gtk::INVALID_LIST_POSITION);
                self.article_html = None;
                self.article_uri = None;
                self.article_item_id = None;
                self.all_articles.clear();
                self.search_query.clear();
                self.search_mode = false;
                self.selected_tag = None;
                self.available_tags.clear();
                self.tag_model
                    .splice(0, self.tag_model.n_items(), &[&gettext("All")]);
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
                            Err(e) => CommandMsg::Error(format!(
                                "{}: {}",
                                gettext("Failed to refresh articles"),
                                e
                            )),
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
                        match instapaper::archive_bookmark(&client, &tokens, bookmark_id).await {
                            Ok(_) => CommandMsg::ArticleArchived(item_id),
                            Err(e) => CommandMsg::Error(format!(
                                "{}: {}",
                                gettext("Failed to archive article"),
                                e
                            )),
                        }
                    });
                }
            }
            AppMsg::CopyArticleUrl => match self.article_uri.clone() {
                Some(uri) => {
                    let _ = crate::persistence::clipboard::copy(&uri);
                    let toast = adw::Toast::builder()
                        .title(&gettext("URL copied to clipboard"))
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
                            AddBookmarkOutput::BookmarkAdded(url, tags) => {
                                AppMsg::AddBookmarkCompleted(url, tags)
                            }
                            AddBookmarkOutput::Cancelled => AppMsg::AddBookmarkCancelled,
                        },
                    );
                    self.add_bookmark_dialog = Some(add_bookmark_dialog);
                }
            }
            AppMsg::AddBookmarkCompleted(url, tags) => {
                if let Some(tokens) = self.tokens.clone() {
                    sender.oneshot_command(async move {
                        let client = instapaper::client();
                        match instapaper::add_bookmark(&client, &tokens, &url, &tags).await {
                            Ok(_) => CommandMsg::BookmarkAdded,
                            Err(e) => CommandMsg::Error(format!(
                                "{}: {}",
                                gettext("Failed to add bookmark"),
                                e
                            )),
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
            AppMsg::SetTagFilter(tag) => {
                self.selected_tag = tag;
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

                for a in &entries {
                    if !a.tags.is_empty() {
                        println!("Article '{}' has tags: {:?}", a.title, a.tags);
                    }
                }
                println!(
                    "Total articles: {}, articles with tags: {}",
                    entries.len(),
                    entries.iter().filter(|a| !a.tags.is_empty()).count()
                );

                let mut tags: Vec<String> = entries
                    .iter()
                    .flat_map(|a| a.tags.iter().cloned())
                    .collect::<HashSet<String>>()
                    .into_iter()
                    .collect();
                tags.sort();
                self.available_tags = tags;
                println!("Available tags: {:?}", self.available_tags);

                let all_label = gettext("All");
                let mut tag_items: Vec<&str> = vec![&all_label];
                tag_items.extend(self.available_tags.iter().map(|s| s.as_str()));
                self.tag_model
                    .splice(0, self.tag_model.n_items(), &tag_items);

                self.rebuild_article_list();

                let persisted: Vec<PersistedArticle> = entries
                    .iter()
                    .map(|a| PersistedArticle {
                        title: a.title.clone(),
                        uri: a.uri.clone(),
                        item_id: a.item_id.clone(),
                        description: a.description.clone(),
                        time: a.time,
                        tags: a.tags.clone(),
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
                    .title(&gettext("Bookmark added successfully"))
                    .timeout(3)
                    .build();
                self.toaster.add_toast(toast);
                sender.input(AppMsg::RefreshArticles);
            }
            CommandMsg::Error(error) => {
                self.loading = false;
                let toast = adw::Toast::builder().title(&error).timeout(5).build();
                self.toaster.add_toast(toast);
            }
        }
    }

    fn shutdown(&mut self, widgets: &mut Self::Widgets, _output: relm4::Sender<Self::Output>) {
        let current_articles: Vec<PersistedArticle> = self
            .all_articles
            .iter()
            .map(|a| PersistedArticle {
                title: a.title.clone(),
                uri: a.uri.clone(),
                item_id: a.item_id.clone(),
                description: a.description.clone(),
                time: a.time,
                tags: a.tags.clone(),
            })
            .collect();
        let _ = articles::save_articles(&current_articles);

        widgets.save_window_size().unwrap();
    }
}

impl App {
    fn filter_articles(&self) -> Vec<Article> {
        self.all_articles
            .iter()
            .filter(|a| {
                if let Some(ref tag) = self.selected_tag {
                    if !a.tags.contains(tag) {
                        return false;
                    }
                }
                if !self.search_query.is_empty() {
                    let query_lower = self.search_query.to_lowercase();
                    if !a.title.to_lowercase().contains(&query_lower) {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect()
    }

    fn rebuild_article_list(&mut self) {
        let filtered = self.filter_articles();

        // AdwSidebar drops its selection when items are removed and
        // auto-selects the first item when items are first added, so restore
        // it explicitly after a rebuild to keep the selection stable across
        // filter changes.
        let restore_pos = self
            .selected_item_id
            .as_ref()
            .and_then(|id| filtered.iter().position(|a| &a.item_id == id));

        self.visible_articles = filtered;

        let section = &self.sidebar_section;
        section.remove_all();
        for article in &self.visible_articles {
            section.append(article.to_sidebar_item());
        }

        let selected = restore_pos.map_or(gtk::INVALID_LIST_POSITION, |pos| pos as u32);
        self.sidebar.set_selected(selected);

        hide_sidebar_item_chrome(&self.sidebar);
    }
}

/// AdwSidebar renders every item as `[icon][title box][suffix]` in a
/// horizontal box, with the built-in title box set to expand.
///
/// Cauldron draws the whole article card through the item's `suffix`, so hide
/// the built-in icon/title box to keep the card left-aligned and let it use the
/// full row width.
fn hide_sidebar_item_chrome(widget: &impl IsA<gtk::Widget>) {
    let widget = widget.upcast_ref::<gtk::Widget>();

    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Ok(row) = current.clone().downcast::<gtk::ListBoxRow>() {
            if let Some(row_child) = gtk::prelude::ListBoxRowExt::child(&row) {
                let mut part = row_child.first_child();
                while let Some(part_widget) = part {
                    if !part_widget.has_css_class("article-card") {
                        part_widget.set_visible(false);
                    }
                    part = part_widget.next_sibling();
                }
            }
        }

        hide_sidebar_item_chrome(&current);
        child = current.next_sibling();
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

fn build_shortcuts_dialog() -> adw::ShortcutsDialog {
    let dialog = adw::ShortcutsDialog::new();

    let general = adw::ShortcutsSection::new(Some(&pgettext("shortcut window", "General")));
    general.add(adw::ShortcutsItem::from_action(
        &pgettext("shortcut window", "Show Shortcuts"),
        "win.show-help-overlay",
    ));
    general.add(adw::ShortcutsItem::from_action(
        &pgettext("shortcut window", "Quit"),
        "app.quit",
    ));
    dialog.add(general);

    dialog
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
    use crate::testing::widget_inspection;

    fn make_article(title: &str, id: &str, tags: Vec<String>) -> Article {
        Article {
            title: title.to_string(),
            uri: format!("https://example.com/{}", id),
            item_id: id.to_string(),
            description: format!("About {}", title),
            time: 0.0,
            tags,
        }
    }

    #[gtk::test]
    fn test_hide_sidebar_item_chrome_leaves_only_the_card() {
        let section = adw::SidebarSection::new();
        let sidebar = adw::Sidebar::new();
        sidebar.append(section.clone());
        section.append(make_article("A title", "1", vec![]).to_sidebar_item());

        hide_sidebar_item_chrome(&sidebar);

        let list_box: gtk::ListBox =
            widget_inspection::find_descendant_by_type(&sidebar).expect("sidebar list box");

        let mut checked_row = false;
        let mut child = list_box.first_child();
        while let Some(current) = child {
            if let Ok(row) = current.clone().downcast::<gtk::ListBoxRow>() {
                let row_box = gtk::prelude::ListBoxRowExt::child(&row)
                    .and_then(|w| w.downcast::<gtk::Box>().ok())
                    .expect("row content box");

                let mut part = row_box.first_child();
                while let Some(part_widget) = part {
                    assert_eq!(
                        part_widget.has_css_class("article-card"),
                        part_widget.is_visible(),
                        "only the article card should stay visible in the row"
                    );
                    part = part_widget.next_sibling();
                }
                checked_row = true;
            }
            child = current.next_sibling();
        }

        assert!(checked_row, "expected a sidebar row");
    }

    fn filter_by(all_articles: &[Article], query: &str, tag: Option<&str>) -> Vec<Article> {
        all_articles
            .iter()
            .filter(|a| {
                if let Some(tag) = tag {
                    if !a.tags.contains(&tag.to_string()) {
                        return false;
                    }
                }
                if !query.is_empty() {
                    let query_lower = query.to_lowercase();
                    if !a.title.to_lowercase().contains(&query_lower) {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect()
    }

    #[test]
    fn test_filter_articles_logic() {
        let all_articles = vec![
            make_article("Rust Programming Language", "1", vec![]),
            make_article("Python Tutorial", "2", vec![]),
            make_article("Advanced Rust Patterns", "3", vec![]),
        ];

        let filtered = filter_by(&all_articles, "rust", None);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].title, "Rust Programming Language");
        assert_eq!(filtered[1].title, "Advanced Rust Patterns");

        let filtered_upper = filter_by(&all_articles, "RUST", None);
        assert_eq!(filtered_upper.len(), 2);

        let filtered_none = filter_by(&all_articles, "javascript", None);
        assert_eq!(filtered_none.len(), 0);

        let filtered_empty = filter_by(&all_articles, "", None);
        assert_eq!(filtered_empty.len(), 3);

        let filtered_partial = filter_by(&all_articles, "python", None);
        assert_eq!(filtered_partial.len(), 1);
        assert_eq!(filtered_partial[0].title, "Python Tutorial");
    }

    #[test]
    fn test_filter_articles_by_tag() {
        let all_articles = vec![
            make_article(
                "Rust Book",
                "1",
                vec!["rust".to_string(), "programming".to_string()],
            ),
            make_article("Python Guide", "2", vec!["python".to_string()]),
            make_article("Rust Patterns", "3", vec!["rust".to_string()]),
            make_article("No Tags", "4", vec![]),
        ];

        let filtered = filter_by(&all_articles, "", Some("rust"));
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].title, "Rust Book");
        assert_eq!(filtered[1].title, "Rust Patterns");

        let filtered_python = filter_by(&all_articles, "", Some("python"));
        assert_eq!(filtered_python.len(), 1);
        assert_eq!(filtered_python[0].title, "Python Guide");

        let filtered_none = filter_by(&all_articles, "", Some("nonexistent"));
        assert_eq!(filtered_none.len(), 0);

        let filtered_all = filter_by(&all_articles, "", None);
        assert_eq!(filtered_all.len(), 4);
    }

    #[test]
    fn test_filter_articles_by_tag_and_search() {
        let all_articles = vec![
            make_article(
                "Rust Book",
                "1",
                vec!["rust".to_string(), "programming".to_string()],
            ),
            make_article("Rust Patterns", "2", vec!["rust".to_string()]),
            make_article("Python Guide", "3", vec!["programming".to_string()]),
        ];

        let filtered = filter_by(&all_articles, "rust", Some("rust"));
        assert_eq!(filtered.len(), 2);

        let filtered2 = filter_by(&all_articles, "book", Some("rust"));
        assert_eq!(filtered2.len(), 1);
        assert_eq!(filtered2[0].title, "Rust Book");

        let filtered3 = filter_by(&all_articles, "guide", Some("rust"));
        assert_eq!(filtered3.len(), 0);
    }

    #[test]
    fn test_collect_available_tags() {
        let articles = vec![
            make_article(
                "A",
                "1",
                vec!["rust".to_string(), "programming".to_string()],
            ),
            make_article(
                "B",
                "2",
                vec!["python".to_string(), "programming".to_string()],
            ),
            make_article("C", "3", vec![]),
        ];

        let mut tags: Vec<String> = articles
            .iter()
            .flat_map(|a| a.tags.iter().cloned())
            .collect::<HashSet<String>>()
            .into_iter()
            .collect();
        tags.sort();

        assert_eq!(tags, vec!["programming", "python", "rust"]);
    }
}
