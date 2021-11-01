use crate::common::Fail;
use crate::common::MsgResult;
use crate::common::Success;
use crate::game::Game;
use crate::game::Player;
use actix::prelude::*;
use rand::prelude::ThreadRng;
use std::collections::HashMap;
use std::time::Duration;

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

/// verify that the sender's session is associated with their user on the relay_server
#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct VerifySession {
    pub user_id: Option<String>,
    pub addr: Recipient<Message>,
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

fn do_send_log(addr: &actix::Recipient<Message>, message: String) {
    if let Err(err) = addr.do_send(Message(message)) {
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
            do_send_log(session, json);
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

        // The HTTP GET:login endpoint uses Connect {} to log in the user
        // There is no socket in that case so msg.addr has to be None
        dbg!(msg.addr.clone());
        if matches!(res, ConnectResult::Success(_)) && msg.addr.is_some() {
            let addr = msg.addr.expect("no address in msg");
            let res = self.sessions.insert(user_id.clone(), addr.clone());
            dbg!(user_id.clone(), addr.clone(), res.clone());
            if let Some(res_addr) = res {
                do_send_log(
                    &res_addr,
                    MsgResult::alert("detected duplicate session, please log back in".to_string()),
                );
                do_send_log(&res_addr, MsgResult::logout());
                do_send_log(
                    &addr,
                    MsgResult::alert("detected duplicate session, please log back in".to_string()),
                );
            };
        }
        dbg!(res.clone());
        return MessageResult(res);
    }
}

impl Handler<VerifySession> for RelayServer {
    type Result = ();
    fn handle(&mut self, msg: VerifySession, _: &mut Context<Self>) {
        let user_id_opt = msg.user_id;
        if let Some(user_id) = user_id_opt {
            if let Some(addr) = self.sessions.get(&user_id) {
                if addr == &msg.addr {
                    return;
                }
            }
        }
        do_send_log(&msg.addr, MsgResult::logout());
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
        let mut new_game = false;
        if res_game.is_none() {
            let mut game = Game::new(game_id.clone(), 18);
            let host_op = game.set_host(host_user_id.clone()).map(|_| ());
            if host_op.is_err() {
                return MessageResult(host_op);
            }
            self.games.insert(game_id.clone(), game.clone());
            res_game = Some(game);
            new_game = true;
        }
        let res = MsgResult::host_game(&res_game.expect("res_game is handled"))
            .map(|msg_result| {
                if let Some(session) = self.sessions.get(&host_user_id) {
                    do_send_log(session, msg_result);
                    if new_game {
                        do_send_log(session, MsgResult::alert("new game created".to_string()));
                    } else {
                        do_send_log(session, MsgResult::alert("rejoined game".to_string()));
                    }
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
                            do_send_log(session, MsgResult::joined(json));
                        } else {
                            do_send_log(session, MsgResult::join_game(json));
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
                    .map(|game_players| (MsgResult::replenish(&game_players), game))
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
