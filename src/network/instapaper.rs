use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;

use crate::config::{CONSUMER_KEY, CONSUMER_SECRET};
use crate::persistence::token::TokenPair;

const BASE_URL: &str = "https://www.instapaper.com";

#[derive(Debug)]
pub enum InstapaperError {
    Network(reqwest::Error),
    InvalidCredentials,
    RateLimited,
    ServiceUnavailable,
    ParseError(String),
}

impl From<reqwest::Error> for InstapaperError {
    fn from(err: reqwest::Error) -> Self {
        InstapaperError::Network(err)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct InstapaperUser {
    pub user_id: i64,
    pub username: String,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct InstapaperBookmark {
    pub bookmark_id: i64,
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub progress: f64,
    #[serde(default)]
    pub time: f64,
    #[serde(default)]
    pub hash: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub starred: String,
    // Capture any other fields we don't explicitly need
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum InstapaperResponse {
    User(InstapaperUser),
    Bookmark(InstapaperBookmark),
    Meta(MetaResponse),
    Error(ErrorResponse),
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub error_code: i32,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct MetaResponse {
    // Meta objects may have additional fields, but we don't need them
    #[serde(flatten)]
    extra: HashMap<String, serde_json::Value>,
}

// Request structs for OAuth signing
#[derive(oauth1_request::Request)]
struct XAuthRequest<'a> {
    x_auth_username: &'a str,
    x_auth_password: &'a str,
    x_auth_mode: &'a str,
}

#[derive(oauth1_request::Request)]
struct EmptyRequest {}

#[derive(oauth1_request::Request)]
struct BookmarksListRequest {
    limit: u32,
}

#[derive(oauth1_request::Request)]
struct BookmarkArchiveRequest {
    bookmark_id: i64,
}

#[derive(oauth1_request::Request)]
struct BookmarkAddRequest<'a> {
    url: &'a str,
}

pub fn client() -> Client {
    reqwest::Client::new()
}

/// Authenticate with Instapaper using xAuth
/// Returns OAuth token pair on success
pub async fn authenticate(
    client: &Client,
    username: &str,
    password: &str,
) -> Result<TokenPair, InstapaperError> {
    let url = format!("{}/api/1/oauth/access_token", BASE_URL);

    let request = XAuthRequest {
        x_auth_username: username,
        x_auth_password: password,
        x_auth_mode: "client_auth",
    };

    println!(
        "consumer key: {:?}, consumer secret: {:?}, user: {:?}, pass: {:?}",
        CONSUMER_KEY, CONSUMER_SECRET, username, password
    );
    // For xAuth, we use empty token credentials (only consumer credentials)
    let token = oauth1_request::Token::from_parts(CONSUMER_KEY, CONSUMER_SECRET, "", "");

    let auth_header = oauth1_request::post(&url, &request, &token, oauth1_request::HmacSha1::new());

    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&auth_header).expect("Invalid auth header"),
    );
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/x-www-form-urlencoded"),
    );

    let body = format!(
        "x_auth_username={}&x_auth_password={}&x_auth_mode=client_auth",
        urlencoding::encode(username),
        urlencoding::encode(password)
    );

    let response = client.post(&url).headers(headers).body(body).send().await?;

    if response.status() == 401 {
        return Err(InstapaperError::InvalidCredentials);
    }

    if response.status() == 503 {
        return Err(InstapaperError::ServiceUnavailable);
    }

    let text = response.text().await?;

    // Response format: oauth_token=xxx&oauth_token_secret=yyy
    let mut oauth_token = String::new();
    let mut oauth_token_secret = String::new();

    for pair in text.split('&') {
        let mut parts = pair.splitn(2, '=');
        if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
            match key {
                "oauth_token" => oauth_token = value.to_string(),
                "oauth_token_secret" => oauth_token_secret = value.to_string(),
                _ => {}
            }
        }
    }

    if oauth_token.is_empty() || oauth_token_secret.is_empty() {
        return Err(InstapaperError::ParseError(
            "Failed to parse OAuth tokens".to_string(),
        ));
    }

    Ok(TokenPair {
        oauth_token,
        oauth_token_secret,
    })
}

pub async fn verify_credentials(
    client: &Client,
    tokens: &TokenPair,
) -> Result<InstapaperUser, InstapaperError> {
    let url = format!("{}/api/1/account/verify_credentials", BASE_URL);

    let request = EmptyRequest {};
    let token = oauth1_request::Token::from_parts(
        CONSUMER_KEY,
        CONSUMER_SECRET,
        &tokens.oauth_token,
        &tokens.oauth_token_secret,
    );

    let auth_header = oauth1_request::post(&url, &request, &token, oauth1_request::HmacSha1::new());

    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&auth_header).expect("Invalid auth header"),
    );

    let response = client.post(&url).headers(headers).send().await?;

    if response.status() == 401 {
        return Err(InstapaperError::InvalidCredentials);
    }

    let items: Vec<InstapaperResponse> = response
        .json()
        .await
        .map_err(|e| InstapaperError::ParseError(format!("Failed to parse response: {}", e)))?;

    for item in items {
        if let InstapaperResponse::User(user) = item {
            return Ok(user);
        }
        if let InstapaperResponse::Error(err) = item {
            if err.error_code == 1040 {
                return Err(InstapaperError::RateLimited);
            }
            return Err(InstapaperError::ParseError(format!(
                "API error {}: {}",
                err.error_code, err.message
            )));
        }
    }

    Err(InstapaperError::ParseError(
        "No user in response".to_string(),
    ))
}

pub async fn get_bookmarks(
    client: &Client,
    tokens: &TokenPair,
) -> Result<Vec<InstapaperBookmark>, InstapaperError> {
    let url = format!("{}/api/1/bookmarks/list", BASE_URL);

    let request = BookmarksListRequest { limit: 500 };
    let token = oauth1_request::Token::from_parts(
        CONSUMER_KEY,
        CONSUMER_SECRET,
        &tokens.oauth_token,
        &tokens.oauth_token_secret,
    );

    let auth_header = oauth1_request::post(&url, &request, &token, oauth1_request::HmacSha1::new());

    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&auth_header).expect("Invalid auth header"),
    );
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/x-www-form-urlencoded"),
    );

    let response = client
        .post(&url)
        .headers(headers)
        .body("limit=500")
        .send()
        .await?;

    if response.status() == 401 {
        return Err(InstapaperError::InvalidCredentials);
    }

    // Instapaper returns an array with meta, user, and bookmark objects
    let items: Vec<InstapaperResponse> = response
        .json()
        .await
        .map_err(|e| InstapaperError::ParseError(format!("Failed to parse response: {}", e)))?;

    println!("Parsed {} items from Instapaper API", items.len());

    let bookmarks: Vec<InstapaperBookmark> = items
        .into_iter()
        .filter_map(|item| {
            if let InstapaperResponse::Bookmark(bookmark) = item {
                Some(bookmark)
            } else {
                None
            }
        })
        .collect();

    println!("Extracted {} bookmarks", bookmarks.len());

    Ok(bookmarks)
}

pub async fn archive_bookmark(
    client: &Client,
    tokens: &TokenPair,
    bookmark_id: i64,
) -> Result<(), InstapaperError> {
    let url = format!("{}/api/1/bookmarks/archive", BASE_URL);

    let request = BookmarkArchiveRequest { bookmark_id };
    let token = oauth1_request::Token::from_parts(
        CONSUMER_KEY,
        CONSUMER_SECRET,
        &tokens.oauth_token,
        &tokens.oauth_token_secret,
    );

    let auth_header = oauth1_request::post(&url, &request, &token, oauth1_request::HmacSha1::new());

    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&auth_header).expect("Invalid auth header"),
    );
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/x-www-form-urlencoded"),
    );

    let body = format!("bookmark_id={}", bookmark_id);

    let response = client.post(&url).headers(headers).body(body).send().await?;

    if response.status() == 401 {
        return Err(InstapaperError::InvalidCredentials);
    }

    Ok(())
}

pub async fn add_bookmark(
    client: &Client,
    tokens: &TokenPair,
    url: &str,
) -> Result<InstapaperBookmark, InstapaperError> {
    let api_url = format!("{}/api/1/bookmarks/add", BASE_URL);

    let request = BookmarkAddRequest { url };
    let token = oauth1_request::Token::from_parts(
        CONSUMER_KEY,
        CONSUMER_SECRET,
        &tokens.oauth_token,
        &tokens.oauth_token_secret,
    );

    let auth_header =
        oauth1_request::post(&api_url, &request, &token, oauth1_request::HmacSha1::new());

    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&auth_header).expect("Invalid auth header"),
    );
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/x-www-form-urlencoded"),
    );

    let body = format!("url={}", urlencoding::encode(url));

    let response = client
        .post(&api_url)
        .headers(headers)
        .body(body)
        .send()
        .await?;

    if response.status() == 401 {
        return Err(InstapaperError::InvalidCredentials);
    }

    // Instapaper returns an array with the newly added bookmark
    let items: Vec<InstapaperResponse> = response
        .json()
        .await
        .map_err(|e| InstapaperError::ParseError(format!("Failed to parse response: {}", e)))?;

    for item in items {
        if let InstapaperResponse::Bookmark(bookmark) = item {
            return Ok(bookmark);
        }
        if let InstapaperResponse::Error(err) = item {
            if err.error_code == 1040 {
                return Err(InstapaperError::RateLimited);
            }
            return Err(InstapaperError::ParseError(format!(
                "API error {}: {}",
                err.error_code, err.message
            )));
        }
    }

    Err(InstapaperError::ParseError(
        "No bookmark in response".to_string(),
    ))
}
