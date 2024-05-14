use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct PocketCodeResponse {
    pub code: String,
}

#[derive(Serialize)]
pub struct PocketAccessTokenRequest {
    pub consumer_key: String,
    pub code: String,
}

#[derive(Deserialize)]
pub struct PocketAccessTokenResponse {
    pub access_token: String,
    pub username: String,
}
