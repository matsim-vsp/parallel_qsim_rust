use rust_qsim::simulation::config::{
    DrtProcessType, PartitionMethod, Profiling, RoutingMode, WriteEvents,
};
use rust_qsim::simulation::config_simple::SimpleConfig;
use std::fs;
use std::path::PathBuf;

#[test]
fn parse_empty_modules() {
    let config: SimpleConfig = serde_yaml::from_str("modules: {}").unwrap();
    assert_eq!(config.modules.network, None);
    assert_eq!(config.modules.population, None);
    assert_eq!(config.modules.vehicles, None);
    assert_eq!(config.modules.ids, None);
    assert_eq!(config.modules.partitioning, None);
    assert_eq!(config.modules.output, None);
    assert_eq!(config.modules.routing, None);
    assert_eq!(config.modules.simulation, None);
    assert_eq!(config.modules.computational_setup, None);
    assert_eq!(config.modules.drt, None);
}

#[test]
fn parse_all_modules() {
    let yaml = fs::read_to_string("tests/resources/simple_config/example.yml").unwrap();
    let config: SimpleConfig = serde_yaml::from_str(&yaml).unwrap();

    assert_eq!(config.modules.network, Some(PathBuf::from("path/to/network")));
    assert_eq!(config.modules.population, Some(PathBuf::from("path/to/population")));
    assert_eq!(config.modules.vehicles, Some(PathBuf::from("path/to/vehicles")));
    assert_eq!(config.modules.ids, Some(PathBuf::from("path/to/ids")));

    let p = config.modules.partitioning.unwrap();
    assert_eq!(p.num_parts, 1);
    assert_eq!(p.method, PartitionMethod::None);

    let o = config.modules.output.unwrap();
    assert_eq!(o.output_dir, PathBuf::from("./test_output"));
    assert_eq!(o.profiling, Profiling::None);
    assert_eq!(o.write_events, WriteEvents::Proto);

    let r = config.modules.routing.unwrap();
    assert_eq!(r.mode, RoutingMode::UsePlans);

    let s = config.modules.simulation.unwrap();
    assert_eq!(s.start_time, 0);
    assert_eq!(s.end_time, 86400);
    assert_eq!(s.sample_size, 1.0);
    assert_eq!(s.stuck_threshold, 10);
    assert_eq!(s.main_modes, vec!["car"]);

    let cs = config.modules.computational_setup.unwrap();
    assert!(!cs.global_sync);
    assert_eq!(cs.adapter_worker_threads, 3);
    assert_eq!(cs.retry_time_seconds, 600);

    let drt = config.modules.drt.unwrap();
    assert_eq!(drt.process_type, DrtProcessType::OneProcess);
    assert_eq!(drt.services.len(), 1);
    assert_eq!(drt.services[0].mode, "drt_a");
}

#[test]
fn roundtrip_serialize_deserialize() {
    let yaml = fs::read_to_string("tests/resources/simple_config/example.yml").unwrap();
    let config: SimpleConfig = serde_yaml::from_str(&yaml).unwrap();
    let serialized = serde_yaml::to_string(&config).unwrap();
    let roundtripped: SimpleConfig = serde_yaml::from_str(&serialized).unwrap();
    assert_eq!(config, roundtripped);
}
