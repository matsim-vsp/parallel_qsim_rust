// needed for the `with` function on Registry
use macros::integration_test;
use rust_qsim::simulation::profiling::routing::RoutingSpanDurationToCSVLayer;
use std::path::Path;
use std::str::FromStr;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, Layer, Registry};
use uuid::{NoContext, Timestamp, Uuid};

/// all tests are marked with serial to avoid race conditions on the subscriber registry

#[integration_test(rust_qsim)]
fn test_creation() {
    let path = Path::new("./test_output/simulation/profiling/routing/test_creation.csv");
    let (_, guard) = RoutingSpanDurationToCSVLayer::new(path);
    drop(guard);
}

#[integration_test(rust_qsim)]
fn test_all_events() {
    let path = Path::new("./test_output/simulation/profiling/routing/test_events.csv");
    let (layer, guard) = RoutingSpanDurationToCSVLayer::new(path);

    let filtered = layer.with_filter(EnvFilter::new("test_tracing=trace"));
    run_test(filtered.boxed());
    drop(guard);

    let rows = read_csv_structs(path);
    assert_eq!(rows.len(), 2);
    let expected = get_expected();
    assert_eq!(rows.first(), Some(&expected[0]));
    assert_eq!(rows.get(1), Some(&expected[1]));
}

#[integration_test(rust_qsim)]
fn test_info_events() {
    let path = Path::new("./test_output/simulation/profiling/routing/test_info_events.csv");
    let (layer, guard) = RoutingSpanDurationToCSVLayer::new(path);

    let filtered = layer.with_filter(EnvFilter::new("test_tracing=info"));
    run_test(filtered.boxed());
    drop(guard);

    let rows = read_csv_structs(path);
    assert_eq!(rows.len(), 1);
    // The first event is by the inner span, which has level info.
    assert_eq!(rows.first(), Some(&get_expected()[0]));
}

#[integration_test(rust_qsim)]
fn test_module_filtering() {
    let path = Path::new("./test_output/simulation/profiling/routing/test_module_filtering.csv");
    let (layer, guard) = RoutingSpanDurationToCSVLayer::new(path);

    let filtered = layer
        .with_filter(EnvFilter::new("test_tracing::foo::bar=info"))
        .boxed();
    run_test(filtered);

    drop(guard);

    let rows = read_csv_structs(path);
    assert_eq!(rows.len(), 1);
    // Only the inner function should be recorded due to module filtering.
    assert_eq!(rows.first(), Some(&get_expected()[0]));
}

#[integration_test(rust_qsim)]
fn test_module_filtering_with_level() {
    let path = Path::new(
        "./test_output/simulation/profiling/routing/test_module_filtering_with_level.csv",
    );
    let (layer, guard) = RoutingSpanDurationToCSVLayer::new(path);

    let filtered = layer
        .with_filter(EnvFilter::new("test_tracing::foo::bar=warn"))
        .boxed();

    run_test(filtered.boxed());
    drop(guard);

    let rows = read_csv_structs(path);
    assert_eq!(rows.len(), 0);
}

fn run_test(layer: Box<dyn Layer<Registry> + Send + Sync>) {
    let layered = tracing_subscriber::registry().with(layer);
    // this default is set thread-wise, which is why serial tests are required
    let guard = tracing::subscriber::set_default(layered);
    let ts = Timestamp::from_unix(NoContext, 1, 1);
    let uuid = Uuid::new_v7(ts);

    foo::f(42, uuid.as_u128(), "person1", "car");
    drop(guard);
}

fn get_expected() -> Vec<RoutingRow> {
    let e1 = RoutingRow {
        target: "test_tracing::foo::bar".to_string(),
        func_name: "b".to_string(),
        sim_time: 43,
        request_uuid: "2418384578988518367448237822".parse().unwrap(),
        person_id: "person2".to_string(),
        mode: "bike".to_string(),
    };

    let e2 = RoutingRow {
        target: "test_tracing::foo".to_string(),
        func_name: "f".to_string(),
        sim_time: 42,
        request_uuid: "1209464644267738956154342694".parse().unwrap(),
        person_id: "person1".to_string(),
        mode: "car".to_string(),
    };

    vec![e1, e2]
}

fn read_csv_structs(path: &Path) -> Vec<RoutingRow> {
    use csv::Reader;
    use std::fs::File;

    let file = File::open(path).unwrap();
    let mut reader = Reader::from_reader(file);
    let mut rows = Vec::new();
    for result in reader.records() {
        let record = result.unwrap();
        let row = RoutingRow {
            // skip the first column as this is the (non-deterministic) timestamp
            target: record.get(1).unwrap().to_string(),
            func_name: record.get(2).unwrap().to_string(),
            // skip the third column as this is the (non-deterministic) duration
            sim_time: record.get(4).unwrap().parse().unwrap(),
            request_uuid: record.get(5).unwrap().parse().unwrap(),
            person_id: record.get(6).unwrap().to_string(),
            mode: record.get(7).unwrap().to_string(),
        };
        rows.push(row);
    }
    rows
}

#[derive(Debug, PartialEq)]
struct RoutingRow {
    target: String,
    func_name: String,
    sim_time: i64,
    request_uuid: ComparableUuid,
    person_id: String,
    mode: String,
}

#[derive(Debug)]
struct ComparableUuid(u128);

impl FromStr for ComparableUuid {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse::<u128>() {
            Ok(uuid_u128) => Ok(ComparableUuid(uuid_u128)),
            Err(_) => Err(()),
        }
    }
}

impl PartialEq for ComparableUuid {
    /// Compare the 48-bit Unix-ms timestamp from a UUIDv7 (stored as u128).
    fn eq(&self, other: &Self) -> bool {
        self.v7_timestamp_ms() == other.v7_timestamp_ms()
    }
}

impl ComparableUuid {
    /// Extract the 48-bit Unix-ms timestamp from a UUIDv7 (stored as u128).
    fn v7_timestamp_ms(&self) -> u64 {
        // UUIDv7 layout is big-endian with the top 48 bits = timestamp (ms)
        // Shift right by 80 (128 - 48) to bring those bits to the bottom.
        (self.0 >> 80) as u64
    }
}

pub(crate) mod foo {
    use tracing::{info, instrument};

    #[instrument(level = "trace")]
    pub(crate) fn f(sim_time: u64, uuid: u128, person_id: &str, mode: &str) {
        info!("some_function");
        bar::b(sim_time + 1, "person2", "bike");
    }

    pub(crate) mod bar {
        use tracing::{info, instrument};
        use uuid::{NoContext, Timestamp, Uuid};

        #[instrument(level = "info")]
        pub(crate) fn b(now: u64, person_id: &str, mode: &str) {
            info!("some_function");
            let ts = Timestamp::from_unix(NoContext, 2, 2);
            let new_uuid = Uuid::new_v7(ts);
            info!(uuid = new_uuid.as_u128());
        }
    }
}
