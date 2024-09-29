use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use url::form_urlencoded;

#[derive(Serialize)]
pub struct PocketInitiateOauthRequest {
    consumer_key: String,
    redirect_uri: String,
}

#[derive(Deserialize)]
pub struct PocketCodeResponse {
    pub code: String,
}

#[derive(Serialize)]
pub struct PocketAccessTokenRequest {
    consumer_key: String,
    code: String,
}

#[derive(Deserialize)]
pub struct PocketAccessTokenResponse {
    pub access_token: String,
    pub username: String,
}

#[derive(Serialize)]
pub struct PocketEntriesRequest {
    consumer_key: String,
    access_token: String,
    count: String,
    total: String,
    state: String,
    sort: String,
}

#[derive(Serialize)]
pub struct PocketArchiveEntryRequest {
    consumer_key: String,
    access_token: String,
    actions: Vec<PocketArchiveAction>,
}

#[derive(Serialize)]
pub struct PocketArchiveAction {
    action: String,
    item_id: String,
}

fn headers() -> HeaderMap {
    let mut headers = HeaderMap::new();

    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        HeaderName::from_static("x-accept"),
        HeaderValue::from_static("application/json"),
    );

    headers
}

pub fn client() -> Client {
    reqwest::Client::new()
}

pub async fn initiate_login(client: &Client) -> PocketCodeResponse {
    let headers = headers();

    let request_params = PocketInitiateOauthRequest {
        consumer_key: "99536-5a753dbe04d6ade99e80b4ab".to_owned(),
        redirect_uri: "pocket://kekw".to_owned(),
    };

    let res = client
        .post("https://getpocket.com/v3/oauth/request")
        .headers(headers)
        .json(&request_params)
        .send()
        .await
        .expect("Unexpected error");

    let code_response: PocketCodeResponse =
        res.json().await.expect("Could not decode the response");

    code_response
}

pub async fn authorize(client: &Client, auth_code: &str) -> PocketAccessTokenResponse {
    let headers = headers();

    let request_params = PocketAccessTokenRequest {
        consumer_key: "99536-5a753dbe04d6ade99e80b4ab".to_owned(),
        code: auth_code.to_owned(),
    };

    let res = client
        .post("https://getpocket.com/v3/oauth/authorize")
        .headers(headers)
        .json(&request_params)
        .send()
        .await
        .expect("Unexpected error");

    let code_response: PocketAccessTokenResponse =
        res.json().await.expect("Could not decode the response");

    code_response
}

pub async fn get_entries(client: &Client, access_token: &str) -> serde_json::Value {
    let headers = headers();
    let request_params = PocketEntriesRequest {
        consumer_key: "99536-5a753dbe04d6ade99e80b4ab".to_owned(),
        count: "30".to_owned(),
        access_token: access_token.to_owned(),
        total: "1".to_owned(),
        state: "unread".to_owned(),
        sort: "newest".to_owned(),
    };

    let entries: serde_json::Value = client
        .post("https://getpocket.com/v3/get")
        .headers(headers)
        .json(&request_params)
        .send()
        .await
        .expect("Unexpected error")
        .json()
        .await
        .expect("lmao");

    entries
}

pub async fn archive(client: &Client, access_token: &str, item_id: &str) -> () {
    let headers = headers();
    let request_params = PocketArchiveEntryRequest {
        consumer_key: "99536-5a753dbe04d6ade99e80b4ab".to_owned(),
        access_token: access_token.to_owned(),
        actions: vec![PocketArchiveAction {
            item_id: item_id.to_owned(),
            action: "archive".to_owned(),
        }],
    };

    client
        .post("https://getpocket.com/v3/send")
        .headers(headers)
        .json(&request_params)
        .send()
        .await
        .expect("Unexpected error");

    ()
}

pub fn encode_pocket_uri(auth_code: &str) -> String {
    let encoded_pocket_params: String = form_urlencoded::Serializer::new(String::new())
        .append_pair("request_token", auth_code)
        .append_pair("redirect_uri", "pocket://kekw")
        .finish();

    format!(
        "https://getpocket.com/auth/authorize?{}",
        encoded_pocket_params
    )
}
