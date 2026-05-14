//! 全进程共享一个 `Mutex<SqliteConnection>` 给 SSH 管理器用。
//!
//! 现状: openWarp 的主写入连接在专门的写线程里(see `app/src/persistence/sqlite.rs`)
//! 通过 `ModelEvent` channel 异步处理。给 SSH 管理器接入那个事件总线要加 6+ enum
//! 变体 + 跨 crate 的类型暴露,代价过高。
//!
//! 替代方案:**SQLite WAL 模式天然支持多写连接**(写互斥但带 busy_timeout 重试),
//! 这里再开一个独立写连接,行为完全本地化在本 crate 里。SSH 管理器的写操作是
//! 用户驱动(创建/删除节点),频率极低,与主写线程的冲突可忽略。
//!
//! 路径由调用方在初始化时传入(`set_database_path`),避免本 crate 直接依赖 app
//! 层的 `database_file_path()`。未传路径时,`with_conn` 返回 `Err(NotInitialized)`。

use anyhow::{Result, anyhow};
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

static DB_PATH: OnceLock<PathBuf> = OnceLock::new();
static CONN: OnceLock<Mutex<SqliteConnection>> = OnceLock::new();

/// 由 app 启动时调用一次,传入 sqlite db 文件路径。重复调用会被忽略
/// (OnceLock 语义)。
pub fn set_database_path(path: PathBuf) {
    let _ = DB_PATH.set(path);
}

fn open() -> Result<SqliteConnection> {
    let path = DB_PATH
        .get()
        .ok_or_else(|| anyhow!("warp_ssh_manager::db: database path not initialized"))?;
    let url = path.to_string_lossy();
    let mut conn = SqliteConnection::establish(&url)?;
    conn.batch_execute(
        "PRAGMA foreign_keys = ON; \
         PRAGMA busy_timeout = 2000; \
         PRAGMA journal_mode = WAL;",
    )?;
    Ok(conn)
}

/// 锁内执行闭包。首次调用时 lazy 打开连接;后续调用复用。
pub fn with_conn<R>(f: impl FnOnce(&mut SqliteConnection) -> Result<R>) -> Result<R> {
    let mtx = CONN.get_or_init(|| Mutex::new(open().expect("warp_ssh_manager db open")));
    let mut guard = mtx
        .lock()
        .map_err(|_| anyhow!("warp_ssh_manager db mutex poisoned"))?;
    f(&mut guard)
}
