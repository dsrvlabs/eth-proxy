use std::time::{Instant};
use std::sync::atomic::{AtomicUsize, Ordering};
use log::debug;


pub enum HealthCheckEnum {
    Geth(GethHealthCheck),
    OpNode(OpNodeHealthCheck),
    Basic(BasicHealthCheck),
}

impl HealthCheck for HealthCheckEnum {
    async fn health_check(&self, url: &str) -> bool {
        match self {
            HealthCheckEnum::Geth(health_check) => health_check.health_check(url).await,
            HealthCheckEnum::OpNode(health_check) => health_check.health_check(url).await,
            HealthCheckEnum::Basic(health_check) => health_check.health_check(url).await,
        }
    }
}
pub trait HealthCheck: Send + Sync {
    async fn health_check(&self, url: &str) -> bool;
}

pub struct BasicHealthCheck {
}

impl HealthCheck for BasicHealthCheck {
    async fn health_check(&self, url: &str) -> bool {
        let client = reqwest::Client::new();
        let response = client.get(format!("{}/health", url)).send().await;
        match response {
            Ok(response) => response.status().is_success(),
            _ => false,
        }
    }
}

pub struct OpNodeHealthCheck {
}

impl HealthCheck for OpNodeHealthCheck {
    async fn health_check(&self, url: &str) -> bool {
        true
    }
}

pub struct GethHealthCheck {
}

impl HealthCheck for GethHealthCheck {
    async fn health_check(&self, url: &str) -> bool {
        let req_body = r#"{"jsonrpc":"2.0","method":"net_peerCount","params":[],"id":1}"#;
        let client = reqwest::Client::new();

        let response = client.post(url).body(req_body).send().await;
        let response = match response {
            Ok(response) => response,
            _ => return false,
        };

        let resp_body = response.text().await.unwrap_or_default();
        let json: serde_json::Value = serde_json::from_str(&resp_body).unwrap_or_default();
        let peer_count = json["result"].as_str().unwrap_or_default();
        if peer_count.parse::<u64>().unwrap_or(0) <= 0 {
            debug!("peer_count: {} {}", url, peer_count);
            return false;
        }

        let req_body = r#"{"jsonrpc":"2.0","method":"eth_syncing","params":[],"id":1}"#;
        let response = client.post(url).body(req_body).send().await;
        let response = match response {
            Ok(response) => response,
            _ => {
                debug!("syncing: err {}", url);
                return false;
            }
        };

        let json: serde_json::Value = response.json().await.unwrap_or_default();
        let syncing = json["result"]["syncing"].as_bool().unwrap_or(true);

        debug!("syncing: {} {}", url, syncing);
        return !syncing;
    }
}

#[derive(Clone)]
pub struct Endpoint {
    pub url: String,
    pub latency: u64,
    pub alive: bool,
}

pub trait EndpointChooseStrategy: Send + Sync {
    fn get_endpoint(&mut self) -> Option<&Endpoint>;
    fn available_count(&self) -> u32;
}

pub struct RoundRobinStrategy {
    endpoints: Vec<Endpoint>,
    current_index: AtomicUsize,
}

impl RoundRobinStrategy {
    pub fn new(endpoints: Vec<Endpoint>) -> Self {
        Self { endpoints, current_index: AtomicUsize::new(0) }
    }

    pub fn get_endpoints_mut(&mut self) -> &mut Vec<Endpoint> {
        &mut self.endpoints
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
    fn available_count(&self) -> u32 {
        self.endpoints.iter().filter(|e| e.alive).count() as u32
    }
}