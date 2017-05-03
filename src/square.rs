//! Types for square grids.

use graph::{Graph, Node};
use visible_graph::{GraphPt, IndexedSegment, VisibleGraph};

/// A grid of 1✕1 squares, of a given number of rows and columns. A cell's
/// neighbors are those above, below, and to the left and right of it; diagonal
/// connections are not neigbors.
///
/// In graph space, the grid constructed by the call `SquareGrid::new(r, c)`
/// extends from `(0,0)` to `(c, r)`. Node are numbered in row-major order,
/// bottom to top, left to right.
#[derive(Clone, Debug, Eq, PartialEq)]
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

    fn edges(&self) -> Node {
        // Each row has self.cols-1 horizontal edges;
        // each column has self.rows-1 vertical edges.
        (self.rows * (self.cols - 1) +
         self.cols * (self.rows - 1))
    }

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
    fn edges() {
        assert_eq!(SquareGrid::new(4, 7).edges(), 55);
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
    fn bounds(&self) -> GraphPt {
        GraphPt(self.cols as f32, self.rows as f32)
    }

    fn center(&self, node: Node) -> GraphPt {
        let (row, col) = self.node_rc(node);
        GraphPt(col as f32 + 0.5, row as f32 + 0.5)
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

    fn endpoints(&self) -> Vec<GraphPt> {
        let mut points = Vec::new();
        for r in 0 .. self.rows + 1 {
            for c in 0 .. self.cols + 1 {
                points.push(GraphPt(c as f32, r as f32))
            }
        }
        points
    }

    /// A `SquareGrid` recognizes boundary hits by excluding hits near segment
    /// endpoints and hits near node centers as ambiguous. Those exclusion areas
    /// are squares, centered on the point in question. We let the squares
    /// overlap a bit, leaving rectangles around each boundary line segment to
    /// treat as hits.
    fn boundary_hit(&self, point: &GraphPt) -> Option<(Node, Node)> {
        // Exclude points further than this from the side of a node, or nearer
        // than this to a square corner. Clearly, this must be less than 0.5, to
        // avoid ambiguity.
        const TOLERANCE: f32 = 0.2;

        // Exclude points outside the grid altogether, or on the outer edges.
        // This lets us assume that every hit we find is an interior boundary,
        // with another node on the other side.
        let GraphPt(max_x, max_y) = self.bounds();
        if point.0 < TOLERANCE || point.0 > max_x - TOLERANCE ||
            point.1 < TOLERANCE || point.1 > max_y - TOLERANCE
        {
            return None;
        }

        // Return `true` if `val` is no further than `distance` from the nearest integer.
        fn near(val: f32, distance: f32) -> bool {
            (val - val.round()).abs() <= distance
        }

        // Exclude points near corners.
        if near(point.0, TOLERANCE) && near(point.1, TOLERANCE) {
            return None;
        }

        // Recognize points near vertical edges. We know these points cannot
        // also be near horizontal edges, since we've already excluded corners.
        if near(point.0, TOLERANCE) {
            // Both the round and floor here produce positive numbers, given the
            // exclusions above.
            let col = point.0.round() as usize;
            let row = point.1.floor() as usize;

            return Some((self.rc_node(row, col),
                         self.rc_node(row, col - 1)));
        }

        // Recognize points near horizontal edges. As above, just transposed.
        if near(point.1, TOLERANCE) {
            let col = point.0.floor() as usize;
            let row = point.1.round() as usize;

            return Some((self.rc_node(row, col),
                         self.rc_node(row - 1, col)));
        }

        None
    }
}

#[cfg(test)]
mod square_grid_as_visible_graph {
    use visible_graph::{GraphPt, VisibleGraph};
    use super::SquareGrid;

    /// Construct a GraphPt. For brevity in tests.
    fn gp(x: f32, y: f32) -> GraphPt { GraphPt(x, y) }

    #[test]
    fn bounds() {
        assert_eq!(SquareGrid::new(4, 7).bounds(), gp(7.0, 4.0));
    }

    #[test]
    fn center() {
        let grid = SquareGrid::new(4, 7);
        assert_eq!(grid.center(0), gp(0.5, 0.5));
        assert_eq!(grid.center(1), gp(1.5, 0.5));
        assert_eq!(grid.center(6), gp(6.5, 0.5));

        assert_eq!(grid.center(7), gp(0.5, 1.5));
        assert_eq!(grid.center(9), gp(2.5, 1.5));

        assert_eq!(grid.center(21), gp(0.5, 3.5));
        assert_eq!(grid.center(22), gp(1.5, 3.5));
        assert_eq!(grid.center(27), gp(6.5, 3.5));
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
            into_eq_points(vec![gp(0.0, 0.0), gp(0.0, 1.0),
                                gp(1.0, 0.0), gp(1.0, 1.0)]));

        assert_same_elements!(
            into_eq_points(SquareGrid::new(3, 2).endpoints()),
            into_eq_points(vec![gp(0.0, 0.0), gp(1.0, 0.0), gp(2.0, 0.0),
                                gp(0.0, 1.0), gp(1.0, 1.0), gp(2.0, 1.0),
                                gp(0.0, 2.0), gp(1.0, 2.0), gp(2.0, 2.0),
                                gp(0.0, 3.0), gp(1.0, 3.0), gp(2.0, 3.0)]));

    }

    #[test]
    fn boundary() {
        use graph::Node;
        use std::ops::Range;
        use test_utils::{into_points, SegmentWithPoints};
        use visible_graph::GraphPt;

        fn swp(start: GraphPt, end: GraphPt, neighbor: Option<Node>) -> SegmentWithPoints
        {
            SegmentWithPoints::new(&Range { start, end }, neighbor)
        }

        let grid = SquareGrid::new(1, 1);
        let endpoints = grid.endpoints();
        assert_same_elements!(
            into_points(&grid.boundary(0), &endpoints),
            vec![swp(gp(0.0, 0.0), gp(1.0, 0.0), None),
                 swp(gp(1.0, 0.0), gp(1.0, 1.0), None),
                 swp(gp(1.0, 1.0), gp(0.0, 1.0), None),
                 swp(gp(0.0, 1.0), gp(0.0, 0.0), None)]);

        let grid = SquareGrid::new(3, 2);
        let endpoints = grid.endpoints();
        assert_same_elements!(
            into_points(&grid.boundary(0), &endpoints),
            vec![swp(gp(0.0, 0.0), gp(1.0, 0.0), None),
                 swp(gp(1.0, 0.0), gp(1.0, 1.0), Some(1)),
                 swp(gp(1.0, 1.0), gp(0.0, 1.0), Some(2)),
                 swp(gp(0.0, 1.0), gp(0.0, 0.0), None)]);
        assert_same_elements!(
            into_points(&grid.boundary(3), &endpoints),
            vec![swp(gp(1.0, 1.0), gp(2.0, 1.0), Some(1)),
                 swp(gp(2.0, 1.0), gp(2.0, 2.0), None),
                 swp(gp(2.0, 2.0), gp(1.0, 2.0), Some(5)),
                 swp(gp(1.0, 2.0), gp(1.0, 1.0), Some(2))]);
    }

    #[test]
    fn boundary_hit() {
        // These tests are not black-box: they know the general algorithm
        // `boundary_hit` implements, and the value of TOLERANCE. But they
        // should mostly be okay with any reasonable hit definition.

        use graph::Node;
        use super::SquareGrid;

        // Make a result from `boundary_hit` easier to compare by putting the
        // nodes in increasing order, if there are any. Terse name because
        // it's local, and we're using it in lots of tests.
        fn s(opt: Option<(Node, Node)>) -> Option<(Node, Node)> {
            opt.map(|(a, b)| if a > b { (b, a) } else { (a, b) })
        }

        let grid = SquareGrid::new(3, 4);

        // Wildly outside the grid.
        assert_eq!(grid.boundary_hit(&gp(-100.0, -100.0)), None);
        assert_eq!(grid.boundary_hit(&gp(-100.0, 1.5)),    None);
        assert_eq!(grid.boundary_hit(&gp(-100.0, 2000.0)), None);

        assert_eq!(grid.boundary_hit(&gp(2.0, -100.0)),    None);
        assert_eq!(grid.boundary_hit(&gp(2.0, 2000.0)),    None);

        assert_eq!(grid.boundary_hit(&gp(2000.0, -100.0)), None);
        assert_eq!(grid.boundary_hit(&gp(2000.0, 1.5)),    None);
        assert_eq!(grid.boundary_hit(&gp(2000.0, 2000.0)), None);

        // Nearby outside.
        assert_eq!(grid.boundary_hit(&gp(2.0, -0.5)), None);
        assert_eq!(grid.boundary_hit(&gp(4.5,  1.5)), None);
        assert_eq!(grid.boundary_hit(&gp(2.0,  3.5)), None);
        assert_eq!(grid.boundary_hit(&gp(-0.5, 1.5)), None);

        // On corners.
        assert_eq!(grid.boundary_hit(&gp(0.0, 0.0)), None);
        assert_eq!(grid.boundary_hit(&gp(4.0, 0.0)), None);
        assert_eq!(grid.boundary_hit(&gp(4.0, 3.0)), None);
        assert_eq!(grid.boundary_hit(&gp(0.0, 3.0)), None);

        // On sides.
        assert_eq!(grid.boundary_hit(&gp(3.5, 0.0)), None);
        assert_eq!(grid.boundary_hit(&gp(4.0, 2.3)), None);
        assert_eq!(grid.boundary_hit(&gp(1.7, 3.0)), None);
        assert_eq!(grid.boundary_hit(&gp(0.0, 1.2)), None);

        // Interior horizontal.
        assert_eq!(s(grid.boundary_hit(&gp(0.5, 1.1))), Some((0, 4)));
        assert_eq!(s(grid.boundary_hit(&gp(3.6, 1.9))), Some((7, 11)));

        // Interior vertical.
        assert_eq!(s(grid.boundary_hit(&gp(2.1, 1.3))), Some((5, 6)));
        assert_eq!(s(grid.boundary_hit(&gp(3.0, 2.7))), Some((10, 11)));

        // Inside the grid but not close to any boundary line.
        assert_eq!(s(grid.boundary_hit(&gp(2.4, 1.6))), None);
        assert_eq!(s(grid.boundary_hit(&gp(1.3, 0.7))), None);
    }
}
