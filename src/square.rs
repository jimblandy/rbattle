//! Types for square grids.

use graph::{Graph, Node};
use visible_graph::{IndexedSegment, Point, VisibleGraph};

/// A grid of 1✕1 squares, of a given number of rows and columns. A cell's
/// neighbors are those above, below, and to the left and right of it; diagonal
/// connections are not neigbors.
///
/// In graph space, the grid constructed by the call `SquareGrid::new(r, c)`
/// extends from `(0,0)` to `(c, r)`. Node are numbered in row-major order,
/// bottom to top, left to right.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SquareGrid {
    rows: usize,
    cols: usize
}

impl SquareGrid {
    /// Construct a `SquareGrid` with the given number of rows and columns.
    pub fn new(rows: usize, cols: usize) -> SquareGrid {
        SquareGrid { rows, cols }
    }

    /// Return the row and column of `node`.
    fn node_rc(&self, node: Node) -> (usize, usize) {
        assert!(node < self.nodes());
        (node / self.cols, node % self.cols)
    }

    /// Return the `Node` index for the node at the given row and column.
    fn rc_node(&self, row: usize, col: usize) -> Node {
        assert!(row < self.rows);
        assert!(col < self.cols);
        row * self.cols + col
    }
}

impl Graph for SquareGrid {
    fn nodes(&self) -> Node { self.rows * self.cols }

    fn neighbors(&self, node: Node) -> Vec<usize> {
        let mut neighbors = Vec::new();

        let (row, col) = self.node_rc(node);
        if row + 1 < self.rows {
            neighbors.push(self.rc_node(row + 1, col));
        }
        if col + 1 < self.cols {
            neighbors.push(self.rc_node(row, col + 1));
        }
        if row >= 1 {
            neighbors.push(self.rc_node(row - 1, col));
        }
        if col >= 1 {
            neighbors.push(self.rc_node(row, col - 1));
        }

        neighbors
    }
}

#[cfg(test)]
mod square_grid_as_graph {
    use graph::Graph;
    use super::SquareGrid;

    #[test]
    fn nodes() {
        assert_eq!(SquareGrid::new(4, 7).nodes(), 28);
        assert_eq!(SquareGrid::new(0, 100).nodes(), 0);
    }

    #[test]
    fn neighbors() {
        let grid = SquareGrid::new(4,7);

        // Corners.
        assert_same_elements!(grid.neighbors(0), vec![1, 7]);
        assert_same_elements!(grid.neighbors(6), vec![5, 13]);
        assert_same_elements!(grid.neighbors(21), vec![14, 22]);
        assert_same_elements!(grid.neighbors(27), vec![26, 20]);

        // Edges.
        assert_same_elements!(grid.neighbors(4), vec![3, 5, 11]);
        assert_same_elements!(grid.neighbors(7), vec![0, 8, 14]);
        assert_same_elements!(grid.neighbors(20), vec![13, 19, 27]);
        assert_same_elements!(grid.neighbors(23), vec![22, 16, 24]);

        // Interior points.
        assert_same_elements!(grid.neighbors(8), vec![7, 9, 1, 15]);
        assert_same_elements!(grid.neighbors(17), vec![16, 18, 10, 24]);

        let grid = SquareGrid::new(1, 3);
        assert_same_elements!(grid.neighbors(0), vec![1]);
        assert_same_elements!(grid.neighbors(1), vec![0,2]);
        assert_same_elements!(grid.neighbors(2), vec![1]);

        let grid = SquareGrid::new(3, 1);
        assert_same_elements!(grid.neighbors(0), vec![1]);
        assert_same_elements!(grid.neighbors(1), vec![0,2]);
        assert_same_elements!(grid.neighbors(2), vec![1]);

        let grid = SquareGrid::new(1, 1);
        assert_same_elements!(grid.neighbors(0), vec![]);
    }
}

impl VisibleGraph for SquareGrid {
    fn bounds(&self) -> (f32, f32) {
        (self.cols as f32, self.rows as f32)
    }

    fn center(&self, node: Node) -> Point {
        let (row, col) = self.node_rc(node);
        (col as f32 + 0.5, row as f32 + 0.5)
    }

    fn radius(&self, _node: Node) -> f32 { 0.5 }

    fn boundary(&self, node: Node) -> Vec<IndexedSegment> {
        let (rows, cols) = (self.rows, self.cols);

        // The number of endpoints in a single row of endpoints. This is
        // self.cols + 1, since we have to include the outermost boundary.
        let pt_cols = self.cols + 1;

        let (row, col) = self.node_rc(node);

        let mut segments = Vec::new();

        // Index of southwestern corner.
        let sw = row * pt_cols + col;

        // north
        segments.push(IndexedSegment {
            line: sw + pt_cols .. sw + pt_cols + 1,
            neighbor: if row + 1 < rows { Some(node + cols) } else { None }
        });

        // east
        segments.push(IndexedSegment {
            line: sw + pt_cols + 1 .. sw + 1,
            neighbor: if col + 1 < cols { Some(node + 1) } else { None }
        });

        // south
        segments.push(IndexedSegment {
            line: sw + 1 .. sw,
            neighbor: if 0 < row { Some(node - cols) } else { None }
        });

        // west
        segments.push(IndexedSegment {
            line: sw .. sw + pt_cols,
            neighbor: if 0 < col { Some(node - 1) } else { None }
        });

        segments
    }

    fn endpoints(&self) -> Vec<Point> {
        let mut points = Vec::new();
        for r in 0 .. self.rows + 1 {
            for c in 0 .. self.cols + 1 {
                points.push((c as f32, r as f32))
            }
        }
        points
    }

    /// A `SquareGrid` recognizes boundary hits by excluding hits near segment
    /// endpoints and hits near node centers as ambiguous. Those exclusion areas
    /// are squares, centered on the point in question. We let the squares
    /// overlap a bit, leaving rectangles around each boundary line segment to
    /// treat as hits.
    fn boundary_hit(&self, point: Point) -> Option<(Node, Node)> {
        // Exclude points further than this from the side of a node, or nearer
        // than this to a square corner.
        const TOLERANCE: f32 = 0.2;

        // Return `true` if `val` is no further than `distance` from the nearest integer.
        fn near(val: f32, distance: f32) -> bool {
            (val - val.round()).abs() <= distance
        }

        // Exclude points near corners.
        if near(point.0, TOLERANCE) && near(point.1, TOLERANCE) {
            return None;
        }

        // Recognize points near vertical edges. This test suffices because
        // we've already excluded corners.
        if near(point.0, TOLERANCE) {
            let col = point.0.round() as usize;
            let row = point.1.floor() as usize;

            // Boundary lines on the left and right edges are not boundaries
            // between two nodes.
            if col == 0 || col == self.cols {
                return None;
            }

            return Some((self.rc_node(col, row),
                         self.rc_node(col - 1, row)));
        }

        // Recognize points near horizontal edges. Similarly.
        if near(point.1, TOLERANCE) {
            let col = point.0.floor() as usize;
            let row = point.0.round() as usize;

            // Boundary lines on the bottom and top edges are not boundaries
            // between two nodes.
            if row == 0 || row == self.rows {
                return None;
            }

            return Some((self.rc_node(col, row),
                         self.rc_node(col, row - 1)));
        }

        None
    }
}

#[cfg(test)]
mod square_grid_as_visible_graph {
    use visible_graph::VisibleGraph;
    use super::SquareGrid;

    #[test]
    fn bounds() {
        assert_eq!(SquareGrid::new(4, 7).bounds(), (7.0, 4.0));
    }

    #[test]
    fn center() {
        let grid = SquareGrid::new(4, 7);
        assert_eq!(grid.center(0), (0.5, 0.5));
        assert_eq!(grid.center(1), (1.5, 0.5));
        assert_eq!(grid.center(6), (6.5, 0.5));

        assert_eq!(grid.center(7), (0.5, 1.5));
        assert_eq!(grid.center(9), (2.5, 1.5));

        assert_eq!(grid.center(21), (0.5, 3.5));
        assert_eq!(grid.center(22), (1.5, 3.5));
        assert_eq!(grid.center(27), (6.5, 3.5));
    }

    #[test]
    fn radius() {
        assert_eq!(SquareGrid::new(1, 1).radius(0), 0.5);
    }

    #[test]
    fn endpoints() {
        use test_utils::into_eq_points;

        assert_same_elements!(
            into_eq_points(SquareGrid::new(1, 1).endpoints()),
            into_eq_points(vec![(0.0, 0.0), (0.0, 1.0),
                                (1.0, 0.0), (1.0, 1.0)]));

        assert_same_elements!(
            into_eq_points(SquareGrid::new(3, 2).endpoints()),
            into_eq_points(vec![(0.0, 0.0), (1.0, 0.0), (2.0, 0.0),
                                (0.0, 1.0), (1.0, 1.0), (2.0, 1.0),
                                (0.0, 2.0), (1.0, 2.0), (2.0, 2.0),
                                (0.0, 3.0), (1.0, 3.0), (2.0, 3.0)]));

    }

    #[test]
    fn boundary() {
        use graph::Node;
        use std::ops::Range;
        use test_utils::{into_points, SegmentWithPoints};
        use visible_graph::Point;

        fn swp(start: Point, end: Point, neighbor: Option<Node>) -> SegmentWithPoints
        {
            SegmentWithPoints::new(&Range { start, end }, neighbor)
        }

        let grid = SquareGrid::new(1, 1);
        let endpoints = grid.endpoints();
        assert_same_elements!(
            into_points(&grid.boundary(0), &endpoints),
            vec![swp((0.0, 0.0), (1.0, 0.0), None),
                 swp((1.0, 0.0), (1.0, 1.0), None),
                 swp((1.0, 1.0), (0.0, 1.0), None),
                 swp((0.0, 1.0), (0.0, 0.0), None)]);

        let grid = SquareGrid::new(3, 2);
        let endpoints = grid.endpoints();
        assert_same_elements!(
            into_points(&grid.boundary(0), &endpoints),
            vec![swp((0.0, 0.0), (1.0, 0.0), None),
                 swp((1.0, 0.0), (1.0, 1.0), Some(1)),
                 swp((1.0, 1.0), (0.0, 1.0), Some(2)),
                 swp((0.0, 1.0), (0.0, 0.0), None)]);
        assert_same_elements!(
            into_points(&grid.boundary(3), &endpoints),
            vec![swp((1.0, 1.0), (2.0, 1.0), Some(1)),
                 swp((2.0, 1.0), (2.0, 2.0), None),
                 swp((2.0, 2.0), (1.0, 2.0), Some(5)),
                 swp((1.0, 2.0), (1.0, 1.0), Some(2))]);
    }
}
