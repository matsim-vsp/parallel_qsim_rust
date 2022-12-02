extern crate core;
pub mod config;
pub mod controller;
mod io;
pub mod parallel_simulation;

// this module is used to tinker with rust problems in a simple fashion
#[allow(dead_code)]
mod experiments;

// this was the first try of the simulation in a single threaded manner
#[allow(dead_code)]
mod simulation;
