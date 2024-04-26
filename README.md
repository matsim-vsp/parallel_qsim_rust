# Rust Q-Sim

This is a port of Matsim's Q-Sim to Rust. My current notes on the project
are [here](https://docs.google.com/document/d/1DkrSJ7KnKXfy2qg8wWyE7c9OPqOUB63px6wmkwuIS9M/edit?usp=sharing)

The most recent release can be cited with the following reference

[![DOI](https://zenodo.org/badge/498376436.svg)](https://zenodo.org/doi/10.5281/zenodo.10960722)

The project is described in two pre-prints:
- [High-Performance Simulations for Urban Planning: Implementing Parallel Distributed Multi-Agent Systems in MATSim - (Submitted to ISPDC)](https://svn.vsp.tu-berlin.de/repos/public-svn/publications/vspwp/2024/24-10/LaudanEtAl2024DistributeQSim_submitted.pdf)
- [Real-Time Routing in Traffic Simulations: A Distributed Event Processing Approach - (Submitted to ISPDC)](https://svn.vsp.tu-berlin.de/repos/public-svn/publications/vspwp/2024/24-12/HeinrichEtAl2024RealTimeRoutingInTrafficSimulationsADistributedEventProcessingApproach_submitted.pdf)

## Set Up Rust

Install Rust for your operating system as described [here](https://www.rust-lang.org/tools/install). For WSL this would
be

```
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Set Up Prerequisites

The project relies on METIS and MPI as external dependencies. This means those dependencies are not
compiled with the project, but need to be present on the operating system.

### METIS

The project uses the [metis](https://crates.io/crates/metis) crate as a dependency which
is a wrapper for the [METIS C Library](https://github.com/KarypisLab/METIS). The C-Library is 
expected to be present on the machine. Also, the `metis` crate requires `libclang` on the machine 
this project is built on.

### MPI

The project uses MPI for message passing between processes. The message passing is implemented using the
[rsmpi](https://github.com/rsmpi/rsmpi) crate as an interface around a corresponding C-library on the system.

We maintain our own fork of the project because we depend on a more recent commit than the last release on
[crates.io](https://crates.io/), due to some `bingen` issue with MacOS 13.4 or higher.

The actual implementation of the message passing library is up to the system the project is run on. A good candiate
is [open-mpi](https://www.open-mpi.org/).

### Install dependencies

The dependencies named above need to be installed before the project can be buit

#### Linux - apt

Install dev versions of required packages because dev stuff is required during compilation

```
$ sudo apt -y install libclang-dev llvm-dev libmetis-dev libopenmpi-dev
```

#### MacOs

The dependencies are available via [homebrew](https://brew.sh/) on macOS.

```
$ brew install metis open-mpi
```

The project contains a `config.toml` which tries to set the `CPATH` and the `RUSTFLAGS` environment variable. In case
this doesn't work, they need to be set like the following:
```
$ export CPATH=$HOMEBREW_PREFIX/include
$ export RUSTFLAGS="-L$HOMEBREW_PREFIX/lib"
```

Both variables are necessary to compile the METIS and MPI wrapper libraries.

#### Math Cluster

The math cluster has all dependencies installed. They need to be enabled via the module system:
```
$ module load metis-5.1 ompi/gcc/4.1.2
```
#### HLRN

The HLRN cluster has all dependencies installed. They need to be enabled via the module system:
```
$ module load gcc/9.3.0 llvm/9.0.0 openmpi/gcc.9/4.1.4 metis/5.1.0
```

## Run the project

The project is built using cargo.

```
$ cargo build --release
```

Then a simulation can be started like the following:
```
$ mpirun -np 2 ./target/release/mpi_qsim --config-path /path/to/config.yml
```

It is also possible to execute a build before running by executing the following. This is way, one doesn't
forget to re-compile before running.
```
$  cargo mpirun --np 2 --release --bin mpi_qsim -- --config-path /path/to/config.yaml
```

We also have a

### Test

Run `$ cargo test` to execute all tests. To have immediate output use `$ cargo test -- --nocapture`
