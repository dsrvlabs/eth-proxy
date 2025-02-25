use std::time::{Instant};
use std::sync::atomic::{AtomicUsize, Ordering};
use log::debug;

pub struct Endpoint {
    pub url: String,
    pub latency: u64,
    pub alive: bool,
}

impl Endpoint {
    // TODO: Consensus layer health check

    pub async fn execution_layer_health_check(&mut self) {
        let client = reqwest::Client::new();

        let body = r#"{"jsonrpc":"2.0","method":"eth_syncing","params":[],"id":1}"#;

        let start = Instant::now();
        let response = client.post(&self.url).body(body).send().await;
        let duration = start.elapsed();

        match response {
            Ok(response) => {
                let body = response.text().await.unwrap_or_default();
                let json: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
                let syncing = json["result"]["syncing"].as_bool().unwrap_or(true);
                debug!("syncing: {} {:?} {:?}", self.url, syncing, duration);
                self.alive = !syncing;
            }
            _ => {
                debug!("syncing: {} false {:?}", self.url, duration);
                self.alive = false;
            }
        }
    }
}

pub trait EndpointChooseStrategy: Send + Sync {
    fn get_endpoint(&mut self) -> Option<&Endpoint>;
}

pub struct RoundRobinStrategy {
    endpoints: Vec<Endpoint>,
    current_index: AtomicUsize,
}

impl RoundRobinStrategy {
    pub fn new(endpoints: Vec<Endpoint>) -> Self {
        Self { endpoints, current_index: AtomicUsize::new(0) }
    }
}

impl EndpointChooseStrategy for RoundRobinStrategy {
    fn get_endpoint(&mut self) -> Option<&Endpoint> {
        for _ in 0..self.endpoints.len() {
            let index = self.current_index.fetch_add(1, Ordering::Relaxed);
            let endpoint = self.endpoints.get(index % self.endpoints.len());
            if endpoint.unwrap().alive {
                return Some(endpoint.unwrap());
            }
        }

        None
    }
}