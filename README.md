# Rust Q-Sim

This is a port of Matsim's Q-Sim to Rust. My current notes on the project
are [here](https://docs.google.com/document/d/1DkrSJ7KnKXfy2qg8wWyE7c9OPqOUB63px6wmkwuIS9M/edit?usp=sharing)

## Set Up

Install Rust for your operating system as described [here](https://www.rust-lang.org/tools/install). For WSL this would
be

```
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
The project requires the nightly version of rust, because the [rust_road_router](https://github.com/kit-algo/rust_road_router)
uses features available in the nightly build of standard library. 

Install the nightly build
```
$ rustup install nightly
```
The version against which the project is build against is fixed in `rust-toolchain.toml`.

Please make sure you are using the rust nightly build version. It can be installed by calling

```
$ rustup default nightly
```

The reason for using the nightly version of rust is that one dependency (`rust_road_router`) requires it.

### Prerequisites

This project uses the [metis](https://crates.io/crates/metis) crate as a dependency.
This crate is a wrapper for the [METIS C Library](https://github.com/KarypisLab/METIS).
It requires Metis and Clang as Prerequisites.

Also, this project uses [MPI](https://docs.open-mpi.org/en/v5.0.x/) with the help of [rsmpi](https://github.com/rsmpi/rsmpi)
as a wrapper over the MPI-C API. To compile and run the project an MPI-Implementation must be installed. For example
OpenMPI

Also, this project uses Google Protocol Buffers for serializing messages. 

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

To run the program only the `Metis` module is necessary

#### HLRN

HLRN has `Clang` and `Metis` installed as modules. To enable compilation of the programm add the
following modules.

1. `$ module load gcc/9.2.0`
2. `$ module load llvm/9.0.0`
3. `$ module load metis/5.1.0`

To run the program only the `Metis` module is necessary

### Set up in IntelliJ/CLion

Programming Rust in IntelliJ is possible by installing
the [Rust Plugin](https://plugins.jetbrains.com/plugin/8182-rust/docs) developed by JetBrains. However, some features
such as a Debugger are not available there. For this one can use Clion, which is the C-IDE by JetBrains. This also
requires the [Rust Plugin](https://plugins.jetbrains.com/plugin/8182-rust/docs) to be installed.

To set things up with WSL, I followed
the [WSL Toolchain](https://plugins.jetbrains.com/plugin/8182-rust/docs/rust-project-settings.html#wsl-toolchain)
section of the [Rust Plugin](https://plugins.jetbrains.com/plugin/8182-rust/docs) documentation. To make the 'right
click run as'
feature work I had to change the default execution environment of the project to WSL.

- Edit Configuration -> Edit Configuration Templates... ->
  Cargo -> Manage Targets... -> + -> WSL
- On the bottom of that dialog select the newly created target as default for the project

This can probably done somewhere else as well...

To use the rust nightly build from CLion go to Settings -> Rust -> Standard Library and select the nightly folder.

To enable code completion on generated items it is necessary to set `org.rust.cargo.evaluate.build.scripts` to `true`
in `Experimental Features` Dialog

### Set up on the Math Cluster / HLRN

Since Rust is build into a binary executable (this is important for Java Developers 🙃) it has to be built on the
machine on which the program is supposed to be run. For this the following steps are necessary:

Make sure `Clang` and `Metis` are available as described in Prerequisites

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
