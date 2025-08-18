// This is free and unencumbered software released into the public domain.

use std::string::String;

#[derive(Clone, Debug)]
pub enum LoginResult {
    GotToken { login_token: String },
    NeedCode { message: String },
}
