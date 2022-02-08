use std::{
    cmp::Ordering,
    collections::{BTreeSet, HashMap, HashSet},
};

use crate::ovec;

use std::iter::FromIterator;

fn hashset(v: Vec<String>) -> HashSet<String> {
    HashSet::from_iter(v)
}

#[derive(Debug, Clone, Hash, Eq)]
/// preferential voting ballet where votes are ordered from 1st pref in 0th entry onwards
pub struct PrefBallot {
    prefs: Vec<String>,
    /// voter ID
    voter: String,
}

// order by length of preferences and then voter (voter name is unique in practice)
impl Ord for PrefBallot {
    fn cmp(&self, other: &Self) -> Ordering {
        let c = Ord::cmp(&self.prefs.len(), &other.prefs.len());
        if c == Ordering::Equal {
            return Ord::cmp(&self.voter, &other.voter);
        };
        c
    }
}
impl PartialOrd for PrefBallot {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(Ord::cmp(&self.prefs.len(), &other.prefs.len()))
    }
}
impl PartialEq for PrefBallot {
    fn eq(&self, other: &Self) -> bool {
        Ord::cmp(&self, &other) == Ordering::Equal
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct AllocBallot {
    ballot: PrefBallot,
    allocated: String,
}
type VoteCount = HashMap<String, HashSet<AllocBallot>>;
// TODO implement flag for single voting / preferential voting
// - prevent removing candidates for preferential voting
// - limit to 1 preference for single voting
#[derive(Debug, Clone)]
pub struct Election {
    /// name of election
    name: String,
    /// collection of valid candidates
    candidates: HashSet<String>,
    /// collection of valid voters
    voters: HashSet<String>,
    /// tally of votes for each candidate indexed by candidate ID
    vote_count: VoteCount,
    /// initial tally of votes before applying preferential voting,
    init_vote_count: Option<VoteCount>,
    /// current ballot of each voter indexed by voter ID
    voter_ballots: HashMap<String, AllocBallot>,
    /// whether the election is open to new vote submissions
    open: bool,
    /// longest vector of preferences
    ballots_ordered: BTreeSet<PrefBallot>,
}

impl Election {
    pub fn new(name: &str) -> Election {
        Election {
            name: name.to_owned(),
            candidates: HashSet::new(),
            voters: HashSet::new(),
            vote_count: HashMap::new(),
            init_vote_count: None,
            voter_ballots: HashMap::new(),
            open: true,
            ballots_ordered: BTreeSet::new(),
        }
    }

    pub fn set_candidates(&mut self, candidates: HashSet<String>) {
        self.candidates = candidates;
    }

    pub fn set_voters(&mut self, voters: HashSet<String>) {
        self.voters = voters;
    }

    pub fn check_open(&self) -> Result<(), String> {
        if !self.open {
            return Err(format!("{} is closed to new votes", self.name).to_owned());
        }
        Ok(())
    }

    pub fn check_voter_id(&self, voter_id: &str) -> Result<(), String> {
        if !self.voters.contains(voter_id) {
            return Err(format!("{} is not a voter in {}", voter_id, self.name).into());
        }
        Ok(())
    }

    pub fn check_candidate_id(&self, candidate_id: &str) -> Result<(), String> {
        if !self.candidates.contains(candidate_id) {
            return Err(format!("{} is not a candidate in {}", candidate_id, self.name).into());
        }
        Ok(())
    }

    /// remove ballot of voter from vote_count and voter_ballot
    pub fn remove_ballot(&mut self, voter_id: &str) -> Result<(), String> {
        self.check_open()?;
        self.check_voter_id(voter_id)?;
        // remove ballot from candidate's vote tally
        if let Some(old_vote) = self.voter_ballots.get(voter_id) {
            self.vote_count.get_mut(&old_vote.allocated).and_then(|v| {
                v.remove(old_vote);
                Some(())
            });
        }
        // remove ballot from voter
        let ballot_op = self.voter_ballots.remove(voter_id);
        // remove ballet from ordered ballots
        if let Some(ballot) = ballot_op {
            self.ballots_ordered.remove(&ballot.ballot);
        }
        Ok(())
    }

    pub fn vote(&mut self, voter_id: &str, prefs: Vec<String>) -> Result<(), String> {
        // <VALIDATE>
        // remove voter's old ballot if it exists (indirectly checks if open, if voter id exists)
        self.remove_ballot(voter_id)?;
        // check prefs length
        if &prefs.len() < &1 || &prefs.len() > &self.candidates.len() {
            return Err("bad ballot preferences".into());
        }
        // candidates must be in candidates
        for candidate_id in &prefs {
            self.check_candidate_id(candidate_id)?;
        }
        // no repeats allowed
        let dupes: HashSet<String> = hashset(prefs.clone());
        if dupes.len() != prefs.len() {
            return Err("ballot contains duplicates".into());
        }

        // <EXECUTE>
        let ballot = PrefBallot {
            prefs: prefs.clone(),
            voter: voter_id.to_owned(),
        };
        // track ballot with highest preferences
        self.ballots_ordered.insert(ballot.clone());
        // create allocated ballot
        let ballot_alloc = AllocBallot {
            allocated: ballot.prefs[0].clone(),
            ballot,
        };

        // insert candidate into vote tally if none exists
        if let None = self.vote_count.get(&ballot_alloc.allocated) {
            self.vote_count
                .insert(ballot_alloc.allocated.to_owned(), HashSet::new());
        }
        if let Some(count) = self.vote_count.get_mut(&ballot_alloc.allocated) {
            count.insert(ballot_alloc.clone());
        }
        // insert voter's vote in voter vote
        self.voter_ballots.insert(voter_id.into(), ballot_alloc);
        Ok(())
    }

    /// remove voter from voters and add to candidates
    pub fn move_voter_to_candidate(&mut self, voter_id: &str) -> Result<(), String> {
        self.check_voter_id(voter_id)?;
        self.remove_ballot(voter_id)?;
        self.voters.remove(voter_id);
        self.candidates.insert(voter_id.into());
        self.vote_count.insert(voter_id.into(), HashSet::new());
        Ok(())
    }

    /// remove candidate from candidates and add to voters
    pub fn move_candidate_to_voter(&mut self, candidate_id: &str) -> Result<(), String> {
        self.check_candidate_id(candidate_id)?;
        self.check_open()?;
        self.candidates.remove(candidate_id);
        // remove ballots for ex-candidate to avoid invalid votes
        let voter_ballots = &mut self.voter_ballots;
        self.vote_count.get(candidate_id).and_then(|f| {
            for v in f {
                voter_ballots.remove(&v.ballot.voter);
            }
            Some(())
        });

        self.vote_count.remove(candidate_id);
        self.voters.insert(candidate_id.into());
        Ok(())
    }

    /// get a voter's vote if any
    pub fn get_voter_ballot(&self, voter_id: &str) -> Option<String> {
        self.voter_ballots
            .get(voter_id)
            .and_then(|f| f.ballot.prefs.iter().next().cloned())
    }

    /// get the candidates with the highest number votes (can be more than 1 candidate with most votes)
    pub fn get_winners(&mut self) -> HashSet<String> {
        // candidates must have at least 1 vote, candidates with empty hashsets are ignored
        let mut best = 1;
        let mut winners: HashSet<String> = HashSet::new();
        for (k, v) in &self.vote_count {
            if v.len() == best {
                winners.insert(k.to_owned());
            } else if v.len() > best {
                winners = HashSet::new();
                winners.insert(k.to_owned());
                best = v.len();
            }
        }
        winners
    }

    /// apply preferential voting candidate votes https://web.archive.org/web/20210313023849/https://aec.gov.au/learn/files/poster-counting-hor-pref-voting.pdf
    /// apply optional based preferential voting process to vote_count
    pub fn apply_preferential_voting(&mut self) -> Result<(), String> {
        self.open = false;
        self.init_vote_count = Some(self.vote_count.clone());
        let half = self.voter_ballots.len() / 2;
        let mut processing = 0;
        let mut pref = 0_usize;
        let max_prefs = self
            .ballots_ordered
            .iter()
            .next_back()
            .ok_or("max length of preferences unavailable")?
            .prefs
            .len();
        while processing > -1 && processing < 10000 {
            processing += 1;
            // 1. find the lowest and highest voted
            let mut max = (0_usize, Vec::new());
            let mut min = (usize::MAX, String::new());
            for (id, votes) in &self.vote_count {
                if votes.len() > max.0 {
                    max = (votes.len(), ovec![id]);
                } else if votes.len() == max.0 {
                    max.1.push(id.clone());
                } else if votes.len() > 0 && votes.len() < min.0 {
                    min = (votes.len(), id.clone());
                }
            }
            // 2. if highest is majority, finish
            // 3.1. if highest preference == nth pref then finish
            // 2.2. if no minimum was found then finish
            if max.0 <= half && pref < max_prefs && min.0 < max.0 {
                // 4. else increment pref, take ballots from min voted and redistribute
                pref += 1;
                // replace candidate votes with empty list
                let min_ballots = self
                    .vote_count
                    .insert(min.1, HashSet::new())
                    .ok_or("min voted candidate not found in vote count?".to_owned())?;
                // move votes from min candidate into ballots next preferences
                for mut ballot in min_ballots {
                    if ballot.ballot.prefs.len() > pref {
                        let vote = &ballot.ballot.prefs[pref];
                        if !self.vote_count.contains_key(vote) {
                            // create set for candidates without votes in any previous round
                            self.vote_count.insert(vote.clone(), HashSet::new());
                        }
                        ballot.allocated = vote.to_owned();
                        self.voter_ballots
                            .insert(ballot.ballot.voter.clone(), ballot.clone());
                        let candidate =
                            self.vote_count.get_mut(vote).ok_or("vote_count vanished")?;
                        candidate.insert(ballot);
                    }
                }
            } else {
                processing = -1;
            }
            // 5. repeat from 1.
        }
        Ok(())
    }

    /// reset votes (keep candidates and voters)
    pub fn reset(&mut self) {
        self.open = true;
        self.vote_count = HashMap::new();
        self.init_vote_count = None;
        self.voter_ballots = HashMap::new();
    }
}

#[cfg(test)]
mod tests {

    use crate::ovec;

    use super::*;

    #[test]
    fn test_curse_election() -> Result<(), String> {
        let mut el = Election::new("test");
        assert_eq!(el.candidates.len(), 0);
        assert_eq!(el.voters.len(), 0);
        assert_eq!(el.vote_count.len(), 0);
        assert_eq!(el.init_vote_count, None);
        assert_eq!(el.voter_ballots.len(), 0);
        assert_eq!(el.open, true);
        assert_eq!(el.ballots_ordered.len(), 0);

        // setting candidates works
        let list = ovec!["a", "b", "c", "d", "e", "f", "g", "h"];
        el.set_candidates(hashset(list));
        assert_eq!(el.candidates.len(), 8);
        assert_eq!(el.voters.len(), 0);

        // voting does not work
        assert_eq!(
            el.vote("a", ovec!["b"]),
            Err("a is not a voter in test".into())
        );

        // no ballot
        assert_eq!(el.get_voter_ballot("a"), None);

        // removing ballot fails
        assert_eq!(
            el.remove_ballot("a"),
            Err("a is not a voter in test".into())
        );

        // no votes means no one wins
        let w = el.get_winners();
        assert_eq!(w, HashSet::new());

        // moving voter to candidate fails
        assert_eq!(
            el.move_voter_to_candidate("a"),
            Err("a is not a voter in test".into())
        );

        // converting candidate to voter works
        el.move_candidate_to_voter("b")?;
        assert!(el.voters.contains("b"));
        assert!(!el.candidates.contains("b"));

        // removing ballot does nothing
        el.remove_ballot("b")?;

        // moving valid voter back to candidate is fine
        el.move_voter_to_candidate("b")?;

        // voting for nonexistant candidate fails
        el.move_candidate_to_voter("b")?;
        assert_eq!(
            el.vote("b", ovec!["xyz"]),
            Err("xyz is not a candidate in test".into())
        );
        // empty vote fails
        assert_eq!(
            el.vote("b", Vec::new()),
            Err("bad ballot preferences".into())
        );

        // there is no vote_count or voter_ballot
        assert_eq!(el.vote_count.len(), 0);
        assert_eq!(el.voter_ballots.len(), 0);

        // valid vote works
        el.vote("b", ovec!["a"])?;

        let vote = AllocBallot {
            allocated: "a".into(),
            ballot: PrefBallot {
                prefs: ovec!["a"],
                voter: "b".into(),
            },
        };
        // a votes are in vote_count
        assert!(el.vote_count.get("a").unwrap().contains(&vote));
        assert!(el.vote_count.get("a").unwrap().len() == 1);
        assert!(el.vote_count.len() == 1);
        assert_eq!(el.vote_count.get("b"), None);
        // b vote is in voter_ballots
        assert_eq!(el.voter_ballots.get("b"), Some(&vote));
        assert_eq!(el.voter_ballots.get("a"), None);
        // a is b's vote
        assert_eq!(el.get_voter_ballot("b"), Some("a".into()));

        // a wins
        assert_eq!(el.get_winners(), hashset(ovec!["a"]));

        // move a from candidates to voters
        el.move_candidate_to_voter("a")?;

        // a's votes are removed, b's ballot is removed
        assert_eq!(el.vote_count.get("a"), None);
        assert_eq!(el.voter_ballots.get("b"), None);

        // no one wins
        assert!(el.get_winners().len() == 0);

        // a votes for c, b votes for d
        el.vote("a", ovec!["c"])?;
        el.vote("b", ovec!["d"])?;

        // c and d win together
        assert_eq!(el.get_winners(), hashset(ovec!["c", "d"]));

        // c moves to voters, vote for d and f, d wins, f loses
        el.move_candidate_to_voter("c")?;
        el.vote("a", ovec!["d"])?;
        el.vote("c", ovec!["f"])?;
        assert_eq!(el.get_winners(), hashset(ovec!["d"]));

        // d moves to voters, f wins with leftover votes
        el.move_candidate_to_voter("d")?;
        assert_eq!(el.get_winners(), hashset(ovec!["f"]));

        // empty
        el.reset();
        assert_eq!(el.vote_count, HashMap::new());
        assert_eq!(el.voter_ballots, HashMap::new());

        // changes vote
        el.vote("a", ovec!["f"])?;
        assert_eq!(el.get_winners(), hashset(ovec!["f"]));
        el.vote("a", ovec!["g"])?;
        assert_eq!(el.get_winners(), hashset(ovec!["g"]));

        Ok(())
    }

    #[test]
    fn test_preferential_voting_basic() -> Result<(), String> {
        let mut el = Election::new("test");

        // setting candidates, voters works
        let cand = ovec!["a", "b", "c"];
        let voters = ovec!["a", "b", "c", "d"];
        el.set_candidates(hashset(cand));
        el.set_voters(hashset(voters));
        dbg!("start 1");
        // single pref minority wins
        el.vote("a", ovec!["a"])?;
        el.apply_preferential_voting()?;

        // voting after applying preferential voting fails
        let e = el.vote("a", ovec!["a"]);
        assert_eq!(e, Err("test is closed to new votes".into()));

        assert_eq!(el.get_winners(), hashset(ovec!["a"]));

        el.reset();
        dbg!("start 2");
        // if two candidates have winning amount of votes, both win
        el.vote("a", ovec!["a"])?;
        el.vote("b", ovec!["b"])?;
        el.vote("c", ovec!["a"])?;

        // cannot have duplicates
        let e = el.vote("d", ovec!["a", "a"]);
        assert_eq!(e, Err("ballot contains duplicates".into()));

        el.vote("d", ovec!["b"])?;

        el.apply_preferential_voting()?;
        assert_eq!(el.get_winners(), hashset(ovec!["a", "b"]));

        Ok(())
    }

    #[test]
    fn test_preferential_voting_random() -> Result<(), String> {
        let mut el = Election::new("test");
        let cand = ovec!["a", "b", "c"];
        let voters = ovec!["a", "b", "c", "d"];
        el.set_candidates(hashset(cand));
        el.set_voters(hashset(voters));
        // if two candidates have minimum amount of votes, one of them is randomly eliminated
        // ballot arrangement
        // a - c, b, a
        // b - a, c
        // c - b, a, c
        // d - c, a, b
        // 1st rnd c: 2, a: 1, b: 1
        // one of a and b is randomly eliminated and their votes redistributed
        // 2nd rnd (a elim) c: 3, b: 1
        //  - c wins majority
        // 2nd rnd (b elim) c: 2, a: 2
        //  - c and a win, no minimum left
        el.vote("a", ovec!["c", "b", "a"])?;
        el.vote("b", ovec!["a", "c"])?;
        el.vote("c", ovec!["b", "a", "c"])?;
        el.vote("d", ovec!["c", "a", "b"])?;
        el.apply_preferential_voting()?;
        let win = el.get_winners();
        assert!(win == hashset(ovec!["a", "c"]) || win == hashset(ovec!["c"]));
        Ok(())
    }

    #[test]
    fn test_preferential_voting_removing_ballots() -> Result<(), String> {
        let mut el = Election::new("test");
        let cand = ovec!["a", "b", "c"];
        let voters = ovec!["a", "b", "c", "d", "e"];
        el.set_candidates(hashset(cand));
        el.set_voters(hashset(voters));
        // testing removing ballot with longest preferences adjusts to second longest ballot
        // ballot arrangement
        // a - a
        // b - c
        // c - b, c
        // d - c, b
        // e - a, b, c
        // 1st round a: 2, b: 1, c: 2
        // 2nd round
        // a wins majority at 3rd round
        el.vote("a", ovec!["a"])?;
        el.vote("b", ovec!["c"])?;
        el.vote("c", ovec!["b", "c"])?;
        el.vote("d", ovec!["c", "b"])?;
        el.vote("e", ovec!["a", "b", "c"])?;

        let max_prefs = el.ballots_ordered.iter().next_back().unwrap().prefs.len();
        assert_eq!(max_prefs, 3);

        //
        el.remove_ballot("e")?;
        let max_prefs = el.ballots_ordered.iter().next_back().unwrap().prefs.len();
        assert_eq!(max_prefs, 2);

        el.apply_preferential_voting()?;
        let w = el.get_winners();
        assert_eq!(w, hashset(ovec!["c"]));

        el.reset();

        // removing 1 of 2 ballots that are longest works
        el.vote("a", ovec!["a"])?;
        el.vote("b", ovec!["c"])?;
        el.vote("c", ovec!["b", "c"])?;
        el.vote("d", ovec!["c", "b", "a"])?;
        el.vote("e", ovec!["a", "b", "c"])?;

        el.remove_ballot("e")?;

        let max_prefs = el.ballots_ordered.iter().next_back().unwrap().prefs.len();
        assert_eq!(max_prefs, 3);

        Ok(())
    }

    #[test]
    fn test_orphaned_ballots_preferential() -> Result<(), String> {
        let mut el = Election::new("test");
        let cands = ovec!["a", "b", "c", "d", "_"];
        // _ cand illustrates preference never used
        let votrs = ovec!["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k"];
        el.set_candidates(hashset(cands));
        el.set_voters(hashset(votrs.clone()));
        // ballot arrangement
        // rnd 1 - a: 4, b: 4, c: 2, d: 1
        //      - d eliminated, c += 1
        // rnd 2 - a: 4, b: 4, c: 3
        //      - c eliminated, d += 1, a += 1, b += 1
        // rnd 3 - a: 5, b: 5, d: 1
        //      - d eliminated, a += 1
        // rnd 4 a: 6 - a wins
        let votes = vec![
            ovec!["a"],
            ovec!["a"],
            ovec!["a"],
            ovec!["a"],
            ovec!["b"],
            ovec!["b"],
            ovec!["b"],
            ovec!["b"],
            ovec!["c", "_", "a"],
            ovec!["c", "_", "d", "a"],
            ovec!["d", "c", "b"],
        ];
        let mut i = 0;
        for v in votes {
            el.vote(&votrs[i], v)?;
            i += 1;
        }
        el.apply_preferential_voting()?;
        assert_eq!(el.get_winners(), hashset(ovec!["a"]));
        Ok(())
    }
}
