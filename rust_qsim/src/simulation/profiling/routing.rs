use crate::simulation::profiling::{
    end_timing, start_timing, ModeWrapper, PersonIdWrapper, SimTimeWrapper, SpanDuration,
    UuidWrapper,
};
use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tracing::field::{Field, Visit};
use tracing::span::Attributes;
use tracing::{Id, Level, Metadata};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

pub struct RoutingSpanDurationToCSVLayer<W: std::io::Write> {
    writer: Arc<Mutex<csv::Writer<W>>>,
    /// Note: TRACE > DEBUG > INFO > WARN > ERROR
    min_level: Level,
    target: String,
}

/// WriterGuard is used to ensure that the writer is flushed at the end.
/// Not 100% sure if this is really needed as the csv::Writer already implements Drop trait. Paul, nov '25.
pub struct WriterGuard<W: std::io::Write> {
    writer: Arc<Mutex<csv::Writer<W>>>,
}

impl<W: std::io::Write> Drop for WriterGuard<W> {
    fn drop(&mut self) {
        let mut writer = self.writer.lock().unwrap();
        writer.flush().unwrap();
    }
}

impl<W: std::io::Write> RoutingSpanDurationToCSVLayer<W> {
    #[rustfmt::skip]
    pub fn new(write: W, level: Level, target: &str) -> (Self, WriterGuard<W>) {
        let mut raw_writer = csv::Writer::from_writer(write);

        raw_writer
            .write_record(["timestamp", "target", "func_name", "duration", "sim_time",
            "request_uuid", "person_id", "mode"])
            .unwrap();
        let writer = Arc::new(Mutex::new(raw_writer));

        let s = Self {
            writer: writer.clone(),
            min_level:level,
            target: target.to_string(),
        };

        (s, WriterGuard { writer })
    }
}

impl<S, W> Layer<S> for RoutingSpanDurationToCSVLayer<W>
where
    // if not LookupSpan, cannot access span data like `span.extensions_mut()`
    S: tracing::Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
    W: std::io::Write + 'static,
{
    // enable spans that match target and level
    fn enabled(&self, metadata: &Metadata<'_>, _ctx: Context<'_, S>) -> bool {
        let span = metadata.is_span();
        let target = metadata
            .module_path()
            .map(|m| m.starts_with(self.target.as_str()))
            .unwrap_or(false);
        let level = metadata.level() >= &self.min_level;

        span && target && level
    }

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

    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        start_timing(id, ctx);
    }

    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        end_timing(id, ctx)
    }

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

#[derive(Default)]
struct RoutingMetadataVisitor {
    sim_time: Option<SimTimeWrapper>,
    uuid: Option<UuidWrapper>,
    person_id: Option<PersonIdWrapper>,
    mode: Option<ModeWrapper>,
}

impl Visit for RoutingMetadataVisitor {
    fn record_u64(&mut self, field: &Field, value: u64) {
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
    // needed for the `with` function on Registry
    use tracing_subscriber::layer::SubscriberExt;
    use uuid::{NoContext, Timestamp, Uuid};

    #[test]
    fn test_creation() {
        let (_, guard) = RoutingSpanDurationToCSVLayer::new(
            std::io::stdout(),
            tracing::Level::INFO,
            "rust_qsim",
        );
        drop(guard);
    }

    #[test]
    fn test_events() {
        let (layer, guard) = RoutingSpanDurationToCSVLayer::new(
            std::io::stdout(),
            tracing::Level::INFO,
            "rust_qsim::simulation::profiling::routing::tests",
        );

        let layered = tracing_subscriber::registry().with(layer);
        tracing::subscriber::set_global_default(layered).unwrap();

        let ts = Timestamp::from_unix(NoContext, 1, 1);
        let uuid = Uuid::new_v7(ts);

        foo::foo(42, uuid.as_u128(), "person1", "car");

        drop(guard)
    }

    pub(crate) mod foo {
        use tracing::{info, instrument};

        #[instrument(level = "trace")]
        pub(crate) fn foo(sim_time: u64, uuid: u128, person_id: &str, mode: &str) {
            info!("some_function");
            bar::bar(sim_time + 1, "person2", "bike");
        }

        pub(crate) mod bar {
            use crate::extend_span;
            use tracing::{instrument, trace};
            use uuid::{NoContext, Timestamp, Uuid};

            #[instrument(level = "info")]
            pub(crate) fn bar(now: u64, person_id: &str, mode: &str) {
                trace!("some_function");
                let ts = Timestamp::from_unix(NoContext, 2, 2);
                let new_uuid = Uuid::new_v7(ts);
                extend_span!(uuid = new_uuid.as_u128());
            }
        }
    }
}
