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
use graph::Graph;
use map::Map;
use state::{State, MAX_GOOP, Occupied};
use math::{compose, inverse, midpoint, scale_transform, translate_transform};
use mouse::{Mouse, Display, OutflowState};
use visible_graph::{GraphPt, VisibleGraph};

use glium::{Blend, DrawParameters, Frame, IndexBuffer, Program, Surface, VertexBuffer};
use glium::backend::Facade;
use glium::index::{NoIndices, PrimitiveType};

use std::cell::RefCell;
use std::time::Duration;

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

    /// Cached information for drawing mouse interaction.
    mouse: MouseDrawer,
}

impl Drawer {
    pub fn new(display: &Facade, map: &Map) -> Result<Drawer>
    {
        let map_drawer = MapDrawer::new(display, map)?;
        let outflows = OutflowsDrawer::new(display, map)?;
        let goop = GoopDrawer::new(display, map)?;
        let mouse = MouseDrawer::new(display, map)?;

        Ok(Drawer { map: map_drawer, outflows, goop, mouse })
    }

    /// Draw `state` on `frame`
    ///
    /// Return the current transformation from window coordinates to game
    /// coordinates, for use by the controller.
    pub fn draw(&self,
                frame: &mut Frame,
                time: Duration,
                state: &State,
                mouse: &Mouse) -> Result<[[f32; 3]; 3]>
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
        self.goop.draw(frame, &graph_to_device, time, &state.nodes, &state.map)?;
        self.outflows.draw(frame, &graph_to_device, &state.nodes, &state.map)?;
        self.mouse.draw(frame, &graph_to_device, state, mouse)?;

        // Compute the transformation from window coordinates (pixels) to game
        // coordinates, for the mouse handling to use. In window coordinates:
        //
        // - The positive y axis points down, not up.
        // - The origin is at the upper left, not in the center.
        // - Values range from (0,0) to (width,height), not (-1,-1) to (1,1).
        //
        // We compute this in two steps: first the transformation from window
        // coordinates to normalized device coordinates, and then the
        // transformation from there to game coordinates.
        let window_to_device
            = compose(translate_transform(-1.0, 1.0),
                      scale_transform(2.0 / (width as f32), -2.0 / (height as f32)));
        let device_to_game = inverse(game_to_device)
            .expect("graph_to_game transformation should be invertible");

        let window_to_game = compose(device_to_game, window_to_device);

        Ok(window_to_game)
    }
}

struct MapDrawer {
    /// Shader program for drawing the map.
    program: Program,

    /// Vertices of the graph's boundary lines.
    vertices: VertexBuffer<GraphVertex>,

    /// Indices for the graph's boundary lines.
    indices: IndexBuffer<u32>,

    /// Draw parameters for drawing the map.
    draw_params: DrawParameters<'static>
}

impl MapDrawer {
    fn new(display: &Facade, map: &Map) -> Result<MapDrawer>
    {
        let graph = &map.graph;

        let program = Program::from_source(display,
                                           include_str!("map.vert"),
                                           include_str!("map.frag"),
                                           None)
            .chain_err(|| "compiling map shaders")?;

        // It's a little annoying that we have to do this map to convert GraphPt
        // to GraphVertex, but I'd rather do this than a transmute.
        let vertices: Vec<GraphVertex> = graph.endpoints().into_iter()
            .map(|GraphPt(point)| GraphVertex { point })
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
    fn draw(&self, frame: &mut Frame, to_device: &[[f32; 3]; 3], _map: &Map) -> Result<()>
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
struct GraphVertex { point: [f32; 2] }

implement_vertex!(GraphVertex, point);

struct OutflowsDrawer {
    /// Shader program for drawing the outflows.
    program: Program,

    /// Vertices of the outflows' endpoints.
    vertices: RefCell<VertexBuffer<GraphVertex>>,

    /// Draw parameters for outflows.
    draw_params: DrawParameters<'static>
}

impl OutflowsDrawer {
    fn new(display: &Facade, map: &Map) -> Result<OutflowsDrawer>
    {
        let graph = &map.graph;

        let program = Program::from_source(display,
                                           include_str!("map.vert"),
                                           include_str!("outflow.frag"),
                                           None)
            .chain_err(|| "compiling outflow shaders")?;

        let vertices = VertexBuffer::empty_persistent(display,
                                                      2 * graph.edges())
            .chain_err(|| "allocating outflow vertex buffer")?;

        let draw_params = DrawParameters {
            line_width: Some(5.0),
            .. Default::default()
        };

        Ok(OutflowsDrawer {
            program,
            vertices: RefCell::new(vertices),
            draw_params
        })
    }

    fn draw(&self,
            frame: &mut Frame,
            to_device: &[[f32; 3]; 3],
            nodes: &[Option<Occupied>],
            map: &Map)
            -> Result<()>
    {
        // Build vertex positions for all goop outflows.
        let mut vertices = Vec::new();
        for (node, state) in nodes.iter().enumerate() {
            match state {
                &Some(ref occupied) => {
                    let GraphPt(start) = map.graph.center(node);
                    for &outflow in &occupied.outflows {
                        let GraphPt(end) = map.graph.center(outflow);
                        let mid = midpoint(start, end);

                        vertices.push(GraphVertex { point: start });
                        vertices.push(GraphVertex { point: mid });
                    }
                },
                _ => ()
            }
        }

        // Glium seems to have a bug with zero-length slices. Let's not argue
        // with it.
        if vertices.len() > 0 {
            // Write the indices to an appropriately sized slice of `self.indices`.
            self.vertices.borrow_mut().slice_mut(0..vertices.len())
                .expect("more outflow edges than graph claimed")
                .write(&vertices);

            frame.draw(self.vertices.borrow().slice(0..vertices.len()).unwrap(),
                       &NoIndices(PrimitiveType::LinesList),
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

/// A point in UV space. A parameter passed to fragment shaders.
#[derive(Copy, Clone, Debug)]
struct UVVertex { vertex_uv: [f32; 2] }

implement_vertex!(UVVertex, vertex_uv);

/// Cached information about drawing the levels of goop present at each node.
///
/// We draw goop levels by placing a square (two triangles) on each node large
/// enough to cover the largest goop circle we'd like to draw. We then pretend
/// we have a texture containing a circle of radius 1, and set the texture
/// coordinates on the squares vertices to draw the circle sized appropriately.
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

    /// Vertices for the squares on each node, without texture coordinates.
    /// These are a function of the map, and so are fixed from one frame to the
    /// next. The vertices for node `i` are at `4*i .. 4*i + 4`, going
    /// counterclockwise through the quadrants.
    squares: VertexBuffer<GraphVertex>,

    /// Vertices of the texture coordinates of each node's square. Parallel to
    /// the `squares` vertex buffer. This is a "persistent" vertex buffer: its
    /// contents change on each frame, based on goop levels.
    textures: RefCell<VertexBuffer<UVVertex>>,

    /// Index buffer for the squares on nodes. This is a function of the map,
    /// and is fixed from one frame to the next. The triangles for node `i` are
    /// at `6*i .. 6*i + 3` and `6*i + 3 .. 6*i + 6`.
    indices: IndexBuffer<u32>,

    /// Draw parameters for goop squares.
    draw_params: DrawParameters<'static>,
}


/// Given an RGB triple, return the position in the texture of the center of the
/// circle of radius one with that color.
fn color_to_circle((r, g, b): (u8, u8, u8)) -> [f32; 2] {
    // Take the upper four bits of each component, and combine them into a
    // twelve-bit value.
    let (r, g, b) = ((r >> 4) as u32, (g >> 4) as u32, (b >> 4) as u32);
    let index = r << 8 | g << 4 | b;

    // Space out the circles by MAX_GOOP, just to be safe.
    [(index + 1) as f32 * (MAX_GOOP as f32), 0.0]
}

/// A type that can be constructed from a coordinate pair.
trait TwoD {
    fn new(x: f32, y: f32) -> Self;
}

impl TwoD for GraphVertex {
    fn new(x: f32, y: f32) -> Self { GraphVertex { point: [x, y] } }
}

impl TwoD for UVVertex {
    fn new(x: f32, y: f32) -> Self { UVVertex { vertex_uv: [x, y] } }
}

// Push onto the end of `vec` the coordinates of the corners of an
// axis-aligned square with the given `center` and a side length of `2 *
// radius`. The corners are pushed in counterclockwise order, starting
// in the first quadrant.
fn push_corners<T: TwoD>(vec: &mut Vec<T>, center: [f32; 2], radius: f32) {
    vec.push(T::new(center[0] + radius, center[1] + radius));
    vec.push(T::new(center[0] - radius, center[1] + radius));
    vec.push(T::new(center[0] - radius, center[1] - radius));
    vec.push(T::new(center[0] + radius, center[1] - radius));
}


impl GoopDrawer {
    fn new(display: &Facade, map: &Map) -> Result<GoopDrawer>
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
            push_corners(&mut squares, graph.center(node).0, radius);
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

    fn draw(&self,
            frame: &mut Frame,
            to_device: &[[f32; 3]; 3],
            time: Duration,
            nodes: &[Option<Occupied>],
            map: &Map) -> Result<()>
    {
        assert_eq!(nodes.len(), map.graph.nodes());

        let mut textures = Vec::with_capacity(nodes.len() * 4);
        for state in nodes {
            match state {
                &Some(ref occupied) if occupied.goop > 0 => {
                    // Find the center of the circle of this player's color.
                    let center = color_to_circle(map.player_colors[occupied.player.0]);

                    // Compute the radius of a circle whose area is MAX_GOOP
                    // if a unit circle has an area of `goop`.
                    let max_radius = (MAX_GOOP as f32 / occupied.goop as f32).sqrt();

                    push_corners(&mut textures, center, max_radius);
                }
                _ => {
                    // This node holds no goop. Set its texture coordinates to
                    // refer to a blank part of the texture. The shader ensures
                    // that the leftmost circle is at the origin, so everything
                    // to the left of the y axis is blank.
                    push_corners(&mut textures, [-(MAX_GOOP as f32), 0.0], 1.0);
                }
            }
        }
        assert_eq!(textures.len(), textures.capacity());

        let time_as_float =
            time.as_secs() as f32 + time.subsec_nanos() as f32 / 1e9;

        self.textures.borrow_mut().write(&textures);
        frame.draw((&self.squares, &*self.textures.borrow()),
                   &self.indices,
                   &self.program,
                   &uniform! {
                       graph_to_device: *to_device,
                       circle_spacing: MAX_GOOP as f32,
                       time: time_as_float
                   },
                   &self.draw_params)
            .chain_err(|| "drawing goop")?;

        Ok(())
    }
}

/// Graphics state for drawing mouse interactions.
///
/// Our mouse interactions are pretty simple. The `mouse::Display` enum
/// specifies what state the interface is in, and it's up to this type to decide
/// what that state looks like:
///
/// - Hover(outflow): draw outflow in a light, transparent gray.
///
/// - Active(outflow): Draw outflow in a solid yellow.
struct MouseDrawer {
    /// Shader program for drawing outflows being clicked upon.
    program: Program,

    /// Vertices of the outflow.
    outflow: RefCell<VertexBuffer<GraphVertex>>,
}

impl MouseDrawer {
    fn new(display: &Facade, _map: &Map) -> Result<MouseDrawer>
    {
        let program = Program::from_source(display,
                                           include_str!("map.vert"),
                                           include_str!("mouse.frag"),
                                           None)
            .chain_err(|| "compiling mouse shaders")?;

        let outflow = VertexBuffer::empty_persistent(display, 2)
            .chain_err(|| "allocating mouse vertex buffer")?;

        Ok(MouseDrawer { program, outflow: RefCell::new(outflow) })
    }

    fn draw(&self, frame: &mut Frame,
            to_device: &[[f32; 3]; 3],
            state: &State,
            mouse: &Mouse) -> Result<()>
    {
        match mouse.display(state) {
            Display::Nothing => Ok(()),

            Display::Outflow { nodes: (from, to), state: outflow_state } => {
                // Prepare the vertices.
                let graph = &state.map.graph;
                let GraphPt(start) = graph.center(from);
                let GraphPt(end) = graph.center(to);
                let mid = midpoint(start, end);
                let outflow = [GraphVertex { point: start },
                               GraphVertex { point: mid }];
                self.outflow.borrow_mut().write(&outflow);

                match outflow_state {
                    OutflowState::Hover => {
                        frame.draw(&*self.outflow.borrow(),
                                   &NoIndices(PrimitiveType::LinesList),
                                   &self.program,
                                   &uniform! {
                                       graph_to_device: *to_device,
                                       // transparent black
                                       color: [0.0_f32, 0.0, 0.0, 0.5],
                                   },
                                   &DrawParameters {
                                       line_width: Some(5.0),
                                       blend: Blend::alpha_blending(),
                                       .. Default::default()
                                   })
                            .chain_err(|| "drawing hover mouse outflow")
                    }

                    OutflowState::Active => {
                        frame.draw(&*self.outflow.borrow(),
                                   NoIndices(PrimitiveType::LinesList),
                                   &self.program,
                                   &uniform! {
                                       graph_to_device: *to_device,
                                       // yellow
                                       color: [0.94_f32, 0.96, 0.0, 1.0],
                                   },
                                   &DrawParameters {
                                       line_width: Some(5.0),
                                       .. Default::default()
                                   })
                            .chain_err(|| "drawing active mouse outflow")
                    }
                }
            }
        }
    }
}
