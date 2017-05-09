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
        assert!(rows * cols > 0);
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
        unimplemented!();
    }

    fn neighbors(&self, node: Node) -> Vec<usize> {
        unimplemented!();
    }
}

#[cfg(test)]
mod square_grid_as_graph {
    use graph::Graph;
    use super::SquareGrid;

    #[test]
    fn nodes() {
        assert_eq!(SquareGrid::new(4, 7).nodes(), 28);
    }

    #[test]
    fn edges() {
        assert_eq!(SquareGrid::new(4, 7).edges(), 90);
        assert_eq!(SquareGrid::new(1, 100).edges(), 198);
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
        unimplemented!();
    }

    fn center(&self, node: Node) -> GraphPt {
        unimplemented!();
    }

    fn radius(&self) -> f32 { unimplemented!(); }

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
                points.push(GraphPt([c as f32, r as f32]))
            }
        }
        points
    }

    /// A `SquareGrid` recognizes edge hits by dividing each square into four
    /// triangular quadrants: north, south, east, and west. Points very near the
    /// diagonals or grid lines are excluded as ambiguous.
    fn edge_hit(&self, &GraphPt(point): &GraphPt) -> Option<(Node, Node)> {
        // Exclude points closer than this to a grid line.
        const TOLERANCE: f32 = 0.05;

        // Check how close `val` is to the nearest integer. If it is within
        // `distance`, return true.
        fn near(val: f32, distance: f32) -> bool {
            (val - val.round()).abs() <= distance
        }

        // Exclude points outside the grid altogether, or on the outer edges.
        // This lets us assume that every hit we find is an interior boundary,
        // with another node on the other side.
        let GraphPt(bounds) = self.bounds();
        if point[0] < 0.0 || point[0] > bounds[0] ||
            point[1] < 0.0 || point[1] > bounds[1]
        {
            return None;
        }

        // Exclude points near grid lines.
        if near(point[0], TOLERANCE) || near(point[1], TOLERANCE) {
            return None;
        }

        // Find the originating node.
        let (c, r) = (point[0] as i32, point[1] as i32);

        // Find the position of `point` within that node's area.
        let fract_x = point[0].fract();
        let fract_y = point[1].fract();

        // Exclude points near diagonals.
        if (fract_x - fract_y).abs() < TOLERANCE {
            return None;
        }
        if (fract_x + fract_y).abs() < TOLERANCE {
            return None;
        }

        // Identify the quadrant.
        let (dx, dy) =
            if fract_y < fract_x {            // south or east
                if fract_y < 1.0 - fract_x {
                    (0, -1)                     // south
                } else {
                    (1, 0)                      // east
                }
            } else {                            // north or west
                if fract_y < 1.0 - fract_x {
                    (-1, 0)                     // west
                } else {
                    (0, 1)                      // north
                }
            };

        // Is there actually another node in that direction?
        if 0 <= c + dx && c + dx < self.cols as i32 &&
            0 <= r + dy && r + dy < self.rows as i32
        {
            Some((self.rc_node(r as usize, c as usize),
                  self.rc_node((r + dy) as usize, (c + dx) as usize)))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod square_grid_as_visible_graph {
    use visible_graph::{GraphPt, VisibleGraph};
    use super::SquareGrid;

    /// Construct a GraphPt. For brevity in tests.
    fn gp(x: f32, y: f32) -> GraphPt { GraphPt([x, y]) }

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
        assert_eq!(SquareGrid::new(1, 1).radius(), 0.5);
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

        use super::SquareGrid;

        let grid = SquareGrid::new(3, 4);

        // Wildly outside the grid.
        assert_eq!(grid.edge_hit(&gp(-100.0, -100.0)), None);
        assert_eq!(grid.edge_hit(&gp(-100.0, 1.5)),    None);
        assert_eq!(grid.edge_hit(&gp(-100.0, 2000.0)), None);

        assert_eq!(grid.edge_hit(&gp(2.0, -100.0)),    None);
        assert_eq!(grid.edge_hit(&gp(2.0, 2000.0)),    None);

        assert_eq!(grid.edge_hit(&gp(2000.0, -100.0)), None);
        assert_eq!(grid.edge_hit(&gp(2000.0, 1.5)),    None);
        assert_eq!(grid.edge_hit(&gp(2000.0, 2000.0)), None);

        // Nearby outside.
        assert_eq!(grid.edge_hit(&gp(2.0, -0.5)), None);
        assert_eq!(grid.edge_hit(&gp(4.5,  1.5)), None);
        assert_eq!(grid.edge_hit(&gp(2.0,  3.5)), None);
        assert_eq!(grid.edge_hit(&gp(-0.5, 1.5)), None);

        // On corners.
        assert_eq!(grid.edge_hit(&gp(0.0, 0.0)), None);
        assert_eq!(grid.edge_hit(&gp(4.0, 0.0)), None);
        assert_eq!(grid.edge_hit(&gp(4.0, 3.0)), None);
        assert_eq!(grid.edge_hit(&gp(0.0, 3.0)), None);

        // On sides.
        assert_eq!(grid.edge_hit(&gp(3.5, 0.0)), None);
        assert_eq!(grid.edge_hit(&gp(4.0, 2.3)), None);
        assert_eq!(grid.edge_hit(&gp(1.7, 3.0)), None);
        assert_eq!(grid.edge_hit(&gp(0.0, 1.2)), None);

        // Interior north.
        assert_eq!(grid.edge_hit(&gp(0.5, 0.9)), Some((0, 4)));
        assert_eq!(grid.edge_hit(&gp(3.6, 1.8)), Some((7, 11)));
        assert_eq!(grid.edge_hit(&gp(1.4, 1.9)), Some((5, 9)));

        // Interior south.
        assert_eq!(grid.edge_hit(&gp(0.5, 1.1)), Some((4, 0)));
        assert_eq!(grid.edge_hit(&gp(3.6, 2.2)), Some((11, 7)));
        assert_eq!(grid.edge_hit(&gp(1.4, 2.1)), Some((9, 5)));

        // Interior east
        assert_eq!(grid.edge_hit(&gp(0.9, 0.4)), Some((0, 1)));
        assert_eq!(grid.edge_hit(&gp(2.8, 2.5)), Some((10, 11)));
        assert_eq!(grid.edge_hit(&gp(1.9, 1.5)), Some((5, 6)));

        // Interior west
        assert_eq!(grid.edge_hit(&gp(1.1, 0.6)), Some((1, 0)));
        assert_eq!(grid.edge_hit(&gp(3.2, 2.5)), Some((11, 10)));
        assert_eq!(grid.edge_hit(&gp(2.1, 1.6)), Some((6, 5)));
    }
}
