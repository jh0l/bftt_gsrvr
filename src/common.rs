use rand::Rng;

use serde::{Deserialize, Serialize};

use crate::game::{Game, GamePlayers};

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

pub struct MsgResult;

impl MsgResult {
    fn json_string<V>(value: &V, cmd: &str) -> Result<String, String>
    where
        V: Serialize,
    {
        serde_json::to_string(value)
            .and_then(|json| Ok(format!("{} {}", cmd, json)))
            .or_else(|err| Err(format!("{:?}", err)))
    }

    pub fn login(msg: &SuccessResult) -> Result<String, String> {
        MsgResult::json_string(msg, "/login")
    }

    pub fn logout(msg: &str) -> String {
        format!("/logout {}", msg).to_string()
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

    pub fn replenish(game_players: &GamePlayers) -> Result<String, String> {
        MsgResult::json_string(game_players, "/replenish")
    }

    pub fn error(msg: &str) -> String {
        format!("/error {}", msg).to_string()
    }

    pub fn alert(msg: String) -> String {
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
