use crate::common::gen_rng_string;
use crate::common::ActionPointUpdate;
use crate::common::ConfigGameOp;
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
use serde::Deserialize;
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
#[derive(Message, Debug, Clone, Deserialize)]
#[rtype(result = "()")]
pub struct ConfigGame {
    pub game_id: String,
    pub user_id: String,
    pub op: ConfigGameOp,
}

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
    sessions: RelayServerSessions,
    /// map of Game IDs to corresponding game
    games: HashMap<String, Game>,
    /// random number generator
    rng: ThreadRng,
}

struct RelayServerSessions {
    map: HashMap<String, Recipient<Message>>,
    /// map of User IDs to corresponding session key for session verification
    verification_keys: HashMap<String, String>,
}

impl RelayServerSessions {
    pub fn new() -> RelayServerSessions {
        RelayServerSessions {
            map: HashMap::new(),
            verification_keys: HashMap::new(),
        }
    }
    fn do_send_log(&self, addr: &actix::Recipient<Message>, message: String) {
        if let Err(err) = addr.do_send(Message(message)) {
            println!("[srv/m] do_send error: {:?}", err)
            // TODO send errors to logging record
        }
    }
    pub fn verify_session(&mut self, msg: VerifySession) {
        let user_id_opt = msg.user_id;
        if let Some(user_id) = user_id_opt {
            if let Some(sesh_key) = self.verification_keys.get(&user_id) {
                if sesh_key == &msg.token {
                    // user must have user_id and valid session token for session to verify
                    if let Some(addr) = self.map.get(&user_id) {
                        if addr == &msg.addr {
                            return;
                        }
                    }
                    self.do_send_log(&msg.addr, MsgResult::alert("new session"));
                    // if user's session is untracked and session key is verified, replace self.sessions[user_id] with it
                    self.map.insert(user_id.clone(), msg.addr.clone());
                    return;
                }
            }
        }
        self.do_send_log(&msg.addr, MsgResult::logout("VerifySession"));
    }
    pub fn send_user(&self, user_id: &str, msg: &str) {
        if let Some(session) = self.map.get(user_id) {
            self.do_send_log(session, msg.to_string());
        }
        // TODO log missing sessions
    }
    pub fn send_all(
        &self,
        keys: std::collections::hash_map::Keys<'_, std::string::String, Player>,
        msg: &str,
    ) {
        for k in keys {
            self.send_user(k, msg);
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
            sessions: RelayServerSessions::new(),
            games: HashMap::new(),
            rng: rand::thread_rng(),
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
                let old_sesh = self.sessions.map.insert(user_id.clone(), addr.clone());
                if let Some(res_addr) = old_sesh {
                    if res_addr != addr {
                        self.sessions
                            .do_send_log(&res_addr, MsgResult::logout("Connect"));
                    }
                };
                // new session key used for determining newest authorized session of user
                let key = gen_rng_string(4);
                succ_res.token = Some(key.clone());
                self.sessions.verification_keys.insert(user_id.clone(), key);
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
        self.sessions.verify_session(msg);
    }
}

impl Handler<Disconnect> for RelayServer {
    type Result = ();
    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        let res = self.sessions.map.remove(&msg.user_id);
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
            let mut game = Game::new(game_id.clone(), BOARD_SIZE, self.rng.clone());
            let host_op = game.set_host(host_user_id.clone()).map(|_| ());
            if host_op.is_err() {
                return MessageResult(host_op);
            }
            self.games.insert(game_id.clone(), game.clone());
            self.user_games
                .insert(host_user_id.clone(), game_id.clone());
            res_game = Some(game);
            new_game = true;
        }
        // send json response to client (serialization can fail)
        let game = res_game.expect("res_game is handled");
        let res = MsgResult::host_game(&game)
            .map(|msg_result| {
                self.sessions.send_user(&host_user_id, &msg_result);
                // send action points update to host
                let apu = ActionPointUpdate::new(
                    &host_user_id,
                    &game_id,
                    game.players[&host_user_id].action_points,
                );
                let apu_msg = MsgResult::action_point_update(&apu)
                    .unwrap_or_else(|e| MsgResult::error("joined action_point_update", &e));
                self.sessions.send_user(&host_user_id, &apu_msg);
                let alert = if new_game {
                    MsgResult::alert("new game created")
                } else {
                    MsgResult::alert("rejoined game")
                };
                self.sessions.send_user(&host_user_id, &alert);
            })
            .map_err(|e| format!("{:?}", e).to_owned());
        MessageResult(res)
    }
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
                // dont lock user into game if game is over
                if !game.is_end_phase() {
                    user_games.insert(user_id.clone(), game_id.clone());
                }
                Ok(game)
            })
            // prepare json of game updated with new player
            .and_then(|game| {
                // only send game json to current players if player 'Joined'
                let msg = MsgResult::joined(&game.players[&user_id])
                    .unwrap_or_else(|e| MsgResult::error("player_joined", &e));
                if matches!(insert_player_result, InsertPlayerResult::Joined) {
                    for k in &mut game.players.keys() {
                        if k != &user_id {
                            sessions.send_user(k, &msg);
                        }
                    }
                }
                let msg = MsgResult::join_game(&game)
                    .unwrap_or_else(|e| MsgResult::error("join_game", &e));
                // send game json to player that joined (or rejoined)
                sessions.send_user(&user_id, &msg);
                let apu = ActionPointUpdate::new(
                    &user_id,
                    &game_id,
                    game.players[&user_id].action_points,
                );
                let apu_msg = MsgResult::action_point_update(&apu)
                    .unwrap_or_else(|e| MsgResult::error("joined action_point_update", &e));
                sessions.send_user(&user_id, &apu_msg);
                Ok(())
            });
        MessageResult(res)
    }
}

impl Handler<ConfigGame> for RelayServer {
    type Result = ();
    fn handle(&mut self, msg: ConfigGame, _: &mut Context<Self>) -> Self::Result {
        let ConfigGame {
            game_id,
            user_id,
            op,
        } = msg;
        let sessions = &self.sessions;
        self.games
            .get_mut(&game_id)
            .ok_or("Game not found".to_owned())
            .and_then(|game| {
                if game.host_user_id != Some(user_id.clone()) {
                    return Err("only host can configure game".to_owned());
                }
                game.configure(&op)
                    .map(|res| (MsgResult::conf_game(&game, &res), game))
            })
            .and_then(|(msg_result, game)| {
                let json = msg_result?;
                // send game
                sessions.send_all(game.players.keys(), &json);
                Ok(())
            })
            .unwrap_or_else(|e| {
                sessions.send_user(&user_id, &MsgResult::error("conf_game", &e));
            });
    }
}

impl Handler<StartGame> for RelayServer {
    type Result = MessageResult<StartGame>;
    fn handle(&mut self, msg: StartGame, ctx: &mut Context<Self>) -> Self::Result {
        let StartGame { game_id, user_id } = msg;
        let sessions = &self.sessions;
        let res = self
            .games
            .get_mut(&game_id)
            .ok_or("Game not found".to_owned())
            .and_then(|game| {
                if game.host_user_id != Some(user_id.clone()) {
                    return Err("Only host can start game".to_owned());
                }
                game.start_game()
                    .map(|_| (MsgResult::start_game(&game), game))
            })
            .and_then(|(msg_result, game)| {
                let json = msg_result?;
                // send game
                sessions.send_all(game.players.keys(), &json);
                // send action points to each player
                for (player_id, player) in &game.players {
                    let apu = ActionPointUpdate::new(player_id, &game_id, player.action_points);
                    let msg: String = MsgResult::action_point_update(&apu)
                        .unwrap_or_else(|e| MsgResult::error("action_point_update", &e));
                    sessions.send_user(&player_id, &msg);
                }
                // schedule replenish
                ctx.notify_later(
                    Replenish {
                        game_id: game.game_id.clone(),
                    },
                    Duration::from_secs(game.config.turn_time_secs),
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
        let msg = match MsgResult::user_status(&res) {
            Ok(msg) => msg,
            Err(e) => MsgResult::error("user_status", &e),
        };
        self.sessions.send_user(&user_id, &msg);
    }
}

impl Handler<PlayerActionRequest> for RelayServer {
    type Result = ();
    fn handle(&mut self, msg: PlayerActionRequest, ctx: &mut Context<Self>) -> Self::Result {
        let PlayerActionRequest {
            user_id,
            game_id,
            action,
        } = msg;
        let sessions = &self.sessions;
        let games = &mut self.games;
        let user_games = &mut self.user_games;
        let res = user_games
            .get(&user_id)
            .ok_or("user games not found".to_string())
            .and_then(|user_game_id| {
                if user_game_id != &game_id {
                    return Err("user game id invalid".to_string());
                }
                games.get_mut(&game_id).ok_or("game id bad".to_string())
            })
            .and_then(|game| {
                game.player_action(&user_id, action).map(|e| {
                    // if game is over then remove user_games entry for all players in the game
                    // stops users from being locked into the game
                    if game.is_end_phase() {
                        for user_id in game.players.keys() {
                            user_games.remove(user_id);
                            // tell RelayServer to send user new /user_status update through user session
                            ctx.notify(UserStatus {
                                user_id: user_id.to_string(),
                            });
                        }
                    }
                    (e, game)
                })
            })
            // TODO rewind game action upon json serialization error
            .and_then(|((res, apu), game)| {
                MsgResult::player_action(&res).map(|json| (json, game, apu))
            });
        match res {
            Err(e) => sessions.send_user(&user_id, &MsgResult::error("player_action", &e)),
            Ok((json, game, apu_user_ids)) => {
                // send action point updates
                for (uid, gid, ap) in apu_user_ids {
                    let apu = ActionPointUpdate::new(&uid, &gid, ap);
                    let msg = MsgResult::action_point_update(&apu)
                        .unwrap_or_else(|e| MsgResult::alert(&e));
                    sessions.send_user(&uid, &msg);
                }
                // send game updates
                self.sessions.send_all(game.players.keys(), &json)
            }
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
                let apu = game.replenish()?;
                Ok((game, apu))
            })
            .and_then(|(game, apu)| {
                for (uid, gid, ap) in apu {
                    let apu = ActionPointUpdate::new(&uid, &gid, ap);
                    let msg = MsgResult::action_point_update(&apu)
                        .unwrap_or_else(|e| MsgResult::alert(&e));
                    sessions.send_user(&uid, &msg);
                }
                ctx.notify_later(
                    Replenish { game_id },
                    Duration::from_secs(game.config.turn_time_secs),
                );
                Ok(())
            });
        if res.is_err() {
            dbg!(&res);
        }
        MessageResult(res)
    }
}
