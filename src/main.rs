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
extern crate tokio_codec;
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
use visible_graph::GraphPt;

use glium::{Display, Surface};
use glium::glutin::{ContextBuilder, ElementState, Event, EventsLoop, KeyboardInput,
                    ModifiersState, MouseButton, VirtualKeyCode, WindowBuilder,
                    WindowEvent};
use glium::glutin::dpi::PhysicalPosition;

use std::io::Write;
use std::net::SocketAddr;
use std::time::Instant;

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

    let mut events_loop = EventsLoop::new();
    let window = WindowBuilder::new()
        .with_title("rbattle".to_string());
    let context = ContextBuilder::new();
    let display = Display::new(window, context, &events_loop)
        .chain_err(|| "unable to open window")?;

    let drawer = Drawer::new(&display, &map)
        .chain_err(|| "failed to construct Drawer for map")?;

    let mut mouse = Mouse::new(participant.get_player(), map.clone());

    let start = Instant::now();
    loop {
        // Record when this frame started.
        let time = start.elapsed();

        // Take a snapshot of the current state and operate on that.
        let state = participant.snapshot();

        // It seems like glium always makes a frame take a full 16ms, regardless
        // of how much work we ask it to do, but I don't see anything in the
        // documentation about this. We're leaning on that for now to keep
        // timing consistent, but we'll need to add something to control timing
        // explicitly to avoid depending on this behavior.
        let mut frame = display.draw();
        frame.clear_color(1.0, 1.0, 1.0, 1.0);
        let status = drawer.draw(&mut frame, time, &state, &mouse);
        frame.finish()
            .chain_err(|| "drawing finish failed")?;

        let window_to_game = status?;
        let window_to_graph = compose(map.game_to_graph, window_to_game);

        let mut done = None;
        events_loop.poll_events(|event| {
            if let Event::WindowEvent { event, .. } = event {
                match event {
                    WindowEvent::CloseRequested => {
                        done = Some(Ok(()));
                    }

                    WindowEvent::CursorMoved { position, .. } => {
                        let hidpi_factor = display.gl_window().get_hidpi_factor();
                        let PhysicalPosition { x, y } = position.to_physical(hidpi_factor);
                        let graph_pos = apply(window_to_graph, [x as f32, y as f32]);
                        mouse.move_to(GraphPt(graph_pos));
                    }

                    WindowEvent::MouseInput {
                        button: MouseButton::Left,
                        state: ElementState::Pressed,
                        ..
                    } => {
                        mouse.click();
                    }

                    WindowEvent::MouseInput {
                        button: MouseButton::Left,
                        state: ElementState::Released,
                        ..
                    } => {
                        if let Some(action) = mouse.release() {
                            participant.request_action(action);
                        }
                    }

                    WindowEvent::KeyboardInput {
                        input: KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            ..
                        },
                        ..
                    } => {
                        std::process::exit(0);
                    }

                    WindowEvent::KeyboardInput {
                        input: KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: Some(VirtualKeyCode::W),
                            modifiers: ModifiersState { ctrl: true, .. },
                            ..
                        },
                        ..
                    } => {
                        std::process::exit(0);
                    }

                    _ => ()
                }
            }
        });

        if let Some(result) = done {
            return result;
        }
    }
}
