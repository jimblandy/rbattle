use graph::Node;
use visible_graph::VisibleGraph;
use map::{Map, Player};
use std::rc::Rc;

/// The complete state of an RBattle game board.
#[derive(Debug, Clone)]
pub struct State<G: VisibleGraph> {
    /// The map being played on.
    pub map: Rc<Map<G>>,

    /// Which nodes are owned, and which are vacant. Indexed by node id.
    pub nodes: Vec<Option<OwnedNode>>,
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
