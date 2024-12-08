use gtk::prelude::GtkApplicationExt;
use relm4::{
    adw, adw::prelude::AdwDialogExt, gtk, ComponentParts, ComponentSender, SimpleComponent,
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
            .license_type(gtk::License::Apache20)
            // Insert your website here
            .website("https://github.com/dottorblaster/cauldron")
            // Insert your Issues page
            .issue_url("https://github.com/dottorblaster/cauldron/issues")
            // Insert your application name here
            .application_name("Cauldron")
            .version(VERSION)
            .translator_credits("translator-credits")
            .copyright("© 2024 Alessio Biancalana")
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
        widgets.present(Some(&relm4::main_application().windows()[0]));

        ComponentParts { model, widgets }
    }

    fn update_view(&self, _dialog: &mut Self::Widgets, _sender: ComponentSender<Self>) {}
}
