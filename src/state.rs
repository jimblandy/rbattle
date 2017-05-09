//! Module for the dynamic state of the game.
//!
//! This module defines the `State` type and its entourage. The data needed to
//! run an RBattle game is split into three categories:
//!
//! - `State` holds all the significant varying state of a game. What changes as the game proceeds
//!   are the amount of goop in each node, and the outflows the user has chosen;
//!   these are the `State`.
//!
//! - The positions of nodes and their boundaries and the locations of goop
//!   sources are all fixed, and are part of the `Map`.
//!
//! - Interface elements enter hover and active states as the user mouses
//!   around, but those states are ephemeral; they are part of the `Mouse` type.
//!   When the user actually completes an interaction with an interface element,
//!   only then is the `State` affected.

use graph::{Node, Graph};
use map::Map;
use square::SquareGrid;
use xorshift::XorShift128Plus;

use rand::Rng;

use std::hash::{Hash, Hasher};
use std::iter::repeat;
use std::sync::Arc;

/// The complete state of an RBattle game board.
#[derive(Clone)]
pub struct State {
    /// The map being played on.
    pub map: Arc<Map>,

    /// Which nodes are occupied, and which are vacant. Indexed by node id.
    pub nodes: Vec<Option<Occupied>>,

    /// The random number generator used to drive the goop flow algorithm.
    rng: XorShift128Plus
}

/// A player id number.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Player(pub usize);

/// The state of a node that is occupied by some player.
#[derive(Clone, Debug, Hash, PartialEq, Serialize, Deserialize)]
pub struct Occupied {
    /// The player who controls this node.
    pub player: Player,

    /// Which neighbors of this node it sends goop out to.
    pub outflows: Vec<Node>,

    /// How much goop this node holds. Ranges from 0 to MAX_GOOP.
    pub goop: usize,
}

pub const MAX_GOOP: usize = 15;

/// Return a pair of mutable references to the `i`'th and `j`'th elements of
/// `slice`, where `i != j`.
fn index_mut_pair<T>(slice: &mut [T], i: usize, j: usize) -> (&mut T, &mut T) {
    if i < j {
        let (left, right) = slice.split_at_mut(j);
        (&mut left[i], &mut right[0])
    } else if j < i {
        let (left, right) = slice.split_at_mut(i);
        (&mut right[0], &mut left[j])
    } else {
        panic!("can't borrow two mutable references to the same element");
    }
}

impl State {
    pub fn new(params: GameParameters) -> State {
        let graph = SquareGrid::new(params.board.0, params.board.1);
        let map = Arc::new(Map::new(graph, params.sources, params.colors));

        let mut nodes: Vec<Option<Occupied>> = repeat(None).take(map.graph.nodes()).collect();
        // Ensure that each source is occupied by its player.
        for (player, &source) in map.sources.iter().enumerate() {
            nodes[source] = Some(Occupied {
                player: Player(player),
                outflows: vec![],
                goop: 0
            });
        }

        const SEED: [u64; 2] = [0xcd9d5eaaf04bc9a7, 0x4602cc7098d01ef9];
        State { map, nodes, rng: XorShift128Plus::new(SEED) }
    }

    /// Return a SerializableState that can be used to recreate this state.
    pub fn serializable(&self) -> SerializableState {
        SerializableState {
            map: (*self.map).clone(),
            nodes: self.nodes.clone(),
            rng: self.rng.clone()
        }
    }

    /// Reconstitute a State from a SerializableState. This will not share the
    /// map with the original, but that's just a space optimization; the map is
    /// immutable anyway.
    pub fn from_serializable(ser: SerializableState) -> State {
        State { map: Arc::new(ser.map), nodes: ser.nodes, rng: ser.rng }
    }

    /// Let one unit of goop flow through each outflow.
    ///
    /// There are algorithms for finding the flow through a graph precisely, but
    /// we need something simpler here. We just visit every outgoing edge in a
    /// random order, and propagate a unit of goop if the destination permits
    /// it.
    fn flow(&mut self) {
        // Build a vector of (from, to) pairs.
        let mut outflow_list = Vec::new();
        for node in 0..self.map.graph.nodes() {
            if let &Some(ref occupied) = &self.nodes[node] {
                for &outflow in &occupied.outflows {
                    outflow_list.push((node, outflow))
                }
            }
        }

        // Put the pairs in a random order.
        self.rng.shuffle(&mut outflow_list);

        while let Some((from_index, to_index)) = outflow_list.pop() {
            let (from_node, to_node) = index_mut_pair(&mut self.nodes, from_index, to_index);
            let attacked = simulate_flow(from_node, to_node);

            if attacked {
                // `to_node` is being attacked. Disregard any outflows from it this turn.
                outflow_list.retain(|&(from, _)| from != to_index);
            }
        }
    }

    /// Let sources generate new goop.
    fn generate_goop(&mut self) {
        for &source in &self.map.sources {
            match &mut self.nodes[source] {
                &mut None => panic!("source nodes should always be occupied by someone"),
                &mut Some(Occupied { ref mut goop, .. }) => {
                    if *goop < MAX_GOOP {
                        *goop += 1;
                    }
                }
            }
        }
    }

    /// Advance `self` to the next state.
    pub fn advance(&mut self) {
        self.flow();
        self.generate_goop();
    }

    /// Apply `action` to this state.
    pub fn take_action(&mut self, action: &Action) {
        println!("take_action({:?})", action);
        match action {
            &Action::ToggleOutflow { player, from, to } => {
                match &mut self.nodes[from] {
                    // This node is empty. Don't change it.
                    &mut None => (),

                    // Some other player owns this node. Do nothing.
                    &mut Some(Occupied { player: p, .. }) if p != player => (),

                    // We own this node. Toggle the given outflow.
                    &mut Some(Occupied { ref mut outflows, .. }) => {
                        if outflows.contains(&to) {
                            outflows.retain(|&dest| dest != to);
                        } else {
                            outflows.push(to);
                        }
                    }
                }
            }
        }
    }
}

/// Simulate goop flow from a given cell `from_node` to another cell, `to_node`.
///
/// This only simulates flow in that particular direction;
/// if two neighboring cells are both trying to ooze into each other,
/// then the State::flow() function will helpfully call this twice,
/// once for each direction.
///
/// See the tests to learn the detailed rules of this function's behavior.
///
/// Return true if `from_node` attacked `to_node`—that is, if the two nodes are
/// occupied by two different players, and any goop flowed. (The caller needs to
/// know about this, because in this case it must stop `to_node`'s outflows.)
///
fn simulate_flow(from_node: &mut Option<Occupied>, to_node: &mut Option<Occupied>) -> bool {
    match (from_node, to_node) {
        // We shouldn't have generated a pair for an empty source, and
        // when we clear a node we're supposed to remove pairs from
        // `outflow` that originate there.
        (&mut None, _) => panic!("outflow from empty node"),

        // Source has no goop. No effect.
        (&mut Some(Occupied { goop: 0, .. }), _) => false,

        // Goop flowing into an unoccupied node. New player claims ownership.
        (&mut Some(Occupied { player, ref mut goop, .. }),
         &mut ref mut to @ None) => {
            *goop -= 1;
            *to = Some(Occupied { player, outflows: vec![], goop: 1 });
            false
        },

        // Goop flowing into a node occupied by the same player.
        (&mut Some(Occupied { player: from_player, goop: ref mut from_goop, .. }),
         &mut Some(Occupied { player: to_player,   goop: ref mut to_goop, .. }))
            if from_player == to_player =>
        {
            if *from_goop > 0 && *to_goop < MAX_GOOP {
                *from_goop -= 1;
                *to_goop += 1;
            }
            false
        }

        // Goop flowing into a node occupied by another player, but
        // doesn't clear it. All outflow from destination stopped.
        (&mut Some(Occupied { goop: ref mut from_goop, .. }),
         &mut Some(Occupied { outflows: ref mut to_outflows,
                               goop:     ref mut to_goop, .. }))
            if *to_goop > 1 =>
        {
            *from_goop -= 1;
            *to_goop -= 1;
            to_outflows.clear();
            true
        },

        // Goop flowing into an occupied node, succeeds in clearing it.
        (&mut Some(Occupied { player, goop: ref mut from_goop, .. }),
         &mut Some(ref mut target)) => {
            *from_goop -= 1;
            target.player = player;
            target.outflows.clear();
            target.goop = 1 - target.goop;
            true
        }
    }
}

#[test]
fn test_flow_into_unoccupied_cell() {
    // The kingdom of Florin is invading the kingdom of Guilder and flooding it with goop.
    let mut florin = Some(Occupied { player: Player(1), outflows: vec![2], goop: 15 });
    let mut guilder = None;

    // This isn't considered an attack, since Guilder was completely unoccupied.
    assert_eq!(simulate_flow(&mut florin, &mut guilder), false);

    // One unit of goop flowed. The newly occupied territory now belongs to player 1.
    assert_eq!(florin, Some(Occupied { player: Player(1), outflows: vec![2], goop: 14 }));
    assert_eq!(guilder, Some(Occupied { player: Player(1), outflows: vec![], goop: 1 }));
}

#[test]
fn test_flow_empties_cell() {
    // Same scenario, except this time there is just one unit of goop in Florin.
    let mut florin = Some(Occupied { player: Player(1), outflows: vec![2], goop: 1 });
    let mut guilder = None;

    // As above, this isn't considered an attack.
    assert_eq!(simulate_flow(&mut florin, &mut guilder), false);

    // One unit of goop flowed, leaving Florin emptied of goop (but still
    // considered occupied by player 1).
    assert_eq!(florin, Some(Occupied { player: Player(1), outflows: vec![2], goop: 0 }));
    assert_eq!(guilder, Some(Occupied { player: Player(1), outflows: vec![], goop: 1 }));

    // In the next tick of the game, no more goop flows, because Florin is now empty.
    assert_eq!(simulate_flow(&mut florin, &mut guilder), false);

    // The state after this second simulated step is therefore exactly the same as before.
    assert_eq!(florin, Some(Occupied { player: Player(1), outflows: vec![2], goop: 0 }));
    assert_eq!(guilder, Some(Occupied { player: Player(1), outflows: vec![], goop: 1 }));
}

#[test]
fn test_no_flow_from_empty_cell() {
    // Florin, alas, has ambitions of conquest but no goop to carry them out with.
    let mut florin = Some(Occupied { player: Player(1), outflows: vec![2, 3, 4], goop: 0 });

    // Florin can try to flow into a cell that's never been occupied, but since
    // Florin has no goop, the cell does *not* become occupied.
    let mut zolot = None;
    assert_eq!(simulate_flow(&mut florin, &mut zolot), false);
    assert_eq!(florin, Some(Occupied { player: Player(1), outflows: vec![2, 3, 4], goop: 0 }));
    assert_eq!(zolot, None);

    // Nor can Florin attack a cell occupied by an opponent with goop.
    let mut guilder = Some(Occupied { player: Player(2), outflows: vec![5, 6], goop: 1 });
    // The attempt does not count as an attack, since no goop flowed.
    assert_eq!(simulate_flow(&mut florin, &mut guilder), false);
    assert_eq!(florin, Some(Occupied { player: Player(1), outflows: vec![2, 3, 4], goop: 0 }));
    assert_eq!(guilder, Some(Occupied { player: Player(2), outflows: vec![5, 6], goop: 1 }));

    // Same deal even if Guilder also has no goop.
    guilder = Some(Occupied { player: Player(2), outflows: vec![5, 6], goop: 0 });
    assert_eq!(simulate_flow(&mut florin, &mut guilder), false);
    assert_eq!(florin, Some(Occupied { player: Player(1), outflows: vec![2, 3, 4], goop: 0 }));
    assert_eq!(guilder, Some(Occupied { player: Player(2), outflows: vec![5, 6], goop: 0 }));

    // Nor can Florin reinforce Guilder if they happen to be occupied by the same player.
    guilder = Some(Occupied { player: Player(1), outflows: vec![5, 6], goop: 0 });
    assert_eq!(simulate_flow(&mut florin, &mut guilder), false);
    assert_eq!(florin, Some(Occupied { player: Player(1), outflows: vec![2, 3, 4], goop: 0 }));
    assert_eq!(guilder, Some(Occupied { player: Player(1), outflows: vec![5, 6], goop: 0 }));
}

#[test]
fn test_friendly_flow() {
    // Florin is sending goop into the friendly neighboring province of Krugerrand.
    let mut florin     = Some(Occupied { player: Player(1), outflows: vec![2], goop: 8 });
    let mut krugerrand = Some(Occupied { player: Player(1), outflows: vec![3], goop: 0 });
    // This isn't an attack, since the same player occupies both.
    assert_eq!(simulate_flow(&mut florin, &mut krugerrand), false);
    // A unit of goop actually flowed.
    assert_eq!(florin, Some(Occupied { player: Player(1), outflows: vec![2], goop: 7 }));
    // Krugerrand received the goop, and its outflows are unaffected.
    assert_eq!(krugerrand, Some(Occupied { player: Player(1), outflows: vec![3], goop: 1 }));

    // It works even if the destination already has more goop than Florin.
    krugerrand.as_mut().unwrap().goop = 9;
    assert_eq!(simulate_flow(&mut florin, &mut krugerrand), false);
    assert_eq!(florin.unwrap().goop, 6);
    assert_eq!(krugerrand.unwrap().goop, 10);
}

#[test]
fn test_friendly_flow_empties_cell() {
    // Florin can send its only unit of goop to a neighboring empty or nonempty friendly cell.
    let mut florin = Some(Occupied { player: Player(1), outflows: vec![2, 3], goop: 1 });
    let mut guilder = Some(Occupied { player: Player(1), outflows: vec![4], goop: 0 });
    // This isn't an attack, since the same player occupies both.
    assert_eq!(simulate_flow(&mut florin, &mut guilder), false);
    // A unit of goop actually flowed.
    assert_eq!(florin, Some(Occupied { player: Player(1), outflows: vec![2, 3], goop: 0 }));
    // Guilder received the goop, and its outflows are unaffected.
    assert_eq!(guilder, Some(Occupied { player: Player(1), outflows: vec![4], goop: 1 }));

    // Restore Florin's 1 unit of goop and try again.
    florin.as_mut().unwrap().goop = 1;
    assert_eq!(simulate_flow(&mut florin, &mut guilder), false);
    assert_eq!(florin, Some(Occupied { player: Player(1), outflows: vec![2, 3], goop: 0 }));
    assert_eq!(guilder, Some(Occupied { player: Player(1), outflows: vec![4], goop: 2 }));
}

#[test]
fn test_friendly_flow_max_goop() {
    // Florin can't pump any more goop into its friendly neighbor Pfennig,
    // which has the maximum amount already.
    let mut florin  = Some(Occupied { player: Player(1), outflows: vec![2], goop: 3 });
    let mut pfennig = Some(Occupied { player: Player(1), outflows: vec![4], goop: MAX_GOOP });

    assert_eq!(simulate_flow(&mut florin, &mut pfennig), false);
    assert_eq!(florin, Some(Occupied { player: Player(1), outflows: vec![2], goop: 3 }));
    assert_eq!(pfennig, Some(Occupied { player: Player(1), outflows: vec![4], goop: MAX_GOOP }));
}

#[test]
fn test_attack_empty_cell() {
    // Florin siezes the opportunity to invade Guilder, which is left unguarded.
    let mut florin  = Some(Occupied { player: Player(1), outflows: vec![2], goop: 3 });
    let mut guilder = Some(Occupied { player: Player(2), outflows: vec![1], goop: 0 });

    // This is an attack!
    assert_eq!(simulate_flow(&mut florin, &mut guilder), true);
    // Afterwards, player 1 controls Guilder. Note that Guilder's `.outflows`
    // field is cleared. Since Guilder is being attacked, flow through it is inhibited.
    assert_eq!(florin, Some(Occupied { player: Player(1), outflows: vec![2], goop: 2 }));
    assert_eq!(guilder, Some(Occupied { player: Player(1), outflows: vec![], goop: 1 }));

    // The same thing happens even if Florin invades with its last unit of goop.
    florin.as_mut().unwrap().goop = 1;
    guilder = Some(Occupied { player: Player(2), outflows: vec![1], goop: 0 });
    assert_eq!(simulate_flow(&mut florin, &mut guilder), true);
    assert_eq!(florin, Some(Occupied { player: Player(1), outflows: vec![2], goop: 0 }));
    assert_eq!(guilder, Some(Occupied { player: Player(1), outflows: vec![], goop: 1 }));
}

#[test]
fn test_attack_occupied_cell() {
    // Florin can attack Guilder when both cells have positive amounts of goop.
    let mut florin  = Some(Occupied { player: Player(1), outflows: vec![2], goop: 2 });
    let mut guilder = Some(Occupied { player: Player(2), outflows: vec![1], goop: 2 });

    assert_eq!(simulate_flow(&mut florin, &mut guilder), true);
    // In this case, the outcome is that one unit of Player 1 goop flows into
    // Guilder, *cancelling out* one unit of Player 2 goop. Again, Guilder's
    // `.outflows` field is cleared.
    assert_eq!(florin,  Some(Occupied { player: Player(1), outflows: vec![2], goop: 1 }));
    assert_eq!(guilder, Some(Occupied { player: Player(2), outflows: vec![], goop: 1 }));

    // Now Player 2 quickly clicks on the boundary, populating `.outflows`
    // again, in an attempt to counter-attack.
    guilder.as_mut().unwrap().outflows = vec![1];

    // In the next tick of the game, the same thing happens again. This time,
    // Guilder is reduced to 0 goop, so the attacker (Player 1) is considered
    // victorious and gains control.
    assert_eq!(simulate_flow(&mut florin, &mut guilder), true);
    assert_eq!(florin,  Some(Occupied { player: Player(1), outflows: vec![2], goop: 0 }));
    assert_eq!(guilder, Some(Occupied { player: Player(1), outflows: vec![], goop: 0 }));
}

#[test]
fn test_attack_occupied_cell_losing() {
    // Florin is again attacking Guilder, but this time it's a losing battle.
    let mut florin  = Some(Occupied { player: Player(1), outflows: vec![2], goop: 1 });
    let mut guilder = Some(Occupied { player: Player(2), outflows: vec![1], goop: MAX_GOOP });

    // This still counts as an attack, and Guilder's outflows are still inhibited.
    assert_eq!(simulate_flow(&mut florin, &mut guilder), true);
    assert_eq!(florin,  Some(Occupied { player: Player(1), outflows: vec![2], goop: 0 }));
    assert_eq!(guilder, Some(Occupied { player: Player(2), outflows: vec![], goop: MAX_GOOP - 1 }));
}

#[test]
#[should_panic]
fn test_flow_from_unoccupied_cell() {
    // This should never happen, because the simulator won't pass an unoccupied
    // cell to simulate_flow().
    let mut zolot = None;
    let mut zorkmid = None;
    simulate_flow(&mut zolot, &mut zorkmid);
}

/// Actions that can be taken on a `State`.
#[derive(Clone, Debug)]
pub enum Action {
    /// The `player` has requested to toggle the outflow
    /// from `from` to `to`.
    ToggleOutflow { player: Player, from: Node, to: Node },
}

/// A set of parameters that can be used to initialize a game.
pub struct GameParameters {
    /// The dimensions of the board.
    pub board: (usize, usize),

    /// The position of the sources on the board. The number of players is the
    /// length of this vector.
    pub sources: Vec<Node>,

    /// The color assigned to each player, as an RGB triplet. This must be the
    /// same length as `sources`.
    pub colors: Vec<(u8, u8, u8)>
}


/// Hashing a state includes everything but the Map.
impl Hash for State {
    fn hash<H>(&self, state: &mut H)
        where H: Hasher
    {
        self.nodes.hash(state);
        self.rng.hash(state);
    }
}

#[derive(Serialize, Deserialize)]
pub struct SerializableState {
    map: Map,
    nodes: Vec<Option<Occupied>>,
    rng: XorShift128Plus
}
