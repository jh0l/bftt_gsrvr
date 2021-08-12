use actix::prelude::*;
use actix_cors::Cors;
use actix_http::http::header;
use actix_redis::RedisSession;
use actix_session::Session;
use actix_web::{
    middleware, web,
    web::{get, post, resource},
    App, HttpResponse, HttpServer, Result,
};

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::relay_server::ws_route;

mod common;
mod game;
mod relay_server;

use common::Identity;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct IndexResponse {
    user_id: Option<String>,
    msg: Option<String>,
}

async fn index(session: Session) -> Result<HttpResponse> {
    let user_id: Option<String> = session.get::<String>("user_id").unwrap();
    let msg: Option<String> = session.get::<String>("token").unwrap();
    Ok(HttpResponse::Ok().json(IndexResponse { user_id, msg }))
}

async fn login(
    user: web::Json<Identity>,
    session: Session,
    relay_data: web::Data<Addr<relay_server::RelayServer>>,
) -> Result<HttpResponse> {
    let Identity { user_id, password } = user.into_inner();
    let res = relay_data
        .send(relay_server::Connect {
            user: relay_server::User {
                user_id: user_id.clone(),
                password: password.clone(),
            },
            addr: None,
        })
        .await
        .expect("login contact with relay failed");
    match res {
        relay_server::ConnectResult::Fail(_) => {
            Ok(HttpResponse::Unauthorized().json(IndexResponse {
                user_id: Some(user_id),
                msg: Some("pasword does not match saved".to_owned()),
            }))
        }
        relay_server::ConnectResult::Success(s) => {
            session.set("user_id", &user_id)?;
            session.set("token", &password)?;
            session.renew();
            let msg = Some(
                match s {
                    relay_server::Success::Exists => "Exists",
                    relay_server::Success::New => "New",
                }
                .to_owned(),
            );
            Ok(HttpResponse::Ok().json(IndexResponse {
                user_id: Some(user_id),
                msg,
            }))
        }
    }
}

async fn logout(session: Session) -> Result<HttpResponse> {
    let id: Option<String> = session.get("user_id")?;
    if let Some(x) = id {
        session.purge();
        Ok(format!("Logged out: {}", x).into())
    } else {
        Ok("Could not log out anonymous user".into())
    }
}

fn get_p_key() -> Vec<u8> {
    let key_temp = std::env::var("PRIVATE_KEY");
    if let Ok(key) = key_temp {
        let b = key.into_bytes();
        return b;
    }
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                                abcdefghijklmnopqrstuvwxyz\
                                0123456789)(*&^%$#@!~";
    const PASSWORD_LEN: usize = 64;
    let mut rng = rand::thread_rng();

    let password: String = (0..PASSWORD_LEN)
        .map(|_| {
            let idx = rng.gen_range(0, CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();

    println!("{:?}", password);
    panic!("set PRIVATE_KEY in .env e.g {}", password);
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();
    let private_key = get_p_key();
    std::env::set_var("RUST_LOG", "actix_web=info,actix_redis=info");
    env_logger::init();

    let relay = relay_server::RelayServer::new().start();

    HttpServer::new(move || {
        App::new()
            // redis session middleware
            .wrap(
                Cors::default()
                    .allowed_origin("http://localhost:3000")
                    .allowed_methods(vec!["GET", "POST"])
                    .allowed_headers(vec![
                        header::AUTHORIZATION,
                        header::ACCEPT,
                        header::CONTENT_TYPE,
                    ])
                    .supports_credentials()
                    .max_age(3600),
            )
            .wrap(RedisSession::new("127.0.0.1:6379", &private_key).cookie_http_only(false))
            // enable logger - always register actix-web Logger middleware last
            .wrap(middleware::Logger::default())
            .data(relay.clone())
            .service(resource("/").route(get().to(index)))
            .service(resource("/login").route(post().to(login)))
            .service(resource("/logout").route(post().to(logout)))
            .service(resource("/ws/").to(ws_route))
        // .configure(services::config)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
