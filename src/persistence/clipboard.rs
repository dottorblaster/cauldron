use relm4::gtk::gdk::prelude::DisplayExt;
use relm4::gtk::gdk::Display;

// TODO: convert the return to a result
pub fn copy(text: &str) {
    let display = Display::default().unwrap();
    let clipboard = display.clipboard();
    clipboard.set_text(text);
}
