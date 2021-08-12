use rand::distributions::Uniform;
use rand::prelude::{Distribution, ThreadRng};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
pub struct Pos {
    pub x: usize,
    pub y: usize,
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
pub struct Board(Vec<Vec<Option<String>>>);

impl Board {
    pub fn new(size: u16) -> Board {
        let mut board = Vec::with_capacity(size as usize);
        for _ in 0..size {
            let mut row = Vec::with_capacity(size as usize);
            for _ in 0..size {
                row.push(None);
            }
            board.push(row);
        }
        Board(board)
    }

    // pub fn set(&mut self, x: usize, y: usize, v: Option<String>) -> Result<(), String> {
    //     let len = self.0.len();
    //     if x >= len || y >= len || x < 0 || y < 0 {
    //         return Err("out of range".to_owned());
    //     }
    //     self.0[x][y] = v;
    //     return Ok(());
    // }

    pub fn index_mut(&mut self, x: usize, y: usize) -> Result<&mut Option<String>, String> {
        let len = self.0.len();
        if x >= len || y >= len {
            return Err("out of range".to_owned());
        }
        return Ok(&mut self.0[x][y]);
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum GamePhase {
    Init,
    InProg,
    End,
}

#[derive(Debug, Clone, Serialize)]
pub struct Game {
    pub game_id: String,
    pub phase: GamePhase,
    pub host_user_id: Option<String>,
    pub players: HashMap<String, Player>,
    pub turn_time_secs: u32,
    pub board: Board,
    pub turn_end_unix: i64,
}

impl Game {
    pub fn new(game_id: String, size: u16) -> Game {
        Game {
            phase: GamePhase::Init,
            game_id,
            host_user_id: None,
            players: HashMap::new(),
            turn_time_secs: 60,
            board: Board::new(size),
            turn_end_unix: 0,
        }
    }

    pub fn set_host(mut self, host_id: String) -> Self {
        self.host_user_id = Some(host_id.clone());
        self.insert_player(host_id).expect("setting host");
        self
    }

    pub fn insert_player(&mut self, user_id: String) -> Result<String, String> {
        if matches!(self.phase, GamePhase::Init) {
            self.players
                .insert(user_id.clone(), Player::new(user_id.clone()));
            return Ok(format!("player {} joined", user_id).to_owned());
        } else if self.players.contains_key(&user_id) {
            return Ok(format!("player {} rejoined", user_id).to_owned());
        }
        return Err("Game in progress".to_owned());
    }

    pub fn start_game(&mut self, rnd: &mut ThreadRng) -> Result<String, String> {
        let len = self.board.0.len();
        let die = Uniform::from(0..len);
        for (k, player) in &mut self.players {
            let mut res = false;
            while res == false {
                let x = die.sample(rnd);
                let y = die.sample(rnd);
                self.board
                    .index_mut(x, y)
                    .and_then(|r| {
                        if r.is_none() {
                            *r = Some(k.clone());
                            player.pos = Pos { x, y };
                            res = true;
                        }
                        Ok(())
                    })
                    .expect("bad indexing");
            }
        }
        self.phase = GamePhase::InProg;
        Ok("ok".to_owned())
    }

    // pub fn set_turn_time(&mut self, new_time: u32) -> Result<(), String> {
    //     if new_time > 0 && matches!(self.phase, GamePhase::Init) {
    //         self.turn_time_secs = new_time;
    //         return Ok(());
    //     }
    //     Err("cannot change turn time".to_owned())
    // }
}
