use reqwest::Client;

const USER_AGENT: &str = concat!("aerial-registry/", env!("CARGO_PKG_VERSION"), " (aerial-registry)");

pub fn build_client() -> reqwest::Result<Client> {
    Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(15))
        .build()
}
