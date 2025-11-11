use crate::simulation::profiling::{
    create_file, end_timing, start_timing, ModeWrapper, PersonIdWrapper, SimTimeWrapper,
    SpanDuration, UuidWrapper,
};
use std::fmt::Debug;
use std::fs::File;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tracing::field::{Field, Visit};
use tracing::span::Attributes;
use tracing::{Event, Id, Level, Metadata};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

const HEADER: [&str; 8] = [
    "timestamp",
    "target",
    "func_name",
    "duration",
    "sim_time",
    "request_uuid",
    "person_id",
    "mode",
];

pub struct RoutingSpanDurationToCSVLayer {
    writer: Arc<Mutex<csv::Writer<File>>>,
    /// Note: TRACE > DEBUG > INFO > WARN > ERROR
    min_level: Level,
    module: String,
}

/// WriterGuard is used to ensure that the writer is flushed at the end.
/// Not 100% sure if this is really needed as the csv::Writer already implements Drop trait. Paul, nov '25.
pub struct WriterGuard {
    writer: Arc<Mutex<csv::Writer<File>>>,
}

impl Drop for WriterGuard {
    fn drop(&mut self) {
        let mut writer = self.writer.lock().unwrap();
        writer.flush().unwrap();
    }
}

impl RoutingSpanDurationToCSVLayer {
    pub fn new(path: &Path, level: Level, module: &str) -> (Self, WriterGuard) {
        let file = create_file(path);
        let mut raw_writer = csv::Writer::from_writer(file);

        raw_writer.write_record(HEADER).unwrap();
        let writer = Arc::new(Mutex::new(raw_writer));

        let s = Self {
            writer: writer.clone(),
            min_level: level,
            module: module.to_string(),
        };

        (s, WriterGuard { writer })
    }
}

impl<S> Layer<S> for RoutingSpanDurationToCSVLayer
where
    // if not LookupSpan, cannot access span data like `span.extensions_mut()`
    S: tracing::Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    /// Enables all events to be processed if they have the minimal level and are in the specified module.
    fn enabled(&self, metadata: &Metadata<'_>, _ctx: Context<'_, S>) -> bool {
        let target = self.in_module(metadata);
        let level = metadata.level() >= &self.min_level;
        target && level
    }

    /// Sets the fields in the span extensions.
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).expect("should exist");
        let mut extensions = span.extensions_mut();
        extensions.insert(SpanDuration::new());

        let mut visitor = RoutingMetadataVisitor::default();
        attrs.record(&mut visitor as &mut dyn Visit);

        if let Some(sim_time) = visitor.sim_time {
            extensions.insert(sim_time);
        }
        if let Some(uuid) = visitor.uuid {
            extensions.insert(uuid);
        }
        if let Some(person_id) = visitor.person_id {
            extensions.insert(person_id);
        }
        if let Some(mode) = visitor.mode {
            extensions.insert(mode);
        }
    }

    /// This function registers events from the same module as the current span and sets the uuid
    /// of the span. This should only be used if the field is not initialized yet when the span is created.
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let x = ctx.current_span().id().unwrap().clone();
        let span = ctx.span(&x).expect("Span should be there!");

        let span_target = span.metadata().module_path().unwrap();
        let module = span_target == event.metadata().module_path().unwrap();

        if !module {
            return;
        }

        let mut visitor = RoutingMetadataVisitor::default();
        event.record(&mut visitor);

        let mut exts = span.extensions_mut();
        if let Some(uuid) = visitor.uuid {
            let v = exts.replace(uuid);
            if v.is_some() {
                panic!("Uuid is already present in span. This can occur, if the current event is not registered \
                by the span you think it is. Check module and level span and event! Also check your layer attributes, \
                as these are used to filter events and spans. Event: {:?}", event);
            }
        }
    }

    /// Set the start time of the span in the extension
    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        start_timing(id, ctx);
    }

    /// Set the duration of the span in the extension.
    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        end_timing(id, ctx)
    }

    /// Write csv entry.
    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let writer = &mut *self.writer.lock().unwrap();

        let span = ctx.span(&id).expect("Span should be there!");
        let extensions = span.extensions();
        let meta = span.metadata();

        let timestep = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string();
        let target = meta.target();
        let func_name = meta.name();
        let duration = extensions
            .get::<SpanDuration>()
            .unwrap()
            .elapsed
            .to_string();
        let sim_time = extensions
            .get::<SimTimeWrapper>()
            .map_or(-1, |sim_time| sim_time.0 as i64)
            .to_string();
        let request_uuid = extensions
            .get::<UuidWrapper>()
            .map_or("-1".to_string(), |uuid| uuid.0.to_string());
        let person_id = extensions
            .get::<PersonIdWrapper>()
            .map_or("", |person_id| person_id.0.as_str());
        let mode = extensions
            .get::<ModeWrapper>()
            .map_or("", |mode| mode.0.as_str());

        writer
            .write_record([
                &timestep,
                target,
                func_name,
                &duration,
                &sim_time,
                &request_uuid,
                person_id,
                mode,
            ])
            .unwrap_or_else(|e| panic!("Failed to write record. {}", e));

        // extensions and span must be dropped explicitly, says the tracing documentation
        drop(extensions);
        drop(span);
    }
}

impl RoutingSpanDurationToCSVLayer {
    fn in_module(&self, metadata: &Metadata) -> bool {
        metadata
            .module_path()
            .map(|m| format!("{}::{}", m, metadata.name()).starts_with(self.module.as_str()))
            .unwrap_or(false)
    }
}

#[derive(Default)]
struct RoutingMetadataVisitor {
    sim_time: Option<SimTimeWrapper>,
    uuid: Option<UuidWrapper>,
    person_id: Option<PersonIdWrapper>,
    mode: Option<ModeWrapper>,
}

impl Visit for RoutingMetadataVisitor {
    fn record_u64(&mut self, field: &Field, value: u64) {
        // be gentle here: try sim_time and any field that contains "now", i.e. "_now".
        if field.name().eq("sim_time") || field.name().contains("now") {
            self.sim_time = Some(SimTimeWrapper(value));
        }
    }

    fn record_u128(&mut self, field: &Field, value: u128) {
        if field.name().eq("uuid") {
            self.uuid = Some(UuidWrapper(value));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name().eq("person_id") {
            self.person_id = Some(PersonIdWrapper(value.to_string()));
        }
        if field.name().eq("mode") {
            self.mode = Some(ModeWrapper(value.to_string()));
        }
    }

    fn record_debug(&mut self, _field: &Field, _value: &dyn Debug) {
        // nothing to do here
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::profiling::routing::RoutingSpanDurationToCSVLayer;
    use std::path::Path;
    use std::str::FromStr;
    // needed for the `with` function on Registry
    use tracing_subscriber::layer::SubscriberExt;
    use uuid::{NoContext, Timestamp, Uuid};

    #[test]
    fn test_creation() {
        let path = Path::new("./test_output/simulation/profiling/routing/test_creation.csv");
        let (_, guard) =
            RoutingSpanDurationToCSVLayer::new(path, tracing::Level::INFO, "rust_qsim");
        drop(guard);
    }

    #[test]
    fn test_all_events() {
        let path = Path::new("./test_output/simulation/profiling/routing/test_events.csv");
        let (layer, guard) = RoutingSpanDurationToCSVLayer::new(
            path,
            tracing::Level::INFO,
            "rust_qsim::simulation::profiling::routing::tests",
        );

        run_test(layer);
        drop(guard);

        let rows = read_csv_structs(path);
        assert_eq!(rows.len(), 2);
        let expected = get_expected();
        assert_eq!(rows.first(), Some(&expected[0]));
        assert_eq!(rows.get(1), Some(&expected[1]));
    }

    #[test]
    fn test_trace_events() {
        let path = Path::new("./test_output/simulation/profiling/routing/test_trace_events.csv");
        let (layer, guard) = RoutingSpanDurationToCSVLayer::new(
            path,
            tracing::Level::TRACE,
            "rust_qsim::simulation::profiling::routing::tests",
        );

        run_test(layer);
        drop(guard);

        let rows = read_csv_structs(path);
        assert_eq!(rows.len(), 1);
        // The second event is by the outer span, which has level trace.
        assert_eq!(rows.first(), Some(&get_expected()[1]));
    }

    #[test]
    fn test_module_filtering() {
        let path =
            Path::new("./test_output/simulation/profiling/routing/test_module_filtering.csv");
        let (layer, guard) = RoutingSpanDurationToCSVLayer::new(
            path,
            tracing::Level::INFO,
            "rust_qsim::simulation::profiling::routing::tests::foo::bar",
        );

        run_test(layer);
        drop(guard);

        let rows = read_csv_structs(path);
        assert_eq!(rows.len(), 1);
        // Only the inner function should be recorded due to module filtering.
        assert_eq!(rows.first(), Some(&get_expected()[0]));
    }

    #[test]
    fn test_module_filtering_with_level() {
        let path = Path::new(
            "./test_output/simulation/profiling/routing/test_module_filtering_with_level.csv",
        );
        let (layer, guard) = RoutingSpanDurationToCSVLayer::new(
            path,
            tracing::Level::TRACE,
            "rust_qsim::simulation::profiling::routing::tests::foo::bar",
        );

        run_test(layer);
        drop(guard);

        let rows = read_csv_structs(path);
        assert_eq!(rows.len(), 0);
    }

    fn run_test(layer: RoutingSpanDurationToCSVLayer) {
        let layered = tracing_subscriber::registry().with(layer);
        let guard = tracing::subscriber::set_default(layered);
        let ts = Timestamp::from_unix(NoContext, 1, 1);
        let uuid = Uuid::new_v7(ts);

        foo::f(42, uuid.as_u128(), "person1", "car");
        drop(guard);
    }

    fn get_expected() -> Vec<RoutingRow> {
        let e1 = RoutingRow {
            target: "rust_qsim::simulation::profiling::routing::tests::foo::bar".to_string(),
            func_name: "b".to_string(),
            sim_time: 43,
            request_uuid: "2418384578988518367448237822".parse().unwrap(),
            person_id: "person2".to_string(),
            mode: "bike".to_string(),
        };

        let e2 = RoutingRow {
            target: "rust_qsim::simulation::profiling::routing::tests::foo".to_string(),
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
}
