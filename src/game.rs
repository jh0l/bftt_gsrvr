use rand::distributions::Uniform;
use rand::prelude::{Distribution, ThreadRng};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pos {
    pub x: usize,
    pub y: usize,
}

impl Pos {
    /// Calculates the manhattan distance between the two provided grid cells
    pub fn xy_distances(a: &Pos, b: &Pos) -> Pos {
        let x = if a.x < b.x { b.x - a.x } else { a.x - b.x };
        let y = if a.y < b.y { b.y - a.y } else { a.y - b.y };
        Pos { x, y }
    }

    pub fn key(&self) -> String {
        format!("{},{}", self.x, self.y)
    }
}

impl Display for Pos {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{},{}", self.x, self.y)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Player {
    pub user_id: String,
    pub lives: u32,
    pub action_points: u32,
    pub pos: Pos,
    pub range: usize,
}

impl Player {
    pub fn new(user_id: String) -> Player {
        Player {
            user_id,
            lives: 0,
            action_points: 0,
            pos: Pos {
                x: usize::MAX,
                y: usize::MAX,
            },
            range: 2,
        }
    }

    pub fn has_lives(&self) -> Result<(), String> {
        if self.lives < 1 {
            return Err("player has no lives".to_string());
        }
        Ok(())
    }

    pub fn has_action_points(&self, required: u32) -> Result<(), String> {
        if self.action_points < required {
            return Err("insufficient action points".to_string());
        }
        Ok(())
    }

    /// validate action points
    /// validate player range ability
    /// validate range ability against move distance
    pub fn moveable_in_prog(&self, pos: &Pos) -> Result<(), String> {
        self.has_action_points(1)?;
        let dist = Pos::xy_distances(&self.pos, pos);
        if dist.x > self.range || dist.y > self.range {
            return Err("move out of range".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Board {
    map: HashMap<String, String>,
    size: usize,
}

impl Board {
    pub fn new(size: usize) -> Board {
        Board {
            map: HashMap::new(),
            size,
        }
    }

    pub fn in_bounds(&mut self, pos: &Pos, check_occupied: bool) -> Result<(), String> {
        let Pos { x, y } = pos;
        if x >= &self.size || y >= &self.size {
            return Err("out of range".to_string());
        }
        if check_occupied && self.map.get(&pos.key()).is_some() {
            return Err("space is occupied".to_string());
        }
        Ok(())
    }

    // pub fn set(&mut self, x: usize, y: usize, v: Option<String>) -> Result<(), String> {
    //     let len = self.0.len();
    //     if x >= len || y >= len || x < 0 || y < 0 {
    //         return Err("out of range".to_owned());
    //     }
    //     self.0[x][y] = v;
    //     return Ok(());
    // }
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

pub enum InsertPlayerResult {
    Joined,
    Rejoined,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct AttackAction {
    target_user_id: String,
    lives_effect: i8,
}
#[derive(Deserialize, Serialize, Debug)]
pub struct GiveAction {
    target_user_id: String,
}
#[derive(Deserialize, Debug)]
pub struct MoveAction {
    pos: Pos,
}
#[derive(Deserialize, Debug)]
pub enum ActionType {
    Attack(AttackAction),
    Give(GiveAction),
    Move(MoveAction),
}
#[derive(Deserialize, Debug)]
pub struct PlayerAction {
    pub user_id: String,
    pub game_id: String,
    pub action: ActionType,
}

#[derive(Serialize, Debug)]
pub struct MoveEvent {
    from: Pos,
    to: Pos,
}
#[derive(Serialize, Debug)]
pub enum ActionTypeEvent {
    Attack(AttackAction),
    Give(GiveAction),
    Move(MoveEvent),
}
#[derive(Serialize, Debug)]
pub struct PlayerResponse {
    user_id: String,
    game_id: String,
    action: ActionTypeEvent,
    phase: GamePhase,
}

pub const BOARD_SIZE: u16 = 18;

impl Game {
    pub fn new(game_id: String, size: u16) -> Game {
        Game {
            phase: GamePhase::Init,
            game_id,
            host_user_id: None,
            players: HashMap::new(),
            turn_time_secs: 10,
            board: Board::new(size as usize),
            turn_end_unix: 0,
        }
    }

    pub fn set_host(&mut self, host_id: String) -> Result<(), String> {
        self.host_user_id = Some(host_id.clone());
        self.insert_player(host_id).map(|_| ())
    }

    pub fn insert_player(&mut self, user_id: String) -> Result<InsertPlayerResult, String> {
        if self.players.contains_key(&user_id) {
            return Ok(InsertPlayerResult::Rejoined);
        }
        if matches!(self.phase, GamePhase::Init) {
            self.players
                .insert(user_id.clone(), Player::new(user_id.clone()));
            return Ok(InsertPlayerResult::Joined);
        }
        return Err("game in progress".to_owned());
    }

    pub fn start_game(&mut self, rnd: &mut ThreadRng) -> Result<(), String> {
        if !matches!(self.phase, GamePhase::Init) {
            return Err("Game already started".to_owned());
        }
        let die = Uniform::from(0..self.board.size);
        for (k, player) in &mut self.players {
            player.lives = 3;
            player.action_points = 1;
            // set player's position randomly
            if player.pos.x == usize::MAX {
                let mut res = false;
                while res == false {
                    let x = die.sample(rnd);
                    let y = die.sample(rnd);
                    let pos = Pos { x, y };
                    if !self.board.map.contains_key(&pos.key()) {
                        self.board.map.insert(pos.key(), k.clone());
                        player.pos = pos;
                        res = true;
                    }
                }
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
                player.action_points += 1;
            }
        }
        self.turn_end_unix = from_now(self.turn_time_secs);
        Ok(GamePlayers {
            game_id: self.game_id.clone(),
            players: self.players.to_owned(),
        })
    }

    pub fn player_action(
        &mut self,
        user_id: &str,
        action: ActionType,
    ) -> Result<PlayerResponse, String> {
        if matches!(self.phase, GamePhase::End) {
            return Err("game over".to_string());
        }
        let player = self
            .players
            .get_mut(user_id)
            .ok_or("player not found".to_string())?;
        let action = action;
        let mut action_event: Option<ActionTypeEvent> = None;
        match &action {
            ActionType::Move(walk) => {
                // <VALIDATE>
                // validate bounds
                // validate tile occupation
                self.board.in_bounds(&walk.pos, true)?;
                if matches!(self.phase, GamePhase::InProg) {
                    // validate action points
                    // validate player range ability
                    // validate move distance against range ability
                    player.moveable_in_prog(&walk.pos)?;
                    // validate lives
                    player.has_lives()?;
                    // <EXECUTE>
                    player.action_points -= 1;
                };
                // <EXECUTE>
                // set MoveActionEvent
                action_event = Some(ActionTypeEvent::Move(MoveEvent {
                    from: player.pos.clone(),
                    to: walk.pos.clone(),
                }));
                // remove user from current pos
                if player.pos.x != usize::MAX {
                    self.board
                        .map
                        .remove(&player.pos.key())
                        .ok_or("player desynchronized")?;
                }
                // set player coords
                player.pos = walk.pos.clone();
                // place user_id in new pos
                self.board
                    .map
                    .insert(player.pos.key().clone(), user_id.to_string());
            }
            ActionType::Attack(attack) => {}
            ActionType::Give(give) => {}
        }
        let action = action_event.ok_or("action event uninitialized".to_string())?;
        Ok(PlayerResponse {
            game_id: self.game_id.clone(),
            user_id: user_id.to_string(),
            phase: self.phase.clone(),
            action,
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
