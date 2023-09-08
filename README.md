# Rust Q-Sim

This is a port of Matsim's Q-Sim to Rust. My current notes on the project
are [here](https://docs.google.com/document/d/1DkrSJ7KnKXfy2qg8wWyE7c9OPqOUB63px6wmkwuIS9M/edit?usp=sharing)

## Set Up Rust

Install Rust for your operating system as described [here](https://www.rust-lang.org/tools/install). For WSL this would
be

```
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
The project requires the nightly version of rust, because [rust_road_router](https://github.com/kit-algo/rust_road_router)
uses features available in the nightly build of the standard library. 

Install the nightly build
```
$ rustup install nightly
```
The version against which the project is build against is fixed in `rust-toolchain.toml`. And
should be picked automatically. It is also possible to set the rust version manually with the 
following command:

```
$ rustup default nightly
```

## Set Up Prerequisites
This project has multiple dependencies which are not compiled with the project but must be 
present on the operating system the project is compiled on.

### METIS
This project uses the [metis](https://crates.io/crates/metis) crate as a dependency which
is a wrapper for the [METIS C Library](https://github.com/KarypisLab/METIS). The C-Library is 
expected to be present on the machine. Also, the `metis` crate requires `libclang` on the machine 
this project is built on.

We use our own [fork](https://github.com/Janekdererste/metis-rs) of the `metis` crate. This is because
both the `metis` and `rsmpi` crate use `bindgen` but with different versions to bind to the 
C-Implementations of the respective libraries. This lead to build errors where `libclang` was 
not loaded properly. The fork sets the `libgen` version in the `metis` crate to the same 
version as `rsmpi`'s `bindgen version 

### Routing
This project uses the [rust-road-router](https://github.com/kit-algo/rust_road_router) project for routing.
The preprocessing of this routing library relies on [InertialFlowCutter](https://github.com/paulheinr/InertialFlowCutter).
Since some router related tests check the preprocessing, you need to configure it correctly even if you do not enable routing in the simulation.

#### Install dependencies
Install them via:

```shell
sudo apt install libtbb-dev
sudo apt-get install libreadline8 libreadline-dev
sudo apt-get install zlib1g-dev
```

#### Install InertialFlowCutter
_Instructions from [GitHub repository](https://github.com/kit-algo/InertialFlowCutter/)._

Clone https://github.com/paulheinr/InertialFlowCutter.git next to this repository. 
There is an environment variable defined in `.cargo/config.toml` which points to the home directory of the InertialFlowCutter repository.
By default, it is set to `"../InertialFlowCutter"`. This is why it should be cloned next to this repository.

**Watch out:** InertialFlowCutter works with `cmake 3.16` for sure. With `cmake 3.25`, which comes with Ubuntu 22.x, there are errors (`cmake` doesn't find `dlltools`).

In the top level folder of the InertialFlowCutter repository run

```shell
mkdir build && cd build
cmake -DCMAKE_BUILD_TYPE=Release ..
make
```

### MPI
This project uses [MPI](https://docs.open-mpi.org/en/v5.0.x/) for Message Passing. The raw
C-Api is abstracted by the [rsmpi](https://github.com/rsmpi/rsmpi) crate. As with METIS an MPI
Implementation is expected to be present on the machine the program is build and run on.

#### Windows Subsystem for Linux - Probably Ubuntu in general

On Windows Subsystem for Linux and in the ci-build we install the pre-requisites as follows:
```
$ sudo apt -y install libclang-dev llvm-dev libmetis-dev libopenmpi-dev
```

#### Math Cluster

The math cluster provides our prerequisites as modules. The modules must be loaded with the 
following command.

```
$ module load metis-5.1 ompi/gcc/4.1.2
```

#### HLRN

The HLRN cluster provides our prerequisites as modules. The modules and their respecting
prerequisites can be loaded as follows:

```
$ module load gcc/9.3.0 llvm/9.0.0 openmpi/gcc.9/4.1.4 metis/5.1.0
```

#### MacOS
Install metis dependency
```
$ homebrew install metis
```
Install open-mpi dependency
```
$ homebrew install open-mpi
```
The paths for open-mpi dependencies are figure out by `rsmpi` automagically. For `metis` these paths must be set manually
```
export CPATH=$HOMEBREW_PREFIX/include
export RUSTFLAGS="-L$HOMEBREW_PREFIX/lib"
```
So far, the build doesn't execute, as `rust_road_router` uses the `affinity` crate which currently only works on Windows and Linux :-(


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
in `Experimental Features` Dialog. This is important to get typing assistance on types generated
from protobuf.

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

Run `$ cargo test` to execute all tests. To have emmediate output use `$ cargo test -- --nocapture
