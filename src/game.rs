use rand::distributions::Uniform;
use rand::prelude::{Distribution, ThreadRng};
use serde::Serialize;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

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
    pub range: u16,
}

impl Player {
    pub fn new(user_id: String) -> Player {
        Player {
            user_id,
            lives: 0,
            moves: 0,
            pos: Pos { x: 0, y: 0 },
            range: 2,
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
        return Ok(&mut self.0[y][x]);
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
    pub turn_time_secs: u64,
    pub board: Board,
    pub turn_end_unix: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GamePlayers {
    pub game_id: String,
    pub players: HashMap<String, Player>,
}

impl Game {
    pub fn new(game_id: String, size: u16) -> Game {
        Game {
            phase: GamePhase::Init,
            game_id,
            host_user_id: None,
            players: HashMap::new(),
            turn_time_secs: 10,
            board: Board::new(size),
            turn_end_unix: 0,
        }
    }

    pub fn set_host(&mut self, host_id: String) -> Result<String, String> {
        self.host_user_id = Some(host_id.clone());
        self.insert_player(host_id)
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

    pub fn start_game(&mut self, rnd: &mut ThreadRng) -> Result<(), String> {
        if !matches!(self.phase, GamePhase::Init) {
            return Err("Game already started".to_owned());
        }
        let len = self.board.0.len();
        let die = Uniform::from(0..len);
        for (k, player) in &mut self.players {
            player.lives = 3;
            player.moves = 1;
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
                    .map_err(|_| "bad Distribution sample index")?;
            }
        }
        self.phase = GamePhase::InProg;
        self.turn_end_unix = from_now(self.turn_time_secs);
        Ok(())
    }

    pub fn replenish(&mut self) -> Result<GamePlayers, String> {
        if !matches!(self.phase, GamePhase::InProg) {
            return Err("Game not in progress".to_owned());
        }
        for player in self.players.values_mut() {
            if player.lives > 0 {
                player.moves += 1;
            }
        }
        self.turn_end_unix = from_now(self.turn_time_secs);
        Ok(GamePlayers {
            game_id: self.game_id.clone(),
            players: self.players.to_owned(),
        })
    }
}

fn from_now(to_secs: u64) -> u64 {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();
    since_the_epoch + to_secs
}
