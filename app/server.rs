use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;
use env_logger;
use log::{info, error};

use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use reqwest::Client;
use clap::Parser;

use eth_proxy::{Endpoint, EndpointChooseStrategy, RoundRobinStrategy, GethHealthCheck, HealthCheck, OpNodeHealthCheck, HealthCheckEnum, BasicHealthCheck, BeaconHealthCheck};


#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// List of Ethereum execution layer endpoints
    #[arg(long, env = "ETH_ENDPOINTS", value_delimiter = ',')]
    endpoints: Vec<String>,

    /// Server bind address
    #[arg(long, env = "BIND_ADDRESS", default_value = "127.0.0.1:8080")]
    bind_address: String,

    /// Health check interval in seconds
    #[arg(long, env = "HEALTH_CHECK_INTERVAL", default_value = "5")]
    health_check_interval: u64,

    #[arg(long, default_value = "geth", env = "CLIENT_TYPE")]
    client_type: String,
}

async fn health(data: web::Data<AppState>) -> impl Responder {
    let strategy = data.strategy.lock().unwrap();
    let available_count = strategy.available_count();
    if available_count == 0 {
        return HttpResponse::ServiceUnavailable().body("No available endpoints");
    }
    HttpResponse::Ok().body(format!("Available endpoints: {}", available_count))
}

async fn proxy(req: HttpRequest, body: web::Bytes, data: web::Data<AppState>) -> impl Responder {
    let client = Client::new();

    let mut strategy = data.strategy.lock().unwrap();
    let endpoint = strategy.get_endpoint();

    info!("headers: {:?}", req.headers());
    info!("method: {:?}", req.method());
    info!("body: {:?}", body);

    let method = match *req.method() {
        actix_web::http::Method::GET => reqwest::Method::GET,
        actix_web::http::Method::POST => reqwest::Method::POST,
        actix_web::http::Method::PUT => reqwest::Method::PUT,
        actix_web::http::Method::DELETE => reqwest::Method::DELETE,
        actix_web::http::Method::PATCH => reqwest::Method::PATCH,
        actix_web::http::Method::HEAD => reqwest::Method::HEAD,
        actix_web::http::Method::OPTIONS => reqwest::Method::OPTIONS,
        _ => reqwest::Method::GET,
    };

    if endpoint.is_none() {
        return HttpResponse::ServiceUnavailable().body("No available endpoints");
    }

    info!("url: {:?}", endpoint.unwrap().url);

    let mut request = client.request(method, endpoint.unwrap().url.clone());
    for (key, value) in req.headers().iter() {
        request = request.header(key.as_str(), value.as_bytes());
    }

    let start = Instant::now();
    let response = request.body(body).send().await;
    let duration = start.elapsed();

    info!("Request duration: {:?}", duration);

    match response {
        Ok(resp) => {
            let status = resp.status();
            let headers = resp.headers().clone();
            let body = resp.text().await.unwrap_or_default();

            let mut response = HttpResponse::build(
                actix_web::http::StatusCode::from_u16(status.as_u16()).unwrap(),
            );
            for (key, value) in headers.iter() {
                response.insert_header((key.as_str(), value.as_bytes()));
            }
            return response.body(body);
        }
        Err(e) => {
            error!("Error forwarding request: {}", e);
            // TODO: If failed. does this endpoint need to be failed?
            return HttpResponse::InternalServerError().finish();
        }
    }
}

#[derive(Clone)]
struct AppState {
    strategy: Arc<Mutex<dyn EndpointChooseStrategy + Send + Sync>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    info!("Starting server");

    let args = Args::parse();

    if args.endpoints.is_empty() {
        error!("No endpoints provided. Please set via --endpoints or ETH_ENDPOINTS environment variable");
        std::process::exit(1);
    }

    let endpoints: Vec<Endpoint> = args.endpoints.iter().map(|s| Endpoint {
        url: s.to_string(),
        latency: 0,
        alive: true, // Initialize as alive
    }).collect();

    let strategy = Arc::new(Mutex::new(RoundRobinStrategy::new(endpoints)));
    let app_state = AppState { strategy: strategy.clone() };

    let mut health_checks = Vec::new();
    for endpoint_url in args.endpoints.iter() {
        let interval = args.health_check_interval;
        let url = endpoint_url.clone();
        let client_type = args.client_type.clone();
        let strategy = strategy.clone();

        let handle = tokio::spawn(async move {
            loop {
                info!("Health checking: {}", url);
                let health_check = match client_type.as_str() {
                    "opnode" => HealthCheckEnum::OpNode(OpNodeHealthCheck {}),
                    "el" => HealthCheckEnum::Geth(GethHealthCheck {}),
                    "cl" => HealthCheckEnum::Beacon(BeaconHealthCheck {}),
                    _ => HealthCheckEnum::Basic(BasicHealthCheck {}),
                };

                let is_healthy = health_check.health_check(&url).await;
                info!("Health check result for {}: {}", url, is_healthy);

                // Update endpoint status in shared state
                {
                    let mut strategy = strategy.lock().unwrap();
                    if let Some(endpoint) = strategy.get_endpoints_mut().iter_mut().find(|e| e.url == url) {
                        endpoint.alive = is_healthy;
                    }
                } // Lock is dropped here

                tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
            }
        });

        health_checks.push(handle);
    }

    info!("Starting server on {}", args.bind_address);
    HttpServer::new(move || App::new()
        .app_data(web::Data::new(app_state.clone()))
        .route("/healthz", web::route().to(health))
        .route("/{tail:.*}", web::route().to(proxy)))
        .bind(&args.bind_address)
        .unwrap()
        .run()
        .await?;

    Ok(())
}
