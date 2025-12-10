use crate::simulation::profiling::{
    create_file, end_timing, extract_entries, start_timing, Mode, PersonId, SimTime, SpanDuration,
    Uuid, WriterGuard,
};
use std::fmt::Debug;
use std::fs::File;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::field::{Field, Visit};
use tracing::span::Attributes;
use tracing::{Event, Id};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

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

pub struct RoutingSpanDurationToCSVLayer {
    writer: Arc<Mutex<csv::Writer<File>>>,
}

impl RoutingSpanDurationToCSVLayer {
    pub fn new(path: &Path) -> (Self, WriterGuard) {
        let file = create_file(path);
        let mut raw_writer = csv::Writer::from_writer(file);

        raw_writer.write_record(HEADER).unwrap();
        let writer = Arc::new(Mutex::new(raw_writer));

        let s = Self {
            writer: writer.clone(),
        };

        (s, WriterGuard { writer })
    }
}

impl<S> Layer<S> for RoutingSpanDurationToCSVLayer
where
    // if not LookupSpan, cannot access span data like `span.extensions_mut()`
    S: tracing::Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
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

    /// This function registers events from the same module as the current span and sets the uuid & mode
    /// of the span. This should only be used if the field is not initialized yet when the span is created.
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        // We might have tracing events from other modules, which are not part of a span.
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
                assert!(v.is_none(),"Uuid is already present in span. This can occur, if the current event is not registered \
                by the span you think it is. Check module and level span and event! Also check your layer attributes, \
                as these are used to filter events and spans. Event: {:?}", event);
            }

            if let Some(mode) = visitor.mode {
                let v = exts.replace(mode);
                assert!(v.is_none(),"Mode is already present in span. This can occur, if the current event is not registered \
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

        let (timestep, target, func_name, duration, sim_time) = extract_entries(&extensions, meta);
        let request_uuid = extensions
            .get::<Uuid>()
            .map_or("-1".to_string(), |uuid| uuid.0.to_string());
        let person_id = extensions
            .get::<PersonId>()
            .map_or("", |person_id| person_id.0.as_str());
        let mode = extensions.get::<Mode>().map_or("", |mode| mode.0.as_str());

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
    sim_time: Option<SimTime>,
    uuid: Option<Uuid>,
    person_id: Option<PersonId>,
    mode: Option<Mode>,
}

impl Visit for RoutingMetadataVisitor {
    fn record_u64(&mut self, field: &Field, value: u64) {
        // be gentle here: try sim_time and any field that contains "now", i.e. "_now".
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

    fn record_debug(&mut self, _field: &Field, _value: &dyn Debug) {
        // nothing to do here
    }
}

// tests are integration tests as they require exclusive execution
