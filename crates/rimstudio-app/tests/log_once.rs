//! A failed command logs exactly one error line, with its error id. This is one test in its own
//! binary: scoped subscribers and the process wide cache of call site interest do not mix with other
//! test threads that hit the same call sites without a subscriber.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use common::fixture;
use rimstudio_app::dispatch;
use serde_json::json;

#[test]
fn a_failed_command_logs_exactly_one_error_line_with_its_error_id() {
    use std::sync::{Arc, Mutex};

    use tracing::field::{Field, Visit};
    use tracing_subscriber::layer::{Context, Layer, SubscriberExt};

    #[derive(Default)]
    struct Seen(Mutex<Vec<String>>);

    struct Grab(Option<String>);
    impl Visit for Grab {
        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            if field.name() == "error_id" {
                self.0 = Some(format!("{value:?}").trim_matches('"').to_owned());
            }
        }
        fn record_str(&mut self, field: &Field, value: &str) {
            if field.name() == "error_id" {
                self.0 = Some(value.to_owned());
            }
        }
    }

    struct Count(Arc<Seen>);
    impl<S: tracing::Subscriber> Layer<S> for Count {
        fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
            if *event.metadata().level() == tracing::Level::ERROR {
                let mut grab = Grab(None);
                event.record(&mut grab);
                let mut lines = self.0.0.lock().unwrap();
                lines.push(grab.0.unwrap_or_default());
            }
        }
    }

    let f = fixture(false);
    let seen = Arc::new(Seen::default());
    let subscriber = tracing_subscriber::registry().with(Count(Arc::clone(&seen)));
    let error = tracing::subscriber::with_default(subscriber, || {
        dispatch(
            &f.app,
            "settings_update",
            json!({"reset": ["nonsense.key"]}),
        )
        .unwrap_err()
    });
    let lines = seen.0.lock().unwrap().clone();
    assert_eq!(
        lines,
        vec![error.error_id.clone()],
        "one error line, carrying the id"
    );
}
