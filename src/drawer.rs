//! Drawing maps and game states on the screen with Glium.
//!
//! The `drawer` module defines types that know how to draw a graph and a game
//! state on the screen using Glium calls.
//!
//! # Coordinate spaces
//!
//! Although rbattle is a 2D game, which simplifies things a lot, it needs to
//! work with a few different coordinate systems. Working from the the most
//! concrete (pixels on the screen) to the abstract (nodes on the graph):
//!
//! - Window coordinates. The window's area is a rectangular matrix of pixels,
//!   with (0,0) at the upper left, and the positive x and y axes pointing to
//!   the right and down.
//!
//! - Normalized device coordinates (NDC). Normalized device coordinates cover
//!   the window in a resolution-independent way. In NDC, (0,0) is the center of
//!   the screen; the positive x and y axes point to the right and up, as is
//!   traditional in mathematics; and the edges of the window are -1 and 1 for
//!   both axes.
//!
//! - Game coordinates: (-1, -1) and (1, 1) are the lower-left and upper-right
//!   coordinates of the overall display of the game. We choose the
//!   transformation between this and NDC to keep the game display fully visible
//!   and centered in the window.
//!
//! - Graph space coordinates: the coordinate system defined by the VisibleGraph
//!   implementation, where nodes' areas fall in the axis-aligned bounding box
//!   between (0,0) and upper_right, where upper_right is what you get from
//!   VisibleGraph::bounds().

use errors::*;
use map::Map;
use state::State;
use math::{compose, scale_transform, translate_transform};
use visible_graph::{GraphPt, VisibleGraph};

use glium::{DrawParameters, Frame, IndexBuffer, Program, Surface, VertexBuffer};
use glium::backend::Facade;
use glium::index::PrimitiveType;
//use glium::vertex::Vertex;

/// A `Drawer` knows how to draw a `State` on a Glium `Frame`.
///
/// A `Drawer` is constructed from a `Map`, and then is given specific `State`
/// values that use that `Map` to draw a complete frame of the game on a Glium
/// `Frame` value, representing one frame of video.
///
/// The `Drawer` is the right place to hold Glium state that persists between
/// frames, like vertex and index buffers for the map.
pub struct Drawer {
    /// Cached information needed to drawing the map, excluding the map itself.
    /// This holds vertex and index buffers, shader programs, transformations,
    /// and the like.
    pub map: MapDrawer
}

impl Drawer {
    pub fn new<G>(display: &Facade, map: &Map<G>) -> Result<Drawer>
        where G: VisibleGraph
    {
        Ok(Drawer {
            map: MapDrawer::new(display, map)?
        })
    }

    pub fn draw<G>(&self, frame: &mut Frame, state: &State<G>) -> Result<()>
        where G: VisibleGraph
    {
        self.map.draw(frame, &state.map)
    }
}

pub struct MapDrawer {
    /// Vertexes of the graph's boundary lines.
    vertices: VertexBuffer<GraphVert>,

    /// Indices for the graph's boundary lines.
    indices: IndexBuffer<u32>,

    /// Transformation from graph space to game space.
    graph_to_game: [[f32; 3]; 3],

    /// The aspect ratio (width / height) of the game rectangle.
    game_aspect: f32,

    /// Shader program for drawing the map.
    program: Program,

    /// Draw parameters for drawing the map.
    draw_params: DrawParameters<'static>
}

impl MapDrawer {
    fn new<G>(display: &Facade, map: &Map<G>) -> Result<MapDrawer>
        where G: VisibleGraph
    {
        let graph = &map.graph;

        // It's a little annoying that we have to do this map to convert GraphPt
        // to GraphVert, but I'd rather do this than a transmute.
        let vertices = graph.endpoints().into_iter()
            .map(|point| GraphVert { point: [point.0, point.1] })
            .collect::<Vec<GraphVert>>();
        let vertices = VertexBuffer::new(display, &vertices)
            .chain_err(|| "building buffer for graph vertices")?;

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

        let indices = IndexBuffer::new(display, PrimitiveType::LinesList, &indices)
            .chain_err(|| "building buffer for graph indices")?;

        // Compute the transformation from graph space, where points run from
        // (0, 0) to upper_right, to game space, where points run from (-1, -1)
        // to (1,1).
        let GraphPt(width, height) = graph.bounds();
        let game_aspect = width / height;
        let graph_to_game =
            compose(translate_transform(-1.0, -1.0),
                    scale_transform(2.0 / width, 2.0 / height));

        let program = Program::from_source(display,
                                           include_str!("map.vert"),
                                           include_str!("map.frag"),
                                           None)
            .chain_err(|| "compiling map shaders")?;

        let draw_params = DrawParameters {
            line_width: Some(2.0),
            .. Default::default()
        };

        Ok(MapDrawer {
            vertices, indices, game_aspect, graph_to_game, program, draw_params
        })
    }

    /// Draw `map` on `frame`.
    ///
    /// The map `state` uses must be the same map that was passed to
    /// `MapDrawer::new` when this `MapDrawer` was created.
    fn draw<G>(&self, frame: &mut Frame, _map: &Map<G>) -> Result<()>
        where G: VisibleGraph
    {
        // Compute the aspect ratio of the window (the "device"), assuming
        // square pixels.
        let (width, height) = frame.get_dimensions();
        let device_aspect = width as f32 / height as f32;

        // Compute the transformation from game coordinates to normalized device
        // coordinates. Depending on their relative aspect ratios, the game may
        // be centered either vertically or horizontally within the window.
        let game_to_device =
            if device_aspect > self.game_aspect {
                // Window is wider than game. Game centered horizontally.
                scale_transform(self.game_aspect / device_aspect, 1.0)
            } else {
                // Game is wider than window. Game centered vertically.
                scale_transform(1.0, device_aspect / self.game_aspect)
            };

        let graph_to_device = compose(game_to_device, self.graph_to_game);

        frame.draw(&self.vertices, &self.indices, &self.program,
                   &uniform! {
                       graph_to_device: graph_to_device
                   },
                   &self.draw_params)
            .chain_err(|| "drawing map")?;

        Ok(())
    }
}

/// A vertex in Graph space.
#[derive(Copy, Clone, Debug)]
struct GraphVert { point: [f32; 2] }

implement_vertex!(GraphVert, point);
