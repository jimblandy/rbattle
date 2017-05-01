use graph::Node;
use visible_graph::VisibleGraph;

/// A map on which an RBattle game is played.
///
/// A `Map` holds everything that does not change over the course of an RBattle
/// game. This includes a graph, and a set of nodes that have goop sources.
pub struct Map<G: VisibleGraph> {
    /// The graph of nodes comprising this map's territory.
    pub graph: G,

    /// The nodes of `graph` that contain goop sources.
    pub sources: Vec<Node>
}
