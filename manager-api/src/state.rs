use kube::Client;

#[derive(Clone)]
pub struct AppState {
    pub client: Client,
    pub namespace: String,
    pub token: Option<String>,
    pub director_base_url: Option<String>,
    pub http: reqwest::Client,
    pub started_unix_ms: u128,
    pub port: u16,
}
