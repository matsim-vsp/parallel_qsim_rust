# Rust Q-Sim

This is a port of Matsim's Q-Sim to Rust

## Set Up

Install Rust for your operating system as described [here](https://www.rust-lang.org/tools/install). For WSL this would
be

```
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Prerequisites

This project uses the [metis](https://crates.io/crates/metis) crate as a dependency.
This crate is a wrapper for the [METIS C Library](https://github.com/KarypisLab/METIS).
It requires Metis and Clang as Prerequisites. On Windows Subsystem for Linux I executed the following steps to make
things work

1. `$ sudo apt install libclang-dev`
2. `$ sudo apt install libmetis-dev`

On Windows I had to do the following:

1. Install [Cmake](https://cmake.org/download/)
2. Install Visual Studio Build tools, which one needs for rust anyway
3. Try to follow the description in [Windows-Build](https://github.com/KarypisLab/METIS/blob/master/BUILD-Windows.txt)

This doesn't work so far 😔

### Build

Run `$ cargo build` to build the program

### Test

Run `$ cargo test` to execute all tests. To have emmediate output use `$ cargo test -- --nocapture`
