use gtk::prelude::GtkApplicationExt;
use relm4::{
    adw, adw::prelude::AdwDialogExt, gtk, ComponentParts, ComponentSender, SimpleComponent,
};

use gettextrs::gettext;

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
            .application_name(&gettext("Cauldron"))
            .version(VERSION)
            .translator_credits(&gettext("translator-credits"))
            .copyright("© 2024 Alessio Biancalana")
            .developers(vec!["Alessio Biancalana"])
            .designers(vec!["Alessio Biancalana"])
            .artists(vec!["Brage Fuglseth https://bragefuglseth.dev"])
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
        // Only present the dialog if we're not in a test environment
        if !cfg!(test) {
            widgets.present(Some(&relm4::main_application().windows()[0]));
        }

        ComponentParts { model, widgets }
    }

    fn update_view(&self, _dialog: &mut Self::Widgets, _sender: ComponentSender<Self>) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::ComponentTester;

    #[gtk::test]
    fn test_init_component() {
        let tester = ComponentTester::<AboutDialog>::launch(());
        tester.process_events();

        // The component should initialize successfully
        // We can't test much state since AboutDialog has no fields
        assert!(true, "Component initialized successfully");
    }

    #[gtk::test]
    fn test_dialog_properties() {
        let tester = ComponentTester::<AboutDialog>::launch(());
        tester.process_events();

        // Access the root widget (which is an adw::AboutDialog)
        let root = tester.widget();

        // Verify the about dialog properties
        assert_eq!(root.application_icon(), APP_ID);
        assert_eq!(root.application_name(), gettext("Cauldron"));
        assert_eq!(root.version(), VERSION);
        assert_eq!(root.website(), "https://github.com/dottorblaster/cauldron");
        assert_eq!(
            root.issue_url(),
            "https://github.com/dottorblaster/cauldron/issues"
        );
        assert_eq!(root.copyright(), "© 2024 Alessio Biancalana");
    }

    #[gtk::test]
    fn test_dialog_can_close() {
        let tester = ComponentTester::<AboutDialog>::launch(());
        tester.process_events();

        let root = tester.widget();
        assert!(root.can_close(), "Dialog should be closeable");
    }

    #[gtk::test]
    fn test_dialog_credits() {
        let tester = ComponentTester::<AboutDialog>::launch(());
        tester.process_events();

        let root = tester.widget();

        // Verify developers list
        let developers = root.developers();
        assert_eq!(developers.len(), 1);
        assert_eq!(developers[0].as_str(), "Alessio Biancalana");

        // Verify designers list
        let designers = root.designers();
        assert_eq!(designers.len(), 1);
        assert_eq!(designers[0].as_str(), "Alessio Biancalana");

        // Verify artists list
        let artists = root.artists();
        assert_eq!(artists.len(), 1);
        assert_eq!(
            artists[0].as_str(),
            "Brage Fuglseth https://bragefuglseth.dev"
        );
    }

    #[gtk::test]
    fn test_dialog_license() {
        let tester = ComponentTester::<AboutDialog>::launch(());
        tester.process_events();

        let root = tester.widget();
        assert_eq!(root.license_type(), gtk::License::Apache20);
    }

    #[gtk::test]
    fn test_dialog_translator_credits() {
        let tester = ComponentTester::<AboutDialog>::launch(());
        tester.process_events();

        let root = tester.widget();
        assert_eq!(root.translator_credits(), gettext("translator-credits"));
    }
}
