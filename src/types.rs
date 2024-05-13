use serde::Deserialize;

#[derive(Deserialize)]
pub struct PocketCodeResponse {
    pub code: String,
}
