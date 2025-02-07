# Rust Q-Sim

This is a port of Matsim's Q-Sim to Rust. My current notes on the project
are [here](https://docs.google.com/document/d/1DkrSJ7KnKXfy2qg8wWyE7c9OPqOUB63px6wmkwuIS9M/edit?usp=sharing)

The most recent release can be cited with the following reference

[![DOI](https://zenodo.org/badge/498376436.svg)](https://zenodo.org/doi/10.5281/zenodo.13928119)

The project is described in a journal publication:
- [High-Performance Mobility Simulation: Implementation of a Parallel Distributed Message-Passing Algorithm for MATSim](https://doi.org/10.3390/info16020116)

And two conference papers, which were presented at ISPDC 24 in Chur, Switzerland, July 2024:
- [High-Performance Simulations for Urban Planning: Implementing Parallel Distributed Multi-Agent Systems in MATSim](https://doi.org/10.1109/ISPDC62236.2024.10705395)
- [Real-Time Routing in Traffic Simulations: A Distributed Event Processing Approach](https://doi.org/10.1109/ISPDC62236.2024.10705399)

## Set Up Rust

Install Rust for your operating system as described [here](https://www.rust-lang.org/tools/install). For WSL this would
be

```shell
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
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

```shell
sudo apt -y install libclang-dev llvm-dev libmetis-dev libopenmpi-dev
```

#### MacOs

The dependencies are available via [homebrew](https://brew.sh/) on macOS.

```shell
brew install metis open-mpi
```

The project contains a `config.toml` which tries to set the `CPATH` and the `RUSTFLAGS` environment variable. In case
this doesn't work, they need to be set like the following:

```shell
export CPATH=$HOMEBREW_PREFIX/include
export RUSTFLAGS="-L$HOMEBREW_PREFIX/lib"
```

Both variables are necessary to compile the METIS and MPI wrapper libraries.

#### Math Cluster

The math cluster has all dependencies installed. They need to be enabled via the module system:

```shell
module load metis-5.1 ompi/gcc/4.1.2
```

#### HLRN (CPU-CLX Partition)
https://nhr-zib.atlassian.net/wiki/spaces/PUB/pages/430586/CPU+CLX+partition

##### Setup conda

Unfortunately, there is no `libclang` dependency installed. You need to install it yourself via `conda`. If you use it
for the first time, load the conda module and initialize it, such that it is available in your shell whenever you login:

```shell
module load anaconda3/2023.09
conda init bash
```

Then create your own environment and install `libclang` and `llvmdev`:

```shell
conda create -n your_env_name
conda activate your_env_name
conda install libclang llvmdev
```

##### Load dependencies

The HLRN cluster has **some** dependencies installed. They need to be enabled via the module system:

```shell
module load intel/2024.2 openmpi/gcc/5.0.3
```

So, before you run the project, you need to activate the environment:

```shell
conda activate your_env_name
```

The activation automatically updates the environment variables such that `libclang` files can be found by the compiler.

Source: https://nhr-zib.atlassian.net/wiki/spaces/PUB/pages/430343/Anaconda+conda+and+Mamba

#### HLRN (CPU-GENOA Partition)
https://nhr-zib.atlassian.net/wiki/spaces/PUB/pages/119832634/CPU+Genoa+partition

##### Compilation

In contrast to the CLX partition, you don't need anaconda here. But, you need to compile with AMD compiler. 

```shell
module load openmpi/aocc/5.0.3
export CC=clang
export CXX=clang++
export RUSTFLAGS="-C linker=clang"
```

Hint: You need to set these environment variables, otherwise there are compilation errors (paul, jan'25). 

##### Execution

For some reason, the runtime linker doesn't find the correct libraries. You need to add them manually before execution (in the job script): 

```shell
export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:/sw/comm/openmpi/5.0.3/genoa.el9/aocc/lib
```

## Run the project

The project is built using cargo.

```shell
cargo build --release
```

Then a simulation can be started like the following:

```shell
mpirun -np 2 ./target/release/mpi_qsim --config-path /path/to/config.yml
```

It is also possible to execute a build before running by executing the following. This is way, one doesn't
forget to re-compile before running.

```shell
cargo mpirun --np 2 --release --bin mpi_qsim -- --config-path /path/to/config.yaml
```

We also have a

### Test

Run `$ cargo test` to execute all tests. To have immediate output use `$ cargo test -- --nocapture`

## Create input files

You need to create protobuf files from the xml files. This can be done with the following command:

```shell
cargo run --bin convert_to_binary --release -- --network network.xml --population population.xml --vehicles vehicles.xml --output-dir output --run-id run
```
