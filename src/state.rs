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
use visible_graph::VisibleGraph;
use map::{Map, Player};
use std::rc::Rc;
use rand::{Rng, SeedableRng, XorShiftRng};

/// The complete state of an RBattle game board.
#[derive(Clone)]
pub struct State<G: VisibleGraph> {
    /// The map being played on.
    pub map: Rc<Map<G>>,

    /// Which nodes are owned, and which are vacant. Indexed by node id.
    pub nodes: Vec<Option<OwnedNode>>,

    /// The random number generator used to drive the goop flow algorithm.
    rng: XorShiftRng,
}

/// The state of a node that is owned by some player.
#[derive(Debug, Clone)]
pub struct OwnedNode {
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

impl<G: VisibleGraph> State<G> {
    pub fn new(map: Rc<Map<G>>, nodes: Vec<Option<OwnedNode>>) -> State<G> {
        const SEED: [u32; 4] = [0xcd9d5eaa, 0xf04bc9a7, 0x4602cc70, 0x98d01ef9];
        State { map, nodes, rng: XorShiftRng::from_seed(SEED) }
    }

    /// Let one unit of goop flow through each outflow.
    ///
    /// There are algorithms for finding the flow through a graph precisely, but
    /// we need something simpler here. We just visit every outgoing edge in a
    /// random order, and propagate a unit of goop if the destination permits
    /// it.
    pub fn flow(&mut self) {
        // Build a vector of (from, to) pairs.
        let mut outflow_list = Vec::new();
        for node in 0..self.map.graph.nodes() {
            if let &Some(ref owned) = &self.nodes[node] {
                for &outflow in &owned.outflows {
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
                (&mut Some(OwnedNode { goop, .. }), _) if goop == 0 => (),

                // Goop flowing into an unoccupied node. New player claims ownership.
                (&mut Some(OwnedNode { player, ref mut goop, .. }),
                 &mut ref mut to @ None) => {
                    *goop -= 1;
                    *to = Some(OwnedNode { player, outflows: vec![], goop: 1 });
                },

                // Goop flowing into a node occupied by the same player.
                (&mut Some(OwnedNode { player: from_player, goop: ref mut from_goop, .. }),
                 &mut Some(OwnedNode { player: to_player,   goop: ref mut to_goop, .. }))
                    if from_player == to_player =>
                {
                    if *from_goop > 0 && *to_goop < MAX_GOOP {
                        *from_goop -= 1;
                        *to_goop += 1;
                    }
                }

                // Goop flowing into a node occupied by another player, but
                // doesn't clear it. All outflow from destination stopped.
                (&mut Some(OwnedNode { goop: ref mut from_goop, .. }),
                 &mut Some(OwnedNode { outflows: ref mut to_outflows,
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
                (&mut Some(OwnedNode { player, goop: ref mut from_goop, .. }),
                 &mut ref mut to) => {
                    *from_goop -= 1;
                    *to = Some(OwnedNode { player, outflows: vec![], goop: 0 });
                    // Since all outflow from `to` is cancelled, remove any
                    // pending pairs from `outflow_list`.
                    outflow_list.retain(|&(from, _)| from != to_index);
                }
            }
        }
    }

    /// Let sources generate new goop.
    pub fn generate_goop(&mut self) {
        for &source in &self.map.sources {
            match &mut self.nodes[source] {
                &mut None => panic!("source nodes should always be owned by someone"),
                &mut Some(OwnedNode { ref mut goop, .. }) => {
                    if *goop < MAX_GOOP {
                        *goop += 1;
                    }
                }
            }
        }
    }
}

/// Actions that can be taken on a `State`.
pub enum Action {
    /// Toggle the state of the given outflow.
    ToggleOutflow((Node, Node)),
}


