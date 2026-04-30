# APP-3792: Remote Codebase Indexing — TECH.md

Linear: [APP-3792](https://linear.app/warpdotdev/issue/APP-3792)

Behavior is specified in `specs/APP-3792/PRODUCT.md`. This document updates the branch spec against current `origin/master` and the latest design notes: the daemon owns embedding/sync/cache work using its authenticated token, while the client owns UI state and direct retrieval calls using the daemon-supplied root hash.

## 1. Context
### Current local indexing architecture on master
- `app/src/lib.rs:1825` registers the local `CodebaseIndexManager` singleton.
- `crates/ai/src/index/full_source_code_embedding/manager.rs:167` defines `CodebaseIndexManager`; `manager.rs:186` constructs it from persisted metadata, limits, a `StoreClient`, and a `BulkFilesystemWatcher`.
- `manager.rs:452` handles watcher events, `manager.rs:564` starts indexing a directory, and `manager.rs:850` retrieves relevant files.
- `crates/ai/src/index/full_source_code_embedding/codebase_index.rs:147` defines `CodebaseIndex`, the per-repo owner of the Merkle tree, sync state, snapshot, and retrieval state.
- `crates/ai/src/index/full_source_code_embedding/store_client.rs:15` defines the authenticated backend seam. Its methods are `update_intermediate_nodes`, `generate_embeddings`, `populate_merkle_tree_cache`, `sync_merkle_tree`, `rerank_fragments`, `get_relevant_fragments`, and `codebase_context_config` (`store_client.rs:17-62`).
- `app/src/server/server_api/ai.rs` implements that trait for the client-side `ServerApi`; current master includes the codebase calls around `generate_code_embeddings`, `sync_merkle_tree`, `populate_merkle_tree_cache`, `get_relevant_fragments`, `rerank_fragments`, and `codebase_context_config`.
- `crates/ai/src/index/full_source_code_embedding/snapshot.rs` owns serialized snapshot persistence. The daemon path should reuse the format while changing the base directory.
- `app/src/ai/blocklist/action_model/execute/search_codebase.rs:28` defines `SearchCodebaseExecutor`; the current hydration path uses local file reads after `GetRelevantFilesController`.
- `app/src/ai/agent/api/impl.rs:189-194` explicitly disables `SearchCodebase` for `WarpifiedRemote { host_id: Some(_) }`.
- The existing local UI strings and flows live in `app/src/ai/blocklist/codebase_index_speedbump_banner.rs:20-30` and `app/src/settings_view/code_page.rs:84-98`.

### Current remote-server architecture on master
- `crates/remote_server/proto/remote_server.proto` defines the client/server envelopes. Current messages include `Initialize`, `NavigatedToDirectory`, `ReadFileContext`, and `Authenticate`.
- `app/src/remote_server/server_model.rs:173` stores the daemon-wide `auth_token`; `server_model.rs:514` writes it from `Initialize`, `server_model.rs:532` writes it from `Authenticate`, and `server_model.rs:540` exposes `auth_token()`.
- `app/src/remote_server/server_model.rs:379` dispatches incoming remote-server messages. `server_model.rs:696` handles `NavigatedToDirectory`; `server_model.rs:995` handles `ReadFileContext`.
- `crates/remote_server/src/manager.rs` owns connection setup, initialize, and token rotation from the client side.

### Dependency assumptions
- APP-3801's per-user authenticated daemon model is assumed to land as designed in `specs/APP-3801`: the client sends the current bearer token on `Initialize`, refreshes with `Authenticate`, the daemon stores the credential in memory only, and daemon sockets are partitioned by Warp identity. Remote codebase indexing is the first feature that materially depends on daemon-side upstream calls.
- APP-3790's remote file read path is assumed available for hydrating full file context after retrieval.
- The v1 design assumes daemon-to-`app.warp.dev` egress is available. That was checked with the initial target enterprise environments. If this assumption fails later, the fallback is a client-proxied `StoreClient`, not part of v1.

Daemon responsibilities:
- Check its persisted cache when asked about a repo.
- Build the Merkle tree and fragment metadata from the remote filesystem.
- Read remote file bytes for chunking and fragment hydration.
- Run full and incremental sync with the backend through a daemon-side `StoreClient` authenticated by the APP-3801 token.
- Fetch and respect server-backed codebase-indexing config such as embedding config, batch sizes, and sync cadence.
- Persist snapshots and lightweight metadata on the remote host in an identity-scoped cache.
- Watch the remote filesystem and push status/root-hash updates to the client.

Client responsibilities:
- Decide whether to offer remote indexing, based on feature flags, user settings, active repo, and remote-server capability.
- Render speedbump/settings/status UI for local and remote repos.
- Cache the latest remote index status per `(host_id, repo_path)`, including the current ready root hash and embedding config.
- Expose `SearchCodebase` to the agent only when the active remote repo has a ready index.
- Call the app server directly for retrieval using the current root hash, then call the daemon only to hydrate fragment bytes or full file context.

Backend responsibilities:
- Store and retrieve Merkle-tree/index data and embeddings keyed by hashes.
- Answer `get_relevant_fragments(root_hash, query, repo_metadata, embedding_config)`.
- Rerank candidate fragments.
- Provide codebase context config to both local client indexing and daemon-side remote indexing.

### Why the client needs the root hash
The client needs the current ready root hash so retrieval can be client → app server instead of client → daemon → app server. The root hash is the backend lookup key for the synced index; it is enough for retrieval, while avoiding a full tree sync to the client. The client should not need fragment bytes or the complete Merkle tree to decide search candidates.

### Rejected alternative: daemon keeps only tree/bytes, client StoreClient syncs
Alternative shape: the daemon builds or maintains the remote Merkle tree and fragment bytes, while the client's existing `StoreClient` talks to the backend. The daemon sends the tree/root state back to the client, and the client drives backend sync.

Why rejected for v1:
- New repos would require syncing the entire tree and enough fragment data over SSH before backend sync can complete. That adds heavy startup traffic on the least reliable leg of the system.
- APP-3801 exists specifically to let daemon handlers call Warp services with the user's token; not using it here loses the main benefit.
- The only strong argument is resilience when daemon → `app.warp.dev` egress is blocked. The initial customer check says that egress is acceptable, and if it is unavailable, the product should fail visibly rather than silently route a much heavier protocol through SSH.

### Rejected alternative: daemon handles all retrieval
Alternative shape: the daemon receives `SearchCodebase`, calls `get_relevant_fragments`, hydrates fragments, reranks, and returns final locations.

Why rejected for v1:
- Adds an SSH hop to every retrieval query even though the client already has a valid app-server auth path.
- Makes retrieval unavailable when the daemon's backend connection is flaky even if the client can reach the backend.
- Couples agent retrieval latency to the remote link more than necessary.

## 3. Proposed changes
### 3.1 Extract daemon-compatible indexing code
Create a daemon-compatible indexing crate (for example `crates/codebase_index`) by extracting the current `crates/ai/src/index/full_source_code_embedding/` modules and the minimal supporting types they need. Re-export from `crates/ai` so current client call sites can keep their public paths until cleanup.

The extracted crate should contain:
- `CodebaseIndexManager`, `CodebaseIndex`, `sync_client`, `store_client`, `snapshot`, `merkle_tree`, `chunker`, `fragment_metadata`, `changed_files`, and related tests.
- `CodeContextLocation` / indexing location types needed by retrieval.
- Persistence adapters split so client builds can keep SQLite-backed metadata while the daemon uses a lightweight file-backed metadata store.

Avoid depending on unrelated `crates/ai` agent, MCP, terminal, or UI modules in the daemon. The daemon should only pull the indexing implementation, syntax/chunking dependencies, remote-compatible filesystem/repo metadata dependencies, and GraphQL types needed by `StoreClient`.

### 3.2 Add daemon-side `StoreClient`
Add a daemon-side implementation of the `StoreClient` trait. It should use the APP-3801 daemon credential and a small HTTP/GraphQL client rather than depending on client-side `ServerApi`.

Required behavior:
- Reads the request-scoped token supplied by the remote client/server proto message for operations triggered by that message. The daemon may keep `ServerModel::auth_token()` or an injected token provider as the initialized token cache, but auth-required outbound Warp service requests must not be authorized solely by the cached daemon token.
- Sends the same backend operations the local client sends today: config fetch, Merkle tree sync, embedding generation, intermediate-node update, cache population, relevant-fragment retrieval only if a future daemon-retrieval path needs it, and reranking only if a future daemon-retrieval path needs it.
- Classifies errors into at least unauthenticated, backend unreachable, backend rejected, and internal/unknown so status UI can distinguish actionable failures.
- Redacts tokens from logs and never persists them.

For v1 sync, daemon-side retrieval methods may still be implemented because the trait requires them, but the normal remote retrieval path should use the client's `ServerApi` for `get_relevant_fragments` and `rerank_fragments`.

### 3.3 Add daemon-side index cache and startup bootstrap
The daemon keeps a notion of cached codebase indexes. Use an identity-scoped directory under the remote-server cache root, for example:

- `~/.warp/remote-server/{identity_key}/codebase-indexes/metadata.json`
- `~/.warp/remote-server/{identity_key}/codebase-indexes/snapshots/{repo_key}/...`

Metadata should record at least repo path, status, last ready root hash, embedding config, last indexed time, last error, and enough timestamps to rebuild the local `WorkspaceMetadata` inputs that currently populate the local build queue.

Startup/reconnect behavior:
1. Load metadata and snapshots before accepting indexing requests.
2. For known ready repos, emit `Ready` status with root hash to connected clients.
3. For known repos without a valid snapshot, emit `Not enabled` or `Failed` depending on whether the user had previously enabled indexing.
4. When a client reconnects, respond to status requests from this cache even before the watcher has emitted new events.

Snapshot parsing should follow local behavior: if a snapshot is incompatible or corrupt, delete it and rebuild from scratch rather than leaving the repo permanently failed.

### 3.4 Add remote-server protocol messages
Extend `crates/remote_server/proto/remote_server.proto` with request/response and push messages for remote indexing. Names can be adjusted during implementation, but the protocol needs these concepts:

- `EnableCodebaseIndexing { repo_path, user_preferences, feature_flags }`
- `GetCodebaseIndexStatus { repo_path }`
- `DropCodebaseIndex { repo_path }`
- `RetryCodebaseIndexing { repo_path }`
- `HydrateCodebaseFragments { repo_path, content_hashes }`
- `CodebaseIndexStatusUpdated { repo_path, status }`

Any request message that can lead to auth-required outbound Warp service calls must carry the current client auth token or an equivalent request-scoped bearer credential in the proto payload. In this v1 protocol, that applies at minimum to `EnableCodebaseIndexing` and `RetryCodebaseIndexing`, because they can trigger config fetches, embedding generation, and index sync. If future versions let `HydrateCodebaseFragments` or daemon-side retrieval call Warp services, those messages must also carry the token before those outbound calls are added. Handlers must reject missing or invalid request-scoped tokens instead of falling back to the daemon's stored `auth_token`; the stored token is only a cache/initialization aid and must not make the proxy socket an ambient-authority boundary.

`IndexStatus` should include:
- `state`: not enabled, queued, indexing, ready, stale, failed, disabled, unavailable.
- `progress`: optional current phase and counts.
- `failure_reason`: optional user-readable string plus machine-readable category.
- `root_hash`: present for ready and stale states when a last-ready index exists.
- `embedding_config`: present whenever `root_hash` is present.
- `last_updated_at`: useful for settings and debugging.

The client should receive root hashes only through status responses/pushes. It should never receive the whole Merkle tree.

### 3.5 Add daemon indexing manager wiring
In `app/src/remote_server/mod.rs`, register the extracted indexing manager as a daemon singleton with:
- file-backed persisted metadata,
- daemon-side snapshot base directory,
- daemon-side `StoreClient`,
- `BulkFilesystemWatcher`,
- remote-compatible repo metadata / detected-repo dependencies already used by the daemon.

In `app/src/remote_server/server_model.rs`, add handler arms for the new RPCs:
- `EnableCodebaseIndexing`: check cache first; if miss or stale, enqueue/build index and immediately push queued/indexing status.
- `GetCodebaseIndexStatus`: return the current cached status.
- `DropCodebaseIndex`: remove metadata/snapshot, stop watcher registration for that repo, and push disabled/not-enabled status.
- `RetryCodebaseIndexing`: clear the last error and enqueue a fresh build/sync.
- `HydrateCodebaseFragments`: map content hashes to fragment metadata, read the corresponding remote file bytes, and return fragment content plus locations.

Subscribe once to indexing manager events and fan out `CodebaseIndexStatusUpdated` to connected clients. On disconnect/reconnect, clients should be able to resync via `GetCodebaseIndexStatus`; push messages are opportunistic, not the only source of truth.

### 3.6 Fetch and respect server-backed config on the daemon
The daemon should call `codebase_context_config` through its `StoreClient` before sync work and at the cadence expected by the local implementation. Server-backed values such as embedding config, embedding cadence, generation batch size, and sync batching should be owned by the backend and respected on the remote host.

The client should still send user/client-controlled gates in `EnableCodebaseIndexing`, such as whether the remote-indexing feature flag is enabled, whether the user accepted indexing, and whether persistence is allowed. Do not use client-sent values for server-owned tuning knobs when the daemon can fetch them directly.

### 3.7 Client-side state and UI model
Add a client singleton such as `RemoteCodebaseIndexModel` that subscribes to `RemoteServerManager` events and tracks:
- `(host_id, repo_path) -> RemoteIndexState`
- state, progress, failure reason, root hash, embedding config, last update time
- whether the remote server currently supports indexing

Use this model from:
- `app/src/settings_view/code_page.rs` to render remote entries alongside local entries with a remote tag/host label and the states from PRODUCT §8-14.
- `app/src/ai/blocklist/codebase_index_speedbump_banner.rs` to show the remote-aware speedbump and dispatch `EnableCodebaseIndexing`.
- agent/tool plumbing to decide whether `SearchCodebase` is advertised for remote sessions.

Settings should distinguish local auto-indexing from remote auto-indexing. If implementation chooses to reuse one preference, the product spec must be updated before shipping; the current product expectation is independent control.

### 3.8 Remote retrieval path
When `SearchCodebaseExecutor` runs in `SessionType::WarpifiedRemote { host_id: Some(_) }`:
1. Resolve the active remote repo path.
2. Read `RemoteCodebaseIndexModel` for `(host_id, repo_path)`.
3. If the state is not ready/stale with a root hash, return a typed `SearchCodebaseResult::Failed` reason for indexing-in-progress, failed, disabled, unavailable, or not indexed.
4. Use the client's `ServerApi` to call `get_relevant_fragments(root_hash, query, repo_metadata, embedding_config)`.
5. Call `HydrateCodebaseFragments` on the daemon with the returned content hashes.
6. Use the client's `ServerApi` to call `rerank_fragments(query, hydrated_fragments)`.
7. Convert reranked fragments into `CodeContextLocation`s.
8. Hydrate final file context through the APP-3790 remote `ReadFileContext` path.
9. Return the normal `SearchCodebaseResult::Success { files }`.

The local path remains unchanged.

### 3.9 Feature flag and rollout
Add `FeatureFlag::RemoteCodebaseIndexing` and gate only client-visible behavior:
- speedbump offer,
- settings controls,
- remote tool advertisement,
- remote dispatch branch in `SearchCodebaseExecutor`.

The daemon should not independently check the feature flag. If it receives a valid `EnableCodebaseIndexing` request from an authenticated client build, it should perform the requested work. This avoids requiring daemon/client flag state to be perfectly synchronized.

## 4. End-to-end flows
### New repo
1. Client observes remote navigation into repo.
2. Client asks daemon for index status.
3. Daemon checks metadata/snapshot cache; none found.
4. Client offers speedbump or auto-enables based on settings.
5. Client sends `EnableCodebaseIndexing`.
6. Daemon builds the tree on the remote host.
7. Daemon fetches backend config and syncs missing tree nodes/fragments/embeddings using daemon auth.
8. Daemon saves metadata/snapshot and pushes `Ready { root_hash, embedding_config }`.
9. Client caches status and enables `SearchCodebase`.

### Previously seen repo
1. Client asks daemon for status after navigation or reconnect.
2. Daemon finds a ready cached index.
3. Daemon returns/pushes `Ready { root_hash, embedding_config }`.
4. Client enables retrieval without full rebuild.

### Startup with known repos
1. Daemon loads metadata and snapshots during startup.
2. Client reconnects and requests statuses for known remote repos.
3. Daemon returns ready/failed/not-enabled states from cache.
4. Watcher registration resumes for enabled repos.

### Incremental changes
1. Daemon filesystem watcher fires for a watched repo.
2. Daemon marks the repo stale if a previous root hash exists.
3. Daemon refreshes backend config if due, computes the incremental tree diff, asks backend what is missing, and syncs only missing nodes/fragments/embeddings.
4. Daemon saves the new snapshot/root hash and pushes ready status to the client.
5. Client replaces its cached root hash; new retrievals use the new index.

### Retrieval
1. Client already knows the current ready root hash.
2. Client calls app server for candidate fragment hashes.
3. Client asks daemon to hydrate those hashes into fragment bytes/locations.
4. Client calls app server for reranking.
5. Client hydrates full file context from the remote host and returns the standard result shape to the agent.

## 5. Testing and validation
- Move existing codebase-index unit tests with the extracted crate and run them unchanged. This covers PRODUCT §21-24 and local non-regression in §33.
- Add daemon-side `StoreClient` tests for token-present, missing-token, backend-unreachable, backend-rejected, and config-fetch behavior. This covers PRODUCT §25 and §30-32.
- Add remote-server protocol/handler tests for enable, status, drop, retry, hydrate, pushed status transitions, and rejection of auth-required requests that omit the request-scoped token. This covers PRODUCT §8-14, §21-24, and §29.
- Add client `RemoteCodebaseIndexModel` tests for queued → indexing → ready, ready → stale → ready, failed → retry → ready, disabled, and unavailable transitions. This covers PRODUCT §10-14 and §17-19.
- Add `SearchCodebaseExecutor` tests for remote ready, indexing, failed, unavailable, not indexed, and local fallback paths. Verify the remote ready path calls client `get_relevant_fragments`, daemon hydrate, client rerank, then remote `ReadFileContext` in order. This covers PRODUCT §15-20 and §29.
- Add settings/speedbump UI tests or snapshots for local entries, remote entries, remote tag/host labeling, retry, drop, and independent local/remote automatic indexing settings. This covers PRODUCT §4-14 and §34.
- Add per-user isolation tests around identity-scoped cache paths and daemon auth token usage. This covers PRODUCT §26-28.
- Add manual verification on an open-egress remote host: enable indexing for a new repo, observe progress in settings, run `SearchCodebase`, edit a file, observe stale/ready transition, and verify subsequent retrieval uses the updated repo.
- Add manual verification on a blocked-egress remote host: enable indexing, verify failed status and retry behavior, and verify other remote tools remain usable.

## 6. Risks and mitigations
- **Daemon egress blocked.** Mitigation: product shows failed/unreachable with retry. Keep client-proxied `StoreClient` as a follow-up only if real deployments require it.
- **Binary size increase.** Extracted indexing code brings tree-sitter/chunking/GraphQL dependencies into the daemon. Mitigation: measure daemon binary size before landing and avoid importing unrelated `crates/ai` modules.
- **Config drift between local and remote indexing.** Mitigation: daemon fetches server-backed config via `codebase_context_config`; client only sends user/feature gates.
- **Status push loss during disconnect.** Mitigation: status is cached on the daemon and clients resync with `GetCodebaseIndexStatus` on reconnect.
- **Snapshot corruption/version skew.** Mitigation: match local snapshot behavior by deleting bad snapshots and rebuilding.
- **Credential exposure.** Mitigation: use APP-3801 token provider, never persist tokens, redact protocol logs, and ensure agent context never includes auth material.
- **Proxy-socket auth bypass.** Mitigation: require request-scoped auth tokens on remote client/server proto messages before handlers make auth-required outbound Warp service requests; reject missing or invalid tokens instead of relying on daemon-stored credentials as ambient authority.
- **Root hash staleness.** Mitigation: stale state keeps last ready root hash usable until a new ready hash arrives; failed sync does not overwrite the last ready hash.

## 7. Follow-ups
- Client-proxied `StoreClient` fallback for hosts that cannot reach `app.warp.dev`.
- Shared remote index optimization across Warp users, only if a future authz/access-proof design supports it.
- Daemon-direct telemetry for indexing metrics instead of client-forwarded status-only events.
- Cross-repo remote context across multiple repos on one host.
- Retrieval caching for repeated queries within one remote session.
