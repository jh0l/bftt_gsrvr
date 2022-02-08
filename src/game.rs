use rand::distributions::Uniform;
use rand::prelude::{Distribution, ThreadRng};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::common::{ConfigGameOp, InitPosConfig};
use crate::election::Election;

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
            return Err("insufficient action points".into());
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
            return Err("move out of range".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Board<T> {
    pub map: HashMap<String, T>,
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
            return Err("out of range".into());
        }
        if check_occupied && self.map.get(&pos.key()).is_some() {
            return Err("space is occupied".into());
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
        self.dead.insert(id.into());
        self.alive.remove(id);
    }

    pub fn alive_len(&self) -> usize {
        self.alive.len()
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
    pub board_hearts: Board<u32>,
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
    pos: Pos,
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

#[derive(Deserialize, Serialize, Debug)]
pub struct RedeemTileHearts {
    pos: Pos,
    new_lives: u32,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum RedeemAction {
    TileHearts(RedeemTileHearts),
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
    Redeem(RedeemAction),
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
    Heal(HealAction),
    Revive(ReviveAction),
    Curse(CurseAction),
    Redeem(RedeemAction),
}

#[derive(Serialize, Debug)]
pub struct PlayerResponse {
    user_id: String,
    game_id: String,
    action: ActionTypeEvent,
    phase: GamePhase,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlayerActionResult {
    pub action_point_updates: Vec<(String, String, u32)>,
    pub players_alive_dead: Option<PlayersAliveDead>,
}

pub const TURN_TIME_SECS: u64 = 10;
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
            board_hearts: Board::new(size as usize),
            turn_end_unix: 0,
            config: GameConfig::new(),
            rnd,
            curse_election: Election::new("cursings"),
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
                return Err("game is at max capacity".into());
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
                    &self.board_die(),
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

    pub fn board_die(&self) -> Uniform<usize> {
        Uniform::from(0..self.board.size)
    }

    pub fn configure(
        &mut self,
        conf: &ConfigGameOp,
    ) -> Result<Option<HashMap<String, String>>, String> {
        if !matches!(self.phase, GamePhase::Init) {
            return Err("configuration must be during initialisation".into());
        }
        match conf.clone() {
            ConfigGameOp::TurnTimeSecs(v) => {
                if v < 10 {
                    return Err("minimum of 10 seconds is required".into());
                }
                if v > 60 * 60 * 24 {
                    return Err("maximum of 24 hours is required".into());
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
                    return Err("Cannot set max players below current player count".into());
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
                    let die = self.board_die();
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

    pub fn start_game(&mut self) -> Result<(), String> {
        if !matches!(self.phase, GamePhase::Init) {
            return Err("game already started".to_owned());
        }
        if self.players.len() < 4 {
            return Err("4 or more players required to start a game".to_owned());
        }
        let die = self.board_die();
        for player in self.players.values_mut() {
            if player.pos.x >= self.board.size || player.pos.y >= self.board.size {
                Game::randomly_position(player, &die, &mut self.rnd, &mut self.board);
            }
        }
        self.curse_election
            .set_candidates(self.players_alive_dead.alive.clone());
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

    /// generate new item spawn time in milliseconds based on turn_time_secs
    pub fn new_item_spawn_time_ms(&mut self) -> u64 {
        let bound = self.config.turn_time_secs * 1000;
        let die = Uniform::from(0..bound);
        die.sample(&mut self.rnd)
    }

    /// insert an action point in ap_board
    pub fn spawn_tile_heart(&mut self) -> (Pos, u32) {
        // random positin
        let die = self.board_die();
        let x = die.sample(&mut self.rnd);
        let y = die.sample(&mut self.rnd);
        let pos = Pos { x, y };
        // try adding to existing position
        let v = self
            .board_hearts
            .map
            .get_mut(&pos.key())
            .and_then(|t| {
                *t += 1;
                Some(t.clone())
            })
            .or_else(|| {
                // if position is non existant, insert new position
                self.board_hearts.map.insert(pos.key(), 1);
                Some(1)
            })
            .unwrap();
        (Pos { x, y }, v)
    }

    /// redeem curse election results, replenish living players
    pub fn replenish(
        &mut self,
        cursed: &HashSet<String>,
    ) -> Result<Vec<(String, String, u32)>, String> {
        self.check_in_prog()?;
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

    /// validate a player action then execute the required changes to the game
    /// `player_flux` is a copy of the acting player to be applied at fn end
    /// `target_flux` is a copy of the target player to be applied at match arm end
    /// returns Vec<String> to list players that need action_point updates
    pub fn player_action(
        &mut self,
        user_id: &str,
        action: &ActionType,
    ) -> Result<(PlayerResponse, PlayerActionResult), String> {
        if matches!(self.phase, GamePhase::End) {
            return Err("game over".into());
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
                        return Err("manual initial positioning must be enabled".into());
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
                    .insert(player_flux.pos.key().clone(), user_id.into());
                action_event
            }
            ActionType::Attack(attack) => {
                // <VALIDATE>
                self.check_in_prog()?;
                // validate player is not targeting themselves
                if user_id == attack.target_user_id {
                    return Err("Stop hurting yourself".into());
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
                    return Err("attacking must take 1 life :'(".into());
                }
                // remove player action point
                player_flux.action_points -= ATTACK_COST;
                // remove target life
                target_flux.lives -= ATTACK_LIVES_EFFECT;
                // if target life is 0 then check number of players alive
                // if players alive is 1 then end game
                if target_flux.lives == 0 {
                    self.players_alive_dead.set_dead(&target_flux.user_id);
                    self.curse_election
                        .move_candidate_to_voter(&target_flux.user_id)?;
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
                    return Err("this is a futile endeavour".into());
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
                    return Err("you can't revive yourself".into());
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
                self.curse_election
                    .move_voter_to_candidate(&target_flux.user_id)?;
                if player_flux.lives < 1 {
                    self.players_alive_dead.set_dead(&player_flux.user_id);
                    self.curse_election
                        .move_candidate_to_voter(&player_flux.user_id)?;
                }
                // queue player_alive_dead update
                players_alive_dead = Some(self.players_alive_dead.clone());
                // apply target_flux change
                self.players
                    .insert(target_flux.user_id.clone(), target_flux);
                // return action event
                ActionTypeEvent::Revive({
                    ReviveAction {
                        target_user_id: target_user_id.into(),
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
                    self.curse_election
                        .vote(&user_id, vec![target_user_id.clone()])?;
                    ActionTypeEvent::Curse(CurseAction {
                        target_user_id: Some(target_user_id.into()),
                    })
                } else {
                    self.curse_election.remove_ballot(user_id)?;
                    ActionTypeEvent::Curse(CurseAction {
                        target_user_id: None,
                    })
                };
                // return action event
                res
            }
            ActionType::Redeem(redeem) => {
                // <VALIDATE>
                self.check_in_prog()?;

                // dead players can redeem things
                // <EXECUTE>
                let res = match redeem {
                    RedeemAction::TileHearts(tile_hearts) => {
                        let RedeemTileHearts { pos, new_lives: _ } = tile_hearts;
                        // check player in position
                        if pos != &player_flux.pos {
                            return Err("player not in position".to_owned());
                        }
                        // check position has hearts
                        let board_lives = self
                            .board_hearts
                            .map
                            .get_mut(&pos.key())
                            .ok_or("position heartless".to_owned())?;
                        // add hearts to player
                        player_flux.lives += *board_lives;
                        *board_lives = 0;
                        if player_flux.lives == 1 {
                            // execute revive
                            self.players_alive_dead.set_alive(&player_flux.user_id);
                            self.curse_election
                                .move_voter_to_candidate(&player_flux.user_id)?;
                            players_alive_dead = Some(self.players_alive_dead.clone());
                        }
                        ActionTypeEvent::Redeem(RedeemAction::TileHearts(RedeemTileHearts {
                            new_lives: player_flux.lives,
                            pos: pos.clone(),
                        }))
                    }
                };
                // return action event
                res
            }
        };
        // add player to action point update list
        action_point_updates.push((
            user_id.into(),
            self.game_id.clone(),
            player_flux.action_points,
        ));
        // apply player copy
        self.players
            .insert(player_flux.user_id.clone(), player_flux);
        Ok((
            PlayerResponse {
                game_id: self.game_id.clone(),
                user_id: user_id.into(),
                phase: self.phase.clone(),
                action,
            },
            PlayerActionResult {
                action_point_updates,
                players_alive_dead,
            },
        ))
    }

    pub fn get_player_action(&self, player_id: &str) -> PlayerResponse {
        // TODO match arm for all types of ActionTypeEvent
        PlayerResponse {
            action: ActionTypeEvent::Curse(CurseAction {
                target_user_id: self.curse_election.get_voter_ballot(player_id),
            }),
            user_id: player_id.into(),
            game_id: self.game_id.to_owned(),
            phase: self.phase.clone(),
        }
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
