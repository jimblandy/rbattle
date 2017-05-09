//! Game state that is synchronized between several different players.

use state::{Action, Player, State};
use map::MapParameters;

/// A `SynchronizedState` takes a `State` and communicates with other players to
/// ensure that actions and turns are applied consistently for everyone.
///
/// A given `SynchronizedState` can either be a client or a server. A
/// synchronized group of hosts has one server and N clients. If a
/// `SynchronizedState` is a client, then it receives the game's parameters from
/// the server; if it is a server, it establishes the game's parameters itself,
/// and shares them out to the clients.
///
/// There are two operations you can perform on a `SynchronizedState` once it
/// exits:
///
/// - You can request that a `state::Action` be carried out at some point in the
///   future.
///
/// - You can ask for a snapshot of the current `State`.
///
/// Internally, a `SynchronizedState` starts up a separate thread to handle
/// network interaction.
pub struct SynchronizedState {
    /// The player this state represents. Assigned by the server.
    player: Player,

    /// The current state of the game.
    state: State
}

impl SynchronizedState {
    /// Create a new client SynchronizedState, talking to the server whose
    /// address is `server`.
    //fn new_client(server: SocketAddr) -> SynchronizedState { }

    /// Create a new server SynchronizedState, listening on the address
    /// `server`, using `parameters` to construct the initial game state.
    pub fn new_server(/* server: SocketAddr, */ parameters: MapParameters)
                  -> SynchronizedState
    {
        SynchronizedState {
            player: Player(0),
            state: State::new(parameters)
        }
    }

    /// Return a snapshot of the current state.
    pub fn snapshot(&self) -> State { self.state.clone() }

    /// Return the player number of this SynchronizedState.
    pub fn get_player(&self) -> Player { self.player }

    /// Submit `action` to be performed as soon as possible.
    pub fn request_action(&mut self, action: Action) {
        self.state.take_action(&action);
    }

    pub fn advance(&mut self) {
        self.state.advance();
    }
}
