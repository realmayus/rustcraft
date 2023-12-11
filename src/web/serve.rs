use std::net::SocketAddr;
use std::sync::Arc;

use axum::{http::StatusCode, Json, response::IntoResponse, Router, routing::get};
use axum::extract::State;
use axum::http::{HeaderValue, Method};
use log::info;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tower_http::cors::CorsLayer;

use rustcraft_lib::web::dto::Player;

use crate::serve::ConnectionActorHandle;
use crate::serve::ConnectionActorMessage;
use crate::web::PORT;

pub(crate) async fn init(connections: Arc<tokio::sync::RwLock<Vec<ConnectionActorHandle>>>) {
    let cors = CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods([Method::GET, Method::PUT])
        // allow requests from any origin
        .allow_origin("http://127.0.0.1:8000".parse::<HeaderValue>().unwrap());

    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/players", get(players))
        // .route("/chat", put(send_chat_message))
        .with_state(connections)
        .layer(cors);

    info!("Starting up web server on port {PORT}...");
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], PORT)))
        .await
        .unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

async fn players(State(connections): State<Arc<tokio::sync::RwLock<Vec<ConnectionActorHandle>>>>) -> impl IntoResponse {
    let mut players: Vec<Player> = vec![];
    let connections = connections.read().await;
    for connection in connections.iter() {
        let (sender, receiver) = oneshot::channel();
        connection.send(ConnectionActorMessage::PlayerInfo(sender)).await;
        let player = receiver.await.unwrap();
        players.push(player);
    }
    (StatusCode::OK, Json(players))
}

#[derive(serde::Deserialize)]
struct SendChatQuery {
    text: String,
}

// async fn send_chat_message(
//     State(connections): State<Arc<ConnectionManager>>,
//     query: Query<SendChatQuery>,
// ) -> impl IntoResponse {
//     println!("Sending chat message: {}", query.text);
//     for connection in connections.iter() {
//         connections
//             .send(
//                 connection.key(),
//                 ClientPackets::DisguisedChatMessage(DisguisedChatMessage::new(
//                     Chat::new_text(query.text.clone()),
//                     0.into(),
//                     Chat::new_text("Server".into()),
//                     false,
//                     None,
//                 )),
//             )
//             .await
//             .unwrap();
//     }
//     (StatusCode::OK, Json(()))
// }
