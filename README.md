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

### Set up in IntelliJ/CLion
Programming Rust in IntelliJ is possible by installing the [Rust Plugin](https://plugins.jetbrains.com/plugin/8182-rust/docs) developed by JetBrains. However, some features
such as a Debugger are not available there. For this one can use Clion, which is the C-IDE by JetBrains. This also
requires the [Rust Plugin](https://plugins.jetbrains.com/plugin/8182-rust/docs) to be installed. 

To set things up with WSL, I followed the [WSL Toolchain](https://plugins.jetbrains.com/plugin/8182-rust/docs/rust-project-settings.html#wsl-toolchain)
section of the [Rust Plugin](https://plugins.jetbrains.com/plugin/8182-rust/docs) documentation. To make the 'right click run as' 
feature work I had to change the default execution environment of the project to WSL. 

- Edit Configuration -> Edit Configuration Templates... ->
Cargo -> Manage Targets... -> + -> WSL
- On the bottom of that dialog select the newly created target as default for the project

This can probably done somewhere else as well...


### Build

Run `$ cargo build` to build the program

### Test

Run `$ cargo test` to execute all tests. To have emmediate output use `$ cargo test -- --nocapture`
