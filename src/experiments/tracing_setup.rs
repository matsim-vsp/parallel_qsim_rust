use std::fs::File;
use std::io::Write;

use tracing::metadata::LevelFilter;
use tracing::span::Record;

use tracing::instrument::WithSubscriber;
use tracing::{debug, event, info, Event, Level, Metadata, Subscriber};
use tracing_appender::non_blocking::{NonBlocking, NonBlockingBuilder, WorkerGuard};
use tracing_subscriber::filter::Targets;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::layer::{Context, Filter, Layered, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

fn tracing_function() {
    let (mut non_blocking, _guard) = NonBlockingBuilder::default()
        .buffered_lines_limit(10000)
        .lossy(false)
        //.finish(std::io::stdout());
        .finish(File::create("./with-file.txt").unwrap());

    non_blocking
            .write_all("test non blocking".as_ref())
            .expect("TODO: panic message");



        //let file_appender = tracing_appender::rolling::minutely("./", "appender.txt");
        // let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
   /*      tracing_subscriber::fmt()
            .with_writer(non_blocking)
            .event_format(Formatter)
            .init();
    
    let bla = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_target(true);
        
    */
    

    info!("This is a test info");
}

fn raw_subscriber() {
    let (non_blocking, _guard) = NonBlockingBuilder::default()
        .buffered_lines_limit(10000)
        .lossy(false)
        .finish(File::create("./with-file.txt").unwrap());

    let subscr = tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .event_format(Formatter)
        .finish();

    const EVENT_TARGET: &str = "q-event";
    let event_filter = Targets::new().with_target(EVENT_TARGET, Level::TRACE);

    let layered = subscr.with(CustomLayer.with_filter(event_filter));
    layered.init();

    /*  tracing_subscriber::registry()
         //.with(CustomLayer.with_filter(event_filter))
        // .with(bla)
         .with(InfoLayer.with_filter(LevelFilter::INFO))
         .init();

    */

    // this should be logged by info layer
    info!(a_bool = true, answer = 42, message = "first example");
    // this should be logged by the custom layer
    event!(target: EVENT_TARGET, Level::TRACE, "Some message");

    // this should be ignored.
    debug!("This is a debug message");
}

pub struct NonBlockingWrapper {
    non_blocking: NonBlocking,
    guard: WorkerGuard,
}

impl NonBlockingWrapper {
    fn new() -> NonBlockingWrapper {
        let (non_blocking, guard) = NonBlockingBuilder::default()
            .buffered_lines_limit(10000)
            .lossy(false)
            .finish(File::create("./with-file.txt").unwrap());
        NonBlockingWrapper {
            non_blocking,
            guard,
        }
    }
}
impl<S> Layer<S> for NonBlockingWrapper where S: tracing::Subscriber {}

pub struct CustomLayer;

impl<S> Layer<S> for CustomLayer
where
    S: tracing::Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        println!(
            "Custom Layer tracing event: {} with level: {}",
            event.metadata().target(),
            event.metadata().level()
        );
        let mut visitor = PrintlnVisitor;
        event.record(&mut visitor);
    }
}

pub struct InfoLayer;

impl<S> Layer<S> for InfoLayer
where
    S: tracing::Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        println!(
            "Other layer tracing event: {} with level: {}",
            event.metadata().target(),
            event.metadata().level()
        );
    }
}

struct PrintlnVisitor;

impl tracing::field::Visit for PrintlnVisitor {
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        println!("  field={} value={}", field.name(), value)
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        println!("  field={} value={}", field.name(), value)
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        println!("  field={} value={}", field.name(), value)
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        println!("  field={} value={}", field.name(), value)
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        println!("  field={} value={}", field.name(), value)
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        println!("  field={} value={}", field.name(), value)
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        println!("  field={} value={:?}", field.name(), value)
    }
}

struct Formatter;

impl<S, N> FormatEvent<S, N> for Formatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> std::fmt::Result {
        for field in event.fields() {
            //ctx.field_format()
            writeln!(writer, "Test!!! {}", field)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::experiments::tracing_setup::{raw_subscriber, tracing_function};

    #[test]
    fn test() {
        tracing_function();
    }

    #[test]
    fn test_raw_subscriber() {
        raw_subscriber();
    }
}
