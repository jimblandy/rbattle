use graph::{Graph, Node};

/// A map on which an RBattle game is played.
///
/// A `Map` holds everything that does not change over the course of an RBattle
/// game. This includes a graph, and a set of nodes that have goop sources.
pub struct Map<G: Graph> {
    /// The graph of nodes comprising this map's territory.
    graph: G,

    /// The nodes of `graph` that contain goop sources.
    sources: Vec<Node>
}
