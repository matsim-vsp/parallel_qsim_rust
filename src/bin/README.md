# Network conversion

To run any Contraction Hierarchy algorithm, the network needs to be preprocessed.
It is split into two phases:

1) Metric independent preprocessing
2) Metric dependent preprocessing

During phase 1 an ordering of the nodes is computed which doesn't depend on travel times. This task is not done in Rust,
but by the [InertialFlowCutter](https://github.com/kit-algo/InertialFlowCutter/).

The ordering computed there is then used during phase 2. This is done by `rust-road-router`, thus in out Rust code.

## Phase 1

This phase consists of the following steps:

1. Convert MATSim network into RoutingKit format (text)
2. Convert RoutingKit format into binary format
3. Compute ordering (binary RoutingKit format)
4. Convert ordering to text RoutingKit format

Phase 1 is done in Rust, phases 2-4 are done in external processes which are called in Rust. So you have to configure
those processes properly as described below. But you do only have to call Rust one time initially.

```shell
cargo run --bin network_converter <path to mastim network> <output folder> <InertialFlowCutterPath>
```

e.g.

```shell
cargo run --package rust_q_sim --bin network_converter ./assets/routing_tests/triangle-network.xml ./assets/routing_tests/conversion ../InertialFlowCutter
```

### Pre requirements for phases 1.2 - 1.4

Clone the [InertialFlowCutter](https://github.com/paulheinr/InertialFlowCutter) repository. It needs some extra
libraries. Install them:

```shell
sudo apt install libtbb-dev
sudo apt-get install readline8 readline-dev
sudo apt-get install zlib1g-dev
```

Then follow the installation instruction of InertialFlowCutter.

To execute phases 2-4, execute

```shell
cd src/bin
python3 network_converter.py <InertialFlowCutter home directory> <Network files in textual RoutingKit format> <output file name>
```

## Phase 2

This phase consists of the following steps

1. Read in the ordering from phase 1
2. Calculate contractions  
