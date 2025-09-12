# Architecture Overview

This document provides an overview of the architecture of the project, detailing its main components, their
interactions, and the overall design principles.

## `rust_qsim`

The core implementation of the Rust QSim is oriented towards [MATSim Java](https://github.com/matsim-org/matsim-libs).
In particular, we tried to minimize the differences between the physics of both simulations, including link dynamics
and output of events.

### General Scenario Handling

Starting the simulation mostly works as in MATSim Java. All XML input files need to be converted into protobuf for
faster reading. These files need to be referenced in a configuration file. Based on the config, a scenario is built,
based on that the controller -- pretty much like in MATSim Java.

As the simulation is partitioned, there are two datastructures for holding the scenario, `GlobalScenario` and
`ScenarioPartition`. The `GlobalScenario` is built first when reading the input files.

The controller splits the `GlobalScenario` into a `ScenarioPartitionBuilder` for each partition. The `ScenarioPartition`
is finally instantiated on each QSim separately.

### External Services

As a next step, we integrated the ability to communicate to external services. They are intended to be used during the
simulation for real-time updates of plans (like routing). We have seen in previous work that synchronous calls of such
services slow down the simulation a lot. This is why we implemented a more complex architecture allowing asynchronous
calls to such services.

During execution, we have the following threads running:

- $n$ QSim threads
- $1$ external service adapter thread
- $r$ routing communication threads (used by tokio runtime)

Both $n$ and $r$ are configurable. Any request to an external service is sent to the adapter thread via a channel.
Every thread is able to send requests to the adapter. The adapter thread allows abstraction of the actual external
service: it might mock a service, it can perform calculation itself or forward it to other threads, or it might forward
it to an actual external service.

In every case, the adapter thread answers requests asynchronously (see trait `RequestAdapter`). Therefore, a tokio
runtime is built starting its own threads.

## `macros`
