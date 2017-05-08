/// Handling user interaction with the game.
///
/// This module handles input events like mouse clicks and keyboard input, and
/// turns them into UI effects like hover highlights, and then game moves like
/// outflow toggles.

use graph::Node;
use map::Map;
use state::{Action, Player, State};
use visible_graph::GraphPt;

use std::rc::Rc;

/// The game's state for handling mouse activity.
#[derive(Debug, Clone)]
pub struct Mouse {
    /// The player we represent.
    player: Player,

    /// The map we're controlling.
    map: Rc<Map>,

    /// Where we last saw the mouse. Rather than representing this as a point on
    /// the plane, we keep it in dathe form relevant to our purposes, broken
    /// down by which clickable element it's over.
    position: Affordance,

    /// If the mouse is clicked, this is where the button went down.
    click: Option<Affordance>,
}

/// A thing on the map the user can interact with. Think of this as a mouse
/// position, but put in the terms we actually care about.
///
/// Whenever I see an enum like this I want to make it into an `Option`. But if
/// there were more clickable things on the map, this would be the natural place
/// to list them.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Affordance {
    /// The mouse is not over any interesting element on the map.
    Nothing,

    /// The mouse is over an outflow edge between the two nodes.
    Outflow((Node, Node)),
}

impl Mouse {
    pub fn new(player: Player, map: Rc<Map>) -> Mouse {
        Mouse { player, map, position: Affordance::Nothing, click: None }
    }

    /// Report that the mouse moved to `pos` in graph space coordinates.
    pub fn move_to(&mut self, pos: GraphPt) {
        self.position = match self.map.graph.boundary_hit(&pos) {
            Some(pos) => Affordance::Outflow(pos),
            None => Affordance::Nothing
        }
    }

    /// The main mouse button was clicked at the last reported position.
    pub fn click(&mut self) {
        self.click = Some(self.position);
    }

    /// The main mouse button was released. This may return an action to carry
    /// out on the state.
    pub fn release(&mut self) -> Option<Action> {
        match self.click.take() {
            // If we get a release with no click, ignore.
            None => None,

            Some(affordance) => {
                // If we released on something different from what we clicked
                // on, that's a drag-off, so we do nothing.
                if affordance != self.position {
                    return None;
                }

                match affordance {
                    Affordance::Nothing => None,
                    Affordance::Outflow(pos) =>
                        Some(Action::ToggleOutflow {
                            player: self.player,
                            outflow: pos
                        })
                }
            }
        }
    }

    /// Given `state`, choose how to display the interactive parts of the game
    /// grid.
    pub fn display(&self, _state: &State) -> Display {
        match (self.click, self.position) {
            // We're over something we're not clicking on.
            (None, Affordance::Outflow(pos)) =>
                Display::Outflow { nodes: pos, state: OutflowState::Hover },

            (Some(Affordance::Outflow(cpos)), Affordance::Outflow(mpos)) => {
                if cpos == mpos {
                    // We're clicking on something that we're still over.
                    Display::Outflow { nodes: cpos, state: OutflowState::Active }
                } else {
                    // We clicked on one thing, but moved elsewhere. This is
                    // arguably a distinct state, but treat it like a hover
                    // that's stuck on the click position.
                    Display::Outflow { nodes: cpos, state: OutflowState::Hover }
                }
            }

            // Otherwise, no action.
            _ => Display::Nothing
        }
    }
}

/// How to display the current mouse state. This is always computed as a
/// function of some pair of `State` and `Mouse` values.
pub enum Display {
    Nothing,

    /// We're going to highlight an outflow.
    Outflow { nodes: (Node, Node), state: OutflowState }
}

/// How to highlight an outflow.
pub enum OutflowState {
    /// Draw the outflow as something one could click on. (The mouse is
    /// hovering over it, or was clicked on it but moved off without being
    /// released.)
    Hover,

    /// Draw the outflow as being clicked upon, but not yet released.
    /// (The mouse was clicked on it, and is still over it.)
    Active
}
