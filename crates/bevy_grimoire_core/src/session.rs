//! **Per-turn measurement** (#157, research lens #7–#9).
//!
//! A developer builds a world in turns, so an agent is measured per **turn**, not on a finished
//! world. A [`Session`] is the turns of one scripted conversation: each says what was asked
//! ([`Turn::request`]), whether a good agent acts on it or asks first ([`Turn::expect`]), and the
//! relations ([`crate::relations::Relation`], the same constraint layer a map keeps beside it) the
//! turn must leave true.
//!
//! [`score`] reads what an agent did on each turn ([`Played`]: did it ask, and the placements after
//! it; recorded, never a live model) and reports two failures a finished-world score cannot see:
//!
//! - **Asking wrong, both ways.** An `Ask` turn the agent acted on is [`Ask::Under`]; an `Act` turn it
//!   asked about is [`Ask::Over`]. Asking too often fails as surely as asking too rarely, so both
//!   count.
//! - **Preservation.** After every turn, every earlier turn's relations are checked again. One that
//!   held when the previous turn ended and is broken now is a [`Regression`] of this turn, naming
//!   the turn and the relation. One that never held is reported once, at its own turn, and one
//!   already broken is not counted again until it is mended and broken anew.
//!
//! Engine-free: placements and relations in, a report out.

use serde::{Deserialize, Serialize};

use crate::map::Placed;
use crate::relations::{self, Outcome, Relation, Relations, RELATIONS_VERSION};

/// The format version of a session file.
pub const SESSION_VERSION: u32 = 1;

/// One scripted conversation, turn by turn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub version: u32,
    pub turns: Vec<Turn>,
}

/// What a good agent does with a turn's request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Expect {
    /// The request is clear enough to act on.
    Act,
    /// The request leaves something only the person can settle; a good agent asks first.
    Ask,
}

/// One turn of a [`Session`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Turn {
    /// What the person said.
    pub request: String,
    pub expect: Expect,
    /// What this turn must leave true, and every later turn must keep.
    #[serde(default)]
    pub keeps: Vec<Relation>,
}

/// What an agent did on one turn.
#[derive(Clone, Debug, PartialEq)]
pub struct Played {
    /// It asked the person instead of (or before) acting.
    pub asked: bool,
    /// The map's placements when the turn ended.
    pub placements: Vec<Placed>,
}

/// Whether the agent asked when it should have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ask {
    /// It asked on an `Ask` turn, or acted on an `Act` turn.
    Right,
    /// It asked on an `Act` turn.
    Over,
    /// It acted on an `Ask` turn.
    Under,
}

/// An earlier turn's relation that held when that turn ended and is broken now.
#[derive(Clone, Debug, PartialEq)]
pub struct Regression {
    /// The turn whose relation broke, 0-based.
    pub turn: usize,
    /// The relation's index in that turn's [`Turn::keeps`].
    pub relation: usize,
    /// What the check finds now.
    pub now: Outcome,
}

/// One turn's score.
#[derive(Clone, Debug, PartialEq)]
pub struct TurnScore {
    pub ask: Ask,
    /// This turn's own relations, checked when it ended, in [`Turn::keeps`]' order.
    pub own: Vec<Outcome>,
    /// Earlier turns' relations this turn broke, by turn then relation.
    pub broken: Vec<Regression>,
}

/// A whole session's score.
#[derive(Clone, Debug, PartialEq)]
pub struct Score {
    pub turns: Vec<TurnScore>,
}

impl Score {
    /// Turns where the agent asked and should have acted.
    pub fn over_asked(&self) -> usize {
        self.turns.iter().filter(|t| t.ask == Ask::Over).count()
    }

    /// Turns where the agent acted and should have asked.
    pub fn under_asked(&self) -> usize {
        self.turns.iter().filter(|t| t.ask == Ask::Under).count()
    }

    /// Relations a turn left unmet (or naming a missing piece) when that turn ended.
    pub fn unmet(&self) -> usize {
        self.turns
            .iter()
            .flat_map(|t| &t.own)
            .filter(|o| !matches!(o, Outcome::Met { .. }))
            .count()
    }

    /// Earlier relations a later turn broke.
    pub fn regressions(&self) -> usize {
        self.turns.iter().map(|t| t.broken.len()).sum()
    }

    /// Every turn asked right, met its own relations and broke none of the earlier ones.
    pub fn passed(&self) -> bool {
        self.over_asked() + self.under_asked() + self.unmet() + self.regressions() == 0
    }
}

impl Session {
    /// Read a session file, refusing a version this build does not read and a turn whose relations
    /// the relations file itself would refuse.
    pub fn parse(text: &str) -> Result<Session, String> {
        let s: Session =
            crate::yaml::from_str(text, "session").map_err(|p| format!("session: parse: {}", p.line()))?;
        if s.version != SESSION_VERSION {
            return Err(format!(
                "session: version {} is not the {SESSION_VERSION} this build reads",
                s.version
            ));
        }
        for (i, t) in s.turns.iter().enumerate() {
            let text = as_relations(&t.keeps).to_yaml()?;
            Relations::parse(&text).map_err(|e| format!("session: turn {i}: {e}"))?;
        }
        Ok(s)
    }

    /// The file's text.
    pub fn to_yaml(&self) -> Result<String, String> {
        crate::yaml::to_string(self).map_err(|p| format!("session: write: {}", p.line()))
    }
}

fn as_relations(keeps: &[Relation]) -> Relations {
    Relations {
        version: RELATIONS_VERSION,
        relations: keeps.to_vec(),
    }
}

/// Score what an agent did, one [`Played`] per turn, in order. Refuses a played list that is not
/// one per turn, since a missing turn would read as one that kept everything.
pub fn score(session: &Session, played: &[Played]) -> Result<Score, String> {
    if played.len() != session.turns.len() {
        return Err(format!(
            "session: {} turn(s) played for a session of {}",
            played.len(),
            session.turns.len()
        ));
    }
    let keeps: Vec<Relations> = session.turns.iter().map(|t| as_relations(&t.keeps)).collect();
    // held[j][r]: turn j's relation r was met when the previous turn ended, so a relation is
    // reported broken by the turn that broke it, once, and again only if mended and broken again.
    let mut held: Vec<Vec<bool>> = Vec::with_capacity(keeps.len());
    let mut turns = Vec::with_capacity(keeps.len());
    for (i, (turn, p)) in session.turns.iter().zip(played).enumerate() {
        let ask = match (turn.expect, p.asked) {
            (Expect::Act, true) => Ask::Over,
            (Expect::Ask, false) => Ask::Under,
            _ => Ask::Right,
        };
        // A turn's rows are the map's, yaw in degrees: turned once for every check below (#165).
        let rows = relations::in_radians(&p.placements);
        let own = relations::check(&rows, &keeps[i]);
        let mut broken = Vec::new();
        for (j, earlier) in keeps[..i].iter().enumerate() {
            for (r, now) in relations::check(&rows, earlier).into_iter().enumerate() {
                let met = matches!(now, Outcome::Met { .. });
                if held[j][r] && !met {
                    broken.push(Regression { turn: j, relation: r, now });
                }
                held[j][r] = met;
            }
        }
        held.push(own.iter().map(|o| matches!(o, Outcome::Met { .. })).collect());
        turns.push(TurnScore { ask, own, broken });
    }
    Ok(Score { turns })
}
