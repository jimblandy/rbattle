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
use state::{State, MAX_GOOP, OwnedNode};
use math::{compose, scale_transform};
use visible_graph::{GraphPt, VisibleGraph};

use glium::{DrawParameters, Frame, IndexBuffer, Program, Surface, VertexBuffer};
use glium::backend::Facade;
use glium::index::PrimitiveType;
//use glium::vertex::Vertex;

use std::cell::RefCell;

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
    map: MapDrawer,

    /// Cached information needed to draw outflows.
    outflows: OutflowsDrawer,

    /// Cached information for drawing goop amounts.
    goop: GoopDrawer,
}

impl Drawer {
    pub fn new<G>(display: &Facade, map: &Map<G>) -> Result<Drawer>
        where G: VisibleGraph
    {
        let map_drawer = MapDrawer::new(display, map)?;
        let outflows = OutflowsDrawer::new(display, map)?;
        let goop = GoopDrawer::new(display, map)?;

        Ok(Drawer { map: map_drawer, outflows, goop })
    }

    pub fn draw<G>(&self, frame: &mut Frame, state: &State<G>) -> Result<()>
        where G: VisibleGraph
    {
        let map = &*state.map;

        // Compute the aspect ratio of the window (the "device"), assuming
        // square pixels.
        let (width, height) = frame.get_dimensions();
        let device_aspect = width as f32 / height as f32;

        // Compute the transformation from game coordinates to normalized device
        // coordinates. Depending on their relative aspect ratios, the game may
        // be centered either vertically or horizontally within the window.
        let game_to_device =
            if device_aspect > map.game_aspect {
                // Window is wider than game. Game centered horizontally.
                scale_transform(map.game_aspect / device_aspect, 1.0)
            } else {
                // Game is wider than window. Game centered vertically.
                scale_transform(1.0, device_aspect / map.game_aspect)
            };

        let graph_to_device = compose(game_to_device, map.graph_to_game);

        self.map.draw(frame, &graph_to_device, &state.map)?;
        self.goop.draw(frame, &graph_to_device, &state.nodes, &state.map)?;
        self.outflows.draw(frame, &graph_to_device, &state.nodes)?;
        Ok(())
    }
}

struct MapDrawer {
    /// Shader program for drawing the map.
    program: Program,

    /// Vertexes of the graph's boundary lines.
    vertices: VertexBuffer<GraphVert>,

    /// Indices for the graph's boundary lines.
    indices: IndexBuffer<u32>,

    /// Draw parameters for drawing the map.
    draw_params: DrawParameters<'static>
}

impl MapDrawer {
    fn new<G>(display: &Facade, map: &Map<G>) -> Result<MapDrawer>
        where G: VisibleGraph
    {
        let graph = &map.graph;

        let program = Program::from_source(display,
                                           include_str!("map.vert"),
                                           include_str!("map.frag"),
                                           None)
            .chain_err(|| "compiling map shaders")?;

        // It's a little annoying that we have to do this map to convert GraphPt
        // to GraphVert, but I'd rather do this than a transmute.
        let vertices: Vec<GraphVert> = graph.endpoints().into_iter()
            .map(|point| GraphVert { point: [point.0, point.1] })
            .collect();
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

        let draw_params = DrawParameters {
            line_width: Some(2.0),
            .. Default::default()
        };

        Ok(MapDrawer {
            program, vertices, indices, draw_params
        })
    }

    /// Draw `map` on `frame`.
    ///
    /// The map `state` uses must be the same map that was passed to
    /// `MapDrawer::new` when this `MapDrawer` was created.
    fn draw<G>(&self, frame: &mut Frame, to_device: &[[f32; 3]; 3],_map: &Map<G>) -> Result<()>
        where G: VisibleGraph
    {
        frame.draw(&self.vertices, &self.indices, &self.program,
                   &uniform! {
                       graph_to_device: *to_device
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

struct OutflowsDrawer {
    /// Shader program for drawing the outflows.
    program: Program,

    /// Vertexes of the nodes' center positions.
    centers: VertexBuffer<GraphVert>,

    /// Index buffer for outflows. This is a "persistent" index buffer, updated
    /// once per frame.
    indices: RefCell<IndexBuffer<u32>>,

    /// Draw parameters for outflows.
    draw_params: DrawParameters<'static>
}

impl OutflowsDrawer {
    fn new<G>(display: &Facade, map: &Map<G>) -> Result<OutflowsDrawer>
        where G: VisibleGraph
    {
        let graph = &map.graph;

        let program = Program::from_source(display,
                                           include_str!("map.vert"),
                                           include_str!("outflow.frag"),
                                           None)
            .chain_err(|| "compiling outflow shaders")?;

        let centers: Vec<GraphVert> = (0..graph.nodes())
            .map(|node| {
                let GraphPt(x, y) = graph.center(node);
                GraphVert { point: [x, y] }
            })
            .collect();
        let centers = VertexBuffer::new(display, &centers)
            .chain_err(|| "building buffer for outflow vertices")?;

        let indices = IndexBuffer::empty_persistent(display,
                                                    PrimitiveType::LinesList,
                                                    graph.edges())
            .chain_err(|| "allocating outflow index buffer")?;

        let draw_params = DrawParameters {
            line_width: Some(5.0),
            .. Default::default()
        };

        Ok(OutflowsDrawer { program, centers, indices: RefCell::new(indices), draw_params })
    }

    fn draw(&self, frame: &mut Frame, to_device: &[[f32; 3]; 3], nodes: &[Option<OwnedNode>])
               -> Result<()>
    {
        // Build indices for the goop flow lines we actually need to draw.
        let mut indices = Vec::new();
        for (node, state) in nodes.iter().enumerate() {
            match state {
                &Some(ref owned) => {
                    for &outflow in &owned.outflows {
                        indices.push(node as u32);
                        indices.push(outflow as u32);
                    }
                },
                _ => ()
            }
        }

        // Glium seems to have a bug with zero-length slices. Let's not argue
        // with it.
        if indices.len() > 0 {
            // Write the indices to an appropriately sized slice of `self.indices`.
            self.indices.borrow_mut().slice_mut(0..indices.len())
                .expect("more outflow edges than graph edges")
                .write(&indices);

            frame.draw(&self.centers,
                       self.indices.borrow().slice(0..indices.len()).unwrap(),
                       &self.program,
                       &uniform! {
                           graph_to_device: *to_device
                       },
                       &self.draw_params)
                .chain_err(|| "drawing outflows")?;
        }

        Ok(())
    }
}

/// A point in texture space.
#[derive(Copy, Clone, Debug)]
struct TextureVert { texture: [f32; 2] }

implement_vertex!(TextureVert, texture);

/// Cached information about drawing the levels of goop present at each node.
///
/// We draw goop levels by placing a square (two triangles) on each node large
/// enough to cover the largest goop circle we'd like to draw. We then pretend
/// we have an infinite texture containing a circle of radius 1, and set the
/// texture coordinates on each square to draw the circle sized appropriately.
///
/// We size circles so that their area is proportional to the amount of goop.
/// This seems like the most intuitive visual indicator of amount. This means
/// that the ratio of the radius of the largest circle we'll draw to the
/// smallest is `sqrt(MAX_GOOP)`.
///
/// The trick is that there is no such texture: the fragment shader simply
/// checks whether its pixel's texture coordinates are within 1 of the origin,
/// colors the pixel if it is, and leaves the pixel transparent otherwise.
///
/// Actually, because we need circles of different colors, our imaginary texture
/// has 4096 circles on it. The circle index is a 12-bit value, which we break
/// into four groups of four bits to get R, G, and B values.
///
/// Since we need empty space around each unit circle so that we can draw them
/// as small circles, they are spaced at `sqrt(MAX_GOOP)` intervals.<
struct GoopDrawer {
    /// Shader program for drawing goop.
    program: Program,

    /// Vertexes for the squares on each node, without texture coordinates.
    /// These are a function of the map, and so are fixed from one frame to the
    /// next. The vertices for node `i` are at `4*i .. 4*i + 4`, going
    /// counterclockwise through the quadrants.
    squares: VertexBuffer<GraphVert>,

    /// Vertexes of the texture coordinates of each node's square. Parallel to
    /// the `squares` vertex buffer. This is a "persistent" vertex buffer: its
    /// contents change on each frame, based on goop levels.
    textures: RefCell<VertexBuffer<TextureVert>>,

    /// Index buffer for the squares on nodes. This is a function of the map,
    /// and is fixed from one frame to the next. The triangles for node `i` are
    /// at `6*i .. 6*i + 3` and `6*i + 3 .. 6*i + 6`.
    indices: IndexBuffer<u32>,

    /// Draw parameters for goop squares.
    draw_params: DrawParameters<'static>,
}


/// Given an RGB triple, return the position in the texture of the center of the
/// circle of radius one with that color.
fn color_to_circle((r, g, b): (u8, u8, u8)) -> (f32, f32) {
    // Take the upper four bits of each component, and combine them into a
    // twelve-bit value.
    let (r, g, b) = ((r >> 4) as u32, (g >> 4) as u32, (b >> 4) as u32);
    let index = r << 8 | g << 4 | b;

    // Space out the circles by sqrt(MAX_GOOP).
    ((index + 1) as f32 * (MAX_GOOP as f32).sqrt(), 0.0)
}

/// A type that can be constructed from a coordinate pair.
trait TwoD {
    fn new(x: f32, y: f32) -> Self;
}

impl TwoD for GraphVert {
    fn new(x: f32, y: f32) -> Self { GraphVert { point: [x, y] } }
}

impl TwoD for TextureVert {
    fn new(x: f32, y: f32) -> Self { TextureVert { texture: [x, y] } }
}

// Push onto the end of `vec` the coordinates of the corners of an
// axis-aligned square with the given `center` and a side length of `2 *
// radius`. The corners are pushed in counterclockwise order, starting
// in the first quadrant.
fn push_corners<T: TwoD>(vec: &mut Vec<T>, center: (f32, f32), radius: f32) {
    vec.push(T::new(center.0 + radius, center.1 + radius));
    vec.push(T::new(center.0 - radius, center.1 + radius));
    vec.push(T::new(center.0 - radius, center.1 - radius));
    vec.push(T::new(center.0 + radius, center.1 - radius));
}


impl GoopDrawer {
    fn new<G>(display: &Facade, map: &Map<G>) -> Result<GoopDrawer>
        where G: VisibleGraph
    {
        let program = Program::from_source(display,
                                           include_str!("goop.vert"),
                                           include_str!("goop.frag"),
                                           None)
            .chain_err(|| "compiling outflow shaders")?;

        let graph = &map.graph;

        // Don't take up the node's full area.
        let radius = graph.radius() * 0.8;

        let mut squares = Vec::with_capacity(graph.nodes() * 4);
        for node in 0 .. graph.nodes() {
            let GraphPt(x, y) = graph.center(node);
            push_corners(&mut squares, (x, y), radius);
        }
        let squares = VertexBuffer::new(display, &squares)
            .chain_err(|| "building vertex buffer for goop squares")?;

        let textures = VertexBuffer::empty_persistent(display, squares.len())
            .chain_err(|| "allocating vertex buffer for goop textures")?;

        let mut indices = Vec::with_capacity(graph.nodes() * 6);
        for node in 0 .. graph.nodes() {
            // Index of `node`'s first vertex in `squares` and in `textures`.
            let base = node * 4;

            // Upper-left triangle.
            indices.push((base + 0) as u32);
            indices.push((base + 1) as u32);
            indices.push((base + 2) as u32);

            // Lower-right triangle.
            indices.push((base + 2) as u32);
            indices.push((base + 3) as u32);
            indices.push((base + 0) as u32);
        }
        let indices = IndexBuffer::new(display,
                                       PrimitiveType::TrianglesList,
                                       &indices)
            .chain_err(|| "allocating goop index buffer")?;

        let draw_params = Default::default();

        Ok(GoopDrawer { program, squares,
                        textures: RefCell::new(textures),
                        indices, draw_params })
    }

    fn draw<G>(&self, frame: &mut Frame, to_device: &[[f32; 3]; 3], nodes: &[Option<OwnedNode>], map: &Map<G>)
            -> Result<()>
        where G: VisibleGraph
    {
        assert_eq!(nodes.len(), map.graph.nodes());

        let mut textures = Vec::with_capacity(nodes.len() * 4);
        for state in nodes {
            match state {
                &Some(ref owned) if owned.goop > 0 => {
                    // Find the center of the circle of this player's color.
                    let center = color_to_circle(map.player_colors[owned.player.0]);

                    // Compute the radius of a circle whose area is MAX_GOOP
                    // if a unit circle has an area of `goop`.
                    let max_radius = (MAX_GOOP as f32 / owned.goop as f32).sqrt();

                    push_corners(&mut textures, center, max_radius);
                }
                _ => {
                    // This node holds no goop. Set its texture coordinates to
                    // refer to a blank part of the texture. The shader ensures
                    // that everything to the left of the y axis is blank.
                    push_corners(&mut textures, (-2.0, 0.0), 1.0);
                }
            }
        }
        assert_eq!(textures.len(), textures.capacity());

        self.textures.borrow_mut().write(&textures);
        frame.draw((&self.squares, &*self.textures.borrow()),
                   &self.indices,
                   &self.program,
                   &uniform! {
                       graph_to_device: *to_device,
                       circle_spacing: (MAX_GOOP as f32).sqrt()
                   },
                   &self.draw_params)
            .chain_err(|| "drawing goop")?;

        Ok(())
    }
}
