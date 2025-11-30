use yup_oauth2::{InstalledFlowAuthenticator, authenticator::Authenticator, InstalledFlowReturnMethod};
use yup_oauth2::hyper_rustls::HttpsConnector;
use tracing::{trace};
use anyhow::{Result, Context};

const RES_FOLDER_PATH: &str = "src/app/res/";

pub struct OAuth2Data {
    pub authenticator: Option<Authenticator<HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>>>,
}

impl Default for OAuth2Data {
    fn default() -> Self {
        OAuth2Data {
            authenticator: None,
        }
    }
}

pub async fn oauth2_get_google_authenticator() -> Result<OAuth2Data> {
    /* Read application secret from a file, you can generate this file from the Google Cloud console: https://console.cloud.google.com/ */
    let secret_path = format!("{}credentials.json", RES_FOLDER_PATH);
    let secret = yup_oauth2::read_application_secret(&secret_path)
        .await
        .context("failed to read application secret")?;

    /* Create an authenticator that uses an InstalledFlow to authenticate. The
    authentication tokens are persisted to a file named tokencache.json. The
    authenticator takes care of caching tokens to disk and refreshing tokens once
    they've expired. */
    let token_cache_path = format!("{}tokencache.json", RES_FOLDER_PATH);
    let auth = InstalledFlowAuthenticator::builder(secret, InstalledFlowReturnMethod::HTTPRedirect)
        .persist_tokens_to_disk(token_cache_path)
        .build()
        .await
        .context("failed to create InstalledFlowAuthenticator")?;

    Ok(OAuth2Data {
        authenticator: Some(auth),
    })
}

pub async fn oauth2_get_google_token(oauth2_data: &OAuth2Data, scopes: &[&str]) -> Result<String> {
    /* Ensure authenticator exists; return an error if not. */
    let auth = oauth2_data.authenticator.as_ref().ok_or_else(|| {
        anyhow::anyhow!("Authenticator not initialized")
    })?;

    /* token(<scopes>) is the one important function of this crate; it does everything to
    obtain a token that can be sent e.g. as Bearer token. */
    let token = auth
        .token(scopes)
        .await
        .context("failed to obtain OAuth2 token")?;

    trace!("The token is {:?}", token);
    Ok(token.token().ok_or_else(|| {
        anyhow::anyhow!("Token is empty")
    })?.to_string())
}
