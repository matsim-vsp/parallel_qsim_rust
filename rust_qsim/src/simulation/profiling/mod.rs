pub mod routing;

use arrow2::array::FixedSizeBinaryArray;
use std::error::Error;
use std::fmt::Debug;
use std::fs;
use std::fs::File;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};
use tracing::field::Field;
use tracing::span::Attributes;
use tracing::{Id, Level, Metadata, Subscriber};
use tracing_subscriber::field::Visit;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::{Extensions, LookupSpan};
use tracing_subscriber::Layer;

use arrow2::array::{Array, Int64Array, Utf8Array};
use arrow2::chunk::Chunk;
use arrow2::datatypes::{DataType, Schema};
use arrow2::io::parquet::write::{
    CompressionOptions, Encoding, FileWriter, RowGroupIterator, Version, WriteOptions,
};

// Implementation overview:
// - The layer supports two backends at runtime: CSV and Parquet.
// - The public API exposes constructors `new_csv` and `new_parquet` that return the layer and a WriterGuard.
// - The WriterGuard flushes/writes on drop. CSV uses csv::Writer flush, Parquet writes an Arrow RecordBatch via arrow2.

pub struct SpanDurationToFileLayer {
    backend: Backend,
}

#[non_exhaustive]
pub enum WriterGuard {
    Csv(Arc<Mutex<csv::Writer<File>>>),
    Parquet(Arc<Mutex<BufferedSpanData>>),
}

#[non_exhaustive]
enum Backend {
    Csv {
        writer: Arc<Mutex<csv::Writer<File>>>,
        level: Level,
    },
    Parquet {
        inner: Arc<Mutex<BufferedSpanData>>,
        level: Level,
    },
}

impl Backend {
    fn level(&self) -> &Level {
        match self {
            Backend::Csv { level, .. } => level,
            Backend::Parquet { level, .. } => level,
        }
    }
}

struct SpanDuration {
    elapsed: u64,
    last: Instant,
}

// We need these type wrappers to get distinct types for the extensions
#[derive(Debug)]
struct Uuid(pub u128);
struct PersonId(pub String);
struct Mode(pub String);
struct Rank(pub u64);
struct SimTime(pub u64);

struct MetadataVisitor {
    rank: Option<u64>,
    sim_time: Option<u64>,
}

impl MetadataVisitor {
    fn new() -> Self {
        MetadataVisitor {
            rank: None,
            sim_time: None,
        }
    }
}

impl Visit for MetadataVisitor {
    fn record_u64(&mut self, field: &Field, value: u64) {
        //fetch rank
        if field.name().eq("rank") {
            self.rank = Some(value);
        }

        //fetch now (in some cases, the field name is "now" and in others "_now")
        if field.name().contains("now") {
            self.sim_time = Some(value);
        }
    }

    fn record_debug(&mut self, _field: &Field, _value: &dyn Debug) {
        //nothing to do here
    }
}

// Buffered data for parquet backend
pub struct BufferedSpanData {
    pub timestamp: Vec<u128>,
    pub target: Vec<String>,
    pub func_name: Vec<String>,
    pub duration_ns: Vec<i64>,
    pub sim_time: Vec<i64>,
    pub rank: Vec<i64>,
    pub path: std::path::PathBuf,
}

impl BufferedSpanData {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self {
            timestamp: Vec::new(),
            target: Vec::new(),
            func_name: Vec::new(),
            duration_ns: Vec::new(),
            sim_time: Vec::new(),
            rank: Vec::new(),
            path,
        }
    }

    fn create_parent(&self) {
        let prefix = self.path.parent().unwrap();
        fs::create_dir_all(prefix).unwrap();
    }

    pub fn write_parquet(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Build arrow arrays
        let fields = vec![
            arrow2::datatypes::Field::new("timestamp", DataType::Utf8, false),
            arrow2::datatypes::Field::new("target", DataType::Utf8, false),
            arrow2::datatypes::Field::new("func_name", DataType::Utf8, false),
            arrow2::datatypes::Field::new("duration_ns", DataType::Int64, false),
            arrow2::datatypes::Field::new("sim_time", DataType::Int64, false),
            arrow2::datatypes::Field::new("rank", DataType::Int64, false),
        ];
        let schema = Schema::from(fields);

        let columns: Vec<Box<dyn Array>> = vec![
            Box::new(convert_u128_to_fixed_size_binary(&self.timestamp)),
            Box::new(Utf8Array::<i32>::from_slice(&self.target)),
            Box::new(Utf8Array::<i32>::from_slice(&self.func_name)),
            Box::new(Int64Array::from_slice(self.duration_ns.as_slice())),
            Box::new(Int64Array::from_slice(self.sim_time.as_slice())),
            Box::new(Int64Array::from_slice(self.rank.as_slice())),
        ];

        write_parquet(&schema, columns, &self.path)?
    }
}

impl SpanDurationToFileLayer {
    pub fn new_csv(path: &Path, level: Level) -> (Self, WriterGuard) {
        let file = create_file(path);
        let mut writer = csv::Writer::from_writer(file);
        writer
            .write_record(vec![
                "timestamp",
                "target",
                "func_name",
                "duration_ns",
                "sim_time",
                "rank",
            ])
            .unwrap();

        // wrap the writer into an arc<mutex<...>> so that we can keep a reference which gets dropped
        // at the end of the scope calling this method. The mutex is necessary, because the Layer
        // must be Sync + Send for the tracing_subscriber subscriber
        let writer_ref = Arc::new(Mutex::new(writer));
        let backend = Backend::Csv {
            writer: writer_ref.clone(),
            level,
        };
        (Self { backend }, WriterGuard::Csv(writer_ref))
    }

    pub fn new_parquet(path: &Path, level: Level) -> (Self, WriterGuard) {
        let buf = BufferedSpanData::new(path.to_path_buf());
        buf.create_parent();
        let inner = Arc::new(Mutex::new(buf));
        let backend = Backend::Parquet {
            inner: inner.clone(),
            level,
        };
        (Self { backend }, WriterGuard::Parquet(inner))
    }
}

/// Simple Layer implementation, which records the time elapsed between a span being opened and being
/// closed again. Once a span is closed, it writes the elapsed time into a csv journal
///
/// `Context` is managed by the `tracing_subscriber` library. All functions implemented here are called by the
/// `instrument` macro.
///
/// `Attributes` store all custom fields. The `MetadataVisitor` is used to extract the field values.
///
/// `Span` stores information about the scope of an instrumentation call.
impl<S> Layer<S> for SpanDurationToFileLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        if attrs.metadata().level() > self.backend.level() {
            return;
        }

        let span = ctx.span(id).expect("should exist");
        let mut extensions = span.extensions_mut();

        let option = extensions.replace(SpanDuration::new());
        assert!(option.is_none(), "Trying to initialize Span, but it already exists. This should not happen. \
        It might happen, if multiple Layers are trying to insert the same type into the extensions. \
        Check the configuration of the Layers with respect to their including/excluding module path.");

        let mut visitor = MetadataVisitor::new();
        attrs.record(&mut visitor as &mut dyn Visit);
        if let Some(rank) = visitor.rank {
            extensions.insert(Rank(rank));
        }
        if let Some(sim_time) = visitor.sim_time {
            extensions.insert(SimTime(sim_time));
        }
    }

    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        // respect levels
        if ctx.metadata(id).unwrap().level() > self.backend.level() {
            return;
        }
        start_timing::<S>(id, ctx);
    }

    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        if ctx.metadata(id).unwrap().level() > self.backend.level() {
            return;
        }
        end_timing::<S>(id, ctx);
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let span = ctx.span(&id).expect("Span should be there!");
        let extensions = span.extensions();
        let meta = span.metadata();
        let (timestep, target, func_name, duration, sim_time) = extract_entries(&extensions, meta);
        let rank = extensions.get::<Rank>().map_or(-1, |rank| rank.0 as i64);
        match &self.backend {
            Backend::Csv { writer, .. } => {
                let writer = &mut *writer.lock().unwrap();
                writer
                    .write_record([
                        &timestep.to_string(),
                        target,
                        func_name,
                        &duration,
                        &sim_time,
                        &rank.to_string(),
                    ])
                    .unwrap();

                drop(extensions);
                drop(span);
            }
            Backend::Parquet { inner, .. } => {
                let mut inner = inner.lock().unwrap();
                inner.timestamp.push(timestep);
                inner.target.push(target.to_string());
                inner.func_name.push(func_name.to_string());
                inner
                    .duration_ns
                    .push(duration.parse::<i64>().unwrap_or(-1));
                inner.sim_time.push(sim_time.parse::<i64>().unwrap_or(-1));
                inner.rank.push(rank);

                drop(extensions);
                drop(span);
            }
        }
    }
}

fn extract_entries<'a>(
    extensions: &Extensions,
    meta: &Metadata<'a>,
) -> (u128, &'a str, &'a str, String, String) {
    let timestep = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let target = meta.target();
    let func_name = meta.name();
    let span_duration = extensions
        .get::<SpanDuration>()
        .unwrap()
        .elapsed
        .to_string();
    let sim_time = extensions
        .get::<SimTime>()
        .map_or(-1, |sim_time| sim_time.0 as i64)
        .to_string();
    (timestep, target, func_name, span_duration, sim_time)
}

fn end_timing<S: Subscriber + for<'a> LookupSpan<'a>>(id: &Id, ctx: Context<S>) {
    let span = ctx.span(id).expect("Span should be there");
    let mut extensions = span.extensions_mut();

    if let Some(timing) = extensions.get_mut::<SpanDuration>() {
        let now = Instant::now();
        timing.elapsed += (now - timing.last).as_nanos() as u64;
    }
}

/// Start timing for span
fn start_timing<S: Subscriber + for<'a> LookupSpan<'a>>(id: &Id, ctx: Context<S>) {
    let span = ctx.span(id).expect("Should exist");
    let mut extensions = span.extensions_mut();

    if let Some(timing) = extensions.get_mut::<SpanDuration>() {
        timing.last = Instant::now();
    }
}

pub fn create_file(path: &Path) -> File {
    let prefix = path.parent().unwrap();
    fs::create_dir_all(prefix).unwrap();
    File::create(path).unwrap_or_else(|_e| panic!("Failed to open file at: {path:?}"))
}

fn convert_u128_to_fixed_size_binary(data: &Vec<u128>) -> FixedSizeBinaryArray {
    let byte_width = 16;
    let mut timestamp_data = Vec::with_capacity(data.len() * byte_width);
    for t in data {
        timestamp_data.extend_from_slice(&t.to_le_bytes());
    }
    FixedSizeBinaryArray::new(
        DataType::FixedSizeBinary(byte_width),
        timestamp_data.into(),
        None,
    )
}

fn write_parquet(
    schema: &Schema,
    columns: Vec<Box<dyn Array>>,
    path: &Path,
) -> Result<Result<(), Box<dyn Error>>, Box<dyn Error>> {
    let chunk = Chunk::new(columns);

    let file = File::create(path)?;
    let options = WriteOptions {
        write_statistics: false,
        version: Version::V2,
        compression: CompressionOptions::Snappy,
        data_pagesize_limit: None,
    };

    // Simple plain encoding for all fields
    let encodings = vec![vec![Encoding::Plain]; schema.fields.len()];

    let row_group_iter =
        RowGroupIterator::try_new(vec![Ok(chunk)].into_iter(), schema, options, encodings)?;

    let mut writer = FileWriter::try_new(file, schema.clone(), options)?;
    for rg in row_group_iter {
        let rg = rg?; // RowGroupIter
        writer.write(rg)?;
    }
    writer.end(None)?;
    Ok(Ok(()))
}

impl Drop for WriterGuard {
    fn drop(&mut self) {
        match self {
            WriterGuard::Csv(writer) => {
                let mut writer = writer.lock().unwrap();
                writer.flush().expect("Problem flushing writer");
            }
            WriterGuard::Parquet(inner) => {
                let inner = inner.lock().unwrap();
                if let Err(e) = inner.write_parquet() {
                    eprintln!("Failed to write parquet profiling file: {}", e);
                }
            }
        }
    }
}

impl SpanDuration {
    fn new() -> Self {
        Self {
            elapsed: 0,
            last: Instant::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::thread::sleep;
    use std::time::Duration;

    use tracing::level_filters::LevelFilter;
    use tracing::{info, instrument, Level};
    use tracing_subscriber::fmt::format::FmtSpan;
    use tracing_subscriber::fmt::Layer;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::Layer as OtherLayer;

    use crate::simulation::profiling::SpanDurationToFileLayer;

    #[test]
    fn test_events() {
        let path = PathBuf::from("./test_output/simulation/profiling/test_events.csv");

        let (csv_layer, _guard) = SpanDurationToFileLayer::new_csv(&path, Level::INFO);
        let layers = tracing_subscriber::registry().with(csv_layer).with(
            Layer::new()
                .with_span_events(FmtSpan::CLOSE)
                .with_filter(LevelFilter::TRACE),
        );
        tracing::subscriber::set_global_default(layers).expect("TODO: panic message");

        info!("Before func");
        some_function();
        info!("After func");

        some_other_function(7, std::f32::consts::PI);
    }

    #[instrument]
    fn some_function() {
        info!("Inside some function.")
    }

    #[instrument(level = "trace", fields(rank = 42u32))]
    fn some_other_function(_now: u32, b: f32) {
        info!("Inside some other function");
        sleep(Duration::from_nanos(10));
    }
}
