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
use state::{SerializableState, State};

use futures::{Future};
use futures::future::ok;
use futures::sync::oneshot;
use tokio_proto::TcpServer;
use tokio_service::Service;

use std::io::{Error, ErrorKind};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

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
    Welcome { player: usize, state: SerializableState },
    GameFull,
    Turn(CollectedActions)
}

/// This implements scheduler::Notifier, so that Scheduler can tell
/// SchedulerService when a client should receive a Response::Turn message.
struct OneshotNotifier {
    sender: oneshot::Sender<Response>
}

impl Notifier for OneshotNotifier {
    fn notify(self: Box<Self>, turn: CollectedActions) {
        self.sender.send(Response::Turn(turn))
            .expect("oneshot notifier receiver died");
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
                let notifier = OneshotNotifier { sender };
                let mut guard = self.scheduler.lock().unwrap();
                guard.submit_actions(actions, Box::new(notifier));

                // Turn oneshot errors into io::Error, as this service requires.
                let receiver = receiver.map_err(|e| Error::new(ErrorKind::Other, e));

                Box::new(receiver)
            }
        }
    }
}

pub fn start_server(addr: SocketAddr,
                    parameters: MapParameters) {
    let initial_state = State::new(parameters);
    let scheduler = Arc::new(Mutex::new(Scheduler::new(initial_state)));

    let server = TcpServer::new(JsonProto::<Request, Response>::new(), addr);
    server.serve(move || Ok(SchedulerService { scheduler: scheduler.clone() }));
}
