/// The type of the index of a node in a `Grid`.
pub type Node = usize;

/// A graph of nodes and their neighbors.
///
/// Each node is identified by a `Node` index.
pub trait Graph {
    /// Return the number of nodes in this graph. The graph's nodes have indices
    /// in the range 0..graph.nodes().
    fn nodes(&self) -> Node;

    /// Return a vector of `node`'s neighbors.
    fn neighbors(&self, node: Node) -> Vec<Node>;
}
