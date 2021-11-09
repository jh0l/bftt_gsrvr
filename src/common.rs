use rand::Rng;

use serde::{Deserialize, Serialize};

use crate::game::{Game, GamePlayers, PlayerResponse};

#[derive(Deserialize)]
pub struct Identity {
    pub user_id: String,
    pub password: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SuccessResult {
    pub token: Option<String>,
    pub alert: String,
}

#[derive(Clone, Debug)]
pub enum Fail {
    Password,
}

#[derive(Clone, Debug, Serialize)]
pub struct UserStatusResult {
    pub game_id: Option<String>,
}

pub struct MsgResult;

impl MsgResult {
    fn json_string<V>(cmd: &str, value: &V) -> Result<String, String>
    where
        V: Serialize,
    {
        serde_json::to_string(value)
            .and_then(|json| Ok(format!("{} {}", cmd, json)))
            .or_else(|err| Err(format!("{:?}", err)))
    }

    pub fn login(msg: &SuccessResult) -> Result<String, String> {
        MsgResult::json_string("/login", msg)
    }

    pub fn logout(msg: &str) -> String {
        format!("/logout {}", msg).to_string()
    }

    pub fn host_game(game: &Game) -> Result<String, String> {
        MsgResult::json_string("/host_game_success", game)
    }

    pub fn join_game(json: &str) -> String {
        format!("/join_game_success {}", json).to_string()
    }

    pub fn joined(json: &str) -> String {
        format!("/player_joined {}", json).to_string()
    }

    pub fn start_game(game: &Game) -> Result<String, String> {
        MsgResult::json_string("/start_game", game)
    }

    pub fn user_status(user_status: &UserStatusResult) -> Result<String, String> {
        MsgResult::json_string("/user_status", user_status)
    }

    pub fn replenish(game_players: &GamePlayers) -> Result<String, String> {
        MsgResult::json_string("/replenish", game_players)
    }

    pub fn player_action(action: &PlayerResponse) -> Result<String, String> {
        MsgResult::json_string("/player_action", action)
    }

    pub fn error(msg: &str) -> String {
        format!("/error {}", msg).to_string()
    }

    pub fn alert(msg: &str) -> String {
        format!("/alert {}", msg).to_string()
    }
}

pub fn gen_rng_string(len: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
    abcdefghijklmnopqrstuvwxyz\
    0123456789)(*&^%$#@!~";
    let mut rng = rand::thread_rng();

    (0..len)
        .map(|_| {
            let idx = rng.gen_range(0, CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}
