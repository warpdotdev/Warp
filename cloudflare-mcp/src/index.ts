import { McpAgent } from "agents/mcp";
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";

interface Env {
  CLOUDFLARE_API_TOKEN: string;
  CLOUDFLARE_ACCOUNT_ID: string;
  MCP_OBJECT: DurableObjectNamespace;
}

const CF_BASE = "https://api.cloudflare.com/client/v4";

async function cfFetch(
  path: string,
  token: string,
  init: RequestInit = {},
): Promise<unknown> {
  const res = await fetch(`${CF_BASE}${path}`, {
    ...init,
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
      ...(init.headers as Record<string, string> | undefined),
    },
  });
  const json = (await res.json()) as {
    success: boolean;
    errors?: Array<{ message: string }>;
    result: unknown;
  };
  if (!res.ok || !json.success) {
    const msg =
      json.errors?.map((e) => e.message).join("; ") ?? `HTTP ${res.status}`;
    throw new Error(`Cloudflare API error: ${msg}`);
  }
  return json.result;
}

function ok(data: unknown) {
  return {
    content: [{ type: "text" as const, text: JSON.stringify(data, null, 2) }],
  };
}

function fail(e: unknown) {
  const msg = e instanceof Error ? e.message : String(e);
  return { content: [{ type: "text" as const, text: `Error: ${msg}` }], isError: true };
}

export class CloudflareMCP extends McpAgent<Env> {
  server = new McpServer({ name: "cloudflare-mcp", version: "1.0.0" });

  async init() {
    const acct = () => this.env.CLOUDFLARE_ACCOUNT_ID;
    const cf = (path: string, init?: RequestInit) =>
      cfFetch(path, this.env.CLOUDFLARE_API_TOKEN, init);

    // ── Workers ────────────────────────────────────────────────────────

    this.server.tool(
      "workers_list",
      "List all Workers scripts in the Cloudflare account with metadata (id, handlers, modified_on, usage_model).",
      {},
      async () => {
        try {
          return ok(await cf(`/accounts/${acct()}/workers/scripts`));
        } catch (e) {
          return fail(e);
        }
      },
    );

    this.server.tool(
      "workers_deployments",
      "Get deployment history for a Workers script.",
      { script_name: z.string().describe("Worker script name (id)") },
      async ({ script_name }) => {
        try {
          return ok(
            await cf(
              `/accounts/${acct()}/workers/scripts/${script_name}/deployments`,
            ),
          );
        } catch (e) {
          return fail(e);
        }
      },
    );

    // ── D1 ─────────────────────────────────────────────────────────────

    this.server.tool(
      "d1_list_databases",
      "List all D1 databases in the account.",
      {},
      async () => {
        try {
          return ok(await cf(`/accounts/${acct()}/d1/database`));
        } catch (e) {
          return fail(e);
        }
      },
    );

    this.server.tool(
      "d1_get_database",
      "Get details (name, version, file_size, num_tables) for a D1 database.",
      { database_id: z.string().describe("D1 database ID (UUID)") },
      async ({ database_id }) => {
        try {
          return ok(await cf(`/accounts/${acct()}/d1/database/${database_id}`));
        } catch (e) {
          return fail(e);
        }
      },
    );

    this.server.tool(
      "d1_query",
      "Execute a SQL query against a D1 database. Supports SELECT and DML. Use parameterized queries to avoid injection.",
      {
        database_id: z.string().describe("D1 database ID (UUID)"),
        sql: z.string().describe("SQL statement to execute"),
        params: z
          .array(z.union([z.string(), z.number(), z.boolean(), z.null()]))
          .optional()
          .describe("Positional parameters for the prepared statement"),
      },
      async ({ database_id, sql, params }) => {
        try {
          return ok(
            await cf(`/accounts/${acct()}/d1/database/${database_id}/query`, {
              method: "POST",
              body: JSON.stringify({ sql, params: params ?? [] }),
            }),
          );
        } catch (e) {
          return fail(e);
        }
      },
    );

    // ── R2 ─────────────────────────────────────────────────────────────

    this.server.tool(
      "r2_list_buckets",
      "List all R2 buckets in the account.",
      {},
      async () => {
        try {
          return ok(await cf(`/accounts/${acct()}/r2/buckets`));
        } catch (e) {
          return fail(e);
        }
      },
    );

    this.server.tool(
      "r2_get_bucket",
      "Get details for an R2 bucket (creation date, location, storage class).",
      { bucket_name: z.string().describe("R2 bucket name") },
      async ({ bucket_name }) => {
        try {
          return ok(
            await cf(
              `/accounts/${acct()}/r2/buckets/${encodeURIComponent(bucket_name)}`,
            ),
          );
        } catch (e) {
          return fail(e);
        }
      },
    );

    // ── KV ─────────────────────────────────────────────────────────────

    this.server.tool(
      "kv_list_namespaces",
      "List all Workers KV namespaces in the account.",
      {},
      async () => {
        try {
          return ok(await cf(`/accounts/${acct()}/storage/kv/namespaces`));
        } catch (e) {
          return fail(e);
        }
      },
    );

    this.server.tool(
      "kv_list_keys",
      "List keys in a KV namespace with optional prefix filter and pagination.",
      {
        namespace_id: z.string().describe("KV namespace ID"),
        limit: z
          .number()
          .int()
          .min(1)
          .max(1000)
          .optional()
          .describe("Max number of keys to return (default 1000)"),
        cursor: z
          .string()
          .optional()
          .describe("Pagination cursor from a previous response"),
        prefix: z.string().optional().describe("Only return keys with this prefix"),
      },
      async ({ namespace_id, limit, cursor, prefix }) => {
        try {
          const qs = new URLSearchParams();
          if (limit != null) qs.set("limit", String(limit));
          if (cursor) qs.set("cursor", cursor);
          if (prefix) qs.set("prefix", prefix);
          const q = qs.size ? `?${qs}` : "";
          return ok(
            await cf(
              `/accounts/${acct()}/storage/kv/namespaces/${namespace_id}/keys${q}`,
            ),
          );
        } catch (e) {
          return fail(e);
        }
      },
    );

    // ── AI Gateway ─────────────────────────────────────────────────────

    this.server.tool(
      "ai_gateway_list",
      "List all AI Gateways in the account.",
      {},
      async () => {
        try {
          return ok(await cf(`/accounts/${acct()}/ai-gateway/gateways`));
        } catch (e) {
          return fail(e);
        }
      },
    );

    this.server.tool(
      "ai_gateway_get",
      "Get configuration and statistics for a specific AI Gateway.",
      { gateway_id: z.string().describe("AI Gateway ID (slug)") },
      async ({ gateway_id }) => {
        try {
          return ok(
            await cf(`/accounts/${acct()}/ai-gateway/gateways/${gateway_id}`),
          );
        } catch (e) {
          return fail(e);
        }
      },
    );

    this.server.tool(
      "ai_gateway_logs",
      "Fetch per-request analytics logs for an AI Gateway. Each log entry includes provider, model, input/output token counts, estimated cost, latency, and success status.",
      {
        gateway_id: z.string().describe("AI Gateway ID (slug)"),
        limit: z
          .number()
          .int()
          .min(1)
          .max(100)
          .optional()
          .describe("Number of log entries to return (default 25)"),
        order_by: z
          .enum(["created_at"])
          .optional()
          .describe("Field to sort by"),
        order_by_direction: z
          .enum(["asc", "desc"])
          .optional()
          .describe("Sort direction (default desc)"),
        provider: z
          .string()
          .optional()
          .describe("Filter by provider slug (e.g. anthropic, openai, workers-ai)"),
        model: z.string().optional().describe("Filter by model name"),
        success: z
          .boolean()
          .optional()
          .describe("Filter by request success status"),
      },
      async ({
        gateway_id,
        limit,
        order_by,
        order_by_direction,
        provider,
        model,
        success,
      }) => {
        try {
          const qs = new URLSearchParams();
          if (limit != null) qs.set("limit", String(limit));
          if (order_by) qs.set("order_by", order_by);
          if (order_by_direction) qs.set("order_by_direction", order_by_direction);
          if (provider) qs.set("provider", provider);
          if (model) qs.set("model", model);
          if (success != null) qs.set("success", String(success));
          const q = qs.size ? `?${qs}` : "";
          return ok(
            await cf(
              `/accounts/${acct()}/ai-gateway/gateways/${gateway_id}/logs${q}`,
            ),
          );
        } catch (e) {
          return fail(e);
        }
      },
    );
  }
}

export default {
  fetch(
    request: Request,
    env: Env,
    ctx: ExecutionContext,
  ): Promise<Response> {
    const url = new URL(request.url);
    if (url.pathname === "/mcp") {
      return CloudflareMCP.serveSSE("/mcp").fetch(request, env, ctx);
    }
    return Promise.resolve(
      new Response(
        JSON.stringify({ name: "cloudflare-mcp", version: "1.0.0", endpoint: "/mcp" }),
        { headers: { "Content-Type": "application/json" } },
      ),
    );
  },
};
