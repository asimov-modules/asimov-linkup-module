// This is free and unencumbered software released into the public domain.

use std::string::String;

#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("failed to parse response: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("API response is an error: {0}")]
    Api(String),
    #[error("failed to parse response as expected type, got status {status}: {body}")]
    ParseError {
        status: reqwest::StatusCode,
        body: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error(transparent)]
    Request(#[from] RequestError),
}

impl From<reqwest::Error> for LoginError {
    fn from(value: reqwest::Error) -> Self {
        Self::Request(value.into())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error(transparent)]
    Request(#[from] RequestError),
}

impl From<reqwest::Error> for VerifyError {
    fn from(value: reqwest::Error) -> Self {
        Self::Request(value.into())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),
    #[error("unknown resource: {0}")]
    UnknownResource(String),
    #[error(transparent)]
    Request(#[from] RequestError),
}

impl From<reqwest::Error> for FetchError {
    fn from(value: reqwest::Error) -> Self {
        FetchError::Request(RequestError::Http(value))
    }
}
