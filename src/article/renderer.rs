use gtk::prelude::*;
use html_escape::encode_text;
use relm4::{gtk, ComponentParts, ComponentSender, SimpleComponent};
use scraper::{ElementRef, Html, Node, Selector};

pub struct ArticleRenderer {
    content_box: gtk::Box,
    title_label: gtk::Label,
    metadata_box: gtk::Box,
    vadjustment: gtk::Adjustment,
}

#[derive(Debug)]
pub enum ArticleRendererInput {
    SetTitle(String),
    SetContent(String),
    SetMetadata {
        url: String,
        description: String,
        time: f64,
    },
}

impl SimpleComponent for ArticleRenderer {
    type Init = ();
    type Input = ArticleRendererInput;
    type Output = ();
    type Root = gtk::ScrolledWindow;
    type Widgets = ArticleRendererWidgets;

    fn init_root() -> Self::Root {
        gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .propagate_natural_height(true)
            .hexpand(true)
            .build()
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let main_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(0)
            .margin_start(48)
            .margin_end(48)
            .margin_top(16)
            .margin_bottom(16)
            .build();

        let title_label = gtk::Label::builder()
            .wrap(true)
            .xalign(0.0)
            .selectable(true)
            .visible(false)
            .build();
        title_label.add_css_class("article-title");

        let metadata_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .margin_top(8)
            .margin_bottom(16)
            .visible(false)
            .build();
        metadata_box.add_css_class("article-metadata");

        let content_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(0)
            .build();

        main_box.append(&title_label);
        main_box.append(&metadata_box);
        main_box.append(&content_box);

        root.set_child(Some(&main_box));

        Self::load_css();

        let vadjustment = root.vadjustment();

        let model = Self {
            content_box,
            title_label,
            metadata_box,
            vadjustment,
        };
        let widgets = ArticleRendererWidgets {};

        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            ArticleRendererInput::SetTitle(title) => {
                self.title_label.set_text(&title);
                self.title_label.set_visible(true);
            }
            ArticleRendererInput::SetContent(html) => {
                self.render_html(&html);
                self.vadjustment.set_value(0.0);
            }
            ArticleRendererInput::SetMetadata {
                url,
                description,
                time,
            } => {
                self.render_metadata(&url, &description, time);
            }
        }
    }
}

impl ArticleRenderer {
    fn load_css() {
        use crate::config::RESOURCES_FILE;
        use gtk::{gio, glib};

        // Try to load resources, but don't panic if they're not available (e.g., in tests)
        let res = match gio::Resource::load(RESOURCES_FILE) {
            Ok(r) => r,
            Err(_) => {
                eprintln!("Warning: Could not load gresource file (may be running in test mode)");
                return;
            }
        };

        gio::resources_register(&res);

        let data = match res.lookup_data(
            "/it/dottorblaster/cauldron/article_view/native_style.css",
            gio::ResourceLookupFlags::NONE,
        ) {
            Ok(d) => d,
            Err(_) => {
                eprintln!("Warning: Could not load native_style.css from resources");
                return;
            }
        };

        let css_string = match glib::GString::from_utf8_checked(data.to_vec()) {
            Ok(s) => s,
            Err(_) => {
                eprintln!("Warning: CSS file is not valid UTF-8");
                return;
            }
        };

        let provider = gtk::CssProvider::new();
        provider.load_from_string(&css_string);

        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    }

    fn render_metadata(&self, url: &str, description: &str, time: f64) {
        // Clear existing metadata
        while let Some(child) = self.metadata_box.first_child() {
            self.metadata_box.remove(&child);
        }

        // Extract domain from URL
        let domain = Self::extract_domain(url);

        // Create metadata row with domain and date
        let metadata_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .build();

        if let Some(domain_text) = domain {
            let domain_label = gtk::Label::builder()
                .label(&domain_text)
                .xalign(0.0)
                .selectable(true)
                .build();
            domain_label.add_css_class("article-domain");
            metadata_row.append(&domain_label);

            let separator = gtk::Label::builder().label("·").build();
            separator.add_css_class("article-metadata-separator");
            metadata_row.append(&separator);
        }

        let date_label = gtk::Label::builder()
            .label(&Self::format_date(time))
            .xalign(0.0)
            .selectable(true)
            .build();
        date_label.add_css_class("article-date");
        metadata_row.append(&date_label);

        self.metadata_box.append(&metadata_row);

        // Add description if available
        if !description.is_empty() {
            let desc_label = gtk::Label::builder()
                .label(description)
                .wrap(true)
                .xalign(0.0)
                .selectable(true)
                .build();
            desc_label.add_css_class("article-description");
            self.metadata_box.append(&desc_label);
        }

        self.metadata_box.set_visible(true);
    }

    fn extract_domain(url: &str) -> Option<String> {
        use url::Url;

        if let Ok(parsed_url) = Url::parse(url) {
            if let Some(host) = parsed_url.host_str() {
                let domain = host.strip_prefix("www.").unwrap_or(host);
                return Some(domain.to_string());
            }
        }
        None
    }

    fn format_date(time: f64) -> String {
        use chrono::DateTime;

        if time == 0.0 {
            return String::from("Unknown date");
        }

        let timestamp = time as i64;
        let datetime = DateTime::from_timestamp(timestamp, 0);

        match datetime {
            Some(dt) => {
                let now = chrono::Utc::now();
                let duration = now.signed_duration_since(dt);

                if duration.num_days() == 0 {
                    String::from("Today")
                } else if duration.num_days() == 1 {
                    String::from("Yesterday")
                } else if duration.num_days() < 7 {
                    format!("{} days ago", duration.num_days())
                } else if duration.num_weeks() < 4 {
                    let weeks = duration.num_weeks();
                    if weeks == 1 {
                        String::from("1 week ago")
                    } else {
                        format!("{} weeks ago", weeks)
                    }
                } else {
                    dt.format("%B %d, %Y").to_string()
                }
            }
            None => String::from("Unknown date"),
        }
    }

    fn render_html(&self, html: &str) {
        while let Some(child) = self.content_box.first_child() {
            self.content_box.remove(&child);
        }

        eprintln!(
            "Rendering HTML (first 500 chars): {}",
            &html.chars().take(500).collect::<String>()
        );

        let document = Html::parse_document(html);

        self.process_elements(&document);

        let child_count = self.content_box.observe_children().n_items();
        eprintln!("Content box now has {} children", child_count);

        if child_count == 0 {
            let debug_label = gtk::Label::new(Some(
                "Debug: No content rendered. Check terminal for HTML output.",
            ));
            debug_label.set_wrap(true);
            self.content_box.append(&debug_label);
        }
    }

    fn process_elements(&self, document: &Html) {
        let body_selector = Selector::parse("body > *").unwrap();
        let mut found_elements = false;

        for element in document.select(&body_selector) {
            if let Some(widget) = self.element_to_widget(element) {
                self.content_box.append(&widget);
                found_elements = true;
            }
        }

        if !found_elements {
            let root_selector = Selector::parse("html > *").unwrap();
            for element in document.select(&root_selector) {
                if let Some(widget) = self.element_to_widget(element) {
                    self.content_box.append(&widget);
                    found_elements = true;
                }
            }
        }

        if !found_elements {
            let all_selector =
                Selector::parse("p, h1, h2, h3, h4, h5, h6, pre, blockquote, ul, ol, img").unwrap();
            for element in document.select(&all_selector) {
                if let Some(widget) = self.element_to_widget(element) {
                    self.content_box.append(&widget);
                }
            }
        }
    }

    fn element_to_widget(&self, element: ElementRef) -> Option<gtk::Widget> {
        match element.value().name() {
            "h1" => Some(self.create_heading(element, 1).upcast()),
            "h2" => Some(self.create_heading(element, 2).upcast()),
            "h3" => Some(self.create_heading(element, 3).upcast()),
            "h4" => Some(self.create_heading(element, 4).upcast()),
            "h5" => Some(self.create_heading(element, 5).upcast()),
            "h6" => Some(self.create_heading(element, 6).upcast()),
            "p" => Some(self.create_paragraph(element).upcast()),
            "pre" => Some(self.create_code_block(element).upcast()),
            "blockquote" => Some(self.create_blockquote(element).upcast()),
            "ul" => Some(self.create_list(element, false).upcast()),
            "ol" => Some(self.create_list(element, true).upcast()),
            "img" => Some(self.create_image(element).upcast()),
            _ => None,
        }
    }

    fn create_heading(&self, element: ElementRef, level: u8) -> gtk::Label {
        let text = self.extract_text_with_formatting(element);
        let label = gtk::Label::builder()
            .label(&text)
            .use_markup(true)
            .wrap(true)
            .xalign(0.0)
            .selectable(true)
            .build();

        label.add_css_class(&format!("article-h{}", level));
        label
    }

    fn create_paragraph(&self, element: ElementRef) -> gtk::Label {
        let text = self.extract_text_with_formatting(element);
        let label = gtk::Label::builder()
            .label(&text)
            .use_markup(true)
            .wrap(true)
            .xalign(0.0)
            .selectable(true)
            .build();

        label.add_css_class("article-text");
        label
    }

    fn create_code_block(&self, element: ElementRef) -> gtk::Box {
        let code_text = element.text().collect::<String>();

        let buffer = gtk::TextBuffer::builder().text(&code_text).build();

        let text_view = gtk::TextView::builder()
            .buffer(&buffer)
            .editable(false)
            .cursor_visible(false)
            .wrap_mode(gtk::WrapMode::Word)
            .monospace(true)
            .build();

        text_view.add_css_class("article-code-block");

        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        container.append(&text_view);
        container
    }

    fn create_blockquote(&self, element: ElementRef) -> gtk::Box {
        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .build();

        container.add_css_class("article-blockquote");

        for child in element.children() {
            if let Some(child_element) = ElementRef::wrap(child) {
                if let Some(widget) = self.element_to_widget(child_element) {
                    container.append(&widget);
                }
            }
        }

        container
    }

    fn create_list(&self, element: ElementRef, ordered: bool) -> gtk::Box {
        let container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(4)
            .build();

        let li_selector = Selector::parse("li").unwrap();
        for (index, li) in element.select(&li_selector).enumerate() {
            let item_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(8)
                .build();

            let prefix = if ordered {
                format!("{}.", index + 1)
            } else {
                "•".to_string()
            };

            let bullet = gtk::Label::new(Some(&prefix));
            bullet.set_xalign(0.0);
            bullet.set_valign(gtk::Align::Start);
            item_box.append(&bullet);

            let text = self.extract_text_with_formatting(li);
            let content = gtk::Label::builder()
                .label(&text)
                .use_markup(true)
                .wrap(true)
                .xalign(0.0)
                .hexpand(true)
                .selectable(true)
                .build();

            item_box.append(&content);
            container.append(&item_box);
        }

        container
    }

    fn create_image(&self, element: ElementRef) -> gtk::Box {
        let img_url = element.value().attr("src").unwrap_or("").to_string();

        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        container.add_css_class("article-image");

        if !img_url.is_empty() {
            let button = gtk::Button::builder()
                .label("Load Image")
                .halign(gtk::Align::Center)
                .margin_top(20)
                .margin_bottom(20)
                .build();

            let container_clone = container.clone();
            let url_owned = img_url.clone();

            button.connect_clicked(move |btn| {
                btn.set_sensitive(false);
                btn.set_label("Loading...");

                let container_clone2 = container_clone.clone();
                let url_for_load = url_owned.clone();
                let url_for_error = url_owned.clone();
                let btn_clone = btn.clone();

                gtk::glib::MainContext::default().spawn_local(async move {
                    let result =
                        gtk::gio::spawn_blocking(move || Self::download_image_bytes(&url_for_load))
                            .await;

                    container_clone2.remove(&btn_clone);

                    match result {
                        Ok(Ok(bytes)) => match Self::bytes_to_texture(&bytes) {
                            Ok(texture) => {
                                eprintln!(
                                    "Texture created successfully, size: {}x{}",
                                    texture.width(),
                                    texture.height()
                                );

                                let picture = gtk::Picture::new();
                                picture.set_paintable(Some(&texture));
                                picture.set_content_fit(gtk::ContentFit::Contain);
                                picture.set_can_shrink(true);
                                picture.set_halign(gtk::Align::Center);
                                picture.set_margin_top(20);
                                picture.set_margin_bottom(20);
                                picture.add_css_class("article-image-picture");

                                let natural_width = 2048
                                    .min(texture.width().min(container_clone2.allocated_width()));
                                let aspect_ratio = texture.height() as f64 / texture.width() as f64;
                                let natural_height = (natural_width as f64 * aspect_ratio) as i32;
                                picture.set_size_request(-1, natural_height);

                                container_clone2.append(&picture);
                                eprintln!("Picture widget added with height: {}", natural_height);
                            }
                            Err(e) => {
                                eprintln!("Failed to create pixbuf: {}", e);
                                let error_icon = gtk::Image::from_icon_name("image-missing");
                                error_icon.set_pixel_size(48);
                                error_icon.set_halign(gtk::Align::Center);
                                error_icon.set_margin_top(20);
                                error_icon.set_margin_bottom(20);
                                container_clone2.append(&error_icon);
                            }
                        },
                        Ok(Err(e)) => {
                            eprintln!("Failed to load image from {}: {}", url_for_error, e);
                            let error_icon = gtk::Image::from_icon_name("image-missing");
                            error_icon.set_pixel_size(48);
                            error_icon.set_halign(gtk::Align::Center);
                            error_icon.set_margin_top(20);
                            error_icon.set_margin_bottom(20);
                            container_clone2.append(&error_icon);
                        }
                        Err(_) => {
                            eprintln!("Failed to spawn blocking task");
                            let error_icon = gtk::Image::from_icon_name("image-missing");
                            error_icon.set_pixel_size(48);
                            error_icon.set_halign(gtk::Align::Center);
                            error_icon.set_margin_top(20);
                            error_icon.set_margin_bottom(20);
                            container_clone2.append(&error_icon);
                        }
                    }
                });
            });

            container.append(&button);
        }

        container
    }

    fn download_image_bytes(
        url: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        eprintln!("Loading image from: {}", url);

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let response = client.get(url).send()?;
        eprintln!("Got response with status: {}", response.status());

        let bytes = response.bytes()?;
        eprintln!("Downloaded {} bytes", bytes.len());

        Ok(bytes.to_vec())
    }

    fn bytes_to_texture(bytes: &[u8]) -> Result<gtk::gdk::Texture, Box<dyn std::error::Error>> {
        use gtk::gdk;
        use gtk::gdk_pixbuf;

        eprintln!("Converting {} bytes to texture on main thread", bytes.len());

        let loader = gdk_pixbuf::PixbufLoader::new();
        loader.write(bytes)?;
        loader.close()?;

        let pixbuf = loader.pixbuf().ok_or("Failed to get pixbuf from loader")?;

        eprintln!(
            "Successfully created pixbuf: {}x{}",
            pixbuf.width(),
            pixbuf.height()
        );

        let texture = gdk::Texture::for_pixbuf(&pixbuf);
        eprintln!(
            "Converted to texture: {}x{}",
            texture.width(),
            texture.height()
        );

        Ok(texture)
    }

    fn extract_text_with_formatting(&self, element: ElementRef) -> String {
        let mut result = String::new();

        for child in element.children() {
            match child.value() {
                Node::Text(text) => {
                    result.push_str(&encode_text(text.text.as_ref()));
                }
                Node::Element(_) => {
                    if let Some(child_element) = ElementRef::wrap(child) {
                        let tag_name = child_element.value().name();
                        match tag_name {
                            "strong" | "b" => {
                                result.push_str("<b>");
                                result.push_str(&self.extract_text_with_formatting(child_element));
                                result.push_str("</b>");
                            }
                            "em" | "i" => {
                                result.push_str("<i>");
                                result.push_str(&self.extract_text_with_formatting(child_element));
                                result.push_str("</i>");
                            }
                            "code" => {
                                result.push_str("<tt>");
                                let code_text = child_element.text().collect::<String>();
                                result.push_str(&encode_text(&code_text));
                                result.push_str("</tt>");
                            }
                            "a" => {
                                if let Some(href) = child_element.value().attr("href") {
                                    result.push_str(&format!(
                                        "<a href=\"{}\">{}</a>",
                                        encode_text(href),
                                        self.extract_text_with_formatting(child_element)
                                    ));
                                } else {
                                    result.push_str(
                                        &self.extract_text_with_formatting(child_element),
                                    );
                                }
                            }
                            _ => {
                                result.push_str(&child_element.text().collect::<String>());
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        result
    }
}

pub struct ArticleRendererWidgets {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::ComponentTester;
    use gtk::prelude::*;

    #[gtk::test]
    fn test_init_component() {
        let tester = ComponentTester::<ArticleRenderer>::launch(());

        // Verify the root widget is a ScrolledWindow
        let root = tester.widget();
        assert_eq!(root.type_().name(), "GtkScrolledWindow");
    }

    #[gtk::test]
    fn test_set_title() {
        let tester = ComponentTester::<ArticleRenderer>::launch(());

        // Send SetTitle message - testing that it doesn't panic
        tester.send_input(ArticleRendererInput::SetTitle(
            "Test Article Title".to_string(),
        ));
        tester.process_events();

        // If we get here without panicking, the test passed
        // Note: Widget hierarchy inspection is unreliable in tests
    }

    #[gtk::test]
    fn test_set_metadata() {
        let tester = ComponentTester::<ArticleRenderer>::launch(());

        // Send SetMetadata message - testing that it doesn't panic
        tester.send_input(ArticleRendererInput::SetMetadata {
            url: "https://www.example.com/article".to_string(),
            description: "A sample article description".to_string(),
            time: 1234567890.0,
        });
        tester.process_events();

        // If we get here without panicking, the test passed
    }

    #[test]
    fn test_extract_domain() {
        // Test normal URL
        let domain = ArticleRenderer::extract_domain("https://www.example.com/article");
        assert_eq!(domain, Some("example.com".to_string()));

        // Test URL without www
        let domain = ArticleRenderer::extract_domain("https://github.com/user/repo");
        assert_eq!(domain, Some("github.com".to_string()));

        // Test invalid URL
        let domain = ArticleRenderer::extract_domain("not a url");
        assert_eq!(domain, None);

        // Test URL with subdomain
        let domain = ArticleRenderer::extract_domain("https://blog.example.com/post");
        assert_eq!(domain, Some("blog.example.com".to_string()));
    }

    #[test]
    fn test_format_date_zero() {
        // Test with zero timestamp
        let formatted = ArticleRenderer::format_date(0.0);
        assert_eq!(formatted, "Unknown date");
    }

    #[test]
    fn test_format_date_recent() {
        use chrono::Utc;

        // Test with a timestamp from 2 days ago
        let now = Utc::now();
        let two_days_ago = now - chrono::Duration::days(2);
        let timestamp = two_days_ago.timestamp() as f64;

        let formatted = ArticleRenderer::format_date(timestamp);
        assert_eq!(formatted, "2 days ago");
    }

    #[test]
    fn test_format_date_weeks() {
        use chrono::Utc;

        // Test with a timestamp from 2 weeks ago
        let now = Utc::now();
        let two_weeks_ago = now - chrono::Duration::weeks(2);
        let timestamp = two_weeks_ago.timestamp() as f64;

        let formatted = ArticleRenderer::format_date(timestamp);
        assert_eq!(formatted, "2 weeks ago");
    }

    #[test]
    fn test_format_date_old() {
        // Test with a very old timestamp (January 1, 2020)
        let timestamp = 1577836800.0; // 2020-01-01 00:00:00 UTC

        let formatted = ArticleRenderer::format_date(timestamp);
        // Should be in "Month Day, Year" format
        assert!(formatted.contains("2020"));
        assert!(formatted.contains("January") || formatted.contains("Jan"));
    }

    #[gtk::test]
    fn test_set_content_empty() {
        let tester = ComponentTester::<ArticleRenderer>::launch(());

        // Send empty HTML content - testing that it doesn't panic
        tester.send_input(ArticleRendererInput::SetContent("".to_string()));
        tester.process_events();

        // If we get here without panicking, the test passed
    }

    #[gtk::test]
    fn test_set_content_with_paragraph() {
        let tester = ComponentTester::<ArticleRenderer>::launch(());

        // Send HTML with a paragraph - testing that it doesn't panic
        tester.send_input(ArticleRendererInput::SetContent(
            "<p>This is a test paragraph.</p>".to_string(),
        ));
        tester.process_events();

        // If we get here without panicking, the test passed
    }

    #[gtk::test]
    fn test_multiple_inputs() {
        let tester = ComponentTester::<ArticleRenderer>::launch(());

        // Send multiple inputs in sequence - testing that it doesn't panic
        tester.send_input(ArticleRendererInput::SetTitle("My Article".to_string()));
        tester.send_input(ArticleRendererInput::SetMetadata {
            url: "https://example.com".to_string(),
            description: "Description here".to_string(),
            time: 1234567890.0,
        });
        tester.send_input(ArticleRendererInput::SetContent(
            "<h1>Header</h1><p>Content</p>".to_string(),
        ));
        tester.process_events();

        // If we get here without panicking, the test passed
    }
}
