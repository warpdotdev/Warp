// OpenWarp(本地化,Phase 5):原 `send_telemetry_from_ctx` / `send_telemetry_from_app_ctx`
// 会将事件写入本地 telemetry 队列等待上报。OpenWarp 不需要外发 telemetry,两个宏改
// 为 no-op,仅消费输入避免 unused_variables warning。原调用点(数百处)无需修改,后续
// 可考虑物理删除。

#[macro_export]
macro_rules! send_telemetry_from_ctx {
    ($event:expr, $ctx:expr) => {{
        let _ = &$event;
        let _ = &$ctx;
    }};
}

#[macro_export]
macro_rules! send_telemetry_from_app_ctx {
    ($event:expr, $app_ctx:expr) => {{
        let _ = &$event;
        let _ = &$app_ctx;
    }};
}

#[macro_export]
macro_rules! send_telemetry_sync_from_ctx {
    ($event:expr, $ctx:expr) => {{
        let _ = &$event;
        let _ = &$ctx;
    }};
}

#[macro_export]
macro_rules! send_telemetry_sync_from_app_ctx {
    ($event:expr, $app_ctx:expr) => {{
        let _ = &$event;
        let _ = &$app_ctx;
    }};
}

#[macro_export]
macro_rules! send_telemetry_on_executor {
    ($auth_state:expr, $event:expr, $executor:expr) => {{
        let _ = &$event;
        let _ = &$auth_state;
        let _ = &$executor;
    }};
}
