use crate::simulation::profiling::{
    convert_u128_to_fixed_size_binary, create_file, end_timing, extract_entries, start_timing,
    write_parquet, Mode, PersonId, SimTime, SpanDuration, Uuid,
};
use std::fmt::Debug;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::field::{Field, Visit};
use tracing::span::Attributes;
use tracing::{Event, Id};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

use arrow2::array::{Array, Int64Array, Utf8Array};
use arrow2::datatypes::{DataType, Schema};
use std::fs::File;

const HEADER: [&str; 8] = [
    "timestamp",
    "target",
    "func_name",
    "duration_ns",
    "sim_time",
    "request_uuid",
    "person_id",
    "mode",
];

pub enum RoutingWriterGuard {
    Csv(Arc<Mutex<csv::Writer<File>>>),
    Parquet(Arc<Mutex<BufferedRoutingData>>),
}

pub enum RoutingBackend {
    Csv {
        writer: Arc<Mutex<csv::Writer<File>>>,
    },
    Parquet {
        inner: Arc<Mutex<BufferedRoutingData>>,
    },
}

pub struct RoutingSpanDurationToFileLayer {
    backend: RoutingBackend,
}

impl RoutingSpanDurationToFileLayer {
    pub fn new_csv(path: &Path) -> (Self, RoutingWriterGuard) {
        let file = create_file(path);
        let mut raw_writer = csv::Writer::from_writer(file);
        raw_writer.write_record(HEADER).unwrap();
        let writer = Arc::new(Mutex::new(raw_writer));
        (
            Self {
                backend: RoutingBackend::Csv {
                    writer: writer.clone(),
                },
            },
            RoutingWriterGuard::Csv(writer),
        )
    }

    pub fn new_parquet(path: &Path) -> (Self, RoutingWriterGuard) {
        let buf = BufferedRoutingData::new(path.to_path_buf());
        buf.create_parent();
        let inner = Arc::new(Mutex::new(buf));
        (
            Self {
                backend: RoutingBackend::Parquet {
                    inner: inner.clone(),
                },
            },
            RoutingWriterGuard::Parquet(inner),
        )
    }
}

impl<S> Layer<S> for RoutingSpanDurationToFileLayer
where
    S: tracing::Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
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

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        if let Some(id) = ctx.current_span().id() {
            let span = ctx.span(id).expect("Span should be there!");
            let span_target = span.metadata().target();
            let module = span_target == event.metadata().target();

            if !module {
                return;
            }

            let mut visitor = RoutingMetadataVisitor::default();
            event.record(&mut visitor);

            let mut exts = span.extensions_mut();
            if let Some(uuid) = visitor.uuid {
                let v = exts.replace(uuid);
                assert!(v.is_none(), "Uuid already present in span; unexpected duplicate event registration. Event: {:?}", event);
            }

            if let Some(mode) = visitor.mode {
                let v = exts.replace(mode);
                assert!(v.is_none(), "Mode already present in span; unexpected duplicate event registration. Event: {:?}", event);
            }
        }
    }

    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        start_timing(id, ctx);
    }
    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        end_timing(id, ctx);
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        match &self.backend {
            RoutingBackend::Csv { writer } => {
                let writer = &mut *writer.lock().unwrap();

                let span = ctx.span(&id).expect("Span should be there!");
                let extensions = span.extensions();
                let meta = span.metadata();

                let (timestep, target, func_name, duration, sim_time) =
                    extract_entries(&extensions, meta);
                let request_uuid = extensions
                    .get::<Uuid>()
                    .map_or("-1".to_string(), |uuid| uuid.0.to_string());
                let person_id = extensions
                    .get::<PersonId>()
                    .map_or("", |person_id| person_id.0.as_str());
                let mode = extensions.get::<Mode>().map_or("", |mode| mode.0.as_str());

                writer
                    .write_record([
                        &timestep.to_string(),
                        target,
                        func_name,
                        &duration,
                        &sim_time,
                        &request_uuid,
                        person_id,
                        mode,
                    ])
                    .unwrap_or_else(|e| panic!("Failed to write record. {}", e));

                drop(extensions);
                drop(span);
            }
            RoutingBackend::Parquet { inner } => {
                let span = ctx.span(&id).expect("Span should be there!");
                let extensions = span.extensions();
                let meta = span.metadata();

                let (timestep, _target, func_name, duration, sim_time) =
                    extract_entries(&extensions, meta);
                let request_uuid = extensions.get::<Uuid>().map_or(0, |uuid| uuid.0);
                let person_id = extensions
                    .get::<PersonId>()
                    .map_or("".to_string(), |person_id| person_id.0.clone());
                let mode = extensions
                    .get::<Mode>()
                    .map_or("".to_string(), |mode| mode.0.clone());

                let mut inner = inner.lock().unwrap();
                inner.timestamp.push(timestep);
                inner.target.push(meta.target().to_string());
                inner.func_name.push(func_name.to_string());
                inner
                    .duration_ns
                    .push(duration.parse::<i64>().unwrap_or(-1));
                inner.sim_time.push(sim_time.parse::<i64>().unwrap_or(-1));
                inner.request_uuid.push(request_uuid);
                inner.person_id.push(person_id);
                inner.mode.push(mode);

                drop(extensions);
                drop(span);
            }
        }
    }
}

// Parquet-buffered routing data
pub struct BufferedRoutingData {
    pub timestamp: Vec<u128>,
    pub target: Vec<String>,
    pub func_name: Vec<String>,
    pub duration_ns: Vec<i64>,
    pub sim_time: Vec<i64>,
    pub request_uuid: Vec<u128>,
    pub person_id: Vec<String>,
    pub mode: Vec<String>,
    pub path: std::path::PathBuf,
}

impl BufferedRoutingData {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self {
            timestamp: Vec::new(),
            target: Vec::new(),
            func_name: Vec::new(),
            duration_ns: Vec::new(),
            sim_time: Vec::new(),
            request_uuid: Vec::new(),
            person_id: Vec::new(),
            mode: Vec::new(),
            path,
        }
    }

    fn create_parent(&self) {
        let prefix = self.path.parent().unwrap();
        std::fs::create_dir_all(prefix).unwrap();
    }

    pub fn write_parquet(&self) -> Result<(), Box<dyn std::error::Error>> {
        let fields = vec![
            arrow2::datatypes::Field::new("timestamp", DataType::Utf8, false),
            arrow2::datatypes::Field::new("target", DataType::Utf8, false),
            arrow2::datatypes::Field::new("func_name", DataType::Utf8, false),
            arrow2::datatypes::Field::new("duration_ns", DataType::Int64, false),
            arrow2::datatypes::Field::new("sim_time", DataType::Int64, false),
            arrow2::datatypes::Field::new("request_uuid", DataType::Utf8, false),
            arrow2::datatypes::Field::new("person_id", DataType::Utf8, false),
            arrow2::datatypes::Field::new("mode", DataType::Utf8, false),
        ];
        let schema = Schema::from(fields);

        let columns: Vec<Box<dyn Array>> = vec![
            Box::new(convert_u128_to_fixed_size_binary(&self.timestamp)),
            Box::new(Utf8Array::<i32>::from_slice(&self.target)),
            Box::new(Utf8Array::<i32>::from_slice(&self.func_name)),
            Box::new(Int64Array::from_slice(self.duration_ns.as_slice())),
            Box::new(Int64Array::from_slice(self.sim_time.as_slice())),
            Box::new(convert_u128_to_fixed_size_binary(&self.request_uuid)),
            Box::new(Utf8Array::<i32>::from_slice(&self.person_id)),
            Box::new(Utf8Array::<i32>::from_slice(&self.mode)),
        ];

        write_parquet(&schema, columns, &self.path)?
    }
}

#[derive(Default)]
struct RoutingMetadataVisitor {
    sim_time: Option<SimTime>,
    uuid: Option<Uuid>,
    person_id: Option<PersonId>,
    mode: Option<Mode>,
}

impl Visit for RoutingMetadataVisitor {
    fn record_u64(&mut self, field: &Field, value: u64) {
        if field.name().eq("sim_time") || field.name().contains("now") {
            self.sim_time = Some(SimTime(value));
        }
    }

    fn record_u128(&mut self, field: &Field, value: u128) {
        if field.name().eq("uuid") {
            self.uuid = Some(Uuid(value));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name().eq("person_id") {
            self.person_id = Some(PersonId(value.to_string()));
        }
        if field.name().eq("mode") {
            self.mode = Some(Mode(value.to_string()));
        }
    }

    fn record_debug(&mut self, _field: &Field, _value: &dyn Debug) {}
}

impl Drop for RoutingWriterGuard {
    fn drop(&mut self) {
        match self {
            RoutingWriterGuard::Csv(writer) => {
                let mut writer = writer.lock().unwrap();
                writer.flush().expect("Problem flushing writer");
            }
            RoutingWriterGuard::Parquet(inner) => {
                let inner = inner.lock().unwrap();
                if let Err(e) = inner.write_parquet() {
                    eprintln!("Failed to write routing parquet profiling file: {}", e);
                }
            }
        }
    }
}
