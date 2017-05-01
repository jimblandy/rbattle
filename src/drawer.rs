use map::Map;
use state::State;
use visible_graph::{Point, VisibleGraph};

use glium::{Frame, IndexBuffer, VertexBuffer};
use glium::backend::Facade;
use glium::index::PrimitiveType;
use glium::vertex::Vertex;

use std::iter::once;
use std::ops::Range;

/// A `Drawer` knows how to draw a `State` on a Glium `Frame`.
///
/// A `Drawer` is constructed from a `Map`, and then is given specific `State`
/// values that use that `Map` to draw a complete frame of the game on a Glium
/// `Frame` value, representing one frame of video.
///
/// The `Drawer` is the right place to hold Glium state that persists between
/// frames, like vertex and index buffers for the map.
struct Drawer {
    /// Vertexes of the graph's boundary lines.
    boundary_vertices: VertexBuffer<VPoint>,

    /// Indices for the graph's boundary lines.
    boundary_indices: IndexBuffer<u32>
}

impl Drawer {
    fn new<G>(display: &Facade, map: &Map<G>) -> Drawer
        where G: VisibleGraph
    {
        let graph = &map.graph;

        // It's a little annoying that we have to do this map to convert Point
        // to VPoint, but I'd rather do this than a transmute.
        let vertices = graph.endpoints().into_iter()
            .map(|point| VPoint { point })
            .collect::<Vec<VPoint>>();
        let boundary_vertices = VertexBuffer::new(display, &vertices)
            .expect("building buffer for graph vertices");

        let mut indices = Vec::new();
        for node in 0..graph.nodes() {
            for segment in graph.boundary(node) {
                // A boundary line between two nodes will appear twice in the
                // list. Cull out the duplicates by only retaining segments with
                // no node on the other side, or where the node on the other
                // side has a higher number.
                if match segment.neighbor {
                    None => true,
                    Some(neighbor) => node < neighbor
                } {
                    indices.push(segment.line.start as u32);
                    indices.push(segment.line.end as u32);
                }
            }
        }

        let boundary_indices = IndexBuffer::new(display, PrimitiveType::LinesList, &indices)
            .expect("building buffer for graph indices");

        Drawer { boundary_vertices, boundary_indices }
    }

    fn draw_frame<G>(&self, frame: &Frame, state: &State<G>)
        where G: VisibleGraph
    {
    }
}

/// A newtype for `visible_graph::Point` that implements `glium::vertex::Vertex`.
#[derive(Copy, Clone, Debug)]
struct VPoint { point: Point }

implement_vertex!(VPoint, point);

