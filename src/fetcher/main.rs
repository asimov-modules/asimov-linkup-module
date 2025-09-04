// This is free and unencumbered software released into the public domain.

use asimov_linkup_module::Client;
use asimov_module::{
    ModuleManifest,
    SysexitsError::{self, *},
    secrecy::SecretString,
};
use clientele::{
    StandardOptions,
    crates::clap::{self, Parser},
};
use std::{io::Write, time::Duration};

#[cfg(not(feature = "std"))]
fn main() {
    unimplemented!("asimov-linkup-fetcher requires the 'std' feature")
}

/// ASIMOV Linkup Fetcher
#[derive(Debug, Parser)]
#[command(name = "asimov-linkup-fetcher", long_about)]
struct Options {
    #[clap(flatten)]
    flags: StandardOptions,

    /// The maximum number of resources to list.
    #[arg(value_name = "COUNT", short = 'n', long)]
    limit: Option<usize>,

    /// The output format.
    #[arg(value_name = "FORMAT", short = 'o', long)]
    output: Option<String>,

    urls: Vec<String>,
}

#[cfg(feature = "std")]
#[tokio::main]
async fn main() -> Result<SysexitsError, SysexitsError> {
    // Load environment variables from `.env`:
    clientele::dotenv().ok();

    // Expand wildcards and @argfiles:
    let Ok(args) = clientele::args_os() else {
        return Err(EX_USAGE);
    };
    let options = Options::parse_from(&args);

    #[cfg(feature = "tracing")]
    asimov_module::init_tracing_subscriber(&options.flags).expect("failed to initialize logging");

    if options.flags.version {
        println!("asimov-linkup-fetcher {}", env!("CARGO_PKG_VERSION"));
        return Err(EX_OK);
    }

    if options.urls.is_empty() {
        return Err(EX_OK);
    }

    let manifest = match asimov_module::ModuleManifest::read_manifest("linkup") {
        Ok(manifest) => manifest,
        Err(e) => {
            tracing::error!("failed to read module manifest: {e}");
            return Err(EX_CONFIG);
        }
    };

    // Obtain the Linkup API key from the environment:
    let api_key: SecretString = match manifest.variable("linkup-api-key", None) {
        Ok(api_key) => api_key.into(),
        Err(e) => {
            tracing::error!("failed to get Linkup API key: {e}");
            return Err(EX_CONFIG); // not configured
        }
    };

    let http_client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(30))
        .build()
        .unwrap();

    // Get or create login token
    let login_token = match get_saved_token()? {
        Some(token) => token,
        None => {
            let token = login(&http_client, &manifest).await?;
            save_token(&token)?;
            token
        }
    };

    let mut client = Client::builder()
        .login_token(login_token)
        .api_key(api_key.clone())
        .http_client(http_client.clone())
        .build();

    let mut stdout = std::io::stdout().lock();
    for url in options.urls {
        use asimov_linkup_module::error::{FetchError, RequestError};
        use reqwest::StatusCode;

        let response = match client.fetch(&url).await {
            Ok(r) => r,
            Err(FetchError::Request(RequestError::Http(err)))
                if err.status() == Some(StatusCode::FORBIDDEN) =>
            {
                // Token expired, reset and re-login
                let new_login_token = login(&http_client, &manifest).await?;

                save_token(&new_login_token)?;

                client = Client::builder()
                    .login_token(new_login_token)
                    .api_key(api_key.clone())
                    .http_client(http_client.clone())
                    .build();

                match client.fetch(&url).await {
                    Ok(response) => response,
                    Err(e) => {
                        tracing::error!("request failed: {e}");
                        return Err(EX_UNAVAILABLE);
                    }
                }
            }
            Err(e) => {
                tracing::error!("request failed: {e}");
                return Err(EX_UNAVAILABLE);
            }
        };

        match response {
            serde_json::Value::Array(values) => {
                for value in values {
                    serde_json::to_writer(&mut stdout, &value).unwrap();
                    writeln!(&mut stdout).unwrap();
                }
            }
            value => serde_json::to_writer(&mut stdout, &value).unwrap(),
        }
    }

    Ok(EX_OK)
}

fn get_saved_token() -> Result<Option<String>, SysexitsError> {
    match keyring::Entry::new("asimov-linkup-module", "login-token")
        .unwrap()
        .get_password()
    {
        Ok(token) if token.is_empty() => Ok(None),
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => {
            tracing::error!("failed to read login token from keychain: {e}");
            Err(EX_UNAVAILABLE)
        }
    }
}

fn save_token(token: &str) -> Result<(), SysexitsError> {
    keyring::Entry::new("asimov-linkup-module", "login-token")
        .unwrap()
        .set_password(token)
        .map_err(|e| {
            tracing::error!("failed to save login token to keychain: {e}");
            EX_UNAVAILABLE
        })
}

async fn login(
    http_client: &reqwest::Client,
    manifest: &ModuleManifest,
) -> Result<String, SysexitsError> {
    let api_key: SecretString = manifest
        .variable("linkup-api-key", None)
        .map(Into::into)
        .map_err(|e| {
            tracing::error!("failed to get Linkup API key: {e}");
            EX_CONFIG
        })?;

    let email: SecretString = manifest
        .variable("linkedin-email", None)
        .map(Into::into)
        .map_err(|e| {
            tracing::error!("failed to get LinkedIn email: {e}");
            EX_CONFIG
        })?;

    let password: SecretString = manifest
        .variable("linkedin-password", None)
        .map(Into::into)
        .map_err(|e| {
            tracing::error!("failed to get LinkedIn password: {e}");
            EX_CONFIG
        })?;

    let token = match asimov_linkup_module::login(http_client, &api_key, &email, &password).await {
        Ok(asimov_linkup_module::LoginResult::GotToken { login_token, .. }) => login_token,
        Ok(asimov_linkup_module::LoginResult::NeedCode { message, .. }) => {
            let mut stdout = std::io::stdout().lock();
            let mut stdin = std::io::stdin().lines();

            stdout.write_all(b"Verification code required.\n").unwrap();
            std::writeln!(&mut stdout, "LinkUp API response: `{message}`").unwrap();
            stdout.write_all(b"Enter code:\n").unwrap();

            let code = loop {
                stdout.write_all(b"> ").unwrap();
                stdout.flush().unwrap();

                match stdin.next() {
                    Some(Ok(code)) if !code.trim().is_empty() => break code.trim().into(),
                    Some(Ok(_)) => continue,
                    Some(Err(e)) => {
                        tracing::error!("error while reading input: {e}");
                        return Err(EX_UNAVAILABLE);
                    }
                    None => {
                        tracing::error!("verification code is required");
                        return Err(EX_UNAVAILABLE);
                    }
                }
            };

            match asimov_linkup_module::verify(http_client, &api_key, &email, &code).await {
                Ok(token_string) => token_string,
                Err(e) => {
                    tracing::error!("code verification failed: {e}");
                    return Err(EX_UNAVAILABLE);
                }
            }
        }
        Err(e) => {
            tracing::error!("login flow failed: {e}");
            return Err(EX_UNAVAILABLE);
        }
    };

    Ok(token)
}
