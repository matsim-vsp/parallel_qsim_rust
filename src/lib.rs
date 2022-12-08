extern crate core;

pub mod config;
// usually this is not part of the library. This way we can use the module in integration
// test as well as in main. Don't know whether this has to be like this.
pub mod controller;
pub mod io;
pub mod logging;
pub mod parallel_simulation;

// this module is used to tinker with rust problems in a simple fashion
#[allow(dead_code)]
pub mod experiments;

// this was the first try of the simulation in a single threaded manner
#[allow(dead_code)]
mod simulation;
