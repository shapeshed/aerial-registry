use reqwest::Client;

const USER_AGENT: &str = concat!(
    "aerial-registry/",
    env!("CARGO_PKG_VERSION"),
    " (aerial-registry)"
);

pub fn build_client() -> reqwest::Result<Client> {
    Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(15))
        .build()
}

/// Client for model inference. Local LLM generations regularly take minutes,
/// far beyond the 15s pipeline timeout that suits stream probing, so only
/// connecting is bounded tightly.
pub fn build_ai_client() -> reqwest::Result<Client> {
    Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(600))
        .build()
}
