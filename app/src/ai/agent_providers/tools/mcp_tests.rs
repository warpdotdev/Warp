//! `mcp.rs` 单元测试。
//!
//! 覆盖 P0-3 prompt cache 优化:`build_mcp_tool_defs` 必须**字典序稳定**,
//! 跨请求同一 `MCPContext` 调多次产出 byte-equal 的 tools 列表,否则
//! Anthropic 会判定 tools 字段改动 → 全部缓存层失效。
//!
//! 注:`rmcp::model::Tool` 与 `rmcp::model::Resource`(= `Annotated<RawResource>`)
//! 来自上游 vendor crate,这里只用其公开构造路径(`Tool::new` / `RawResource::new`)。

use rmcp::model::{AnnotateAble, RawResource, Tool};
use serde_json::json;
use std::sync::Arc;

use crate::ai::agent::{MCPContext, MCPServer};

use super::{build_mcp_tool_defs, function_name};

/// 构造一个 `rmcp::model::Tool`,带最小输入 schema。
fn mk_tool(name: &'static str, desc: &'static str) -> Tool {
    let schema: serde_json::Map<String, serde_json::Value> = json!({
        "type": "object",
        "properties": {
            "x": { "type": "string" }
        }
    })
    .as_object()
    .unwrap()
    .clone();
    // `Tool::new` 接受 Arc<JsonObject>,这里直接传 Map(实现了 Into<Arc<JsonObject>>)。
    Tool::new(name, desc, Arc::new(schema))
}

/// 构造 MCPServer。tools 顺序与 resources 顺序按入参原样保留(模拟上游
/// 在 HashMap iterate 顺序下可能传入的乱序输入)。
fn mk_server(
    id: &str,
    name: &str,
    tools: Vec<Tool>,
    resources: Vec<rmcp::model::Resource>,
) -> MCPServer {
    MCPServer {
        id: id.to_owned(),
        name: name.to_owned(),
        description: String::new(),
        resources,
        tools,
    }
}

fn mk_resource(uri: &str, name: &str) -> rmcp::model::Resource {
    // RawResource → Annotated<RawResource>(不带 annotation)。
    // 上游提供的安全转换入口是 `AnnotateAble::no_annotation`。
    RawResource::new(uri, name).no_annotation()
}

/// 同一 ctx,build 两次,产出 (name, description, schema) 三元组必须 byte-equal。
/// 这是 prompt cache 命中的最低门槛 —— 只要不稳定,Anthropic 缓存全部失效。
#[test]
fn build_mcp_tool_defs_is_stable_across_calls() {
    let ctx = MCPContext {
        #[allow(deprecated)]
        resources: vec![],
        #[allow(deprecated)]
        tools: vec![],
        servers: vec![
            mk_server(
                "id-b",
                "server-b",
                vec![mk_tool("zeta", "z"), mk_tool("alpha", "a")],
                vec![],
            ),
            mk_server(
                "id-a",
                "server-a",
                vec![mk_tool("beta", "b"), mk_tool("gamma", "g")],
                vec![],
            ),
        ],
    };
    let r1 = build_mcp_tool_defs(&ctx);
    let r2 = build_mcp_tool_defs(&ctx);
    assert_eq!(r1, r2, "build_mcp_tool_defs 必须确定性产出");
}

/// 输入服务器 / 工具乱序时,输出按 function_name 字典序排序。
/// 这是 P0-3 的核心断言:跨请求若上游 ctx.servers 顺序不同(HashMap iterate
/// 等导致),输出仍 byte-equal。
#[test]
fn build_mcp_tool_defs_outputs_lexicographic_order() {
    let ctx = MCPContext {
        #[allow(deprecated)]
        resources: vec![],
        #[allow(deprecated)]
        tools: vec![],
        servers: vec![
            mk_server(
                "id-b",
                "server-b",
                // 乱序: zeta 在 alpha 前
                vec![mk_tool("zeta", "z"), mk_tool("alpha", "a")],
                vec![],
            ),
            mk_server(
                "id-a",
                "server-a",
                vec![mk_tool("beta", "b"), mk_tool("gamma", "g")],
                vec![],
            ),
        ],
    };
    let out = build_mcp_tool_defs(&ctx);
    let names: Vec<&str> = out.iter().map(|(n, _, _)| n.as_str()).collect();
    // 按 function_name 排序后:server-a/beta < server-a/gamma < server-b/alpha < server-b/zeta
    let expected = [
        function_name(&mk_server("id-a", "server-a", vec![], vec![]), "beta"),
        function_name(&mk_server("id-a", "server-a", vec![], vec![]), "gamma"),
        function_name(&mk_server("id-b", "server-b", vec![], vec![]), "alpha"),
        function_name(&mk_server("id-b", "server-b", vec![], vec![]), "zeta"),
    ];
    assert_eq!(
        names,
        expected.iter().map(|s| s.as_str()).collect::<Vec<_>>()
    );
}

/// 跨请求入参 servers 顺序不同(模拟 HashMap 重排)产出依然 byte-equal。
#[test]
fn build_mcp_tool_defs_invariant_under_servers_permutation() {
    let server_a = mk_server(
        "id-a",
        "server-a",
        vec![mk_tool("beta", "b"), mk_tool("gamma", "g")],
        vec![],
    );
    let server_b = mk_server(
        "id-b",
        "server-b",
        vec![mk_tool("zeta", "z"), mk_tool("alpha", "a")],
        vec![],
    );
    let ctx1 = MCPContext {
        #[allow(deprecated)]
        resources: vec![],
        #[allow(deprecated)]
        tools: vec![],
        servers: vec![server_a.clone(), server_b.clone()],
    };
    let ctx2 = MCPContext {
        #[allow(deprecated)]
        resources: vec![],
        #[allow(deprecated)]
        tools: vec![],
        servers: vec![server_b, server_a],
    };
    assert_eq!(build_mcp_tool_defs(&ctx1), build_mcp_tool_defs(&ctx2));
}

/// 当任意 server 暴露 resources 时,read_resource 描述里 available_uris
/// 也必须按字典序稳定,且 read_resource 永远在数组最末。
#[test]
fn read_resource_description_is_stable_and_sorted() {
    let ctx1 = MCPContext {
        #[allow(deprecated)]
        resources: vec![],
        #[allow(deprecated)]
        tools: vec![],
        servers: vec![mk_server(
            "id-a",
            "srv",
            vec![mk_tool("t", "")],
            vec![
                mk_resource("file:///z.txt", "Z"),
                mk_resource("file:///a.txt", "A"),
            ],
        )],
    };
    // 同 ctx 但 resources 顺序换一下
    let ctx2 = MCPContext {
        #[allow(deprecated)]
        resources: vec![],
        #[allow(deprecated)]
        tools: vec![],
        servers: vec![mk_server(
            "id-a",
            "srv",
            vec![mk_tool("t", "")],
            vec![
                mk_resource("file:///a.txt", "A"),
                mk_resource("file:///z.txt", "Z"),
            ],
        )],
    };
    let r1 = build_mcp_tool_defs(&ctx1);
    let r2 = build_mcp_tool_defs(&ctx2);
    assert_eq!(r1, r2, "read_resource 描述必须 byte-equal");

    let last = r1.last().expect("应至少含 read_resource");
    assert_eq!(last.0, "mcp_read_resource");
    // 排序后 a.txt 在 z.txt 前
    let pos_a = last.1.find("a.txt").expect("应含 a.txt");
    let pos_z = last.1.find("z.txt").expect("应含 z.txt");
    assert!(pos_a < pos_z, "available_uris 必须按字典序排");
}
