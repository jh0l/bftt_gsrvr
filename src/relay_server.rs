use actix::prelude::*;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use rand::prelude::ThreadRng;
use serde::Deserialize;
use serde_json::from_slice;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::game::Player;
use crate::{common::Identity, game::Game};

/// server sends this message to session
#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct Message(pub String);

#[derive(Debug, Clone)]
pub struct User {
    pub user_id: String,
    pub password: String,
}

#[derive(Clone, Debug)]
pub enum Success {
    Exists,
    New,
}

#[derive(Clone, Debug)]
pub enum Fail {
    Password,
}

#[derive(Clone, Debug)]
pub enum ConnectResult {
    Success(Success),
    Fail(Fail),
}

/// New client session with relay server is created
#[derive(Clone, Debug)]
pub struct Connect {
    pub user: User,
    pub addr: Option<Recipient<Message>>,
}
impl actix::Message for Connect {
    type Result = ConnectResult;
}

/// Session is disconnected
#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub user_id: String,
}

/// Host a game, if already exists throw error
#[derive(Message, Clone, Debug)]
#[rtype(result = "Result<(), String>")]
pub struct HostGame {
    pub host_user_id: String,
    pub game_id: String,
}

/// Join game, if non-existant throw error
#[derive(Message, Clone, Debug)]
#[rtype(result = "Result<(), String>")]
pub struct JoinGame {
    /// user id of joiner
    pub user_id: String,
    pub game_id: String,
}

/// Start game, if non-existant throw error
#[derive(Message, Clone, Debug)]
#[rtype(result = "Result<(), String>")]
pub struct StartGame {
    /// user id of joiner
    pub user_id: String,
    pub game_id: String,
}

#[derive(Message, Clone, Debug)]
#[rtype(result = "Result<(), String>")]
pub struct Replenish {
    pub game_id: String,
}

/// `RelayServer` manages users and games
/// relays user requests to games
/// relays game events to users
pub struct RelayServer {
    users: HashMap<String, User>,
    sessions: HashMap<String, Recipient<Message>>,
    games: HashMap<String, Game>,
    rng: ThreadRng,
}

fn do_send_log(addr: &actix::Recipient<Message>, message: &str) {
    if let Err(err) = addr.do_send(Message(message.to_owned())) {
        println!("[srv/m] do_send error: {:?}", err)
    }
}

pub fn send_all(
    msg: String,
    keys: std::collections::hash_map::Keys<'_, std::string::String, Player>,
    sessions: &HashMap<String, Recipient<Message>>,
) {
    for k in keys {
        if let Some(session) = sessions.get(k) {
            let json = msg.clone();
            do_send_log(session, &json);
        }
    }
}

/// Make actor from `RelaySever`
impl Actor for RelayServer {
    // Simple context
    type Context = Context<Self>;
}

impl RelayServer {
    pub fn new() -> RelayServer {
        RelayServer {
            users: HashMap::new(),
            sessions: HashMap::new(),
            games: HashMap::new(),
            rng: rand::thread_rng(),
        }
    }
}

/// Checks if user exists, if so success if passwords match else fails
/// replaces current session address
/// Creates new user if none exists, setting password and session address
impl Handler<Connect> for RelayServer {
    type Result = MessageResult<Connect>;
    #[allow(unused_variables)]
    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) -> Self::Result {
        dbg!(msg.clone());
        let User { user_id, password } = msg.user.clone();
        let msg_addr = msg.addr;
        let res;
        match self.users.get(&user_id) {
            Some(existant) => {
                if existant.password == password {
                    res = ConnectResult::Success(Success::Exists);
                } else {
                    res = ConnectResult::Fail(Fail::Password);
                }
            }
            None => {
                self.users.insert(user_id.clone(), msg.user);
                res = ConnectResult::Success(Success::New);
            }
        }
        if matches!(res, ConnectResult::Success(_)) {
            if let Some(addr) = msg_addr {
                let res = self.sessions.insert(user_id.clone(), addr.clone());
                if let Some(res_addr) = res {
                    do_send_log(&addr, &MsgResult::logout());
                };
            }
        }
        dbg!(res.clone());
        return MessageResult(res);
    }
}

impl Handler<Disconnect> for RelayServer {
    type Result = ();
    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        let res = self.sessions.remove(&msg.user_id);
        if res.is_some() {
            dbg!("disconnected {:?}", msg);
        } else {
            dbg!("unknown {:?}", msg);
        }
    }
}

/// assumes host user exists and session for user exists
/// check game_id is unique
/// create game and add user as host and player
impl Handler<HostGame> for RelayServer {
    type Result = MessageResult<HostGame>;
    #[allow(unused_assignments)]
    fn handle(&mut self, msg: HostGame, _: &mut Context<Self>) -> Self::Result {
        let HostGame {
            game_id,
            host_user_id,
        } = msg;
        let mut res_game = None;
        if let Some(game) = self.games.get(&game_id) {
            // return game if user is already host of the game
            if game.host_user_id == Some(host_user_id.clone()) {
                res_game = Some(game.clone());
            } else {
                return MessageResult(Err(format!("{} exists", game_id).to_owned()));
            }
        }
        if res_game.is_none() {
            let game = Game::new(game_id.clone(), 18).set_host(host_user_id.clone());
            self.games.insert(game_id.clone(), game.clone());
            res_game = Some(game);
        }
        let res = MsgResult::host_game(&res_game.unwrap())
            .map(|msg_result| {
                if let Some(session) = self.sessions.get(&host_user_id) {
                    do_send_log(session, &msg_result);
                }
            })
            .map_err(|e| format!("{:?}", e).to_owned());
        MessageResult(res)
    }
}
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
enum MoveType {
    Attack,
    Move,
    Give,
    Hover,
}
#[derive(Debug, Clone, Serialize)]
struct PlayerMove {
    user_id: String,
    x: u64,
    y: u64,
    action: MoveType,
}

impl Handler<JoinGame> for RelayServer {
    type Result = MessageResult<JoinGame>;
    fn handle(&mut self, msg: JoinGame, _: &mut Context<Self>) -> Self::Result {
        let JoinGame { game_id, user_id } = msg;
        let sessions = &self.sessions;
        let res = self
            .games
            .get_mut(&game_id)
            .ok_or("game not found".to_owned())
            .and_then(|game| game.insert_player(user_id.clone()).map(|_| game))
            .and_then(|game| match serde_json::to_string(&game) {
                Ok(s) => Ok((game, s)),
                Err(e) => Err(format!("{:?}", e).to_owned()),
            })
            .and_then(|(game, game_json)| {
                for k in &mut game.players.keys() {
                    if let Some(session) = sessions.get(k) {
                        let json = game_json.clone();
                        if k != &user_id {
                            do_send_log(session, &MsgResult::joined(json));
                        } else {
                            do_send_log(session, &MsgResult::join_game(json));
                        }
                    }
                }
                Ok(())
            });
        MessageResult(res)
    }
}

impl Handler<StartGame> for RelayServer {
    type Result = MessageResult<StartGame>;
    fn handle(&mut self, msg: StartGame, ctx: &mut Context<Self>) -> Self::Result {
        let StartGame { game_id, user_id } = msg;
        let sessions = &self.sessions;
        let rng = &mut self.rng;
        let res = self
            .games
            .get_mut(&game_id)
            .ok_or("Game not found".to_owned())
            .and_then(|game| {
                if game.host_user_id != Some(user_id.clone()) {
                    return Err("Only host can start game".to_owned());
                }
                game.start_game(rng)
                    .map(|_| (MsgResult::start_game(&game), game))
            })
            .and_then(|(msg_result, game)| {
                let json = msg_result?;
                send_all(json, game.players.keys(), sessions);
                ctx.notify_later(
                    Replenish {
                        game_id: game.game_id.clone(),
                    },
                    Duration::from_secs(game.turn_time_secs),
                );
                Ok(())
            });
        MessageResult(res)
    }
}

impl Handler<Replenish> for RelayServer {
    type Result = MessageResult<Replenish>;
    fn handle(&mut self, msg: Replenish, ctx: &mut Context<Self>) -> Self::Result {
        let Replenish { game_id } = msg;
        let sessions = &self.sessions;
        let res = self
            .games
            .get_mut(&game_id)
            .ok_or("Game not found".to_owned())
            .and_then(|game| {
                game.replenish()
                    .map(|_| (MsgResult::replenish(&game), game))
            })
            .and_then(|(json_res, game)| {
                let json = json_res?;
                send_all(json, game.players.keys(), sessions);
                ctx.notify_later(
                    Replenish {
                        game_id: game.game_id.clone(),
                    },
                    Duration::from_secs(game.turn_time_secs as u64),
                );
                Ok(())
            });
        MessageResult(res)
    }
}

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
    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
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
        ctx: &mut ws::WebsocketContext<Self>,
    ) -> Result<M, ()> {
        match msg {
            Ok(m) => Ok(m),
            Err(e) => {
                ctx.text("/error mailbox error");
                dbg!(e);
                Err(())
            }
        }
    }

    fn relay_connect(
        &mut self,
        id: Identity,
        ctx: &mut ws::WebsocketContext<Self>,
    ) -> Result<(), String> {
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
                            ctx.text("/error FailPassword");
                        }
                        ConnectResult::Success(s) => {
                            act.user_id = Some(user_id);
                            match s {
                                Success::Exists => ctx.text("/login Exists"),
                                Success::New => ctx.text("/login New"),
                            }
                        }
                    }
                }
                fut::ready(())
            })
            .wait(ctx);
        Ok(())
    }

    fn host_game(&self, game_id: &str, ctx: &mut ws::WebsocketContext<Self>) -> Result<(), String> {
        if self.user_id.is_none() {
            return Err("user not logged in".to_owned());
        }
        self.server_addr
            .send(HostGame {
                game_id: game_id.to_owned(),
                host_user_id: self.user_id.clone().unwrap(),
            })
            .into_actor(self)
            .then(|res, act, ctx| {
                if let Ok(res) = act.mailbox_check(res, ctx) {
                    if let Err(msg) = res {
                        ctx.text(format!("/error {:?}", msg));
                    }
                }
                fut::ready(())
            })
            .wait(ctx);
        Ok(())
    }

    fn join_game(&self, game_id: &str, ctx: &mut ws::WebsocketContext<Self>) -> Result<(), String> {
        if self.user_id.is_none() {
            return Err("user not logged in".to_owned());
        }
        self.server_addr
            .send(JoinGame {
                game_id: game_id.to_owned(),
                user_id: self.user_id.clone().unwrap(),
            })
            .into_actor(self)
            .then(|res, act, ctx| {
                if let Ok(res) = act.mailbox_check(res, ctx) {
                    if let Err(msg) = res {
                        ctx.text(format!("/error {:?}", msg));
                    }
                }
                fut::ready(())
            })
            .wait(ctx);
        Ok(())
    }

    fn start_game(
        &self,
        game_id: &str,
        ctx: &mut ws::WebsocketContext<Self>,
    ) -> Result<(), String> {
        if self.user_id.is_none() {
            return Err("user not logged in".to_owned());
        }
        self.server_addr
            .send(StartGame {
                game_id: game_id.to_owned(),
                user_id: self.user_id.clone().unwrap(),
            })
            .into_actor(self)
            .then(|res, act, ctx| {
                if let Ok(res) = act.mailbox_check(res, ctx) {
                    if let Err(msg) = res {
                        ctx.text(format!("/error {:?}", msg));
                    }
                }
                fut::ready(())
            })
            .wait(ctx);
        Ok(())
    }

    fn parse_message(
        &mut self,
        text: &str,
        ctx: &mut ws::WebsocketContext<Self>,
    ) -> Result<(), String> {
        let m = text.trim();
        let v: Vec<&str> = m.splitn(2, ' ').collect();
        if v.len() < 2 {
            return Err("empty request".to_owned());
        }
        let cmd = v[0];
        let msg = v[1];
        match cmd {
            "/login" => self.relay_connect(from_json::<Identity>(&msg)?, ctx),
            "/host_game" => self.host_game(&msg, ctx),
            "/join_game" => self.join_game(&msg, ctx),
            "/start_game" => self.start_game(&msg, ctx),
            _ => Err(format!("unknown command type {:?}", m).to_owned()),
        }
    }
}

impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

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
                    ctx.text(&format!("/error {:?}", err));
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

struct MsgResult;

impl MsgResult {
    fn json_string<V>(value: &V, cmd: &str) -> Result<String, String>
    where
        V: Serialize,
    {
        serde_json::to_string(value)
            .and_then(|json| Ok(format!("{} {}", cmd, json)))
            .or_else(|err| Err(format!("{:?}", err)))
    }

    pub fn logout() -> String {
        "/logout new login elsewhere".to_string()
    }

    pub fn host_game(game: &Game) -> Result<String, String> {
        MsgResult::json_string(game, "/host_game_success")
    }

    pub fn join_game(json: String) -> String {
        format!("/join_game_success {}", json).to_string()
    }

    pub fn joined(json: String) -> String {
        format!("/player_joined {}", json).to_string()
    }

    pub fn start_game(game: &Game) -> Result<String, String> {
        MsgResult::json_string(game, "/start_game")
    }

    pub fn replenish(game: &Game) -> Result<String, String> {
        MsgResult::json_string(game, "/replenish")
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
