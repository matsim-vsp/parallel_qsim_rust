// needed for the `with` function on Registry
use arrow2::array::FixedSizeBinaryArray;
use macros::integration_test;
use rust_qsim::simulation::profiling::routing::{
    RoutingSpanDurationToFileLayer, RoutingWriterGuard,
};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, Layer, Registry};
use uuid::{NoContext, Timestamp, Uuid};

enum Mode {
    Csv,
    Parquet,
}

impl Mode {
    fn expansion(&self) -> &'static str {
        match self {
            Mode::Csv => "csv",
            Mode::Parquet => "parquet",
        }
    }

    fn layer(&self, path: &Path) -> (RoutingSpanDurationToFileLayer, RoutingWriterGuard) {
        let mut buf = PathBuf::from(path);
        buf.set_extension(self.expansion());
        match self {
            Mode::Csv => RoutingSpanDurationToFileLayer::new_csv(Path::new(&buf)),
            Mode::Parquet => RoutingSpanDurationToFileLayer::new_parquet(Path::new(&buf), 50_000),
        }
    }

    fn read_rows(&self, path: &Path) -> Vec<RoutingRow> {
        let mut buf = PathBuf::from(path);
        buf.set_extension(self.expansion());
        match self {
            Mode::Csv => read_csv_structs(Path::new(&buf)),
            Mode::Parquet => read_parquet_structs(Path::new(&buf)),
        }
    }
}

// all tests are marked with serial to avoid race conditions on the subscriber registry

#[integration_test(rust_qsim)]
fn test_creation_csv() {
    test_creation(Mode::Csv);
}

#[integration_test(rust_qsim)]
fn test_creation_parquet() {
    test_creation(Mode::Parquet);
}

fn test_creation(mode: Mode) {
    let path = Path::new("./test_output/simulation/profiling/routing/test_creation");
    let (_, guard) = mode.layer(path);
    drop(guard);
}

#[integration_test(rust_qsim)]
fn test_all_events_csv() {
    test_all_events(Mode::Csv);
}

#[integration_test(rust_qsim)]
fn test_all_events_parquet() {
    test_all_events(Mode::Parquet);
}

fn test_all_events(mode: Mode) {
    let path = Path::new("./test_output/simulation/profiling/routing/test_events");
    let (layer, guard) = mode.layer(path);

    let filtered = layer.with_filter(EnvFilter::new("test_tracing=trace"));
    run_test(filtered.boxed());
    drop(guard);

    let rows = mode.read_rows(path);
    assert_eq!(rows.len(), 2);
    let expected = get_expected();
    assert_eq!(rows.first(), Some(&expected[0]));
    assert_eq!(rows.get(1), Some(&expected[1]));
}

#[integration_test(rust_qsim)]
fn test_info_events_csv() {
    test_info_events(Mode::Csv);
}

#[integration_test(rust_qsim)]
fn test_info_events_parquet() {
    test_info_events(Mode::Parquet);
}

fn test_info_events(mode: Mode) {
    let path = Path::new("./test_output/simulation/profiling/routing/test_info_events");
    let (layer, guard) = mode.layer(path);

    let filtered = layer.with_filter(EnvFilter::new("test_tracing=info"));
    run_test(filtered.boxed());
    drop(guard);

    let rows = mode.read_rows(path);
    assert_eq!(rows.len(), 1);
    // The first event is by the inner span, which has level info.
    assert_eq!(rows.first(), Some(&get_expected()[0]));
}

#[integration_test(rust_qsim)]
fn test_module_filtering_csv() {
    test_module_filtering(Mode::Csv);
}

#[integration_test(rust_qsim)]
fn test_module_filtering_parquet() {
    test_module_filtering(Mode::Parquet);
}

fn test_module_filtering(mode: Mode) {
    let path = Path::new("./test_output/simulation/profiling/routing/test_module_filtering");
    let (layer, guard) = mode.layer(path);

    let filtered = layer
        .with_filter(EnvFilter::new("test_tracing::foo::bar=info"))
        .boxed();
    run_test(filtered);

    drop(guard);

    let rows = mode.read_rows(path);
    assert_eq!(rows.len(), 1);
    // Only the inner function should be recorded due to module filtering.
    assert_eq!(rows.first(), Some(&get_expected()[0]));
}

#[integration_test(rust_qsim)]
fn test_module_filtering_with_level_csv() {
    test_module_filtering_with_level(Mode::Csv);
}

#[integration_test(rust_qsim)]
fn test_module_filtering_with_level_parquet() {
    test_module_filtering_with_level(Mode::Parquet);
}

fn test_module_filtering_with_level(mode: Mode) {
    let path =
        Path::new("./test_output/simulation/profiling/routing/test_module_filtering_with_level");
    let (layer, guard) = mode.layer(path);

    let filtered = layer
        .with_filter(EnvFilter::new("test_tracing::foo::bar=warn"))
        .boxed();

    run_test(filtered.boxed());
    drop(guard);

    let rows = mode.read_rows(path);
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

// Parquet reader helper used by Parquet tests
fn read_parquet_structs(path: &std::path::Path) -> Vec<RoutingRow> {
    use arrow2::array::Int64Array;
    use arrow2::array::Utf8Array;
    use arrow2::io::parquet::read::{infer_schema, read_metadata, FileReader};
    use std::fs::File;

    let mut file = BufReader::new(File::open(path).unwrap());
    let metadata = read_metadata(&mut file).unwrap();
    let schema = infer_schema(&metadata).unwrap();
    let row_groups = metadata.row_groups.clone();

    let reader = FileReader::new(file, row_groups, schema, None, None, None);
    let mut rows = Vec::new();

    for chunk_res in reader {
        let chunk = chunk_res.unwrap();
        // column layout assumed: timestamp, target, func_name, duration_ns, sim_time, request_uuid, person_id, mode
        let target_array = chunk.arrays()[1]
            .as_any()
            .downcast_ref::<Utf8Array<i32>>()
            .unwrap();
        let func_array = chunk.arrays()[2]
            .as_any()
            .downcast_ref::<Utf8Array<i32>>()
            .unwrap();
        let sim_time_array = chunk.arrays()[4]
            .as_any()
            .downcast_ref::<Int64Array>()
            .unwrap();
        let uuid_array = chunk.arrays()[5]
            .as_any()
            .downcast_ref::<FixedSizeBinaryArray>()
            .unwrap();
        let person_array = chunk.arrays()[6]
            .as_any()
            .downcast_ref::<Utf8Array<i32>>()
            .unwrap();
        let mode_array = chunk.arrays()[7]
            .as_any()
            .downcast_ref::<Utf8Array<i32>>()
            .unwrap();

        let len = chunk.len();
        for i in 0..len {
            let x = uuid_array.value(i);
            let uuid = read_u128(x);

            let row = RoutingRow {
                target: target_array.value(i).to_string(),
                func_name: func_array.value(i).to_string(),
                sim_time: sim_time_array.value(i),
                request_uuid: ComparableUuid(uuid),
                person_id: person_array.value(i).to_string(),
                mode: mode_array.value(i).to_string(),
            };
            rows.push(row);
        }
    }

    rows
}

fn read_u128(x: &[u8]) -> u128 {
    let mut buf = [0u8; 16];
    buf.copy_from_slice(x);
    u128::from_le_bytes(buf)
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
