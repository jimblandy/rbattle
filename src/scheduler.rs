//! Scheduling game play.

use state::Player;
use state::{Action, State, SerializableState};

use std::mem::replace;
use std::thread;
use std::time::{Duration, Instant};

/// The shortest amount of time a turn is allowed to take, in nanoseconds.
const MIN_DELAY_NS: u32 = 016_000_000;

/// A `Scheduler` collects actions from all players, and then broadcasts the
/// full list once everyone has submitted their moves for that turn.
///
/// When a player submits their moves, they provide a `Sender` on which
/// `Scheduler` should send the full move list once it is available.
pub struct Scheduler {
    /// The number of the last turn we broadcast out.
    turn: usize,

    /// A scheduler actually maintains its own copy of the game state, for
    /// generating checksums to send to clients.
    state: State,

    /// A vector recording submitted actions and reply channels for every joined
    /// player; the `i`'th element is for `Player(i)`. Once this has actions for
    /// every joined player, we apply all the actions to our state in a given
    /// order, compute the new state's checksum, and then transmit the collected
    /// moves to all the players.
    pending_actions: Vec<Option<(PlayerActions, Box<Notifier + Send>)>>,

    /// The last time we broadcast out turns to everyone. We make sure not
    /// to send out the next move until at least MIN_DELAY_NS after this time.
    last_broadcast: Instant,
}

/// Something that can notify a player of a turn's actions when they have been
/// collected.
pub trait Notifier {
    fn notify(self: Box<Self>, turn: CollectedActions);
}

impl Scheduler {
    pub fn new(initial_state: State) -> Scheduler {
        Scheduler { turn: 0, state: initial_state, pending_actions: vec![],
                    last_broadcast: Instant::now()
        }
    }

    // Add another player to the game. If there is room, return the player's
    // number and a representation of the current game state. Return `None` if
    // there is no room for more players.
    pub fn player_join(&mut self) -> Option<(Player, SerializableState)> {
        if self.pending_actions.len() >= self.state.max_players() {
            None
        } else {
            self.pending_actions.push(None);
            Some((Player(self.pending_actions.len() - 1), self.state.serializable()))
        }
    }

    // Submit `actions` to be carried out as soon as possible. When all players'
    // actions have been collected, send the full list to `reply_to`.
    pub fn submit_actions(&mut self,
                          actions: PlayerActions,
                          reply_to: Box<Notifier + Send>) {
        assert_eq!(actions.turn, self.turn);
        assert!(self.pending_actions[actions.player.0].is_none());
        let player = actions.player.0;
        self.pending_actions[player] = Some((actions, reply_to));

        // Have all the players that have joined finally submitted an action?
        if self.pending_actions.iter().all(|o| o.is_some()) {

            // Make sure at least MIN_DELAY_NS nanoseconds have elapsed since
            // our last broadcast.
            let now = Instant::now();
            let since_last = now - self.last_broadcast;
            if since_last < Duration::new(0, MIN_DELAY_NS) {
                thread::sleep(Duration::new(0, MIN_DELAY_NS) - since_last);
            }

            // Grab the list of pending actions and reset it for the next turn.
            let pendings = replace(&mut self.pending_actions, vec![]);

            // Collect all the actions into a single vector,
            // collect all the reply-to's in another vector,
            // and apply all the actions to our state.
            let mut collected_reply_tos = Vec::new();
            let mut collected_actions = Vec::new();

            for player in pendings {
                let (player_actions, reply_to) = player.unwrap();
                for action in player_actions.actions {
                    self.state.take_action(&action);
                    collected_actions.push(action);
                }
                collected_reply_tos.push(reply_to);
                self.pending_actions.push(None);
            }
            self.state.advance();

            let state_checksum = self.state.checksum();

            // We are now in the new turn.
            self.turn += 1;

            let collected = CollectedActions {
                turn: self.turn,
                actions: collected_actions,
                state_checksum
            };

            // Broadcast out the new state of the world to all players.
            for reply_to in collected_reply_tos {
                reply_to.notify(collected.clone());
            }

            self.last_broadcast = now;
        }
    }
}




/// A set of actions submitted by a single player on a single turn.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerActions {
    // The player submitting these actions.
    pub player: Player,

    // The turn number they believe they're on.
    pub turn: usize,

    // The actions they wish to submit.
    pub actions: Vec<Action>,
}

/// A collection of all actions submitted by all players.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollectedActions {
    // The turn these moves produce when applied.
    pub turn: usize,

    // The actions to apply to the prior state.
    pub actions: Vec<Action>,

    // The hash value of the State that should result, as a checksum.
    pub state_checksum: u64
}
