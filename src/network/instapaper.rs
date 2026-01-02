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

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{Mock, Server};

    fn create_test_tokens() -> TokenPair {
        TokenPair {
            oauth_token: "test_token".to_string(),
            oauth_token_secret: "test_secret".to_string(),
        }
    }

    #[tokio::test]
    async fn test_authenticate_success() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/1/oauth/access_token")
            .with_status(200)
            .with_body("oauth_token=token123&oauth_token_secret=secret456")
            .create_async()
            .await;

        let client = Client::new();
        let result =
            authenticate_with_base_url(&client, "testuser", "testpass", &server.url()).await;

        mock.assert_async().await;
        assert!(result.is_ok());
        let tokens = result.unwrap();
        assert_eq!(tokens.oauth_token, "token123");
        assert_eq!(tokens.oauth_token_secret, "secret456");
    }

    #[tokio::test]
    async fn test_authenticate_invalid_credentials() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/1/oauth/access_token")
            .with_status(401)
            .create_async()
            .await;

        let client = Client::new();
        let result =
            authenticate_with_base_url(&client, "testuser", "wrongpass", &server.url()).await;

        mock.assert_async().await;
        assert!(matches!(result, Err(InstapaperError::InvalidCredentials)));
    }

    #[tokio::test]
    async fn test_authenticate_service_unavailable() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/1/oauth/access_token")
            .with_status(503)
            .create_async()
            .await;

        let client = Client::new();
        let result =
            authenticate_with_base_url(&client, "testuser", "testpass", &server.url()).await;

        mock.assert_async().await;
        assert!(matches!(result, Err(InstapaperError::ServiceUnavailable)));
    }

    #[tokio::test]
    async fn test_authenticate_parse_error() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/1/oauth/access_token")
            .with_status(200)
            .with_body("invalid_response")
            .create_async()
            .await;

        let client = Client::new();
        let result =
            authenticate_with_base_url(&client, "testuser", "testpass", &server.url()).await;

        mock.assert_async().await;
        assert!(matches!(result, Err(InstapaperError::ParseError(_))));
    }

    #[tokio::test]
    async fn test_verify_credentials_success() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/1/account/verify_credentials")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"type":"user","user_id":12345,"username":"testuser"}]"#)
            .create_async()
            .await;

        let client = Client::new();
        let tokens = create_test_tokens();
        let result = verify_credentials_with_base_url(&client, &tokens, &server.url()).await;

        mock.assert_async().await;
        assert!(result.is_ok());
        let user = result.unwrap();
        assert_eq!(user.user_id, 12345);
        assert_eq!(user.username, "testuser");
    }

    #[tokio::test]
    async fn test_verify_credentials_invalid() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/1/account/verify_credentials")
            .with_status(401)
            .create_async()
            .await;

        let client = Client::new();
        let tokens = create_test_tokens();
        let result = verify_credentials_with_base_url(&client, &tokens, &server.url()).await;

        mock.assert_async().await;
        assert!(matches!(result, Err(InstapaperError::InvalidCredentials)));
    }

    #[tokio::test]
    async fn test_verify_credentials_rate_limited() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/1/account/verify_credentials")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"type":"error","error_code":1040,"message":"Rate limit exceeded"}]"#)
            .create_async()
            .await;

        let client = Client::new();
        let tokens = create_test_tokens();
        let result = verify_credentials_with_base_url(&client, &tokens, &server.url()).await;

        mock.assert_async().await;
        assert!(matches!(result, Err(InstapaperError::RateLimited)));
    }

    #[tokio::test]
    async fn test_get_bookmarks_success() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/1/bookmarks/list")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[
                {"type":"meta"},
                {"type":"user","user_id":123,"username":"test"},
                {"type":"bookmark","bookmark_id":1,"title":"Test Article","url":"https://example.com","description":"Test desc","time":1234567890.0,"progress":0.0,"hash":"abc","starred":"0"}
            ]"#)
            .create_async()
            .await;

        let client = Client::new();
        let tokens = create_test_tokens();
        let result = get_bookmarks_with_base_url(&client, &tokens, &server.url()).await;

        mock.assert_async().await;
        assert!(result.is_ok());
        let bookmarks = result.unwrap();
        assert_eq!(bookmarks.len(), 1);
        assert_eq!(bookmarks[0].bookmark_id, 1);
        assert_eq!(bookmarks[0].title, "Test Article");
        assert_eq!(bookmarks[0].url, "https://example.com");
    }

    #[tokio::test]
    async fn test_get_bookmarks_unauthorized() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/1/bookmarks/list")
            .with_status(401)
            .create_async()
            .await;

        let client = Client::new();
        let tokens = create_test_tokens();
        let result = get_bookmarks_with_base_url(&client, &tokens, &server.url()).await;

        mock.assert_async().await;
        assert!(matches!(result, Err(InstapaperError::InvalidCredentials)));
    }

    #[tokio::test]
    async fn test_archive_bookmark_success() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/1/bookmarks/archive")
            .with_status(200)
            .create_async()
            .await;

        let client = Client::new();
        let tokens = create_test_tokens();
        let result = archive_bookmark_with_base_url(&client, &tokens, 12345, &server.url()).await;

        mock.assert_async().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_archive_bookmark_unauthorized() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/1/bookmarks/archive")
            .with_status(401)
            .create_async()
            .await;

        let client = Client::new();
        let tokens = create_test_tokens();
        let result = archive_bookmark_with_base_url(&client, &tokens, 12345, &server.url()).await;

        mock.assert_async().await;
        assert!(matches!(result, Err(InstapaperError::InvalidCredentials)));
    }

    #[tokio::test]
    async fn test_add_bookmark_success() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/1/bookmarks/add")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[
                {"type":"bookmark","bookmark_id":999,"title":"New Article","url":"https://example.com/new","description":"","time":1234567890.0,"progress":0.0,"hash":"xyz","starred":"0"}
            ]"#)
            .create_async()
            .await;

        let client = Client::new();
        let tokens = create_test_tokens();
        let result =
            add_bookmark_with_base_url(&client, &tokens, "https://example.com/new", &server.url())
                .await;

        mock.assert_async().await;
        assert!(result.is_ok());
        let bookmark = result.unwrap();
        assert_eq!(bookmark.bookmark_id, 999);
        assert_eq!(bookmark.title, "New Article");
    }

    #[tokio::test]
    async fn test_add_bookmark_unauthorized() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/1/bookmarks/add")
            .with_status(401)
            .create_async()
            .await;

        let client = Client::new();
        let tokens = create_test_tokens();
        let result =
            add_bookmark_with_base_url(&client, &tokens, "https://example.com/new", &server.url())
                .await;

        mock.assert_async().await;
        assert!(matches!(result, Err(InstapaperError::InvalidCredentials)));
    }

    #[tokio::test]
    async fn test_add_bookmark_rate_limited() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("POST", "/api/1/bookmarks/add")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"type":"error","error_code":1040,"message":"Rate limit exceeded"}]"#)
            .create_async()
            .await;

        let client = Client::new();
        let tokens = create_test_tokens();
        let result =
            add_bookmark_with_base_url(&client, &tokens, "https://example.com/new", &server.url())
                .await;

        mock.assert_async().await;
        assert!(matches!(result, Err(InstapaperError::RateLimited)));
    }

    async fn authenticate_with_base_url(
        client: &Client,
        username: &str,
        password: &str,
        base_url: &str,
    ) -> Result<TokenPair, InstapaperError> {
        let url = format!("{}/api/1/oauth/access_token", base_url);
        let request = XAuthRequest {
            x_auth_username: username,
            x_auth_password: password,
            x_auth_mode: "client_auth",
        };

        let token = oauth1_request::Token::from_parts(CONSUMER_KEY, CONSUMER_SECRET, "", "");
        let auth_header =
            oauth1_request::post(&url, &request, &token, oauth1_request::HmacSha1::new());

        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_str(&auth_header).unwrap());
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

    async fn verify_credentials_with_base_url(
        client: &Client,
        tokens: &TokenPair,
        base_url: &str,
    ) -> Result<InstapaperUser, InstapaperError> {
        let url = format!("{}/api/1/account/verify_credentials", base_url);
        let request = EmptyRequest {};
        let token = oauth1_request::Token::from_parts(
            CONSUMER_KEY,
            CONSUMER_SECRET,
            &tokens.oauth_token,
            &tokens.oauth_token_secret,
        );

        let auth_header =
            oauth1_request::post(&url, &request, &token, oauth1_request::HmacSha1::new());
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_str(&auth_header).unwrap());

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

    async fn get_bookmarks_with_base_url(
        client: &Client,
        tokens: &TokenPair,
        base_url: &str,
    ) -> Result<Vec<InstapaperBookmark>, InstapaperError> {
        let url = format!("{}/api/1/bookmarks/list", base_url);
        let request = BookmarksListRequest { limit: 500 };
        let token = oauth1_request::Token::from_parts(
            CONSUMER_KEY,
            CONSUMER_SECRET,
            &tokens.oauth_token,
            &tokens.oauth_token_secret,
        );

        let auth_header =
            oauth1_request::post(&url, &request, &token, oauth1_request::HmacSha1::new());
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_str(&auth_header).unwrap());
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

        let items: Vec<InstapaperResponse> = response
            .json()
            .await
            .map_err(|e| InstapaperError::ParseError(format!("Failed to parse response: {}", e)))?;

        let bookmarks = items
            .into_iter()
            .filter_map(|item| {
                if let InstapaperResponse::Bookmark(b) = item {
                    Some(b)
                } else {
                    None
                }
            })
            .collect();

        Ok(bookmarks)
    }

    async fn archive_bookmark_with_base_url(
        client: &Client,
        tokens: &TokenPair,
        bookmark_id: i64,
        base_url: &str,
    ) -> Result<(), InstapaperError> {
        let url = format!("{}/api/1/bookmarks/archive", base_url);
        let request = BookmarkArchiveRequest { bookmark_id };
        let token = oauth1_request::Token::from_parts(
            CONSUMER_KEY,
            CONSUMER_SECRET,
            &tokens.oauth_token,
            &tokens.oauth_token_secret,
        );

        let auth_header =
            oauth1_request::post(&url, &request, &token, oauth1_request::HmacSha1::new());
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_str(&auth_header).unwrap());
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

    async fn add_bookmark_with_base_url(
        client: &Client,
        tokens: &TokenPair,
        url: &str,
        base_url: &str,
    ) -> Result<InstapaperBookmark, InstapaperError> {
        let api_url = format!("{}/api/1/bookmarks/add", base_url);
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
        headers.insert(AUTHORIZATION, HeaderValue::from_str(&auth_header).unwrap());
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
}
