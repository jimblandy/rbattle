#![allow(dead_code)]

// `error_chain!` can recurse deeply
#![recursion_limit = "1024"]

#[macro_use] extern crate error_chain;
#[macro_use] extern crate glium;
extern crate rand;

#[cfg(test)]
#[macro_use]
mod test_utils;

mod drawer;
mod errors;
mod graph;
mod map;
mod math;
mod square;
mod state;
mod visible_graph;

use drawer::Drawer;
use graph::Graph;
use map::{Map, Player};
use square::SquareGrid;
use state::{MAX_GOOP, OwnedNode, State};

use glium::glutin::{Event, ElementState, VirtualKeyCode};
use glium::Surface;

use std::iter::repeat;
use std::rc::Rc;
use std::time::{Duration, Instant};

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

    let graph = SquareGrid::new(15, 15);
    let sources = vec![16, 45];
    let colors = vec![(0x9f, 0x20, 0xb1), (0xb1, 0x20, 0x44),
                      (0x20, 0xb1, 0x21), (0x20, 0x67, 0xb1),
                      (0xe0, 0x6f, 0x3a)];
    let map = Rc::new(Map::new(graph, sources, colors));
    let drawer = Drawer::new(&display, &map)
        .chain_err(|| "failed to construct Drawer for map")?;

    let mut state = State::new(map.clone(),
                               repeat(None).take(map.graph.nodes()).collect());

    state.nodes[45] = Some(OwnedNode {
        player: Player(2),
        outflows: map.graph.neighbors(45),
        goop: MAX_GOOP
    });

    state.nodes[30] = Some(OwnedNode {
        player: Player(2),
        outflows: vec![15],
        goop: 0
    });

    state.nodes[16] = Some(OwnedNode {
        player: Player(0),
        outflows: map.graph.neighbors(16),
        goop: MAX_GOOP
    });

    state.nodes[17] = Some(OwnedNode {
        player: Player(1),
        outflows: vec![],
        goop: 2
    });

    loop {
        let mut frame = display.draw();
        frame.clear_color(1.0, 1.0, 1.0, 1.0);
        let status = drawer.draw(&mut frame, &state);
        frame.finish()
            .chain_err(|| "drawing finish failed")?;

        status?;

        for event in display.poll_events() {
            match event {
                Event::Closed => return Ok(()),
                Event::KeyboardInput(ElementState::Pressed, _,
                                     Some(VirtualKeyCode::Space)) => {
                    state.flow();
                    state.generate_goop();
                }
                _ => ()
            }
        }

    }
}
