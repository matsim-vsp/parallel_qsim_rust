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
It requires Metis and Clang as Prerequisites. 

#### Windows Subsystem for Linux - Probably Ubuntu in general
On Windows Subsystem for Linux I executed the following steps to make
things work

1. `$ sudo apt install libclang-dev`
2. `$ sudo apt install libmetis-dev`

#### Math Cluster
The math cluster has `Clang` and `Metis` installed as modules. To make sure the correct versions are enabled run 
the following before building and running

1. `$ module load clang/8.0.1` (only build)
2. `$ mdoule load metis-5.1`

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

### Set up on the Math Cluster
Since Rust is build into a binary executable (this is important for Java Developers 🙃) it has to be built on the 
machine on which the program is supposed to be run. For this the following steps are necessary:

Make sure `Clang` and `Metis` are available

```
$ module laod clang/8.0.1
$ mdoule load metis-5.1
```

Install Rust on the cluster:
```
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
Clone the project
```
$ git clone https://github.com/Janekdererste/rust_q_sim.git ./your-project-folder
```
Build the project from within `your-project-folder`
```
$ cargo build --release
```
Now there should be an executable in `your-project-folder/target/release`. From within `your-project-folder` it can 
be started like
```
$ ./target/release/rust_q_sim 0 3600 2 /path/to/network.xml.gz /path/to/population.xml.gz /output-folder file
```
This would run a simulation from time-step 0 until time-step 3600 (1 hour) on two threads. It would use the 
specified network and population as input and write output files into the output-folder. The last parameter is
the writing mode for events.

This interface is very basic and will hopefully improve soon...

### Build

Run `$ cargo build` to build the program

### Test

Run `$ cargo test` to execute all tests. To have emmediate output use `$ cargo test -- --nocapture`
