use actix::prelude::*;
use rand::rngs::ThreadRng;
use std::collections::HashMap;

use crate::game::Game;

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
    FailPassword,
    SuccExists,
    SuccNew,
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

/// List of available games
pub struct ListGames;

// list of game IDs that can be subscribed to
impl actix::Message for ListGames {
    type Result = Vec<String>;
}

/// Join game, if non-existant throw error
#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct Join {
    /// session id of sender
    pub user_id: String,
    /// publisher id
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

    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) -> Self::Result {
        dbg!(msg.clone());
        let User { user_id, password } = msg.user.clone();
        let msg_addr = msg.addr;
        let mut res = ConnectResult::SuccNew;
        match self.users.get(&user_id) {
            Some(existant) => {
                if existant.password == password {
                    res = ConnectResult::SuccExists;
                } else {
                    res = ConnectResult::FailPassword;
                }
            }
            None => {
                self.users.insert(user_id.clone(), msg.user);
                res = ConnectResult::SuccNew;
            }
        }
        if !matches!(res, ConnectResult::FailPassword) {
            if let Some(addr) = msg_addr {
                let res = self.sessions.insert(user_id.clone(), addr);
                if let Some(addr) = res {
                    do_send_log(&addr, "logged in elsewhere");
                };
            }
        }
        dbg!(res.clone());
        return MessageResult(res);
    }
}
