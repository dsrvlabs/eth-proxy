use actix_web::{web, App, HttpResponse, HttpServer, Responder, HttpRequest};
use reqwest::Client;


// TODOs
// Periodic health check
// Maintain liveness
// Round robin

const servers: [&str; 3] = [
    "http://34.22.74.19:8547",
    "http://35.189.142.38:8547",
    "http://34.87.97.131:8547",
];

async fn proxy(req: HttpRequest, body: web::Bytes) -> impl Responder {
    let client = Client::new();
    let url = format!("{}{}", servers[0], req.uri());

    println!("headers: {:?}", req.headers());
    println!("url: {:?}", url);
    println!("method: {:?}", req.method());
    println!("body: {:?}", body);

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

    let mut request = client.request(method, url);
    for (key, value) in req.headers().iter() {
        request = request.header(key.as_str(), value.as_bytes());
    }

    let response = request.body(body).send().await;

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
            eprintln!("Error forwarding request: {}", e);
            return HttpResponse::InternalServerError().finish();
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .route("/{tail:.*}", web::route().to(proxy))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}