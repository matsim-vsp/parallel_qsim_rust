use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};
use std::{env, fs};

use serde_json::{json, Value};
use tracing::span::Attributes;
use tracing::{trace, Id, Subscriber};
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

pub struct SpanDurationToCSVLayer {
    writer: Arc<Mutex<BufWriter<File>>>,
}

pub struct WriterGuard {
    writer_ref: Arc<Mutex<BufWriter<File>>>,
}

struct SpanDuration {
    elapsed: u64,
    last: Instant,
}

impl SpanDurationToCSVLayer {
    pub fn new(path: &Path) -> (Self, WriterGuard) {
        // create necessary file path and corresponding file wrapped in buffered writer
        let prefix = path.parent().unwrap();
        fs::create_dir_all(prefix).unwrap();
        let file =
            File::create(path).unwrap_or_else(|_e| panic!("Failed to open file at: {path:?}"));
        let mut writer = BufWriter::new(file);

        // write header for csv file
        std::io::Write::write(
            &mut writer,
            "timestamp,target,func_name,duration\n".as_bytes(),
        )
        .unwrap_or_else(|_e| panic!("Failed to write header."));

        // wrap the writer into an arc<mutex<...>> so that we can keep a reference which gets dropped
        // at the end of the scope calling this method. The mutex is necessary, because the Layer
        // must be Sync + Send for the tracing_subscriber subscriber
        let writer_ref = Arc::new(Mutex::new(writer));
        let new_self = Self {
            writer: writer_ref.clone(),
        };
        let guard = WriterGuard { writer_ref };
        (new_self, guard)
    }

    fn write_metadata(writer: &mut BufWriter<File>, m: &tracing::Metadata) {
        // import Write here, to avoid conflicts with std::fmt::Write
        use std::io::Write;

        write!(
            writer,
            "{},{},{},",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            m.target(),
            m.name(),
        )
        .unwrap();
    }
}

/// Simple Layer implementation, which records the time elapsed between a a span being opened and being
/// closed again. Once a span is closed, it writes the elapsed time into a csv journal
impl<S> Layer<S> for SpanDurationToCSVLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, _attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).expect("should exist");
        let mut extensions = span.extensions_mut();
        extensions.insert(SpanDuration::new());
    }

    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).expect("Should exist");
        let mut extensions = span.extensions_mut();

        if let Some(timing) = extensions.get_mut::<SpanDuration>() {
            timing.last = Instant::now();
        }
    }

    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).expect("Span should be there");
        let mut extensions = span.extensions_mut();

        if let Some(timing) = extensions.get_mut::<SpanDuration>() {
            let now = Instant::now();
            timing.elapsed += (now - timing.last).as_nanos() as u64;
        }
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        use std::io::Write;

        let span = ctx.span(&id).expect("Span should be there!");
        let extensions = span.extensions();
        let meta = span.metadata();

        let writer = &mut *self.writer.lock().unwrap();
        Self::write_metadata(writer, meta);

        let span_duration = extensions.get::<SpanDuration>().unwrap();
        write!(writer, "{},", span_duration.elapsed).unwrap();
        writeln!(writer).unwrap();

        // extensions and span must be dropped explicitly, says the tracing documentation
        drop(extensions);
        drop(span);
    }
}

impl Drop for WriterGuard {
    fn drop(&mut self) {
        println!("Writer guard gets dropped.");
        let mut writer = self.writer_ref.lock().unwrap();
        std::io::Write::flush(&mut *writer).expect("TODO: panic message");
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

    use tracing::{info, instrument};
    use tracing_subscriber::fmt::Layer;
    use tracing_subscriber::layer::SubscriberExt;

    use crate::simulation::profiling::SpanDurationToCSVLayer;

    #[test]
    fn test_events() {
        let path = PathBuf::from("./test_output/simulation/profiling/test_events.csv");

        let (csv_layer, _guard) = SpanDurationToCSVLayer::new(&path);
        let layers = tracing_subscriber::registry()
            .with(csv_layer)
            .with(Layer::new().pretty());
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
