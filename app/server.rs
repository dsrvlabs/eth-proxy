use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;
use env_logger;
use log::{info, error};

use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use reqwest::Client;

use eth_proxy::{Endpoint, EndpointChooseStrategy, RoundRobinStrategy};

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

    // TODO: Get from env
    let servers: [&str; 3] = [
        "http://34.22.74.19:8547",
        "http://35.189.142.38:8547",
        "http://34.87.97.131:8547",
    ];

    let endpoints: Vec<Endpoint> = servers.iter().map(|s| Endpoint {
        url: s.to_string(),
        latency: 0,
        alive: false,
    }).collect();

    let mut health_checks = Vec::new();

    for server in servers {
        let handle = tokio::spawn(async {
            loop {
                let mut endpoint = Endpoint {
                    url: server.to_string(),
                    latency: 0,
                    alive: false,
                };

                endpoint.execution_layer_health_check().await;

                // TODO: Periodc
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
        });

        health_checks.push(handle);
    }
    let strategy = Arc::new(Mutex::new(RoundRobinStrategy::new(endpoints)));
    let app_state = AppState { strategy };

    HttpServer::new(move || App::new()
        .app_data(web::Data::new(app_state.clone()))
        .route("/{tail:.*}", web::route().to(proxy)))
        .bind("127.0.0.1:8080")
        .unwrap()
        .run()
        .await?;

    Ok(())
}
