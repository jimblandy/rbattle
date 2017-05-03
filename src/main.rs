#![allow(dead_code)]

// `error_chain!` can recurse deeply
#![recursion_limit = "1024"]

#[macro_use]
extern crate error_chain;

#[macro_use]
extern crate glium;

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
use state::{OwnedNode, State};

use glium::glutin::Event;
use glium::Surface;

use std::iter::repeat;
use std::rc::Rc;

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
    let sources = vec![];
    let colors = vec![(0x9f, 0x20, 0xb1), (0xb1, 0x20, 0x44),
                      (0x20, 0xb1, 0x21), (0x20, 0x67, 0xb1),
                      (0xe0, 0x6f, 0x3a)];
    let map = Rc::new(Map::new(graph, sources, colors));
    let drawer = Drawer::new(&display, &map)
        .chain_err(|| "failed to construct Drawer for map")?;

    let mut victim = 0;
    let mut wait = 0;
    loop {
        let mut state = State {
            map: map.clone(),
            nodes: repeat(None).take(map.graph.nodes()).collect()
        };

        state.nodes[victim] = Some(OwnedNode {
            player: Player(0),
            outflows: map.graph.neighbors(victim),
            goop: 0
        });

        wait += 1;
        if wait > 10 {
            wait = 0;
            victim += 14;
            while victim >= map.graph.nodes() {
                victim -= map.graph.nodes();
            }
        }

        let mut frame = display.draw();
        frame.clear_color(1.0, 0.43, 0.0, 1.0);
        let status = drawer.draw(&mut frame, &state);
        frame.finish()
            .chain_err(|| "drawing finish failed")?;

        status?;

        for event in display.poll_events() {
            match event {
                Event::Closed => return Ok(()),
                _ => ()
            }
        }
    }
}
