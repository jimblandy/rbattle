//! The rbattle game protocol.
//!
//! All hosts participating in an rbattle game have a complete copy of the
//! state, and the algorithm for evolving the state from one moment to the next
//! is strictly deterministic.
//!
//! Every effect the player can have on the game is represented as a
//! `state::Action` value. Given an initial state, and a record of which actions
//! were applied by which players on which turns, you can exactly recreate the
//! progression of the game. Since rbattle is such a simple game, there's only
//! one kind of action: toggling an outflow.
//!
//! In that light, the protocol focuses on gathering user actions, and
//! distributing them out in a way that helps apply them consistently. The
//! protocol has no means for communicating the game state. The only record of
//! the game state transmitted at all is a hash value to detect divergence,
//! which causes the game to end.
//!
//! For simplicity, we designate one host as the server; the protocol doesn't
//! provide for any resilience if the server goes down. All other hosts have TCP
//! connections to the server only.
//!
//! Game play is organized into 'turns', where turns are scheduled at fixed
//! intervals. (We'll aim for 33ms per turn, or 30 turns/second, and see how
//! that goes.) Clients send the server an action list every turn, even if it's
//! an empty action list; and the server broadcasts out the collected action
//! list for every turn, even if it's empty.
//!
//! The server is responsible for coordinating timing. For a given turn duration
//! T, the server broadcasts the list of gathered actions as soon as they are
//! available, but no sooner then T after the last broadcast.
//!
//! Clients should apply received action lists as soon as they are received,
//! advance their state, and send any collected actions immediately.

use map::MapParameters;
use jsonproto::JsonProto;
use scheduler::{CollectedActions, Notifier, PlayerActions, Scheduler};
use state::{Action, Player, SerializableState, State};

use futures::{Future};
use futures::future::ok;
use futures::sync::oneshot;
use serde_json;
use tokio_proto::TcpServer;
use tokio_service::Service;

use std::io::{BufRead, BufReader, BufWriter, Error, ErrorKind, Write};
use std::mem::replace;
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, mpsc, Mutex};
use std::thread;

#[derive(Clone)]
struct SchedulerService {
    scheduler: Arc<Mutex<Scheduler>>
}

/// Requests the server receives from clients.
#[derive(Debug, Serialize, Deserialize)]
enum Request {
    Join,
    Actions(PlayerActions),
}

/// The server's responses to those requests.
#[derive(Debug, Serialize, Deserialize)]
enum Response {
    Welcome { player: Player, state: SerializableState },
    GameFull,
    Turn(CollectedActions)
}

/// This impl allows `Scheduler` to resolve promises returned by
/// SchedulerService::call.
impl Notifier for oneshot::Sender<Response> {
    fn notify(self: Box<Self>, turn: CollectedActions) {
        self.send(Response::Turn(turn))
            .expect("oneshot notifier receiver died");
    }
}

/// This impl allows `Scheduler` to send the actions collected for a turn to the
/// local game.
impl Notifier for mpsc::Sender<CollectedActions> {
    fn notify(self: Box<Self>, turn: CollectedActions) {
        self.send(turn)
            .expect("mpsc notifier receiver died");
    }
}

impl Service for SchedulerService {
    type Request = Request;
    type Response = Response;
    type Error = Error;
    type Future = Box<Future<Item=Response, Error=Error>>;

    fn call(&self, req: Request) -> Self::Future {
        match req {
            Request::Join => {
                let mut guard = self.scheduler.lock().unwrap();
                match guard.player_join() {
                    Some((player, state)) =>
                        Box::new(ok(Response::Welcome { player, state })),
                    None =>
                        Box::new(ok(Response::GameFull))
                }
            },
            Request::Actions(actions) => {
                let (sender, receiver) = oneshot::channel();
                let mut guard = self.scheduler.lock().unwrap();
                guard.submit_actions(actions, Box::new(sender));

                // Turn oneshot errors into io::Error, as this service requires.
                let receiver = receiver.map_err(|e| Error::new(ErrorKind::Other, e));

                Box::new(receiver)
            }
        }
    }
}

/// Information shared between the main thread and helper threads.
struct Shared {
    /// The player this state represents. Assigned by the server.
    player: Player,

    /// The current state of the game.
    state: State,

    /// The queue of actions to be sent to the scheduler on the next turn.
    pending: Vec<Action>
}

impl Shared {
    fn apply_collected_actions(&mut self,
                               collected_actions: CollectedActions)
                               -> PlayerActions
    {
        assert_eq!(self.state.turn + 1, collected_actions.turn);

        for action in collected_actions.actions {
            self.state.take_action(&action);
        }
        self.state.advance();

        // We should have applied the same actions to the same state,
        // and gotten the same checksum.
        assert_eq!(self.state.checksum(),
                   collected_actions.state_checksum,
                   "Game state checksums have diverged!");

        // Now that we've applied the actions from the prior turn, return
        // whatever actions have been queued up in the mean time as our next
        // turn.
        PlayerActions {
            player: self.player,
            turn: self.state.turn,
            actions: replace(&mut self.pending, vec![])
        }
    }
}

pub struct Participant {
    /// The player on the local machine.
    player: Player,

    /// Information shared between the main thread, the server thread, and the
    /// scheduler thread.
    shared: Arc<Mutex<Shared>>,
}

impl Participant {
    pub fn new_server(addr: SocketAddr, params: MapParameters) -> Participant {
        assert!(params.player_colors.len() >= 1);

        // Create a scheduler to coordinate turns amongst the players,
        // and add ourselves as the first player.
        let mut scheduler = Scheduler::new(State::new(params));
        let (player, current_state) = scheduler.player_join().unwrap();

        let scheduler = Arc::new(Mutex::new(scheduler));

        let shared = Arc::new(Mutex::new(Shared {
            player,
            state: State::from_serializable(current_state),
            pending: vec![]
        }));

        let (sender, receiver): (mpsc::Sender<CollectedActions>, _) = mpsc::channel();

        // Create a thread to apply actions received from the scheduler.
        // These variables get moved into the closure.
        let shared_handle = shared.clone();
        let scheduler_handle = scheduler.clone();
        let sender_handle = sender.clone();
        thread::spawn(move || {
            for collected_actions in receiver {
                let mut guard = shared_handle.lock().unwrap();
                let next_actions = guard.apply_collected_actions(collected_actions);

                // Drop the guard on the shared data first, to avoid having to
                // think about lock ordering.
                drop(guard);

                // Submit any requested next actions for the next turn.
                let mut guard = scheduler_handle.lock().unwrap();
                guard.submit_actions(next_actions, Box::new(sender_handle.clone()));
            }
        });

        // Spawn off a second thread to run the server.
        // This variable gets moved into the closure.
        let scheduler_handle = scheduler.clone();
        thread::spawn(move || {
            let server = TcpServer::new(JsonProto::<Request, Response>::new(), addr);
            server.serve(move || {
                Ok(SchedulerService { scheduler: scheduler_handle.clone() })
            });
        });

        // Get the ball rolling by submitting an empty first move.
        {
            let mut guard = scheduler.lock().unwrap();
            let actions = PlayerActions {
                player,
                turn: 0,
                actions: vec![]
            };
            guard.submit_actions(actions, Box::new(sender));
        }

        Participant { player, shared }
    }

    pub fn new_client(addr: SocketAddr) -> Result<Participant, Error> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true)?;

        let (sender, receiver) = mpsc::channel();

        fn setup(reader: &mut BufReader<&TcpStream>, writer: &mut BufWriter<&TcpStream>)
                 -> Result<Shared, Error>
        {
            writeln!(writer, "{}", serde_json::to_string(&Request::Join)?)?;
            writer.flush()?;
            let mut response = String::new();
            reader.read_line(&mut response)?;
            let response = serde_json::from_str(&response)?;
            let (player, state) = match response {
                Response::GameFull => {
                    return Err(Error::new(ErrorKind::Other,
                                          "Connection rejected, game full."));
                }
                Response::Welcome { player, state } => (player, state),
                Response::Turn(_) => {
                    return Err(Error::new(ErrorKind::Other,
                                          "Received unexpected Response::Turn on Join"));
                }
            };

            let shared = Shared {
                player,
                state: State::from_serializable(state),
                pending: vec![]
            };

            // Get the ball rolling by submitting an empty first move.
            let actions = PlayerActions {
                player,
                turn: shared.state.turn,
                actions: vec![]
            };
            writeln!(writer, "{}",
                     serde_json::to_string(&Request::Actions(actions))?)?;
            writer.flush()?;

            Ok(shared)
        }

        // Spawn a thread to read collected actions, apply them to our state,
        // and submit any accumulated actions requested.
        thread::spawn(move || {
            let stream = stream; // take ownership
            let mut reader = BufReader::new(&stream);
            let mut writer = BufWriter::new(&stream);

            let shared = match setup(&mut reader, &mut writer) {
                Err(e) => {
                    sender.send(Err(e)).unwrap();
                    return;
                }
                Ok(shared) => shared
            };

            let player = shared.player;
            let shared = Arc::new(Mutex::new(shared));
            sender.send(Ok((player, shared.clone()))).unwrap();
            drop(sender);

            for line in reader.lines() {
                let line = line.expect("error reading response from server");
                let response: Response = serde_json::from_str(&line)
                    .expect("error parsing response from server");
                let collected_actions = match response {
                    Response::Turn(collected_actions) => collected_actions,
                    otherwise => {
                        panic!("Unexpected response from server: {:?}", otherwise);
                    }
                };

                let mut guard = shared.lock().unwrap();
                let next_actions = guard.apply_collected_actions(collected_actions);

                // Drop the guard on the shared data first, to avoid having to
                // think about lock ordering.
                drop(guard);

                // Submit any requested next actions for the next turn.
                let actions = serde_json::to_string(&Request::Actions(next_actions))
                    .expect("failed to jsonify next actions");
                writeln!(writer, "{}", actions)
                    .expect("Sending next actions to server");
                writer.flush().unwrap();
            }
        });

        let (player, shared) = receiver.recv().unwrap()?;

        Ok(Participant { player, shared })
    }

    /// Return a snapshot of the current state.
    pub fn snapshot(&self) -> State {
        let guard = self.shared.lock().unwrap();
        guard.state.clone()
    }

    /// Return the player number of this SynchronizedState.
    pub fn get_player(&self) -> Player { self.player }

    /// Submit `action` to be performed as soon as possible.
    pub fn request_action(&mut self, action: Action) {
        let mut guard = self.shared.lock().unwrap();
        guard.pending.push(action);
    }
}
