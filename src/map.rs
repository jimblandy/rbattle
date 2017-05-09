use graph::Node;
use math::{compose, inverse, translate_transform, scale_transform};
use visible_graph::{GraphPt, VisibleGraph};
use square::SquareGrid;

/// A map on which an RBattle game is played.
///
/// A `Map` holds everything that does not change over the course of an RBattle
/// game. This includes a graph, and a set of nodes that have goop sources.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Map {
    /// The graph of nodes comprising this map's territory.
    pub graph: SquareGrid,

    /// The nodes of `graph` that contain goop sources.
    pub sources: Vec<Node>,

    /// Coordinate transformation from graph space to game space.
    pub graph_to_game: [[f32; 3]; 3],

    /// Coordinate transformation from game space to graph space.
    /// The inverse of the above.
    pub game_to_graph: [[f32; 3]; 3],

    /// The aspect ratio (width / height) of the game rectangle.
    pub game_aspect: f32,

    /// The color of each player's goop, indexed by player number.
    pub player_colors: Vec<(u8, u8, u8)>,
}

impl Map {
    pub fn new(params: MapParameters) -> Map {
        let MapParameters { size, sources, player_colors } = params;
        let graph = SquareGrid::new(size.0, size.1);

        // Compute the transformation from graph space, where points run from
        // (0, 0) to upper_right, to game space, where points run from (-1, -1)
        // to (1,1).
        let GraphPt(bounds) = graph.bounds();
        let (width, height) = (bounds[0], bounds[1]); // dumb; see issue #23121
        let game_aspect = width / height;
        let graph_to_game =
            compose(translate_transform(-1.0, -1.0),
                    scale_transform(2.0 / width, 2.0 / height));

        // A little margin inside the window is nice.
        let graph_to_game = compose(scale_transform(0.95, 0.95), graph_to_game);

        let game_to_graph = inverse(graph_to_game)
            .expect("graph_to_game transformation should be invertible");

        Map { graph, sources, graph_to_game,
              game_to_graph, game_aspect, player_colors }
    }
}

/// A set of parameters that can be used to initialize a map.
pub struct MapParameters {
    /// The dimensions of the board.
    pub size: (usize, usize),

    /// The position of the sources on the board. The number of players is the
    /// length of this vector.
    pub sources: Vec<Node>,

    /// The color assigned to each player, as an RGB triplet. This must be the
    /// same length as `sources`.
    pub player_colors: Vec<(u8, u8, u8)>
}
