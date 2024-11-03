use relm4::{
    adw, adw::prelude::AdwDialogExt, ComponentParts, ComponentSender, RelmWidgetExt,
    SimpleComponent,
};

use crate::config::{APP_ID, VERSION};

pub struct AboutDialog {}

impl SimpleComponent for AboutDialog {
    type Init = ();
    type Widgets = adw::AboutDialog;
    type Input = ();
    type Output = ();
    type Root = adw::AboutDialog;

    fn init_root() -> Self::Root {
        adw::AboutDialog::builder()
            .application_icon(APP_ID)
            // Insert your license of choice here
            // .license_type(gtk::License::MitX11)
            // Insert your website here
            // .website("https://gitlab.gnome.org/bilelmoussaoui/cauldron/")
            // Insert your Issues page
            // .issue_url("https://gitlab.gnome.org/World/Rust/cauldron/-/issues")
            // Insert your application name here
            .application_name("Relm4-template")
            .version(VERSION)
            .translator_credits("translator-credits")
            .copyright("© 2023 Alessio Biancalana")
            .developers(vec!["Alessio Biancalana"])
            .designers(vec!["Alessio Biancalana"])
            .can_close(true)
            .build()
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = Self {};

        let widgets = root.clone();

        ComponentParts { model, widgets }
    }

    fn update_view(&self, dialog: &mut Self::Widgets, _sender: ComponentSender<Self>) {
        let window = dialog.toplevel_window();
        dialog.present(window.as_ref());
    }
}
