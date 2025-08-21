// This is free and unencumbered software released into the public domain.

use std::string::String;
use std::vec::Vec;

pub const V1_API_URL: &str = "https://api.linkupapi.com/v1";

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "status")]
pub enum LoginResponse {
    #[serde(rename = "success")]
    Success {
        #[serde(flatten)]
        success: LoginResponseType,
    },
    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum LoginResponseType {
    // Already contains login_token and hence requires no further auth flow.
    // {
    //   "status": "success",
    //   "login_token": "<token>",
    //   "message": "Login successful"
    // }
    WithToken {
        login_token: String,
        message: String,
    },
    // {
    //   "status": "success",
    //   "message": "Check your email for verification code",
    //   "email": "<email>"
    // }
    NeedCode {
        email: String,
        message: String,
    },
}

// {
//   "status": "success",
//   "message": "Login successful",
//   "login_token": "<token>"
// }
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "status")]
pub enum VerifyResponse {
    #[serde(rename = "success")]
    Success {
        login_token: String,
        message: Option<String>,
    },
    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "status")]
pub enum FetchResponse {
    // {"status":"success","data": { ... }}
    #[serde(rename = "success")]
    Success { data: serde_json::Value },
    // {"status":"error","message":"Invalid parameter"}
    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct InboxData {
    pub conversations: Vec<serde_json::Value>,
    pub total_results: u32,
    pub next_cursor: Option<String>,
}
