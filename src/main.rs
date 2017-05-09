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
mod scheduler;
mod square;
mod synchronized_state;
mod state;
mod visible_graph;
mod xorshift;

use drawer::Drawer;
use map::MapParameters;
use math::{apply, compose};
use mouse::Mouse;
use state::Player;
use synchronized_state::SynchronizedState;
use visible_graph::GraphPt;

use glium::glutin::{Event, ElementState, MouseButton, VirtualKeyCode};
use glium::Surface;

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

fn run() -> Result<()> {
    use glium::DisplayBuild;

    let display = glium::glutin::WindowBuilder::new()
        .with_title("rbattle".to_string())
        .build_glium()
        .chain_err(|| "unable to open window")?;

    let mut syn_state = SynchronizedState::new_server(MapParameters {
        size: (15, 15),
        sources: vec![32, 42, 182, 192],
        player_colors: vec![(0x9f, 0x20, 0xb1), (0xe0, 0x6f, 0x3a),
                            (0x20, 0xb1, 0x21), (0x20, 0x67, 0xb1)]
    });
    let map = syn_state.snapshot().map.clone();

    let drawer = Drawer::new(&display, &map)
        .chain_err(|| "failed to construct Drawer for map")?;

    let mut mouse = Mouse::new(Player(0), map.clone());

    let mut turn = 0;
    let start = Instant::now();

    // True if we should run a game step on every frame.
    let mut free_running = false;

    loop {
        // Take a snapshot of the current state and operate on that, to avoid
        // holding the SynchronizedState's lock for very long.
        let state = syn_state.snapshot();

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

        let mut single_step = false;
        for event in display.poll_events() {
            match event {
                Event::Closed => return Ok(()),
                Event::KeyboardInput(ElementState::Pressed, _,
                                     Some(VirtualKeyCode::Space)) => {
                    free_running = false;
                    single_step = true;
                }
                Event::KeyboardInput(ElementState::Pressed, _,
                                     Some(VirtualKeyCode::Return)) => {
                    free_running = true;
                }
                Event::MouseMoved(x, y) => {
                    let graph_pos = apply(window_to_graph, [x as f32, y as f32]);
                    mouse.move_to(GraphPt(graph_pos));
                }
                Event::MouseInput(ElementState::Pressed, MouseButton::Left) => {
                    mouse.click();
                }
                Event::MouseInput(ElementState::Released, MouseButton::Left) => {
                    if let Some(action) = mouse.release() {
                        syn_state.request_action(action);
                    }
                }
                _ => ()
            }
        }

        if free_running || single_step {
            println!("Turn {} at {:9.3}s:", turn, elapsed_since(&start));

            let start_generation = Instant::now();
            syn_state.advance();
            println!("    advance to next state: {:9.6}s:", elapsed_since(&start_generation));
            turn += 1;
        }
    }
}

/// Return the number of seconds that have elasped since `instant`. Partial
/// seconds are returned as fractional values.
fn elapsed_since(instant: &Instant) -> f64 {
    let elapsed = instant.elapsed();
    elapsed.as_secs() as f64 +
        (elapsed.subsec_nanos() as f64 / 1e9)
}
