use graph::Node;
use visible_graph::VisibleGraph;
use map::Map;
use std::rc::Rc;

/// A player id number.
#[derive(Debug, Copy, Clone)]
pub struct Player(usize);

/// The complete state of an RBattle game board.
#[derive(Debug, Clone)]
pub struct State<G: VisibleGraph> {
    /// The map being played on.
    pub map: Rc<Map<G>>,

    /// The state of each node on the map. Indexed by node number.
    pub nodes: Vec<NodeState>
}

/// The state of a single node on a game board.
#[derive(Debug, Clone)]
pub struct NodeState {
    /// The player who controls this node, if any.
    pub owner: Option<Player>,

    /// The amount of goop in this node. This ranges from 0 to MAX_GOOP,
    /// inclusive.
    pub goop: usize,

    /// Which neighbors of this node goop flows out to. It is a rule of the game
    /// that nodes with inflows from more than one player may not have any outflows.
    pub outflows: Vec<Node>
}

impl NodeState {
    pub fn empty() -> NodeState {
        NodeState { owner: None, goop: 0, outflows: vec![] }
    }
}

pub const MAX_GOOP: usize = 15;
