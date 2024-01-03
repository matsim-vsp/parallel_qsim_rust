use std::fmt::Debug;
use std::fmt::Write;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Instant, SystemTime};
use std::{env, fs};

use serde_json::{json, Value};
use tracing::field::{Field, FieldSet, Visit};
use tracing::span::Attributes;
use tracing::{field, trace, Event, Id, Subscriber};
use tracing_subscriber::fmt::{format, FmtContext, FormatEvent, FormatFields, MakeWriter};
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;

const DEFAULT_PERFORMANCE_INTERVAL: u32 = 900;

pub fn measure_duration<Out, F: FnOnce() -> Out>(
    now: Option<u32>,
    key: &str,
    metadata: Option<Value>,
    f: F,
) -> Out {
    let start = Instant::now();
    let res = f();
    let duration = start.elapsed();

    let interval = match env::var("RUST_Q_SIM_PERFORMANCE_TRACING_INTERVAL") {
        Ok(interval) => interval
            .parse::<u32>()
            .unwrap_or(DEFAULT_PERFORMANCE_INTERVAL),
        Err(_) => DEFAULT_PERFORMANCE_INTERVAL,
    };

    if now.map_or(true, |time| time % interval == 0) {
        let event = json!({
            "now": now,
            "key": key,
            "duration": duration,
            "metadata": metadata
        });

        trace!(event = event.to_string());
    }
    res
}

struct CustomLayer {
    writer: Arc<Mutex<BufWriter<File>>>,
}

impl CustomLayer {
    fn new(path: &Path) -> Self {
        let header = "timestamp,target,func_name,elapsed_time,\n".to_string();
        let file =
            File::create(path).unwrap_or_else(|_e| panic!("Unable to create file at {:?}", path));
        let mut writer = BufWriter::new(file);
        std::io::Write::write(&mut writer, header.as_bytes()).expect("Failed to write header.");
        Self {
            writer: Arc::new(Mutex::new(writer)),
        }
    }

    fn write_metadata(e: &Event) -> String {
        format!(
            "{},{},{},",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            e.metadata().target(),
            e.metadata().name()
        )
    }
}

impl<S> Layer<S> for CustomLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).expect("should exist");
        let mut extensions = span.extensions_mut();

        println!("{attrs:?}");
        extensions.insert(CustomTimings::new());
    }

    fn on_event(&self, event: &Event, _ctx: Context<'_, S>) {
        let result = Self::write_metadata(event);
        let mut visitor = CustomVisitor { result };
        event.record(&mut visitor as &mut dyn Visit);
        visitor.result.push('\n');
        println!("Writing: {}", visitor.result);

        let mut bla = self.writer.lock().unwrap();
        std::io::Write::write(&mut *bla, visitor.result.as_bytes()).expect("");
        // bla.write(visitor.result.as_bytes()).expect("TODO: panic message");
    }

    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).expect("Should exist");
        let mut extensions = span.extensions_mut();

        if let Some(timing) = extensions.get_mut::<CustomTimings>() {
            timing.last = Instant::now();
        }
    }

    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).expect("Span should be there");
        let mut extensions = span.extensions_mut();

        if let Some(timing) = extensions.get_mut::<CustomTimings>() {
            let now = Instant::now();
            timing.busy += (now - timing.last).as_nanos() as u64;
        }
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        println!("Start close");
        let span = ctx.span(&id).expect("Span should be there!");
        let extensions = span.extensions();
        let callsite = span.metadata().callsite();
        let meta = span.metadata();
        let fs = FieldSet::new(&["time", "custom close"], callsite);

        let time = extensions.get::<CustomTimings>().unwrap();
        let v = [
            (
                &fs.field("time").unwrap(),
                Some(&time.busy as &dyn field::Value),
            ),
            (
                &fs.field("custom close").unwrap(),
                Some(&"custom close message" as &dyn field::Value),
            ),
        ];
        let value_set = fs.value_set(&v);
        Event::child_of(id, meta, &value_set);

        drop(extensions);
        drop(span);
        println!("End close");
    }
}

impl Drop for CustomLayer {
    fn drop(&mut self) {
        println!("Custom layer gets dropped.");
        let mut writer = self.writer.lock().unwrap();
        std::io::Write::flush(&mut *writer).expect("TODO: panic message");
    }
}

struct CustomTimings {
    busy: u64,
    last: Instant,
}

impl CustomTimings {
    fn new() -> Self {
        Self {
            busy: 0,
            last: Instant::now(),
        }
    }
}

struct CustomVisitor {
    result: String,
}

impl Visit for CustomVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        if field.name().eq("time") {
            write!(&mut self.result, "{:?},", value).unwrap();
        }
    }
}

struct CustomWriter {
    inner: Arc<Mutex<BufWriter<File>>>,
}

struct CustomWriterGuard {
    writer_ref: Arc<Mutex<BufWriter<File>>>,
}

impl Drop for CustomWriterGuard {
    fn drop(&mut self) {
        println!("Starting drop of Custom Writer Guard");
        let mut locked = self.writer_ref.lock().unwrap();
        std::io::Write::flush(&mut *locked)
            .unwrap_or_else(|e| panic!("failed to flush writer on drop"));
    }
}

impl CustomWriter {
    fn new(path: &Path) -> (Self, CustomWriterGuard) {
        // create necessary file path and corresponding file wrapped in buffered writer
        let prefix = path.parent().unwrap();
        fs::create_dir_all(prefix).unwrap();
        let file =
            File::create(path).unwrap_or_else(|e| panic!("Failed to open file at: {path:?}"));
        let mut writer = BufWriter::new(file);

        // write header for csv file
        std::io::Write::write(
            &mut writer,
            "timestamp,target,func_name,duration\n".as_bytes(),
        )
        .unwrap_or_else(|e| panic!("Failed to write header."));

        // wire up stuff, so that we can have a guard and so that we are sync and send, because that
        // is required by the tracing_subcriber crate
        let inner = Arc::new(Mutex::new(writer));
        let result = Self {
            inner: inner.clone(),
        };
        let guard = CustomWriterGuard {
            writer_ref: result.inner.clone(),
        };
        (result, guard)
    }
}

struct MakeWriteResult<'a>(MutexGuard<'a, BufWriter<File>>);

impl<'a> MakeWriter<'a> for CustomWriter {
    type Writer = MakeWriteResult<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        let locked = self.inner.lock().unwrap();
        MakeWriteResult(locked)
    }
}

impl std::io::Write for MakeWriteResult<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

struct CustomFormatter;

struct CustomTimeVisitor<'a> {
    writer: &'a mut dyn Write,
}

impl<'a> CustomTimeVisitor<'a> {
    fn new(writer: &'a mut dyn Write) -> Self {
        Self { writer }
    }
}

impl<'a> Visit for CustomTimeVisitor<'a> {
    fn record_u64(&mut self, field: &Field, value: u64) {
        println!("Visitor record u64");
        if field.name().eq("time.busy") {
            write!(&mut self.writer, "{value}")
                .unwrap_or_else(|e| panic!("Failed to write field time."));
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        println!("Visitor record debug");
        if field.name().eq("time.busy") {
            write!(&mut self.writer, "{value:?}")
                .unwrap_or_else(|e| panic!("Failed to write field time."));
        }
    }
}

impl<S, N> FormatEvent<S, N> for CustomFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        _ctx: &FmtContext<'_, S, N>,
        mut writer: format::Writer<'_>,
        e: &Event<'_>,
    ) -> std::fmt::Result {
        write!(
            &mut writer,
            "{},{},{},",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            e.metadata().target(),
            e.metadata().name()
        )
        .unwrap_or_else(|e| panic!("OH no! Can't write."));

        let mut visitor = CustomTimeVisitor::new(&mut writer);
        e.record(&mut visitor);
        writeln!(&mut writer)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::thread::sleep;
    use std::time::Duration;

    use tracing::{info, instrument, Level};
    use tracing_subscriber::filter::FilterExt;
    use tracing_subscriber::fmt::format::FmtSpan;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::{fmt, EnvFilter, Layer};

    use crate::simulation::profiling::{CustomFormatter, CustomWriter};

    #[test]
    fn test_events() {
        let path = PathBuf::from("./test_output/simulation/profiling/test_events");
        let (writer, guard) = CustomWriter::new(&path);
        let custom_formatter = CustomFormatter {};
        let custom_filter = EnvFilter::from_default_env().add_directive(Level::TRACE.into());
        let custom_filter_2 = EnvFilter::from_default_env()
            .add_directive(Level::INFO.into())
            .not();

        // let custom_layer = CustomLayer::new(&path);
        // let writer_ref = custom_layer.writer.clone();
        let layers = tracing_subscriber::registry().with(
            fmt::Layer::new()
                .with_span_events(FmtSpan::CLOSE)
                .with_writer(writer)
                .event_format(custom_formatter)
                .with_filter(custom_filter)
                .with_filter(custom_filter_2),
        );
        tracing::subscriber::set_global_default(layers).expect("TODO: panic message");

        info!("Before func");
        some_function();
        info!("After func");

        some_other_function(42, std::f32::consts::PI);
    }

    #[instrument]
    fn some_function() {
        info!("Inside some function.")
    }

    #[instrument(level = "trace")]
    fn some_other_function(a: u32, b: f32) {
        info!("Inside some other function");
        sleep(Duration::from_nanos(10));
    }
}
