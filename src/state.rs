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

use graph::Node;
use map::Map;
use square::SquareGrid;
use std::sync::Arc;
use rand::{Rng, SeedableRng, XorShiftRng};
use xorshift::XorShift128Plus;

use std::iter::repeat;

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
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Player(pub usize);

/// The state of a node that is occupied by some player.
#[derive(Debug, Clone)]
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
            match index_mut_pair(&mut self.nodes, from_index, to_index) {
                // We shouldn't have generated a pair for an empty source, and
                // when we clear a node we're supposed to remove pairs from
                // `outflow` that originate there.
                (&mut None, _) => panic!("outflow from empty node"),

                // Source has no goop. No effect.
                (&mut Some(Occupied { goop: 0, .. }), _) => (),

                // Goop flowing into an unoccupied node. New player claims ownership.
                (&mut Some(Occupied { player, ref mut goop, .. }),
                 &mut ref mut to @ None) => {
                    *goop -= 1;
                    *to = Some(Occupied { player, outflows: vec![], goop: 1 });
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
                    // Since all outflow from `to` is cancelled, remove any
                    // pending pairs from `outflow_list`.
                    outflow_list.retain(|&(from, _)| from != to_index);
                },

                // Goop flowing into an occupied node, succeeds in clearing it.
                (&mut Some(Occupied { player, goop: ref mut from_goop, .. }),
                 &mut ref mut to) => {
                    *from_goop -= 1;
                    *to = Some(Occupied { player, outflows: vec![], goop: 0 });
                    // Since all outflow from `to` is cancelled, remove any
                    // pending pairs from `outflow_list`.
                    outflow_list.retain(|&(from, _)| from != to_index);
                }
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
    pub fn take_action(&mut self, action: Action) {
        println!("take_action({:?})", action);
        match action {
            Action::ToggleOutflow { player, from, to } => {
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

/// Actions that can be taken on a `State`.
#[derive(Debug)]
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

