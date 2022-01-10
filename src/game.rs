use rand::distributions::Uniform;
use rand::prelude::{Distribution, ThreadRng};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::common::{ConfigGameOp, InitPosConfig};

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
            lives: INIT_LIVES,
            action_points: INIT_ACTION_POINTS,
            pos: Pos {
                x: usize::MAX,
                y: usize::MAX,
            },
            range: INIT_RANGE,
        }
    }

    pub fn is_alive(&self) -> Result<(), String> {
        if self.lives < 1 {
            return Err(format!("{} has no life", self.user_id));
        }
        Ok(())
    }

    pub fn is_dead(&self) -> Result<(), String> {
        if self.lives > 0 {
            return Err(format!("{} is alive", self.user_id));
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
pub struct Board<T> {
    map: HashMap<String, T>,
    size: usize,
}

impl<T> Board<T> {
    pub fn new(size: usize) -> Board<T> {
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
}

#[derive(Debug, Clone, Serialize)]
pub struct PlayersAliveDead {
    alive: HashSet<String>,
    dead: HashSet<String>,
}

impl PlayersAliveDead {
    pub fn new() -> PlayersAliveDead {
        PlayersAliveDead {
            alive: HashSet::new(),
            dead: HashSet::new(),
        }
    }

    pub fn set_alive(&mut self, id: &str) {
        self.alive.insert(id.to_owned());
        self.dead.remove(id);
    }

    pub fn set_dead(&mut self, id: &str) {
        self.dead.insert(id.to_string());
        self.alive.remove(id);
    }

    pub fn alive_len(&self) -> usize {
        self.alive.len()
    }
}

#[derive(Debug, Clone)]
pub struct Election {
    name: String,
    candidates: HashSet<String>,
    voters: HashSet<String>,
    vote_count: HashMap<String, HashSet<String>>,
    voter_vote: HashMap<String, String>,
}

impl Election {
    pub fn new(name: String) -> Election {
        Election {
            name,
            candidates: HashSet::new(),
            voters: HashSet::new(),
            vote_count: HashMap::new(),
            voter_vote: HashMap::new(),
        }
    }

    pub fn remove_vote(&mut self, voter_id: &str) -> Result<(), String> {
        if let Some(old_c_id) = self.voter_vote.get(voter_id) {
            self.vote_count.get_mut(old_c_id).and_then(|v| {
                v.remove(voter_id);
                Some(())
            });
        }
        // insert voter's vote in voter vote
        self.voter_vote.remove(voter_id);
        Ok(())
    }

    pub fn vote(&mut self, voter_id: &str, candidate_id: &str) -> Result<(), String> {
        // <VALIDATE>
        // voter must be in voters
        if !self.voters.contains(voter_id) {
            return Err(format!("player cannot vote in {}", self.name).to_string());
        }
        // candidate must be in candidates
        if !self.candidates.contains(candidate_id) {
            return Err(
                format!("{} is not a candidate in {}", candidate_id, self.name).to_string(),
            );
        }
        // remove old vote if it exists
        if let Some(old_c_id) = self.voter_vote.get(voter_id) {
            self.vote_count.get_mut(old_c_id).and_then(|v| {
                v.remove(voter_id);
                Some(())
            });
        }
        // insert voter's vote in candidate vote count
        if let None = self.vote_count.get(candidate_id) {
            self.vote_count
                .insert(candidate_id.to_string(), HashSet::new());
        }
        if let Some(count) = self.vote_count.get_mut(candidate_id) {
            count.insert(voter_id.to_string());
        }
        // insert voter's vote in voter vote
        self.voter_vote
            .insert(voter_id.to_string(), candidate_id.to_string());
        Ok(())
    }

    /// convert voter to candidate
    pub fn convert_voter(&mut self, voter_id: &str) -> Result<(), String> {
        self.voters.remove(voter_id);
        self.candidates.insert(voter_id.to_string());
        self.voter_vote.remove(voter_id).and_then(|candidate_id| {
            self.vote_count.get_mut(&candidate_id).and_then(|vc| {
                vc.remove(voter_id);
                Some(())
            });
            Some(())
        });
        Ok(())
    }

    /// convert candidate to voter
    pub fn convert_candidate(&mut self, candidate_id: &str) {
        self.candidates.remove(candidate_id);
        self.voters.insert(candidate_id.to_string());
        self.vote_count.remove(candidate_id);
        // TODO remove candidate from voter_votes and notify voters
    }

    /// get a voter's vote if any
    pub fn get_voter_vote(&self, voter_id: &str) -> Option<String> {
        self.voter_vote
            .get(voter_id)
            .and_then(|f| Some(f.to_owned()))
    }

    /// get highest voted candidates
    pub fn redeem(&mut self) -> HashSet<String> {
        // candidates must have at least 1 vote, candidates with empty hashsets are ignored
        let mut len = 1;
        let mut res: HashSet<String> = HashSet::new();
        for (k, v) in &self.vote_count {
            if v.len() == len {
                res.insert(k.to_owned());
            } else if v.len() > len {
                res = HashSet::new();
                res.insert(k.to_owned());
                len = v.len();
            }
        }
        self.reset();
        res
    }

    /// reset votes
    pub fn reset(&mut self) {
        self.vote_count = HashMap::new();
        self.voter_vote = HashMap::new();
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum GamePhase {
    Init,
    InProg,
    End,
}

#[derive(Debug, Clone, Serialize)]
pub struct GameConfig {
    pub turn_time_secs: u64,
    pub max_players: u16,
    pub init_action_points: u32,
    pub init_lives: u32,
    pub init_range: usize,
    pub init_pos: InitPosConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct Game {
    pub game_id: String,
    pub phase: GamePhase,
    pub host_user_id: Option<String>,
    pub players: HashMap<String, Player>,
    pub players_alive_dead: PlayersAliveDead,
    pub board: Board<String>,
    pub object_board: Board<HashMap<String, usize>>,
    pub turn_end_unix: u64,
    pub config: GameConfig,
    #[serde(skip_serializing)]
    rnd: ThreadRng,
    #[serde(skip_serializing)]
    pub curse_election: Election,
}

pub enum InsertPlayerResult {
    Joined,
    Rejoined,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct AttackAction {
    target_user_id: String,
    lives_effect: u32,
}
#[derive(Deserialize, Serialize, Debug)]
pub struct GiveAction {
    target_user_id: String,
}
#[derive(Deserialize, Debug)]
pub struct MoveAction {
    pub pos: Pos,
}
#[derive(Deserialize, Serialize, Debug)]
pub struct RangeUpgradeAction {
    point_cost: u32,
}
#[derive(Deserialize, Serialize, Debug)]
pub struct HealAction {
    point_cost: u32,
}
#[derive(Deserialize, Serialize, Debug)]
pub struct ReviveAction {
    target_user_id: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct CurseAction {
    target_user_id: Option<String>,
}

#[derive(Deserialize, Debug)]
pub enum ActionType {
    Attack(AttackAction),
    Give(GiveAction),
    Move(MoveAction),
    RangeUpgrade(RangeUpgradeAction),
    Heal(HealAction),
    Revive(ReviveAction),
    Curse(CurseAction),
}

#[derive(Deserialize, Debug)]
pub struct PlayerAction {
    pub user_id: String,
    pub game_id: String,
    pub action: ActionType,
}

#[derive(Serialize, Debug)]
pub struct MoveEvent {
    pub from: Pos,
    pub to: Pos,
}
#[derive(Serialize, Debug)]
pub enum ActionTypeEvent {
    Attack(AttackAction),
    Give(GiveAction),
    Move(MoveEvent),
    RangeUpgrade(RangeUpgradeAction),
    Heal(HealAction),
    Revive(ReviveAction),
    Curse(CurseAction),
}

#[derive(Serialize, Debug)]
pub struct PlayerActionResponse {
    user_id: String,
    game_id: String,
    action: ActionTypeEvent,
    phase: GamePhase,
}

#[derive(Serialize, Debug)]
pub struct GameActionResponse {
    pub game_id: String,
    pub action: ActionTypeEvent,
    pub objects: HashMap<String, HashMap<String, usize>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GameStateResult {
    pub action_point_updates: Vec<(String, String, u32)>,
    pub players_alive_dead: Option<PlayersAliveDead>,
}

pub const TURN_TIME_SECS: u64 = 2;
pub const MAX_PLAYERS: u16 = 13;
pub const BOARD_SIZE: u16 = 10;
pub const INIT_RANGE: usize = 2;
pub const INIT_ACTION_POINTS: u32 = 1;
pub const INIT_LIVES: u32 = 3;

// non user-configurable parameters
pub const MOVE_COST: u32 = 1;
pub const ATTACK_LIVES_EFFECT: u32 = 1;
pub const ATTACK_COST: u32 = 1;
pub const RANGE_UPGRADE_COST: u32 = 3;
pub const HEAL_COST: u32 = 3;

impl GameConfig {
    pub fn new() -> GameConfig {
        GameConfig {
            init_range: INIT_RANGE,
            max_players: MAX_PLAYERS,
            init_action_points: INIT_ACTION_POINTS,
            init_lives: INIT_LIVES,
            init_pos: InitPosConfig::Random,
            turn_time_secs: TURN_TIME_SECS,
        }
    }
}

impl Game {
    pub fn new(game_id: String, size: u16, rnd: ThreadRng) -> Game {
        Game {
            phase: GamePhase::Init,
            game_id,
            host_user_id: None,
            players: HashMap::new(),
            players_alive_dead: PlayersAliveDead::new(),
            board: Board::new(size as usize),
            object_board: Board::new(size as usize),
            turn_end_unix: 0,
            config: GameConfig::new(),
            rnd,
            curse_election: Election::new("cursings".to_string()),
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
            // fail if game is full
            if self.players.len() == usize::from(self.config.max_players) {
                return Err("game is at max capacity".to_string());
            }
            let mut player = Player::new(user_id.clone(), self.game_id.clone());
            // set loadout
            player.lives = self.config.init_lives;
            player.action_points = self.config.init_action_points;
            player.range = self.config.init_range;
            if matches!(self.config.init_pos, InitPosConfig::Random) {
                // randomly position player
                Game::randomly_position(
                    &mut player,
                    &self.coord_die(),
                    &mut self.rnd,
                    &mut self.board,
                );
            }
            // insert player
            self.players.insert(user_id.clone(), player);
            self.players_alive_dead.set_alive(&user_id);
            return Ok(InsertPlayerResult::Joined);
        }
        return Err("game cannot be joined".to_owned());
    }

    /// set player's position randomly
    pub fn randomly_position(
        player: &mut Player,
        die: &Uniform<usize>,
        rnd: &mut ThreadRng,
        board: &mut Board<String>,
    ) {
        // remove player from current position
        board.map.remove(&player.pos.key());
        let mut res = false;
        while res == false {
            let x = die.sample(rnd);
            let y = die.sample(rnd);
            let pos = Pos { x, y };
            if !board.map.contains_key(&pos.key()) {
                board.map.insert(pos.key(), player.user_id.clone());
                player.pos = pos;
                res = true;
            }
        }
    }

    pub fn coord_die(&self) -> Uniform<usize> {
        Uniform::from(0..self.board.size)
    }

    pub fn configure(
        &mut self,
        conf: &ConfigGameOp,
    ) -> Result<Option<HashMap<String, String>>, String> {
        if !matches!(self.phase, GamePhase::Init) {
            return Err("configuration must be during initialisation".to_string());
        }
        match conf.clone() {
            ConfigGameOp::TurnTimeSecs(v) => {
                if v < 10 {
                    return Err("minimum of 10 seconds is required".to_string());
                }
                if v > 60 * 60 * 24 {
                    return Err("maximum of 24 hours is required".to_string());
                }
                self.config.turn_time_secs = v;
            }
            ConfigGameOp::MaxPlayers(v) => {
                if self.board.size * self.board.size < v.into() {
                    return Err(format!(
                        "{} players won't fit in a {} by {} board",
                        v, self.board.size, self.board.size,
                    ));
                }
                if self.players.len() > v.into() {
                    return Err("Cannot set max players below current player count".to_string());
                }
                self.config.max_players = v;
            }
            ConfigGameOp::BoardSize(v) => {
                if usize::from(self.config.max_players) > v * v {
                    return Err(format!(
                        "{} players won't fit in a {} by {} board",
                        self.config.max_players, v, v,
                    ));
                }
                self.board.size = v;
            }
            ConfigGameOp::InitActPts(v) => {
                for player in self.players.values_mut() {
                    player.action_points = v;
                }
                self.config.init_action_points = v;
            }
            ConfigGameOp::InitLives(v) => {
                for player in self.players.values_mut() {
                    player.lives = v;
                }
                self.config.init_lives = v;
            }
            ConfigGameOp::InitRange(v) => {
                for player in self.players.values_mut() {
                    player.range = v;
                }
                self.config.init_range = v;
            }
            ConfigGameOp::InitPos(v) => {
                self.config.init_pos = v.clone();
                if let InitPosConfig::Random = v {
                    let mut res: HashMap<String, String> = HashMap::new();
                    let die = self.coord_die();
                    for player in self.players.values_mut() {
                        let pos = player.pos.clone();
                        Game::randomly_position(player, &die, &mut self.rnd, &mut self.board);
                        if pos != player.pos {
                            res.insert(pos.key(), player.user_id.clone());
                        }
                    }
                    if !res.is_empty() {
                        return Ok(Some(res));
                    }
                }
            }
        };
        Ok(None)
    }

    pub fn generate_heart_spawn(&mut self) -> (u64, ActionTypeEvent, String) {
        // get turn end time
        // generate duration between now and then
        // generate random move to position on board
        // return random time and position
        let end = self.config.turn_time_secs;
        let time_die = Uniform::from(0..end);
        let time = time_die.sample(&mut self.rnd);
        let die = self.coord_die();
        let x = die.sample(&mut self.rnd);
        let y = die.sample(&mut self.rnd);
        let pos = Pos { x, y };
        let action = ActionTypeEvent::Move(MoveEvent {
            from: Pos {
                x: usize::MAX,
                y: usize::MAX,
            },
            to: pos,
        });
        (time, action, "heart".to_string())
    }

    pub fn start_game(&mut self) -> Result<(), String> {
        if !matches!(self.phase, GamePhase::Init) {
            return Err("game already started".to_owned());
        }
        if self.players.len() < 4 {
            return Err("4 or more players required to start a game".to_owned());
        }
        let die = self.coord_die();
        for player in self.players.values_mut() {
            if player.pos.x >= self.board.size || player.pos.y >= self.board.size {
                Game::randomly_position(player, &die, &mut self.rnd, &mut self.board);
            }
        }
        self.curse_election.candidates = self.players_alive_dead.alive.clone();
        self.phase = GamePhase::InProg;
        self.turn_end_unix = from_now(self.config.turn_time_secs);
        Ok(())
    }

    /// return true if game phase is ::End
    pub fn is_end_phase(&mut self) -> bool {
        matches!(self.phase, GamePhase::End)
    }

    /// error if game not in progress
    pub fn check_in_prog(&self) -> Result<(), String> {
        if !matches!(self.phase, GamePhase::InProg) {
            return Err("Game not in progress".to_owned());
        }
        Ok(())
    }

    /// redeem curse election results, replenish non-cursed living players
    pub fn replenish(&mut self) -> Result<Vec<(String, String, u32)>, String> {
        self.check_in_prog()?;
        let cursed = self.curse_election.redeem();
        let mut action_point_updates: Vec<(String, String, u32)> = Vec::new();
        for player in self.players.values_mut() {
            if !cursed.contains(&player.user_id) {
                if player.lives > 0 {
                    player.action_points += 1;
                }
            }
            action_point_updates.push((
                player.user_id.clone(),
                self.game_id.clone(),
                player.action_points,
            ));
        }
        self.curse_election.reset();
        self.turn_end_unix = from_now(self.config.turn_time_secs);
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
            // TODO change to PRESET? 3 players for jury to vote on 1,2,3
            if self.players_alive_dead.alive_len() == 1 {
                self.phase = GamePhase::End;
            }
        }
        Ok(())
    }

    pub fn game_action(
        &mut self,
        action: &ActionTypeEvent,
        obj: &str,
    ) -> Result<
        (
            HashMap<String, HashMap<String, usize>>,
            Option<(PlayerActionResponse, GameStateResult)>,
        ),
        String,
    > {
        let mut player_effects: Option<(PlayerActionResponse, GameStateResult)> = None;
        match &action {
            ActionTypeEvent::Move(m) => {
                // <VALIDATE>
                // move must be in bounds
                self.object_board.in_bounds(&m.to, false)?;
                // <EXECUTE>
                // remove from old position (out of bounds m.from positions are ignored)
                self.object_board.map.get_mut(&m.from.key()).and_then(|v| {
                    v.get_mut(obj).and_then(|f| {
                        // counter for object type is pre-existing
                        *f -= 1;
                        Some(())
                    });
                    Some(())
                });
                // check if space occupied by player already
                if let Some(player) = self.board.map.get(&m.to.key()) {
                    // player is present on the tile targeted by object move
                    // apply object directly to player, skip modifying board
                    let player = self
                        .players
                        .get_mut(player)
                        .ok_or("player occupying space not found".to_owned())?;

                    // update player, action point updates
                    player_effects = match obj {
                        "heart" => {
                            // is the player alive or dead?
                            let (action, players_alive_dead) = if player.lives < 1 {
                                // player dead
                                // set player as alive
                                self.players_alive_dead.set_alive(&player.user_id);
                                (
                                    ActionTypeEvent::Revive(ReviveAction {
                                        target_user_id: player.user_id.to_owned(),
                                    }),
                                    Some(self.players_alive_dead.clone()),
                                )
                            } else {
                                // player alive
                                // player life updated only
                                (
                                    ActionTypeEvent::Heal(HealAction {
                                        point_cost: HEAL_COST,
                                    }),
                                    None,
                                )
                            };
                            let par = PlayerActionResponse {
                                game_id: self.game_id.clone(),
                                user_id: player.user_id.clone(),
                                phase: self.phase.clone(),
                                action,
                            };
                            let gsr = GameStateResult {
                                action_point_updates: Vec::new(),
                                players_alive_dead,
                            };
                            Ok(Some((par, gsr)))
                        }
                        _ => Err(format!("unknown object {}", obj).to_owned()),
                    }?;
                    // update player lives
                } else {
                    // add object to position
                    // NOTE: emptied hashmaps and objects should be kept for updating existing objects on the client
                    self.object_board
                        .map
                        .get_mut(&m.to.key())
                        .and_then(|v| {
                            // hashmap for position already exists
                            v.get_mut(obj)
                                .and_then(|o| {
                                    // counter for object type already exists
                                    if *o > 0 {
                                        *o += 1;
                                    }
                                    Some(())
                                })
                                .or_else(|| {
                                    // counter for object type does not exist
                                    v.insert(obj.to_owned(), 1);
                                    Some(())
                                });
                            Some(())
                        })
                        .or_else(|| {
                            // hashmap for position does not exist
                            let mut v = HashMap::new();
                            // counter for object type does not exist
                            v.insert(obj.to_owned(), 1);
                            // hashmap with object counter now exists
                            self.object_board.map.insert(m.to.key(), v);
                            Some(())
                        });
                }
            }
            _ => {
                return Err("action type event not implemented!".to_owned());
            }
        }
        Ok((self.object_board.map.clone(), player_effects))
    }

    /// validate a player action then execute the required changes to the game
    /// `player_flux` is a copy of the acting player to be applied at fn end
    /// `target_flux` is a copy of the target player to be applied at match arm end
    /// returns Vec<String> to list players that need action_point updates
    pub fn player_action(
        &mut self,
        user_id: &str,
        action: &ActionType,
    ) -> Result<(PlayerActionResponse, GameStateResult), String> {
        if matches!(self.phase, GamePhase::End) {
            return Err("game over".to_string());
        }
        let mut action_point_updates: Vec<(String, String, u32)> = Vec::new();
        let mut players_alive_dead = None;
        let mut player_flux = self.clone_player(user_id)?;
        let action: ActionTypeEvent = match action {
            ActionType::Move(walk) => {
                // <VALIDATE>
                // validate bounds
                // validate tile occupation
                self.board.in_bounds(&walk.pos, true)?;
                if matches!(self.phase, GamePhase::InProg) {
                    // validate lives
                    player_flux.is_alive()?;
                    // validate action points
                    // validate player range ability
                    // validate move distance against range ability
                    player_flux.moveable_in_prog(&walk.pos)?;
                    // <EXECUTE>
                    player_flux.action_points -= MOVE_COST;
                } else if matches!(self.phase, GamePhase::Init) {
                    if !matches!(self.config.init_pos, InitPosConfig::Manual) {
                        return Err("manual initial positioning must be enabled".to_string());
                    }
                }
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
                player_flux.is_alive()?;

                let mut target_flux = self.clone_player(&attack.target_user_id)?;
                // validate target is alive
                target_flux.is_alive()?;
                // has action points
                // player in range of target
                player_flux.moveable_in_prog(&target_flux.pos)?;
                // action's lives effect is -1
                if attack.lives_effect != ATTACK_LIVES_EFFECT {
                    return Err("attacking must take 1 life :'(".to_string());
                }
                // remove player action point
                player_flux.action_points -= ATTACK_COST;
                // remove target life
                target_flux.lives -= ATTACK_LIVES_EFFECT;
                // if target life is 0 then check number of players alive
                // if players alive is 1 then end game
                if target_flux.lives == 0 {
                    self.players_alive_dead.set_dead(&target_flux.user_id);
                    self.curse_election.convert_candidate(&target_flux.user_id);
                    // transfer remaining action points to attacker
                    player_flux.action_points += target_flux.action_points;
                    target_flux.action_points = 0;
                    action_point_updates.push((
                        target_flux.user_id.clone(),
                        self.game_id.clone(),
                        0,
                    ));
                    if self.players_alive_dead.alive_len() == 1 {
                        self.phase = GamePhase::End;
                    }
                    players_alive_dead = Some(self.players_alive_dead.clone());
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
                // game must be in progress
                self.check_in_prog()?;
                // player is not targeting themselves
                if user_id == give.target_user_id {
                    return Err("this is a futile endeavour".to_string());
                }
                // player has lives
                player_flux.is_alive()?;
                // target has lives
                let mut target_flux = self.clone_player(&give.target_user_id)?;
                target_flux.is_alive()?;
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
                // game must be in progress
                self.check_in_prog()?;
                // player has lives
                player_flux.is_alive()?;
                // player has enough action points and correct cost estimate
                if player_flux.action_points < RANGE_UPGRADE_COST
                    || range_upgrade.point_cost != RANGE_UPGRADE_COST
                {
                    return Err(format!(
                        "{} action points required to upgrade range",
                        RANGE_UPGRADE_COST
                    ));
                }
                // <EXECUTE>
                // exchange action points for range
                player_flux.action_points -= RANGE_UPGRADE_COST;
                player_flux.range += 1;
                // return action event
                ActionTypeEvent::RangeUpgrade(RangeUpgradeAction {
                    point_cost: range_upgrade.point_cost,
                })
            }
            ActionType::Heal(heal) => {
                // <VALIDATE>
                // game must be in progress
                self.check_in_prog()?;
                // player has lives
                player_flux.is_alive()?;
                if player_flux.action_points < HEAL_COST || heal.point_cost != HEAL_COST {
                    return Err(format!(
                        "{} action points required to heal",
                        RANGE_UPGRADE_COST
                    ));
                }
                // <EXECUTE>
                // exchange action points for life
                player_flux.action_points -= HEAL_COST;
                player_flux.lives += 1;
                // return action event
                ActionTypeEvent::Heal(HealAction {
                    point_cost: heal.point_cost,
                })
            }
            ActionType::Revive(rev) => {
                // <VALIDATE>
                // game must be in progress
                let ReviveAction { target_user_id } = rev;
                self.check_in_prog()?;
                if user_id == target_user_id {
                    return Err("you can't revive yourself".to_string());
                }
                // player has lives
                player_flux.is_alive()?;
                let mut target_flux = self.clone_player(&target_user_id)?;
                // target must be dead
                target_flux.is_dead()?;
                // <EXECUTE>
                // apply target_copy
                player_flux.lives -= 1;
                target_flux.lives += 1;
                self.players_alive_dead.set_alive(&target_flux.user_id);
                self.curse_election.convert_voter(&target_flux.user_id)?;
                if player_flux.lives < 1 {
                    self.players_alive_dead.set_dead(&player_flux.user_id);
                    self.curse_election.convert_candidate(&player_flux.user_id);
                }
                players_alive_dead = Some(self.players_alive_dead.clone());
                self.players
                    .insert(target_flux.user_id.clone(), target_flux);
                ActionTypeEvent::Revive({
                    ReviveAction {
                        target_user_id: target_user_id.to_string(),
                    }
                })
            }
            ActionType::Curse(curse) => {
                let CurseAction { target_user_id } = curse;
                // <VALIDATE>
                self.check_in_prog()?;

                player_flux.is_dead()?;
                let res = if let Some(target_user_id) = target_user_id {
                    let target_flux = self.clone_player(&target_user_id)?;
                    target_flux.is_alive()?;
                    // <EXECUTE>
                    self.curse_election.vote(&user_id, &target_user_id)?;
                    ActionTypeEvent::Curse(CurseAction {
                        target_user_id: Some(target_user_id.to_string()),
                    })
                } else {
                    self.curse_election.remove_vote(user_id)?;
                    ActionTypeEvent::Curse(CurseAction {
                        target_user_id: None,
                    })
                };
                res
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
            PlayerActionResponse {
                game_id: self.game_id.clone(),
                user_id: user_id.to_string(),
                phase: self.phase.clone(),
                action,
            },
            GameStateResult {
                action_point_updates,
                players_alive_dead,
            },
        ))
    }

    pub fn get_player_action(&self, player_id: &str) -> PlayerActionResponse {
        // TODO match arm for all types of ActionTypeEvent
        PlayerActionResponse {
            action: ActionTypeEvent::Curse(CurseAction {
                target_user_id: self.curse_election.get_voter_vote(player_id),
            }),
            user_id: player_id.to_string(),
            game_id: self.game_id.to_string(),
            phase: self.phase.clone(),
        }
    }
}

fn now() -> u64 {
    let start = SystemTime::now();
    start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

fn from_now(to_secs: u64) -> u64 {
    let since_the_epoch = now();
    since_the_epoch + to_secs
}
