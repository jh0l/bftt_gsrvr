use crate::{
    common::{Identity, MsgResult},
    relay_server::{
        Connect, ConnectResult, Disconnect, HostGame, JoinGame, Message, RelayServer, StartGame,
        User, VerifySession,
    },
};
use actix::prelude::*;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use serde::Deserialize;
use serde_json::from_slice;
use std::time::{Duration, Instant};
use ws::WebsocketContext as WSctx;

/// How often heartbeat pings are sent
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
/// How long before lack of client response causes a timeout
pub const CLIENT_TIMEOUT: Duration = Duration::from_secs(60);

pub struct WsSession {
    /// hb increment
    hb: Instant,
    /// relay server
    server_addr: Addr<RelayServer>,
    user_id: Option<String>,
}

fn from_json<'a, T>(des: &'a str) -> Result<T, String>
where
    T: Deserialize<'a>,
{
    from_slice::<T>(des.as_bytes()).map_err(|err| (format!("{:?}", err)))
}

impl WsSession {
    // helper method that sends intermittent ping to client
    // also checks ws client heartbeat and terminates session on timeout
    fn hb(&self, ctx: &mut WSctx<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // check client hearbeats
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                // heartbeat timed out
                dbg!("[srv/s] {:?} TIMED OUT, DISCONNECTING", &act.user_id);

                // stop actor
                ctx.stop();

                // do not ping
                return;
            };
            ctx.ping(b"");
        });
    }

    fn mailbox_check<M>(
        &mut self,
        msg: Result<M, MailboxError>,
        ctx: &mut WSctx<Self>,
    ) -> Result<M, ()> {
        match msg {
            Ok(m) => Ok(m),
            Err(e) => {
                ctx.text(MsgResult::error("mailbox error"));
                dbg!(e);
                Err(())
            }
        }
    }

    fn relay_connect(&mut self, msg: String, ctx: &mut WSctx<Self>) -> Result<(), String> {
        let id = from_json::<Identity>(&msg)?;
        let addr = ctx.address().recipient();
        let user_id = id.user_id.clone();
        self.server_addr
            .send(Connect {
                addr: Some(addr),
                user: User {
                    user_id: id.user_id,
                    password: id.password,
                },
            })
            .into_actor(self)
            .then(|res, act, ctx| {
                let res = act.mailbox_check(res, ctx);
                if let Ok(res) = res {
                    match res {
                        ConnectResult::Fail(_) => {
                            ctx.text(MsgResult::error("FailPassword"));
                        }
                        ConnectResult::Success(s) => {
                            act.user_id = Some(user_id);
                            ctx.text(MsgResult::login(s.to_string()));
                        }
                    }
                }
                fut::ready(())
            })
            .wait(ctx);
        Ok(())
    }

    fn verify_session(&mut self, ctx: &mut WSctx<Self>) -> Result<(), String> {
        self.server_addr
            .send(VerifySession {
                user_id: self.user_id.clone(),
                addr: ctx.address().recipient(),
            })
            .into_actor(self)
            .then(|_, _, _| fut::ready(()))
            .wait(ctx);
        Ok(())
    }

    fn clone_user_id(&self) -> Result<String, String> {
        self.user_id
            .clone()
            .ok_or_else(|| "user not logged in".to_string())
    }

    fn host_game(&self, game_id: String, ctx: &mut WSctx<Self>) -> Result<(), String> {
        let host_user_id = self.clone_user_id()?;
        self.server_addr
            .send(HostGame {
                game_id,
                host_user_id,
            })
            .into_actor(self)
            .then(|res, act, ctx| {
                if let Ok(res) = act.mailbox_check(res, ctx) {
                    if let Err(msg) = res {
                        ctx.text(MsgResult::error(msg.as_str()));
                    }
                }
                fut::ready(())
            })
            .wait(ctx);
        Ok(())
    }

    fn join_game(&self, game_id: String, ctx: &mut WSctx<Self>) -> Result<(), String> {
        let user_id = self.clone_user_id()?;
        self.server_addr
            .send(JoinGame { game_id, user_id })
            .into_actor(self)
            .then(|res, act, ctx| {
                if let Ok(res) = act.mailbox_check(res, ctx) {
                    if let Err(msg) = res {
                        ctx.text(MsgResult::error(msg.as_str()));
                    }
                }
                fut::ready(())
            })
            .wait(ctx);
        Ok(())
    }

    fn start_game(&self, game_id: String, ctx: &mut WSctx<Self>) -> Result<(), String> {
        let user_id = self.clone_user_id()?;

        self.server_addr
            .send(StartGame { game_id, user_id })
            .into_actor(self)
            .then(|res, act, ctx| {
                if let Ok(res) = act.mailbox_check(res, ctx) {
                    if let Err(msg) = res {
                        ctx.text(MsgResult::error(msg.as_str()));
                    }
                }
                fut::ready(())
            })
            .wait(ctx);
        Ok(())
    }

    fn parse_message(&mut self, text: &str, ctx: &mut WSctx<Self>) -> Result<(), String> {
        let m = text.trim();
        let v: Vec<&str> = m.splitn(2, ' ').collect();
        let cmd = v.get(0).ok_or_else(|| "invalid command")?;
        let mut msg = String::new();
        if v.len() == 2 {
            msg = v[1].clone().to_string();
        }
        match *cmd {
            "/login" => self.relay_connect(msg, ctx),
            "/verify" => self.verify_session(ctx),
            "/host_game" => self.host_game(msg, ctx),
            "/join_game" => self.join_game(msg, ctx),
            "/start_game" => self.start_game(msg, ctx),
            _ => Err(format!("unknown command type {:?}", m).to_owned()),
        }
    }
}

impl Actor for WsSession {
    type Context = WSctx<Self>;

    // Method is called on actor start
    // register ws session with RelayServer
    fn started(&mut self, ctx: &mut Self::Context) {
        // start heartbeat with ws client
        self.hb(ctx);
    }

    fn stopping(&mut self, _: &mut Self::Context) -> Running {
        println!("[srv/s] {:?} WS SESSION STOPPING", self.user_id);
        // notify relay server
        if let Some(user_id) = self.user_id.clone() {
            self.server_addr.do_send(Disconnect { user_id });
        }
        Running::Stop
    }
}

/// Handle messages from relay server, we simply send it to peer websocket
impl Handler<Message> for WsSession {
    type Result = ();

    fn handle(&mut self, msg: Message, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

// Handles messages from Websocket client, forwarding text to helper method
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        let msg = match msg {
            Err(err) => {
                println!("[srv/s] RECEIVED ERROR FROM WS CLIENT {:?}", err);
                ctx.stop();
                return;
            }
            Ok(msg) => msg,
        };

        match msg {
            ws::Message::Ping(msg) => {
                self.hb = Instant::now();
                ctx.ping(&msg);
            }
            ws::Message::Pong(_) => self.hb = Instant::now(),
            ws::Message::Text(text) => {
                self.parse_message(&text, ctx).unwrap_or_else(|err| {
                    ctx.text(MsgResult::error(err.as_str()));
                });
            }
            ws::Message::Binary(_) => println!("[srv/s] Unexpected binary"),
            ws::Message::Close(reason) => {
                ctx.close(reason);
                ctx.stop();
            }
            ws::Message::Continuation(_) => ctx.stop(),
            ws::Message::Nop => (),
        }
    }
}

pub async fn ws_route(
    req: HttpRequest,
    stream: web::Payload,
    srv: web::Data<Addr<RelayServer>>,
) -> Result<HttpResponse, Error> {
    ws::start(
        WsSession {
            hb: Instant::now(),
            server_addr: srv.get_ref().clone(),
            user_id: None,
        },
        &req,
        stream,
    )
}
