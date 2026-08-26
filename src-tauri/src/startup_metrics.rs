use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};

static PROCESS_STARTED_AT: OnceLock<Instant> = OnceLock::new();
static SETUP_ELAPSED: OnceLock<Duration> = OnceLock::new();
static BAR_WEBVIEW_ELAPSED: OnceLock<Duration> = OnceLock::new();

pub(crate) fn begin() {
    let _ = PROCESS_STARTED_AT.set(Instant::now());
}

pub(crate) fn mark_setup_complete() {
    mark_once("Tauri setup complete", &SETUP_ELAPSED);
}

pub(crate) fn mark_bar_webview_created() {
    mark_once("Bar WebView created", &BAR_WEBVIEW_ELAPSED);
}

fn mark_once(name: &str, milestone: &OnceLock<Duration>) {
    let Some(started_at) = PROCESS_STARTED_AT.get() else {
        return;
    };
    let elapsed = started_at.elapsed();
    if milestone.set(elapsed).is_ok() {
        log::info!(
            "[startup] {name}: +{:.1} ms",
            elapsed.as_secs_f64() * 1_000.0
        );
    }
}
