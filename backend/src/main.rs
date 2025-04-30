use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{Json, Router, http::StatusCode, routing::post};
use renet_netcode::ConnectToken;
use serde::Deserialize;

#[tokio::main]
async fn main() {
    let app = Router::new().route("/login", post(login));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    axum::serve(listener, app).await.unwrap();
}

async fn login(Json(payload): Json<Login>) -> (StatusCode, Vec<u8>) {
    let client_id = match payload.user.as_str() {
        "test" => 0,
        "test1" => 1,
        _ => return (StatusCode::UNAUTHORIZED, vec![]),
    };

    if payload.pass == "test" {
        let token = ConnectToken::generate(
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap(),
            0,
            30 * 60,
            client_id,
            30 * 60,
            vec!["127.0.0.1:6969".parse().unwrap()],
            None,
            &get_or_gen_key("manager.key").await.unwrap(),
        )
        .unwrap();

        let mut buffer = Vec::new();

        token.write(&mut buffer).unwrap();

        (StatusCode::OK, buffer)
    } else {
        (StatusCode::UNAUTHORIZED, vec![])
    }
}

#[derive(Deserialize)]
struct Login {
    user: String,
    pass: String,
}

async fn get_or_gen_key<P: AsRef<Path>>(path: P) -> std::io::Result<[u8; 32]> {
    let path = path.as_ref();

    match tokio::fs::read(path).await {
        Ok(content) => Ok(content.try_into().unwrap()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let key = renet_netcode::generate_random_bytes::<32>();

            tokio::fs::write(path, &key).await?;

            Ok(key)
        }
        Err(e) => Err(e),
    }
}
