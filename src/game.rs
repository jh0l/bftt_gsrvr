use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
pub struct Pos {
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Player {
    pub user_id: String,
    pub lives: u32,
    pub moves: u32,
    pub pos: Pos,
}

impl Player {
    pub fn new(user_id: String) -> Player {
        Player {
            user_id,
            lives: 0,
            moves: 0,
            pos: Pos { x: 0, y: 0 },
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Game {
    pub game_id: String,
    pub host_user_id: Option<String>,
    pub players: HashMap<String, Player>,
}

impl Game {
    pub fn new(game_id: String) -> Game {
        Game {
            game_id,
            host_user_id: None,
            players: HashMap::new(),
        }
    }
    pub fn set_host(mut self, host_id: String) -> Self {
        self.host_user_id = Some(host_id.clone());
        self.insert_player(host_id)
    }

    pub fn insert_player(mut self, user_id: String) -> Self {
        self.players.insert(user_id.clone(), Player::new(user_id));
        self
    }
}
