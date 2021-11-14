use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};
use webkit2gtk::{
    traits::WebViewExt, LoadEvent, UserContentManager, WebContext, WebView, WebViewExtManual,
};

use crate::application::Cauldron;
use crate::config::{APP_ID, PROFILE};

const GTK_BUILDER_ERROR: &str =
    "Could not build GTK widget from UI file. This should never happen!";

mod imp {
    use super::*;

    use gtk::CompositeTemplate;

    #[derive(Debug, CompositeTemplate)]
    #[template(resource = "/it/dottorblaster/cauldron/ui/window.ui")]
    pub struct CauldronWindow {
        #[template_child]
        pub headerbar: TemplateChild<gtk::HeaderBar>,
        #[template_child]
        pub article_list: TemplateChild<gtk::ListView>,
        #[template_child]
        pub selected_article: TemplateChild<gtk::TextView>,
        #[template_child]
        pub oauth_box: TemplateChild<gtk::Box>,
        pub settings: gio::Settings,
    }

    impl Default for CauldronWindow {
        fn default() -> Self {
            Self {
                headerbar: TemplateChild::default(),
                settings: gio::Settings::new(APP_ID),
                article_list: TemplateChild::default(),
                selected_article: TemplateChild::default(),
                oauth_box: TemplateChild::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CauldronWindow {
        const NAME: &'static str = "CauldronWindow";
        type Type = super::CauldronWindow;
        type ParentType = gtk::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        // You must call `Widget`'s `init_template()` within `instance_init()`.
        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for CauldronWindow {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            // Devel Profile
            if PROFILE == "Devel" {
                obj.add_css_class("devel");
            }

            // Load latest window state
            obj.load_window_size();

            let context = WebContext::default().expect(GTK_BUILDER_ERROR);
            let content_manager = UserContentManager::new();
            let webview =
                WebView::new_with_context_and_user_content_manager(&context, &content_manager);

            self.oauth_box.pack_start(&webview, true, true, 0);
            webview.load_uri("https://dottorblaster.it");
        }
    }

    impl WidgetImpl for CauldronWindow {}
    impl WindowImpl for CauldronWindow {
        // Save window state on delete event
        fn close_request(&self, window: &Self::Type) -> gtk::Inhibit {
            if let Err(err) = window.save_window_size() {
                log::warn!("Failed to save window state, {}", &err);
            }

            // Pass close request on to the parent
            self.parent_close_request(window)
        }
    }

    impl ApplicationWindowImpl for CauldronWindow {}
}

glib::wrapper! {
    pub struct CauldronWindow(ObjectSubclass<imp::CauldronWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow,
        @implements gio::ActionMap, gio::ActionGroup;
}

impl CauldronWindow {
    pub fn new(app: &Cauldron) -> Self {
        glib::Object::new(&[("application", app)]).expect("Failed to create CauldronWindow")
    }

    fn save_window_size(&self) -> Result<(), glib::BoolError> {
        let self_ = imp::CauldronWindow::from_instance(self);

        let (width, height) = self.default_size();

        self_.settings.set_int("window-width", width)?;
        self_.settings.set_int("window-height", height)?;

        self_
            .settings
            .set_boolean("is-maximized", self.is_maximized())?;

        Ok(())
    }

    fn load_window_size(&self) {
        let self_ = imp::CauldronWindow::from_instance(self);

        let width = self_.settings.int("window-width");
        let height = self_.settings.int("window-height");
        let is_maximized = self_.settings.boolean("is-maximized");

        self.set_default_size(width, height);

        if is_maximized {
            self.maximize();
        }
    }
}
