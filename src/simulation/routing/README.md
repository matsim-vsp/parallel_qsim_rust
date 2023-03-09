# Network conversion

To run any Contraction Hierarchy algorithm, the network needs to be preprocessed.
It is split into two phases:

1) Metric independent preprocessing
2) Metric dependent preprocessing

During phase 1 an ordering of the nodes is computed which doesn't depend on travel times. This task is not done in Rust,
but by the [InertialFlowCutter](https://github.com/kit-algo/InertialFlowCutter/).

**Watch out:** InertialFlowCutter works with `cmake 3.16` for sure. With `cmake 3.25`, which comes with Ubuntu 22.x, there are errors (`cmake` doesn't find `dlltools`). 

The ordering computed there is then used during phase 2. This is done by `rust-road-router`, thus in out Rust code.

## Phase 1

This phase consists of the following steps:

1. Convert MATSim network into RoutingKit format (text)
2. Convert RoutingKit format into binary format
3. Compute ordering (binary RoutingKit format)
4. Convert ordering to text RoutingKit format

Phase 1 is done in Rust, phases 2-4 are done in external processes which are called in Rust. 

### Pre requirements for phases 1.2 - 1.4

Clone the [InertialFlowCutter](https://github.com/paulheinr/InertialFlowCutter) repository. It needs some extra
libraries. Install them via:

```shell
sudo apt install libtbb-dev
sudo apt-get install libreadline8 libreadline-dev
sudo apt-get install zlib1g-dev
```

Then follow the installation instruction of InertialFlowCutter.

## Phase 2

This phase consists of the following steps

1. Read in the ordering from phase 1
2. Calculate contractions  
