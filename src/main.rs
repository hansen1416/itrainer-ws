use actix_web::{
    middleware,
    http,
    web,
    App,
    Error,
    HttpRequest,
    HttpResponse,
    HttpServer,
    Responder,
};
use actix_web_actors::ws;
use actix_cors::Cors;
use serde_json::json;
use serde::Serialize;

mod server;
use self::server::MyWebSocket;

// define menu structure
#[derive(Serialize)]
struct Menu {
    name: String,
    children: Vec<Workout>,
}

#[derive(Serialize)]
struct Workout {
    name: String,
    id: String,
}

async fn menu() -> impl Responder {
    // Sample data
    let menu_data = vec![
        Menu {
            name: "yoga".to_string(),
            children: vec![
                Workout { name: "stretch and relax".to_string(), id: "3245gdf".to_string() },
                Workout { name: "beginner".to_string(), id: "fadsfa".to_string() }
            ],
        },
        Menu {
            name: "HIIT".to_string(),
            children: vec![
                Workout { name: "stretch and relax".to_string(), id: "15hfg".to_string() },
                Workout { name: "beginner".to_string(), id: "q3t2fds".to_string() }
            ],
        }
    ];

    HttpResponse::Ok().json(json!(menu_data))
}

async fn index() -> impl Responder {
    HttpResponse::Ok().json(
        json!({
        "message": "Hello from the server!",
        "data": {
            "key1": "value1",
            "key2": 42
        }
    })
    )
}

/// WebSocket handshake and start `MyWebSocket` actor.
async fn echo_ws(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    ws::start(MyWebSocket::new(), &req, stream)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    log::info!("starting HTTP server at http://localhost:3333");

    HttpServer::new(|| {
        let cors = Cors::default()
            .allowed_origin("http://localhost:5173") // Allow specific origin
            .allowed_methods(vec!["GET", "POST", "OPTION"]) // Allow specific methods
            .allowed_headers(
                vec![http::header::AUTHORIZATION, http::header::ACCEPT, http::header::CONTENT_TYPE]
            )
            .max_age(3600); // Cache preflight response for 3600 seconds
        // Preflight Request: When a browser makes a non-simple cross-origin request
        // it first sends a preflight request (an OPTIONS request) to the server
        // to check if the actual request is allowed.
        // Max-Age: The max_age directive in a CORS response indicates
        // how long the browser can cache the results of a preflight request.
        // This means the browser won't send another preflight request for subsequent requests
        // of the same type within that time frame, improving performance.

        App::new()
            .wrap(cors)
            // http routes
            .service(web::resource("/").to(index))
            .service(web::resource("/menu").to(menu))
            // websocket route
            .service(web::resource("/ws").route(web::get().to(echo_ws)))
            // enable logger
            .wrap(middleware::Logger::default())
    })
        .workers(2)
        .bind(("127.0.0.1", 3333))?
        .run().await
}
