//! Example of login and logout using redis-based sessions
//!
//! Every request gets a session, corresponding to a cache entry and cookie.
//! At login, the session key changes and session state in cache re-assigns.
//! At logout, session state in cache is removed and cookie is invalidated.
//!

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

mod game;
mod relay_server;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct IndexResponse {
    user_id: Option<String>,
    msg: String,
}

async fn index(session: Session) -> Result<HttpResponse> {
    let user_id: Option<String> = session.get::<String>("user_id").unwrap();

    Ok(HttpResponse::Ok().json(IndexResponse {
        user_id,
        msg: "".to_owned(),
    }))
}

#[derive(Deserialize)]
struct Identity {
    user_id: String,
    password: String,
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
                password,
            },
            addr: None,
        })
        .await
        .expect("login contact with relay failed");
    match res {
        relay_server::ConnectResult::FailPassword => {
            Ok(HttpResponse::Unauthorized().json(IndexResponse {
                user_id: Some(user_id),
                msg: "pasword does not match saved".to_owned(),
            }))
        }
        relay_server::ConnectResult::SuccExists => {
            session.set("user_id", &user_id)?;
            session.renew();
            Ok(HttpResponse::Ok().json(IndexResponse {
                user_id: Some(user_id),
                msg: "exists".to_owned(),
            }))
        }
        relay_server::ConnectResult::SuccNew => {
            session.set("user_id", &user_id)?;
            session.renew();
            Ok(HttpResponse::Ok().json(IndexResponse {
                user_id: Some(user_id),
                msg: "new".to_owned(),
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
            .wrap(RedisSession::new("127.0.0.1:6379", &private_key))
            // enable logger - always register actix-web Logger middleware last
            .wrap(middleware::Logger::default())
            .data(relay.clone())
            .service(resource("/").route(get().to(index)))
            .service(resource("/login").route(post().to(login)))
            .service(resource("/logout").route(post().to(logout)))
        // .configure(services::config)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
