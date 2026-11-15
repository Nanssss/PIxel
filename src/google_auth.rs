//! Google OAuth2 authentication, for the states reading from a Google API.
//!
//! Built on the `yup-oauth2` crate, which does all the work: asking the user for
//! the authorization on the first run, caching the tokens on disk, and refreshing
//! them once they have expired.

use anyhow::{Context, Result};
use hyper_util::client::legacy::connect::HttpConnector;
use yup_oauth2::{InstalledFlowAuthenticator, InstalledFlowReturnMethod};
use yup_oauth2::authenticator::Authenticator;
use yup_oauth2::hyper_rustls::HttpsConnector;
use crate::EXECUTABLE_DIR;

/// Application secret, relative to the executable directory. Generate this file
/// from the Google Cloud console: https://console.cloud.google.com/
const CREDENTIALS_FILE: &str = "in/credentials.json";

/// Where the tokens are cached, relative to the executable directory
const TOKEN_FILE: &str = "in/tokencache.json";

/// Gives out the access tokens needed to call the Google APIs.
// The field is private: the rest of the app doesn't depend on `yup-oauth2`, and
// only sees `new` and `token`.
pub struct GoogleAuth {
    authenticator: Authenticator<HttpsConnector<HttpConnector>>,
}

impl GoogleAuth {
    /// Prepares the authentication. Only local files are read here: nothing is
    /// asked to Google before the first call to [`GoogleAuth::token`].
    ///
    /// # Errors
    /// If the application secret can't be read, or the authenticator can't be built.
    pub async fn new() -> Result<Self> {
        let secret_path = EXECUTABLE_DIR.join(CREDENTIALS_FILE);
        let secret = yup_oauth2::read_application_secret(&secret_path)
            .await
            .with_context(|| format!("failed to read application secret {secret_path:?}"))?;

        // *Installed flow*: on the first run, the user grants the access in a browser,
        // and Google redirects it to a local HTTP server started by the authenticator.
        let authenticator = InstalledFlowAuthenticator::builder(secret, InstalledFlowReturnMethod::HTTPRedirect)
            .persist_tokens_to_disk(EXECUTABLE_DIR.join(TOKEN_FILE))
            .build()
            .await
            .context("failed to create InstalledFlowAuthenticator")?;

        Ok(GoogleAuth { authenticator })
    }

    /// Returns an access token valid for `scopes`, to be sent as a `Bearer` token.
    ///
    /// The token comes from the cache when it is still valid, so this only goes
    /// through the network when a new one is needed.
    ///
    /// # Errors
    /// If no token could be obtained (no network, authorization refused...).
    // The token is a secret: never log it.
    pub async fn token(&self, scopes: &[&str]) -> Result<String> {
        let token = self.authenticator
            .token(scopes)
            .await
            .context("failed to obtain OAuth2 token")?;

        // `.context()` also works on an `Option`: `None` becomes an error with this message
        Ok(token.token().context("OAuth2 token is empty")?.to_string())
    }
}
