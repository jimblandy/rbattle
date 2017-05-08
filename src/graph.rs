/// The type of the index of a node in a `Grid`.
pub type Node = usize;

/// A directed graph of nodes and their neighbors.
///
/// Each node is identified by a `Node` index.
pub trait Graph {
    /// Return the number of nodes in this graph. The graph's nodes have indices
    /// in the range 0..graph.nodes().
    fn nodes(&self) -> Node;

    /// Return the number of edges in this graph.
    fn edges(&self) -> usize;

    /// Return a vector of `node`'s neighbors.
    fn neighbors(&self, node: Node) -> Vec<Node>;
}
