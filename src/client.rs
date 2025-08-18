// This is free and unencumbered software released into the public domain.

use std::string::String;
use std::vec::Vec;

use asimov_module::secrecy::{ExposeSecret, SecretString};
use serde_json::json;

mod api;
use api::V1_API_URL;

pub mod error;
use error::*;

mod types;
pub use types::*;

#[tracing::instrument(skip_all)]
pub async fn login(
    client: &reqwest::Client,
    api_key: &SecretString,
    email: &SecretString,
    password: &SecretString,
) -> Result<LoginResult, LoginError> {
    let request = json!({
        "email": email.expose_secret(),
        "password": password.expose_secret(),
        "country": "US",
    });

    let api_url = std::format!("{V1_API_URL}/auth/login");

    tracing::debug!(url = api_url, "Requesting...");

    let response = client
        .post(api_url)
        .header("x-api-key", api_key.expose_secret())
        .json(&request)
        .send()
        .await
        .map_err(RequestError::from)?;

    let status = response.status();
    let body = response.text().await.map_err(RequestError::from)?;

    match serde_json::from_str::<api::LoginResponse>(&body) {
        Ok(api::LoginResponse::Success { success }) => match success {
            api::LoginResponseType::WithToken { login_token, .. } => {
                Ok(LoginResult::GotToken { login_token })
            }
            api::LoginResponseType::NeedCode { message, .. } => {
                Ok(LoginResult::NeedCode { message })
            }
        },
        Ok(api::LoginResponse::Error { message }) => Err(RequestError::Api(message).into()),
        Err(err) => {
            tracing::error!(?err, ?body, ?status, "failed to parse response");
            Err(RequestError::ParseError { status, body }.into())
        }
    }
}

#[tracing::instrument(skip_all)]
pub async fn verify(
    client: &reqwest::Client,
    api_key: &SecretString,
    email: &SecretString,
    code: &SecretString,
) -> Result<String, VerifyError> {
    let request = json!({
        "email": email.expose_secret(),
        "code": code.expose_secret(),
        "country": "US",
    });

    let api_url = std::format!("{V1_API_URL}/auth/verify");

    tracing::debug!(url = api_url, "Requesting...");

    let response = client
        .post(api_url)
        .header("x-api-key", api_key.expose_secret())
        .json(&request)
        .send()
        .await
        .map_err(RequestError::from)?;

    let status = response.status();
    let body = response.text().await.map_err(RequestError::from)?;

    match serde_json::from_str::<api::VerifyResponse>(&body) {
        Ok(api::VerifyResponse::Success { login_token, .. }) => Ok(login_token),
        Ok(api::VerifyResponse::Error { message, .. }) => Err(RequestError::Api(message).into()),
        Err(err) => {
            tracing::error!(?err, ?status, ?body, "failed to parse response");
            Err(RequestError::ParseError { status, body }.into())
        }
    }
}

#[derive(Clone, Debug, bon::Builder)]
#[builder(on(SecretString, into))]
pub struct Client {
    #[builder(default)]
    pub http_client: reqwest::Client,
    pub api_key: SecretString,
    pub login_token: SecretString,
}

impl Client {
    #[tracing::instrument(skip(self), fields(url = url.as_ref()))]
    pub async fn fetch(&self, url: impl AsRef<str>) -> Result<serde_json::Value, FetchError> {
        let url = url::Url::try_from(url.as_ref())?;

        if url
            .host_str()
            .is_none_or(|host| !host.ends_with("linkedin.com"))
        {
            return Err(FetchError::UnknownResource(url.into()));
        }

        let path = url.path();
        if path.starts_with("/in/") {
            return self.fetch_profile(&url).await;
        }
        if path.starts_with("/company/") {
            return self.fetch_company(&url).await;
        }
        if path.starts_with("/messaging/thread/") {
            return self
                .fetch_conversation(&url)
                .await
                .map(serde_json::Value::Array);
        }
        if path.starts_with("/messaging") {
            return self.fetch_inbox().await.map(serde_json::Value::Array);
        }
        if path.starts_with("/mynetwork/invite-connect/connections") {
            return self.fetch_connections().await.map(serde_json::Value::Array);
        }

        return Err(FetchError::UnknownResource(url.into()));
    }

    #[tracing::instrument(skip_all)]
    async fn fetch_company(&self, url: &url::Url) -> Result<serde_json::Value, FetchError> {
        let request = json!({
            "company_url": url.as_str(),
            "country": "US",
            "login_token": self.login_token.expose_secret(),
        });

        let api_url = std::format!("{V1_API_URL}/companies/info");

        tracing::debug!(linkedin_url = url.as_str(), url = api_url, "Requesting...");

        let response = self
            .http_client
            .post(api_url)
            .header("x-api-key", self.api_key.expose_secret())
            .json(&request)
            .send()
            .await
            .map_err(RequestError::from)?;

        let status = response.status();
        let body = response.text().await.map_err(RequestError::from)?;

        match serde_json::from_str::<api::FetchResponse>(&body) {
            Ok(api::FetchResponse::Success { data, .. }) => Ok(data),
            Ok(api::FetchResponse::Error { message, .. }) => Err(RequestError::Api(message).into()),
            Err(err) => {
                tracing::error!(?err, ?status, ?body, "failed to parse response");
                Err(RequestError::ParseError { status, body }.into())
            }
        }
    }

    #[tracing::instrument(skip_all)]
    async fn fetch_conversation(
        &self,
        url: &url::Url,
    ) -> Result<Vec<serde_json::Value>, FetchError> {
        // take id from /messaging/thread/:id
        let id = url
            .path_segments()
            .unwrap()
            .into_iter()
            .skip(2)
            .next()
            .unwrap();

        let conv_id = self
            .find_conversation(id)
            .await?
            .ok_or_else(|| FetchError::UnknownResource(url.as_str().into()))?;

        let mut all_messages = Vec::new();
        let mut start_page = 1;

        let batch_end_page_offset = 9;

        loop {
            let request = json!({
                "conversation_id": conv_id,
                "login_token": self.login_token.expose_secret(),
                "country": "US",
                "start_page": start_page,
                "end_page": start_page + batch_end_page_offset
            });

            let api_url = std::format!("{V1_API_URL}/messages/conversation");

            tracing::debug!(
                url = api_url,
                page = start_page,
                "Requesting conversation messages..."
            );

            let response = self
                .http_client
                .post(api_url)
                .header("x-api-key", self.api_key.expose_secret())
                .json(&request)
                .send()
                .await
                .map_err(RequestError::from)?;

            let status = response.status();
            let body = response.text().await.map_err(RequestError::from)?;

            tracing::debug!(body);

            match serde_json::from_str::<api::FetchResponse>(&body) {
                Ok(api::FetchResponse::Success { data }) => {
                    let Some(messages) = data["messages"].as_array() else {
                        break;
                    };

                    all_messages.extend_from_slice(messages);

                    if data["pagination"]["messages_per_page"]
                        .as_u64()
                        .is_none_or(|per_page| messages.len() < per_page as usize)
                    {
                        break;
                    };

                    start_page += batch_end_page_offset + 1;
                }
                Ok(api::FetchResponse::Error { message }) => {
                    return Err(RequestError::Api(message).into());
                }
                Err(err) => {
                    tracing::error!(?err, ?status, ?body, "failed to parse response");
                    return Err(RequestError::ParseError { status, body }.into());
                }
            }
        }

        Ok(all_messages)
    }

    #[tracing::instrument(skip_all)]
    async fn fetch_connections(&self) -> Result<Vec<serde_json::Value>, FetchError> {
        let mut all_connections = Vec::new();
        let mut start_page = 1;

        let batch_end_page_offset = 9;

        loop {
            let request = json!({
                "login_token": self.login_token.expose_secret(),
                "country": "US",
                "start_page": start_page,
                "end_page": start_page + batch_end_page_offset,
            });

            let api_url = std::format!("{V1_API_URL}/network/connections");

            tracing::debug!(
                url = api_url,
                page = start_page,
                "Requesting connections..."
            );

            let response = self
                .http_client
                .post(api_url)
                .header("x-api-key", self.api_key.expose_secret())
                .json(&request)
                .send()
                .await
                .map_err(RequestError::from)?;

            let status = response.status();
            let body = response.text().await.map_err(RequestError::from)?;

            match serde_json::from_str::<api::FetchResponse>(&body) {
                Ok(api::FetchResponse::Success { data }) => {
                    let Some(connections) = data["connections"].as_array() else {
                        break;
                    };

                    if connections.is_empty() {
                        break;
                    }

                    if data["total_results"]
                        .as_u64()
                        .is_some_and(|total| total == 0)
                    {
                        break;
                    }

                    all_connections.extend_from_slice(connections);

                    start_page += batch_end_page_offset + 1;
                }
                Ok(api::FetchResponse::Error { message }) => {
                    return Err(RequestError::Api(message).into());
                }
                Err(err) => {
                    tracing::error!(?err, ?status, ?body, "failed to parse response");
                    return Err(RequestError::ParseError { status, body }.into());
                }
            }
        }

        Ok(all_connections)
    }

    #[tracing::instrument(skip_all)]
    async fn fetch_inbox(&self) -> Result<Vec<serde_json::Value>, FetchError> {
        let mut all_conversations = Vec::new();
        let mut next_cursor: Option<String> = None;

        loop {
            let mut request = json!({
                "login_token": self.login_token.expose_secret(),
                "country": "US",
                // API doesn't accept bigger values? will return `"data":[]` in response which also breaks parsing
                "total_results": 25,
            });

            if let Some(cursor) = &next_cursor {
                request["next_cursor"] = json!(cursor);
            }

            let api_url = std::format!("{V1_API_URL}/messages/inbox");

            tracing::debug!(url = api_url, cursor = ?next_cursor, "Requesting inbox...");

            let response = self
                .http_client
                .post(api_url)
                .header("x-api-key", self.api_key.expose_secret())
                .json(&request)
                .send()
                .await
                .map_err(RequestError::from)?;

            let status = response.status();
            let body = response.text().await.map_err(RequestError::from)?;

            match serde_json::from_str::<api::FetchResponse>(&body) {
                Ok(api::FetchResponse::Success { data }) => {
                    let inbox_data: api::InboxData =
                        serde_json::from_value(data).map_err(|err| {
                            tracing::error!(?err, "failed to parse inbox data");
                            RequestError::InvalidJson(err)
                        })?;

                    all_conversations.extend(inbox_data.conversations);

                    // Check if there are more pages
                    if let Some(cursor) = inbox_data.next_cursor {
                        next_cursor = Some(cursor);
                    } else {
                        break;
                    }
                }
                Ok(api::FetchResponse::Error { message }) => {
                    return Err(RequestError::Api(message).into());
                }
                Err(err) => {
                    tracing::error!(?err, ?status, ?body, "failed to parse response");
                    return Err(RequestError::ParseError { status, body }.into());
                }
            }
        }

        Ok(all_conversations)
    }

    #[tracing::instrument(skip_all)]
    async fn fetch_profile(&self, url: &url::Url) -> Result<serde_json::Value, FetchError> {
        let request = json!({
            "linkedin_url": url.as_str(),
            "country": "US",
            "login_token": self.login_token.expose_secret(),
        });

        let api_url = std::format!("{V1_API_URL}/profile/info");

        tracing::debug!(linkedin_url = url.as_str(), url = api_url, "Requesting...");

        let response = self
            .http_client
            .post(api_url)
            .header("x-api-key", self.api_key.expose_secret())
            .json(&request)
            .send()
            .await
            .map_err(RequestError::from)?;

        let status = response.status();
        let body = response.text().await.map_err(RequestError::from)?;

        match serde_json::from_str::<api::FetchResponse>(&body) {
            Ok(api::FetchResponse::Success { data, .. }) => Ok(data),
            Ok(api::FetchResponse::Error { message, .. }) => Err(RequestError::Api(message).into()),
            Err(err) => {
                tracing::error!(?err, ?status, ?body, "failed to parse response");
                Err(RequestError::ParseError { status, body }.into())
            }
        }
    }

    #[tracing::instrument(skip(self))]
    async fn find_conversation(&self, id: &str) -> Result<Option<String>, FetchError> {
        let mut next_cursor: Option<String> = None;

        loop {
            let mut request = json!({
                "login_token": self.login_token.expose_secret(),
                "country": "US",
                // API doesn't accept bigger values? will return `"data":[]` in response which also breaks parsing
                "total_results": 25,
            });

            if let Some(cursor) = &next_cursor {
                request["next_cursor"] = json!(cursor);
            }

            let api_url = std::format!("{V1_API_URL}/messages/inbox");

            tracing::debug!(url = api_url, cursor = ?next_cursor, "Requesting inbox...");

            let response = self
                .http_client
                .post(api_url)
                .header("x-api-key", self.api_key.expose_secret())
                .json(&request)
                .send()
                .await
                .map_err(RequestError::from)?;

            let status = response.status();
            let body = response.text().await.map_err(RequestError::from)?;

            match serde_json::from_str::<api::FetchResponse>(&body) {
                Ok(api::FetchResponse::Success { data }) => {
                    let inbox_data: api::InboxData =
                        serde_json::from_value(data).map_err(|err| {
                            tracing::error!(?err, "failed to parse inbox data");
                            RequestError::InvalidJson(err)
                        })?;

                    for conv in inbox_data.conversations {
                        let Some(conv_id) = conv["conversation_id"].as_str() else {
                            continue;
                        };
                        if conv_id.contains(id) {
                            return Ok(Some(conv_id.into()));
                        }
                    }

                    // Check if there are more pages
                    if let Some(cursor) = inbox_data.next_cursor {
                        next_cursor = Some(cursor);
                    } else {
                        return Ok(None);
                    }
                }
                Ok(api::FetchResponse::Error { message }) => {
                    return Err(RequestError::Api(message).into());
                }
                Err(err) => {
                    tracing::error!(?err, ?status, ?body, "failed to parse response");
                    return Err(RequestError::ParseError { status, body }.into());
                }
            }
        }
    }
}
