use crate::common::gen_rng_string;
use crate::common::Fail;
use crate::common::MsgResult;
use crate::common::SuccessResult;
use crate::common::UserStatusResult;
use crate::game::ActionType;
use crate::game::Game;
use crate::game::InsertPlayerResult;
use crate::game::Player;
use crate::game::BOARD_SIZE;
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
    Success(SuccessResult),
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
    pub token: String,
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

/// Edit game, if already started, non-existant - throw error

/// Start game, if non-existant throw error
#[derive(Message, Clone, Debug)]
#[rtype(result = "Result<(), String>")]
pub struct StartGame {
    /// user id of joiner
    pub user_id: String,
    pub game_id: String,
}

/// check if user has a game in progress they should know about
#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct UserStatus {
    pub user_id: String,
}

#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct PlayerActionRequest {
    pub user_id: String,
    pub game_id: String,
    pub action: ActionType,
}

#[derive(Message, Clone, Debug)]
#[rtype(result = "Result<(), String>")]
pub struct Replenish {
    pub game_id: String,
}

/// `RelayServer` manages users and games
/// relays user requests to games
/// relays game events to users
/// handles dead sessions and verifying new sessions
pub struct RelayServer {
    /// map of User IDs to corresponding user
    users: HashMap<String, User>,
    /// map of user IDs to the ID of game they're currently in
    user_games: HashMap<String, String>,
    /// map of User IDs to corresponding client session
    sessions: HashMap<String, Recipient<Message>>,
    /// map of User IDs to corresponding session key for session verification
    session_keys: HashMap<String, String>,
    /// map of Game IDs to corresponding game
    games: HashMap<String, Game>,
    /// random number generator
    rng: ThreadRng,
}

fn do_send_log(addr: &actix::Recipient<Message>, message: String) {
    if let Err(err) = addr.do_send(Message(message)) {
        println!("[srv/m] do_send error: {:?}", err)
        // TODO send errors to logging record
    }
}

pub fn send_all(
    msg: &str,
    keys: std::collections::hash_map::Keys<'_, std::string::String, Player>,
    sessions: &HashMap<String, Recipient<Message>>,
) {
    for k in keys {
        if let Some(session) = sessions.get(k) {
            let json = msg.clone();
            do_send_log(session, json.to_string());
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
            user_games: HashMap::new(),
            sessions: HashMap::new(),
            session_keys: HashMap::new(),
            games: HashMap::new(),
            rng: rand::thread_rng(),
        }
    }
    pub fn user_do_send_log(&self, user_id: &str, msg: &str) {
        if let Some(session) = self.sessions.get(user_id) {
            do_send_log(session, msg.to_string());
        }
    }
}

/// Checks if user exists, if so success if passwords match else fails
/// replaces current session address
/// Creates new user if none exists, setting password and session address
/// If Address included, creates a new session key that handles updating sessions
impl Handler<Connect> for RelayServer {
    type Result = MessageResult<Connect>;
    #[allow(unused_variables)]
    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) -> Self::Result {
        dbg!(msg.clone());
        let User { user_id, password } = msg.user.clone();
        let mut res = match self.users.get(&user_id) {
            Some(existant) => {
                if existant.password == password {
                    ConnectResult::Success(SuccessResult {
                        alert: "user exists".to_string(),
                        token: None,
                    })
                } else {
                    ConnectResult::Fail(Fail::Password)
                }
            }
            None => {
                self.users.insert(user_id.clone(), msg.user);
                ConnectResult::Success(SuccessResult {
                    alert: "user created".to_string(),
                    token: None,
                })
            }
        };

        // The HTTP GET:login endpoint uses Connect {} to log in the user
        // There is no socket in that case so msg.addr has to be None
        dbg!(msg.addr.clone());
        if let ConnectResult::Success(ref mut succ_res) = res {
            if msg.addr.is_some() {
                let addr = msg.addr.expect("no address in msg");
                let old_sesh = self.sessions.insert(user_id.clone(), addr.clone());
                if let Some(res_addr) = old_sesh {
                    if res_addr != addr {
                        do_send_log(&res_addr, MsgResult::logout("Connect"));
                    }
                };
                // new session key used for determining newest authorized session of user
                let key = gen_rng_string(4);
                succ_res.token = Some(key.clone());
                self.session_keys.insert(user_id.clone(), key);
            }
        }
        dbg!(res.clone());
        return MessageResult(res);
    }
}

/// session key will determine if a conflicting session verifying will logout
/// or replace an existing session
/// TODO recover messages missed transitioning to new session
/// TODO add timestamp to each message for clients to differentiate resent messages
impl Handler<VerifySession> for RelayServer {
    type Result = ();
    fn handle(&mut self, msg: VerifySession, _: &mut Context<Self>) {
        let user_id_opt = msg.user_id;
        if let Some(user_id) = user_id_opt {
            if let Some(sesh_key) = self.session_keys.get(&user_id) {
                if sesh_key == &msg.token {
                    // user must have user_id and valid session token for session to verify
                    if let Some(addr) = self.sessions.get(&user_id) {
                        if addr == &msg.addr {
                            return;
                        }
                    }
                    do_send_log(&msg.addr, MsgResult::alert("new session"));
                    // if user's session is untracked and session key is verified, replace self.sessions[user_id] with it
                    self.sessions.insert(user_id.clone(), msg.addr.clone());
                    return;
                }
            }
        }
        do_send_log(&msg.addr, MsgResult::logout("VerifySession"));
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
        // return game if user is host of game or err if other user is host
        if let Some(game) = self.games.get(&game_id) {
            if game.host_user_id == Some(host_user_id.clone()) {
                res_game = Some(game.clone());
            } else {
                return MessageResult(Err(format!("{} exists", game_id).to_owned()));
            }
        }
        // ELSE return err if user is already in another game
        else if let Some(game_id) = self.user_games.get(&host_user_id) {
            if let Some(game) = self.games.get(game_id) {
                if game.host_user_id == Some(host_user_id.clone()) {
                    return MessageResult(Err("already in another game".to_string()));
                }
            }
            dbg!("user game outdated", host_user_id.clone(), game_id);
            // TODO user is not actually host of game then ignore user_games?
        }
        let mut new_game = false;
        // create game and set user as host and track in user_games, return err if host op failed
        if res_game.is_none() {
            let mut game = Game::new(game_id.clone(), BOARD_SIZE);
            let host_op = game.set_host(host_user_id.clone()).map(|_| ());
            if host_op.is_err() {
                return MessageResult(host_op);
            }
            self.games.insert(game_id.clone(), game.clone());
            self.user_games.insert(host_user_id.clone(), game_id);
            res_game = Some(game);
            new_game = true;
        }
        // send json response to client (serialization can fail)
        let res = MsgResult::host_game(&res_game.expect("res_game is handled"))
            .map(|msg_result| {
                if let Some(session) = self.sessions.get(&host_user_id) {
                    do_send_log(session, msg_result);
                    if new_game {
                        do_send_log(session, MsgResult::alert("new game created"));
                    } else {
                        do_send_log(session, MsgResult::alert("rejoined game"));
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
    hash: String,
}

impl Handler<JoinGame> for RelayServer {
    type Result = MessageResult<JoinGame>;
    fn handle(&mut self, msg: JoinGame, _: &mut Context<Self>) -> Self::Result {
        let JoinGame { game_id, user_id } = msg;
        // return err if user already in a game
        if let Some(cur_game_id) = self.user_games.get(&user_id) {
            if cur_game_id != &game_id {
                return MessageResult(Err("already in a another game".to_string()));
            }
        }
        let mut insert_player_result = InsertPlayerResult::Joined;
        let user_games = &mut self.user_games;
        let sessions = &self.sessions;
        // get game
        let res = self
            .games
            .get_mut(&game_id)
            .ok_or("game not found".to_owned())
            // insert player into game (may error) and track user_id to game_id
            .and_then(|game| {
                insert_player_result = game.insert_player(user_id.clone())?;
                user_games.insert(user_id.clone(), game_id.clone());
                Ok(game)
            })
            // prepare json of game updated with new player
            .and_then(|game| match serde_json::to_string(&game) {
                Ok(s) => Ok((game, s)),
                Err(e) => Err(format!("{:?}", e).to_owned()),
            })
            // send json to all clients in the game only if user 'Joined'
            .and_then(|(game, game_json)| {
                if matches!(insert_player_result, InsertPlayerResult::Joined) {
                    for k in &mut game.players.keys() {
                        if k != &user_id {
                            if let Some(session) = sessions.get(k) {
                                do_send_log(session, MsgResult::joined(&game_json));
                            }
                        }
                    }
                }
                if let Some(session) = sessions.get(&user_id) {
                    do_send_log(session, MsgResult::join_game(&game_json));
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
                send_all(&json, game.players.keys(), sessions);
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

impl Handler<UserStatus> for RelayServer {
    type Result = ();
    fn handle(&mut self, msg: UserStatus, _: &mut Context<Self>) {
        let user_id = msg.user_id;
        let games = &self.games;
        let res = self
            .user_games
            .get(&user_id)
            .and_then(|game_id| games.get(game_id))
            .and_then(|game| match game.players.contains_key(&user_id) {
                true => Some(UserStatusResult {
                    game_id: Some(game.game_id.clone()),
                }),
                false => None,
            })
            .unwrap_or(UserStatusResult { game_id: None });
        match MsgResult::user_status(&res) {
            Ok(msg) => self.user_do_send_log(&user_id, &msg),
            Err(e) => self.user_do_send_log(&user_id, &MsgResult::error(&e)),
        }
    }
}

impl Handler<PlayerActionRequest> for RelayServer {
    type Result = ();
    fn handle(&mut self, msg: PlayerActionRequest, _: &mut Context<Self>) -> Self::Result {
        let PlayerActionRequest {
            user_id,
            game_id,
            action,
        } = msg;
        let sessions = &self.sessions;
        let games = &mut self.games;
        let res = self
            .user_games
            .get(&user_id)
            .ok_or("user games not found".to_string())
            .and_then(|user_game_id| {
                if user_game_id != &game_id {
                    return Err("user game id invalid".to_string());
                }
                games.get_mut(&game_id).ok_or("game id bad".to_string())
            })
            .and_then(|game| game.player_action(&user_id, action).map(|e| (e, game)))
            // TODO rewind game action upon json serialization error
            .and_then(|(e, game)| MsgResult::player_action(&e).map(|json| (json, game)));
        match res {
            Err(e) => self.user_do_send_log(&user_id, &MsgResult::error(&e)),
            Ok((json, game)) => send_all(&json, game.players.keys(), sessions),
        };
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
                send_all(&json, game.players.keys(), sessions);
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
