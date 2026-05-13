// OpenWarp: telemetry sending has been physically removed. These macros remain as
// compatibility shims for call sites that still describe local UI/business events. They
// intentionally do not evaluate the event expression, so telemetry-only payload
// construction has no runtime cost and cannot keep cloud/reporting dependencies alive.
// Keep the context/executor operands referenced to avoid churn in callers while the
// remaining event-type shell is removed incrementally.

#[macro_export]
macro_rules! send_telemetry_from_ctx {
    ($event:expr, $ctx:expr) => {{
        let _ = stringify!($event);
        let _ = &$ctx;
    }};
}

#[macro_export]
macro_rules! send_telemetry_from_app_ctx {
    ($event:expr, $app_ctx:expr) => {{
        let _ = stringify!($event);
        let _ = &$app_ctx;
    }};
}

#[macro_export]
macro_rules! send_telemetry_sync_from_ctx {
    ($event:expr, $ctx:expr) => {{
        let _ = stringify!($event);
        let _ = &$ctx;
    }};
}

#[macro_export]
macro_rules! send_telemetry_sync_from_app_ctx {
    ($event:expr, $app_ctx:expr) => {{
        let _ = stringify!($event);
        let _ = &$app_ctx;
    }};
}

#[macro_export]
macro_rules! send_telemetry_on_executor {
    ($auth_state:expr, $event:expr, $executor:expr) => {{
        let _ = stringify!($event);
        let _ = &$auth_state;
        let _ = &$executor;
    }};
}
