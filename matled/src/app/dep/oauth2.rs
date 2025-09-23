use yup_oauth2::{InstalledFlowAuthenticator, InstalledFlowReturnMethod};

pub async fn oauth2_login_google() {
    /* Read application secret from a file, you can generate this file from the Google Cloud console: https://console.cloud.google.com/ */
    let secret = yup_oauth2::read_application_secret("src/app/res/credentials.json")
        .await
        .expect("Invalid credentials.json path, file misnamed, or file missing.");

    /* Create an authenticator that uses an InstalledFlow to authenticate. The
    authentication tokens are persisted to a file named tokencache.json. The
    authenticator takes care of caching tokens to disk and refreshing tokens once
    they've expired. */
    let mut auth = InstalledFlowAuthenticator::builder(secret, InstalledFlowReturnMethod::HTTPRedirect)
    .persist_tokens_to_disk("./res/tokencache.json")
    .build()
    .await
    .unwrap();

    /* Add here the scopes you wan to use */
    let scopes = &["https://www.googleapis.com/auth/calendar.readonly"];

    /* token(<scopes>) is the one important function of this crate; it does everything to
    obtain a token that can be sent e.g. as Bearer token. */
    match auth.token(scopes).await {
        Ok(token) => println!("\nThe token is {:?}\n", token),
        Err(e) => println!("\nError: {:?}\n", e),
    }
}
