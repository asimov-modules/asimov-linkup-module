// This is free and unencumbered software released into the public domain.

use asimov_linkup_module::Client;
use asimov_module::SysexitsError::{self, *};
use clientele::{
    StandardOptions,
    crates::clap::{self, Parser},
};

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
async fn main() -> Result<asimov_module::SysexitsError, SysexitsError> {
    // Load environment variables from `.env`:

    use std::io::Write;

    use asimov_module::secrecy::SecretString;
    clientele::dotenv().ok();

    // Expand wildcards and @argfiles:
    let Ok(args) = clientele::args_os() else {
        return Ok(EX_USAGE);
    };
    let options = Options::parse_from(&args);

    #[cfg(feature = "tracing")]
    asimov_module::init_tracing_subscriber(&options.flags).expect("failed to initialize logging");

    if options.flags.version {
        println!("asimov-linkup-fetcher {}", env!("CARGO_PKG_VERSION"));
        return Ok(EX_OK);
    }

    if options.urls.is_empty() {
        return Ok(EX_OK);
    }

    let manifest = match asimov_module::ModuleManifest::read_manifest("linkup") {
        Ok(manifest) => manifest,
        Err(e) => {
            tracing::error!("failed to read module manifest: {e}");
            return Ok(EX_CONFIG);
        }
    };

    // Obtain the Apify API token from the environment:
    let api_key: SecretString = match manifest.variable("linkup-api-key", None) {
        Ok(api_key) => api_key.into(),
        Err(e) => {
            tracing::error!("failed to get LinkUp API key: {e}");
            return Ok(EX_CONFIG); // not configured
        }
    };

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap();

    let token = keyring::Entry::new("asimov-linkup-module", "login-token").unwrap();
    let login_token = match token.get_password() {
        Ok(token) => token,
        Err(keyring::Error::NoEntry) => {
            let email: SecretString = match manifest.variable("linkedin-email", None) {
                Ok(email) => email.into(),
                Err(e) => {
                    tracing::error!("failed to get LinkedIn email: {e}");
                    return Ok(EX_CONFIG); // not configured
                }
            };

            let password: SecretString = match manifest.variable("linkedin-password", None) {
                Ok(pass) => pass.into(),
                Err(e) => {
                    tracing::error!("failed to get LinkedIn password: {e}");
                    return Ok(EX_CONFIG); // not configured
                }
            };

            match asimov_linkup_module::login(&http_client, &api_key, &email, &password).await {
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

                    let token_string =
                        asimov_linkup_module::verify(&http_client, &api_key, &email, &code)
                            .await
                            .map_err(|e| {
                                tracing::error!("code verification failed: {e}");
                                EX_UNAVAILABLE
                            })?;

                    token.set_password(&token_string).unwrap();

                    token_string
                }
                Err(e) => {
                    tracing::error!("login flow failed: {e}");
                    return Err(EX_UNAVAILABLE);
                }
            }
        }
        Err(e) => {
            tracing::error!("failed to read login token from keychain: {e}");
            return Ok(EX_UNAVAILABLE);
        }
    };

    let client = Client::builder()
        .login_token(login_token)
        .api_key(api_key)
        .build();

    let mut stdout = std::io::stdout().lock();
    for url in options.urls {
        let response = client.fetch(&url).await.map_err(|e| {
            tracing::error!("request failed: {e}");
            EX_UNAVAILABLE
        })?;

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

#[cfg(not(feature = "std"))]
fn main() {
    unimplemented!("asimov-linkup-fetcher requires the 'std' feature")
}
