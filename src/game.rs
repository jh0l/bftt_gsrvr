use rand::distributions::Uniform;
use rand::prelude::{Distribution, ThreadRng};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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
    pub game_id: String,
    pub lives: u32,
    #[serde(skip_serializing)]
    pub action_points: u32,
    pub pos: Pos,
    pub range: usize,
}

impl Player {
    pub fn new(user_id: String, game_id: String) -> Player {
        Player {
            user_id,
            game_id,
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
            return Err(format!("{} has no life", self.user_id));
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
    pub players_alive: HashSet<String>,
    pub turn_time_secs: u64,
    pub board: Board,
    pub turn_end_unix: u64,
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
#[derive(Deserialize, Serialize, Debug)]
pub struct RangeUpgradeAction {
    point_cost: i8,
}

#[derive(Deserialize, Debug)]
pub enum ActionType {
    Attack(AttackAction),
    Give(GiveAction),
    Move(MoveAction),
    RangeUpgrade(RangeUpgradeAction),
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
    RangeUpgrade(RangeUpgradeAction),
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
            players_alive: HashSet::new(),
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
            self.players.insert(
                user_id.clone(),
                Player::new(user_id.clone(), self.game_id.clone()),
            );
            self.players_alive.insert(user_id.clone());
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

    /// error if game not in progress
    pub fn check_in_prog(&self) -> Result<(), String> {
        if !matches!(self.phase, GamePhase::InProg) {
            return Err("Game not in progress".to_owned());
        }
        Ok(())
    }

    pub fn replenish(&mut self) -> Result<Vec<(String, String, u32)>, String> {
        self.check_in_prog()?;
        let mut action_point_updates: Vec<(String, String, u32)> = Vec::new();
        for player in self.players.values_mut() {
            if player.lives > 0 {
                player.action_points += 1;
            }
            action_point_updates.push((
                player.user_id.clone(),
                self.game_id.clone(),
                player.action_points,
            ));
        }
        self.turn_end_unix = from_now(self.turn_time_secs);
        Ok(action_point_updates)
    }

    pub fn clone_player(&self, player_id: &str) -> Result<Player, String> {
        let player = self
            .players
            .get(player_id)
            .ok_or(format!("player {} not found", player_id))?
            .clone();
        Ok(player)
    }

    pub fn check_for_end_phase_move(&mut self, player_id: &str) -> Result<(), String> {
        if self
            .players
            .get(player_id)
            .ok_or(format!("{} does not exist", player_id).to_string())?
            .lives
            == 0
        {
            self.players_alive.remove(player_id);
            if self.players_alive.len() == 1 {
                self.phase = GamePhase::End;
            }
        }
        Ok(())
    }

    /// validate a player action then execute the required changes to the game
    /// `player_flux` is a copy of the acting player to be applied at fn end
    /// `target_flux` is a copy of the target player to be applied at match arm end
    /// returns Vec<String> to list players that need action_point updates
    pub fn player_action(
        &mut self,
        user_id: &str,
        action: ActionType,
    ) -> Result<(PlayerResponse, Vec<(String, String, u32)>), String> {
        if matches!(self.phase, GamePhase::End) {
            return Err("game over".to_string());
        }
        let mut action_point_updates: Vec<(String, String, u32)> = Vec::new();
        let mut player_flux = self.clone_player(user_id)?;
        let action: ActionTypeEvent = match &action {
            ActionType::Move(walk) => {
                // <VALIDATE>
                // validate bounds
                // validate tile occupation
                self.board.in_bounds(&walk.pos, true)?;
                if matches!(self.phase, GamePhase::InProg) {
                    // validate lives
                    player_flux.has_lives()?;
                    // validate action points
                    // validate player range ability
                    // validate move distance against range ability
                    player_flux.moveable_in_prog(&walk.pos)?;
                    // <EXECUTE>
                    player_flux.action_points -= 1;
                };
                // <EXECUTE>
                // remove user from current pos
                if player_flux.pos.x != usize::MAX {
                    self.board
                        .map
                        .remove(&player_flux.pos.key())
                        .ok_or("player desynchronized")?;
                }
                // set MoveActionEvent
                let action_event = ActionTypeEvent::Move(MoveEvent {
                    from: player_flux.pos.clone(),
                    to: walk.pos.clone(),
                });
                // set player coords
                player_flux.pos = walk.pos.clone();
                // place user_id in new pos
                self.board
                    .map
                    .insert(player_flux.pos.key().clone(), user_id.to_string());
                action_event
            }
            ActionType::Attack(attack) => {
                // <VALIDATE>
                self.check_in_prog()?;
                // validate player is not targeting themselves
                if user_id == attack.target_user_id {
                    return Err("Stop hurting yourself".to_string());
                }
                // validate player has lives
                player_flux.has_lives()?;

                let mut target_flux = self.clone_player(&attack.target_user_id)?;
                // validate target is alive
                target_flux.has_lives()?;
                // has action points
                // player in range of target
                player_flux.moveable_in_prog(&target_flux.pos)?;
                // action's lives effect is -1
                if attack.lives_effect != -1 {
                    return Err("attacking must take 1 life :'(".to_string());
                }
                // remove player action point
                player_flux.action_points -= 1;
                // remove target life
                target_flux.lives -= 1;
                // if target life is 0 then check number of players alive
                // if players alive is 1 then end game
                if target_flux.lives == 0 {
                    self.players_alive.remove(&target_flux.user_id);
                    if self.players_alive.len() == 1 {
                        self.phase = GamePhase::End;
                    }
                }
                // assign end phase if move ends the game
                self.check_for_end_phase_move(&target_flux.user_id)?;
                // apply target_copy
                self.players
                    .insert(target_flux.user_id.clone(), target_flux);
                // return action event
                ActionTypeEvent::Attack(AttackAction {
                    lives_effect: attack.lives_effect,
                    target_user_id: attack.target_user_id.clone(),
                })
            }
            ActionType::Give(give) => {
                // <VALIDATE>
                // game in progress
                self.check_in_prog()?;
                // player is not targeting themselves
                if user_id == give.target_user_id {
                    return Err("this is a futile endeavour".to_string());
                }
                // player has lives
                player_flux.has_lives()?;
                // target has lives
                let mut target_flux = self.clone_player(&give.target_user_id)?;
                target_flux.has_lives()?;
                // player has action points
                // player in range of target
                player_flux.moveable_in_prog(&target_flux.pos)?;
                // <EXECUTE>
                player_flux.action_points -= 1;
                target_flux.action_points += 1;
                // add target to action point update list
                action_point_updates.push((
                    target_flux.user_id.clone(),
                    self.game_id.clone(),
                    target_flux.action_points,
                ));
                // apply target_copy
                self.players
                    .insert(target_flux.user_id.clone(), target_flux);
                // return action event
                ActionTypeEvent::Give(GiveAction {
                    target_user_id: give.target_user_id.clone(),
                })
            }
            ActionType::RangeUpgrade(range_upgrade) => {
                // <VALIDATE>
                // game in progress
                self.check_in_prog()?;
                // player has lives
                player_flux.has_lives()?;
                // player has enough action points and correct cost estimate
                if player_flux.action_points < 3 || range_upgrade.point_cost != 3 {
                    return Err("3 action points required to upgrade range".to_string());
                }
                // <EXECUTE>
                // exchange action points for range
                player_flux.action_points -= 3;
                player_flux.range += 1;
                // return action event
                ActionTypeEvent::RangeUpgrade(RangeUpgradeAction {
                    point_cost: range_upgrade.point_cost,
                })
            }
        };
        // add player to action point update list
        action_point_updates.push((
            user_id.to_string(),
            self.game_id.clone(),
            player_flux.action_points,
        ));
        // apply player copy
        self.players
            .insert(player_flux.user_id.clone(), player_flux);
        Ok((
            PlayerResponse {
                game_id: self.game_id.clone(),
                user_id: user_id.to_string(),
                phase: self.phase.clone(),
                action,
            },
            action_point_updates,
        ))
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
