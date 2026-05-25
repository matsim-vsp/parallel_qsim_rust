use std::fs::{create_dir, create_dir_all, OpenOptions};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use clap::Parser;
use tracing::{info, info_span};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{fmt, Layer};
use tracing_subscriber::filter::{filter_fn};
use tracing_subscriber::layer::SubscriberExt;
use rust_qsim::simulation::config::{CommandLineArgs, Config};
use rust_qsim::simulation::controller::controller::ControllerBuilder;
use rust_qsim::simulation::id::Id;
use rust_qsim::simulation::scenario::MutableScenario;
use rust_qsim::simulation::scenario::vehicles::InternalVehicleType;

// TODO: This script is not meant to be a library function. It should be outsourced aleks Apr'26

#[cfg_attr(feature = "hotpath", hotpath::main)]
fn main() {
    init_logging_with_benchmark();

    let args = CommandLineArgs::parse();
    info!("Started with args: {:?}", args);

    // Load and adapt config
    let config = Arc::new(Config::from_args(args));

    // Load and adapt mod
    let mut scenario = MutableScenario::load(config);

    add_teleported_vehicle(&mut scenario, "walk");
    add_teleported_vehicle(&mut scenario, "pt");

    // Create and run simulation
    let controller = ControllerBuilder::default_with_scenario(scenario)
        .build()
        .unwrap();

    let sim_span = info_span!("simulation");
    let _enter = sim_span.enter();

    let start = Instant::now();

    controller.run();

    let elapsed = start.elapsed();

    info!(
        target: "benchmark",
        runtime_ms = elapsed.as_millis(),
        runtime_sec = elapsed.as_secs_f64(),
        "simulation_completed"
    );

}

fn init_logging_with_benchmark() {
    create_dir_all("./output").expect("Could not create output folder");

    // Find the next available benchmark_N filename
    let mut index = 0;
    let filename = loop {
        let path = format!("./output/benchmark_{}", index);

        if !Path::new(&path).exists() {
            break path;
        }

        index += 1;
    };


    // Stdout layer: exclude benchmark target
    let stdout_layer = fmt::Layer::new()
        .with_writer(std::io::stdout)
        .with_filter(LevelFilter::INFO);

    // Benchmark file
    let benchmark_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(filename)
        .unwrap();

    // Benchmark-only layer
    let benchmark_layer = fmt::Layer::new()
        .with_writer(benchmark_file)
        .with_ansi(false)
        .with_filter(filter_fn(|metadata| {
            metadata.target() == "benchmark"
        }));

    let collector = tracing_subscriber::registry()
        .with(stdout_layer)
        .with(benchmark_layer);

    tracing::subscriber::set_global_default(collector).unwrap();
}


/// This function was copied from Paul Heinrich's own repository:
/// https://github.com/paulheinr/parallel-qsim-berlin/blob/main/src/main/rust/src/bin/berlin.rs
fn add_teleported_vehicle(scenario: &mut MutableScenario, mode: &str) {
    let id = Id::create(mode);
    scenario.garage.vehicle_types.insert(
        id.clone(),
        InternalVehicleType {
            id,
            length: 1.,
            width: 1.,
            max_v: 1.23,
            pce: 0.1,
            fef: 0.0,
            net_mode: Id::create(mode),
            attributes: Default::default(),
        },
    );

    scenario.population.persons.keys().for_each(|id| {
        scenario.garage.vehicles.insert(
            Id::create(&format!("{}_{}", id, mode)),
            rust_qsim::simulation::scenario::vehicles::InternalVehicle {
                id: Id::create(&format!("{}-{}", id, mode)),
                max_v: 0.833,
                pce: 0.1,
                vehicle_type: Id::create(mode),
                attributes: Default::default(),
            },
        );
    });
}