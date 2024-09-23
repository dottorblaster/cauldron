use crate::config::APP_ID;
use anyhow::Result;
use relm4::gtk::glib;
use std::fs::File;
use std::io::Read;
use std::io::Write;
pub fn save_token(token: &str) -> Result<()> {
    let mut path = glib::user_data_dir();
    path.push(APP_ID);
    std::fs::create_dir_all(&path).expect("Could not create directory.");
    path.push("token");

    let mut file = File::create(path)?;
    file.write_all(token.as_bytes())?;
    Ok(())
}

pub fn read_token() -> Result<String> {
    let mut path = glib::user_data_dir();
    path.push(APP_ID);
    path.push("token");

    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}
