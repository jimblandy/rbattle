#![allow(dead_code)]

// `error_chain!` can recurse deeply
#![recursion_limit = "1024"]

#[macro_use] extern crate error_chain;
#[macro_use] extern crate glium;
#[macro_use] extern crate serde_derive;
extern crate bytes;
extern crate futures;
extern crate rand;
extern crate serde;
extern crate serde_json;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_proto;
extern crate tokio_service;

#[cfg(test)]
#[macro_use]
mod test_utils;

mod drawer;
mod errors;
mod graph;
mod jsonproto;
mod map;
mod math;
mod mouse;
mod protocol;
mod scheduler;
mod square;
mod state;
mod visible_graph;
mod xorshift;

use drawer::Drawer;
use map::MapParameters;
use math::{apply, compose};
use mouse::Mouse;
use protocol::Participant;
use state::Player;
use visible_graph::GraphPt;

use glium::glutin::{Event, ElementState, MouseButton};
use glium::Surface;

use std::io::Write;
use std::net::SocketAddr;

// This only gives access within this module. Make this `pub use errors::*;`
// instead if the types must be accessible from other modules (e.g., within
// a `links` section).
use errors::*;

fn main() {
    if let Err(ref e) = run() {
        use ::std::io::Write;
        let stderr = &mut ::std::io::stderr();
        let errmsg = "Error writing to stderr";

        writeln!(stderr, "error: {}", e).expect(errmsg);

        for e in e.iter().skip(1) {
            writeln!(stderr, "caused by: {}", e).expect(errmsg);
        }

        // The backtrace is not always generated. Try to run this example
        // with `RUST_BACKTRACE=1`.
        if let Some(backtrace) = e.backtrace() {
            writeln!(stderr, "backtrace: {:?}", backtrace).expect(errmsg);
        }

        ::std::process::exit(1);
    }
}

fn usage() -> ! {
    writeln!(std::io::stderr(), "Usage: rbattle (client|server) ADDR")
        .expect("error writing to stderr");
    std::process::exit(1);
}

fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mode = args.next().unwrap_or_else(|| usage());
    let socket_addr: SocketAddr = args.next()
        .unwrap_or_else(|| usage())
        .parse()
        .expect("couldn't parse address");

    let mut participant =
        if mode == "server" {
            Participant::new_server(socket_addr, MapParameters {
                size: (15, 15),
                sources: vec![32, 42, 182, 192],
                player_colors: vec![(0x9f, 0x20, 0xb1), (0xe0, 0x6f, 0x3a),
                                    (0x20, 0xb1, 0x21), (0x20, 0x67, 0xb1)]
            })
        } else if mode == "client" {
            Participant::new_client(socket_addr)?
        } else {
            usage()
        };

    let map = participant.snapshot().map.clone();

    use glium::DisplayBuild;

    let display = glium::glutin::WindowBuilder::new()
        .with_title("rbattle".to_string())
        .build_glium()
        .chain_err(|| "unable to open window")?;

    let drawer = Drawer::new(&display, &map)
        .chain_err(|| "failed to construct Drawer for map")?;

    let mut mouse = Mouse::new(participant.get_player(), map.clone());

    loop {
        // Take a snapshot of the current state and operate on that.
        let state = participant.snapshot();

        // It seems like glium always makes a frame take a full 16ms, regardless
        // of how much work we ask it to do, but I don't see anything in the
        // documentation about this. We're leaning on that for now to keep
        // timing consistent, but we'll need to add something to control timing
        // explicitly to avoid depending on this behavior.
        let mut frame = display.draw();
        frame.clear_color(1.0, 1.0, 1.0, 1.0);
        let status = drawer.draw(&mut frame, &state, &mouse);
        frame.finish()
            .chain_err(|| "drawing finish failed")?;

        let window_to_game = status?;
        let window_to_graph = compose(map.game_to_graph, window_to_game);

        for event in display.poll_events() {
            match event {
                Event::Closed => return Ok(()),
                Event::MouseMoved(x, y) => {
                    let graph_pos = apply(window_to_graph, [x as f32, y as f32]);
                    mouse.move_to(GraphPt(graph_pos));
                }
                Event::MouseInput(ElementState::Pressed, MouseButton::Left) => {
                    mouse.click();
                }
                Event::MouseInput(ElementState::Released, MouseButton::Left) => {
                    if let Some(action) = mouse.release() {
                        participant.request_action(action);
                    }
                }
                _ => ()
            }
        }
    }
}
