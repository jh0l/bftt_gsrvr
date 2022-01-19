use std::collections::HashMap;

use rand::Rng;

use serde::{Deserialize, Serialize};

use crate::game::{Game, Player, PlayerResponse, PlayersAliveDead, Pos};

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

#[derive(Debug, Clone, Serialize)]
pub struct ActionPointUpdate {
    pub user_id: String,
    pub game_id: String,
    pub action_points: u32,
}

impl ActionPointUpdate {
    pub fn new(user_id: &str, game_id: &str, action_points: u32) -> ActionPointUpdate {
        ActionPointUpdate {
            user_id: user_id.to_string(),
            game_id: game_id.to_string(),
            action_points,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InitPosConfig {
    Random,
    Manual,
    // RandomBlind,
    // ManualSecret,
}
#[derive(Debug, Clone, Deserialize)]
pub enum ConfigGameOp {
    TurnTimeSecs(u64),
    MaxPlayers(u16),
    BoardSize(usize),
    InitLives(u32),
    InitRange(usize),
    InitActPts(u32),
    InitPos(InitPosConfig),
}

#[derive(Debug, Clone, Serialize)]
struct GameConfigResult<'a> {
    game: &'a Game,
    result: &'a Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize)]
struct GameTurnEndUnix {
    game_id: String,
    turn_end_unix: u64,
}

#[derive(Debug, Clone, Serialize)]
struct PlayersAliveUpdate {
    game_id: String,
    alive_dead: PlayersAliveDead,
}

#[derive(Debug, Serialize)]
struct BoardActionPoints {
    game_id: String,
    board: HashMap<String, u32>,
    new: Option<Pos>,
    old: Option<Pos>,
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

    pub fn join_game(game: &Game) -> Result<String, String> {
        MsgResult::json_string("/join_game_success", game)
    }

    pub fn joined(json: &Player) -> Result<String, String> {
        MsgResult::json_string("/player_joined", json)
    }

    pub fn conf_game(
        game: &Game,
        result: &Option<HashMap<String, String>>,
    ) -> Result<String, String> {
        let res = GameConfigResult { game, result };
        MsgResult::json_string("/conf_game", &res)
    }

    pub fn start_game(game: &Game) -> Result<String, String> {
        MsgResult::json_string("/start_game", game)
    }

    pub fn board_action_points(
        game: &Game,
        new: Option<Pos>,
        old: Option<Pos>,
    ) -> Result<String, String> {
        let bap = BoardActionPoints {
            board: game.ap_board.map.clone(),
            game_id: game.game_id.to_owned(),
            new,
            old,
        };
        MsgResult::json_string("/board_action_points", &bap)
    }

    pub fn action_point_update(apu: &ActionPointUpdate) -> Result<String, String> {
        MsgResult::json_string("/action_point_update", apu)
    }

    pub fn turn_end_unix(game: &Game) -> Result<String, String> {
        let res = GameTurnEndUnix {
            game_id: game.game_id.clone(),
            turn_end_unix: game.turn_end_unix,
        };
        MsgResult::json_string("/turn_end_unix", &res)
    }

    pub fn user_status(user_status: &UserStatusResult) -> Result<String, String> {
        MsgResult::json_string("/user_status", user_status)
    }

    pub fn player_action(action: &PlayerResponse) -> Result<String, String> {
        MsgResult::json_string("/player_action", action)
    }

    pub fn players_alive_update(
        alive_dead: &PlayersAliveDead,
        game_id: &str,
    ) -> Result<String, String> {
        let res = PlayersAliveUpdate {
            alive_dead: alive_dead.clone(),
            game_id: game_id.to_owned(),
        };
        MsgResult::json_string("/players_alive_update", &res)
    }

    pub fn error(context: &str, msg: &str) -> String {
        format!("/error {}: {}", context, msg).to_string()
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
