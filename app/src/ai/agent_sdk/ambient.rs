//! OpenWarp 本地构建中保留 `warp task` 命令分发表面,但云端 run API 已下线。

use warp_cli::{
    task::{ListTasksArgs, MessageCommand, TaskGetArgs},
    GlobalOptions,
};
use warpui::AppContext;

pub fn list_ambient_agent_tasks(
    _ctx: &mut AppContext,
    _global_options: GlobalOptions,
    _args: ListTasksArgs,
) -> anyhow::Result<()> {
    Err(anyhow::anyhow!(
        "Cloud agent run listing is disabled in OpenWarp"
    ))
}

pub fn get_ambient_agent_task_status(
    _ctx: &mut AppContext,
    _global_options: GlobalOptions,
    _args: TaskGetArgs,
) -> anyhow::Result<()> {
    Err(anyhow::anyhow!(
        "Cloud agent run lookup is disabled in OpenWarp"
    ))
}

pub fn run_message(
    _ctx: &mut AppContext,
    _global_options: GlobalOptions,
    _command: MessageCommand,
) -> anyhow::Result<()> {
    Err(anyhow::anyhow!(
        "Cloud agent messaging is disabled in OpenWarp"
    ))
}
