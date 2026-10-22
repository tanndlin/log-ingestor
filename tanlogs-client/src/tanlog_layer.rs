use std::{fmt, net::ToSocketAddrs, sync::Mutex};

use anyhow::Result;
use tracing::{
    Event, Subscriber,
    field::{Field, Visit},
    span,
};
use tracing_subscriber::{
    layer::{Context, Layer},
    registry::LookupSpan,
};

use crate::TanLogsClient;

pub struct TanLogsLayer {
    client: Mutex<TanLogsClient>,
}

impl TanLogsLayer {
    pub fn new(addr: impl ToSocketAddrs, application_name: impl Into<String>) -> Result<Self> {
        let client = Mutex::new(TanLogsClient::connect(addr, application_name)?);
        Ok(Self { client })
    }
}

#[derive(Default)]
struct MessageVisitor {
    message: String,
    fields: String,
}

struct TanSpanId(i64);

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        use fmt::Write;
        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
        } else {
            let _ = write!(self.fields, " {}={value:?}", field.name());
        }
    }
}

impl<S> Layer<S> for TanLogsLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(&self, _attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).expect("span must exist in registry");
        let span_id = rand::random::<i64>();
        span.extensions_mut().insert(TanSpanId(span_id));
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let span_id = ctx
            .event_span(event)
            .and_then(|s| s.extensions().get::<TanSpanId>().map(|id| id.0));

        let level = match *event.metadata().level() {
            tracing::Level::ERROR => util::Level::Error,
            tracing::Level::WARN => util::Level::Warn,
            tracing::Level::INFO => util::Level::Info,
            tracing::Level::DEBUG => util::Level::Debug,
            tracing::Level::TRACE => util::Level::Trace,
        };

        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        let message = visitor.message + &visitor.fields;

        // A layer can't return errors, so report them and move on.
        let Ok(mut client) = self.client.lock() else {
            return;
        };

        if let Err(e) = client.send(level, message, span_id) {
            eprintln!("tanlogs: failed to send trace: {e:#}");
        }
    }
}
