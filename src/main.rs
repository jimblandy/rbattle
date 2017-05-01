#![allow(dead_code)]

#[macro_use]
extern crate glium;

#[cfg(test)]
#[macro_use]
mod test_utils;

mod drawer;
mod graph;
mod map;
mod square;
mod state;
mod visible_graph;

fn main() {
    println!("Hello, world!");
}
