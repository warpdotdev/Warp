# PDX-76 Implementation Notes

## What was built

`cloudflare-mcp/` — a Cloudflare Worker that exposes 12 MCP tools covering five
Cloudflare product areas: Workers, D1, R2, KV, and AI Gateway analytics.

## File map

```
cloudflare-mcp/
  package.json          # npm deps (agents, @modelcontextprotocol/sdk, zod)
  tsconfig.json         # strict TS targeting Workers runtime
  wrangler.toml         # Worker name, DO binding, migration
  .dev.vars.example     # local dev secrets template
  src/
    index.ts            # all 12 MCP tools + fetch entry point
```

## Tool inventory

| Tool | Cloudflare API endpoint |
|---|---|
| `workers_list` | `GET /accounts/{acct}/workers/scripts` |
| `workers_deployments` | `GET /accounts/{acct}/workers/scripts/{name}/deployments` |
| `d1_list_databases` | `GET /accounts/{acct}/d1/database` |
| `d1_get_database` | `GET /accounts/{acct}/d1/database/{id}` |
| `d1_query` | `POST /accounts/{acct}/d1/database/{id}/query` |
| `r2_list_buckets` | `GET /accounts/{acct}/r2/buckets` |
| `r2_get_bucket` | `GET /accounts/{acct}/r2/buckets/{name}` |
| `kv_list_namespaces` | `GET /accounts/{acct}/storage/kv/namespaces` |
| `kv_list_keys` | `GET /accounts/{acct}/storage/kv/namespaces/{id}/keys` |
| `ai_gateway_list` | `GET /accounts/{acct}/ai-gateway/gateways` |
| `ai_gateway_get` | `GET /accounts/{acct}/ai-gateway/gateways/{id}` |
| `ai_gateway_logs` | `GET /accounts/{acct}/ai-gateway/gateways/{id}/logs` |

## Assumptions

**Auth model:** The MCP endpoint itself is unauthenticated (no OAuth). The
intended deployment is on an internal Workers route (not public) and agents
connect with `mcp-remote` or equivalent. If public exposure is needed, add
Cloudflare Access in front of the Worker via a zero-trust policy — no code
change required.

**`agents` package version:** Pinned to `^0.0.71` based on the Cloudflare
quickstart template circa May 2025. Bump to the latest `0.x` release if the
API has moved; the `McpAgent.serveSSE` call is the one most likely to change.

**`workers_list` vs `workers_get`:** `GET /workers/scripts/{name}` returns raw
JS/WASM content (not JSON), so a standalone get-by-name tool would break the
`cfFetch` JSON parser. The metadata agents need (etag, handlers, modified_on)
is already present in the list response. Use `workers_list` then filter client-
side, or use `workers_deployments` to drill into a specific script's history.

**D1 write access:** `d1_query` is a passthrough — it accepts any SQL. Access
is controlled solely by the API token's D1 permissions. Provide a read-only
token (D1:Read) if the agent should not be able to mutate data.

**AI Gateway aggregate analytics:** The Cloudflare AI Gateway GraphQL analytics
API (aggregate rollups by model/provider/time) was not included. It requires a
different fetch pattern (POST to `/graphql` with an account-scoped query). The
`ai_gateway_logs` tool covers per-request records including token counts, cost,
and latency. Aggregate analytics can be added as a `ai_gateway_analytics` tool
in a follow-up if GraphQL support is needed.

**R2 object listing:** The R2 object list API is S3-compatible
(`/accounts/{acct}/r2/buckets/{name}/objects`) but returns XML, not JSON.
Excluded to keep the `cfFetch` helper uniform. A dedicated R2 object-listing
tool would need its own XML parser or must use the S3 SDK.

## Integration steps

1. Copy `cloudflare-mcp/` into the Helm repo at the same path (repo root).
2. `cd cloudflare-mcp && npm install`
3. Set secrets: `wrangler secret put CLOUDFLARE_API_TOKEN` and
   `wrangler secret put CLOUDFLARE_ACCOUNT_ID`
4. `wrangler deploy`
5. Add to `.mcp.json` (the starter template from PDX-74):

```json
"cloudflare-custom": {
  "command": "npx",
  "args": ["mcp-remote", "https://helm-cloudflare-mcp.<account>.workers.dev/mcp"]
}
```
