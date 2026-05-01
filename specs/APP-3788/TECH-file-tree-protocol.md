# Remote Server File Tree Protocol — Tech Spec

Linear: [APP-3788](https://linear.app/warpdotdev/issue/APP-3788)

## Problem

The remote server binary (`crates/remote_server`) currently only handles `Initialize`/`InitializeResponse`. We need to:
1. Boot repo metadata models on the server so it can index directories and keep file trees up to date
2. Let the client tell the server which directories to index (via `NavigatedToDirectory`)
3. Let the client fetch the initial tree and receive subsequent incremental updates as push messages

## Current State

### Remote server (`crates/remote_server`)

- `ServerModel` singleton handles stdin/stdout protobuf I/O
- `run()` boots a headless warpui app with only `ServerModel`
- Proto schema has only `Initialize`/`InitializeResponse`

### repo_metadata crate

- `LocalRepoMetadataModel` — indexes repos, subscribes to `DetectedRepositories` for auto-indexing, has `emit_incremental_updates: bool` field and emits `IncrementalUpdateReady` when enabled
- `DetectedRepositories` singleton — runs async git detection via `detect_possible_git_repo()`, emits `DetectedGitRepo` events. Uses `DirectoryWatcher` to register watch directories.
- `DirectoryWatcher` singleton — manages filesystem watchers and routes changes to `Repository` subscribers via a `TaskQueue`
- `LocalRepoMetadataModel` also supports lazy-loaded non-git directories via `index_lazy_loaded_path()` (first-level-only tree, `loaded: false` on subdirectories)
- The incremental update types (`RepoMetadataUpdate`, `FileTreeEntryUpdate`, etc.) already exist in `file_tree_update.rs`

### Key insight on two separate watchers

`DirectoryWatcher` and `LocalRepoMetadataModel` each own their own `BulkFilesystemWatcher`. `DirectoryWatcher`'s watcher feeds the `Repository` model (git status, etc.), while `LocalRepoMetadataModel`'s watcher feeds the file tree. Both need to be running on the server.

## Proposed Changes

### 1. Proto schema additions (`remote_server.proto`)

Names and fields mirror the Rust types in `repo_metadata/src/file_tree_update.rs` 1:1 for trivial conversion.

```proto
// ── Shared file tree sub-messages ─────────────────────────────────
// Mirror the Rust types in repo_metadata/src/file_tree_update.rs.

message RepoNodeMetadata {
  oneof node {
    DirectoryNodeMetadata directory = 1;
    FileNodeMetadata file = 2;
  }
}

message DirectoryNodeMetadata {
  string path = 1;
  bool ignored = 2;
  bool loaded = 3;
}

message FileNodeMetadata {
  string path = 1;
  optional string extension = 2;
  bool ignored = 3;
}

// Mirrors FileTreeEntryUpdate in Rust.
message FileTreeEntryUpdate {
  string parent_path_to_replace = 1;
  repeated RepoNodeMetadata subtree_metadata = 2;
}

// ── Client → server ───────────────────────────────────────────────

// "I navigated to this directory, please index it."
message NavigatedToDirectory {
  string path = 1;
}

// Response after the server has run git detection on the requested path.
//
// - is_git = true:  A git repo was found. indexed_path is the repo root.
//                   Full indexing runs in the background; the client should
//                   wait for RepositoryIndexedPush before calling FetchFileTree.
// - is_git = false: No git repo. The directory was lazily indexed at first
//                   level. indexed_path is the standardized input path.
//                   The client can call FetchFileTree immediately.
message NavigatedToDirectoryResponse {
  string indexed_path = 1;
  bool is_git = 2;
}

// "Give me the current tree for this repo."
message FetchFileTree {
  string repo_path = 1;
}

// Sent as one or more responses for the same request_id.
// Client accumulates entries until sync_complete = true.
message FetchFileTreeResponse {
  string repo_path = 1;
  repeated FileTreeEntryUpdate entries = 2;
  bool sync_complete = 3;
}

// ── Server → client push (empty request_id) ───────────────────────

// Mirrors RepoMetadataUpdate in Rust.
message FileTreeUpdatePush {
  string repo_path = 1;
  repeated string remove_entries = 2;
  repeated FileTreeEntryUpdate update_entries = 3;
}

// A repository finished full indexing and is ready for FetchFileTree.
message RepositoryIndexedPush {
  string repo_path = 1;
}
```

Updated envelopes:

```proto
message ClientMessage {
  string request_id = 1;
  oneof message {
    Initialize initialize = 2;
    NavigatedToDirectory navigated_to_directory = 3;
    FetchFileTree fetch_file_tree = 4;
  }
}

message ServerMessage {
  string request_id = 1;
  oneof message {
    InitializeResponse initialize_response = 2;
    ErrorResponse error = 3;
    NavigatedToDirectoryResponse navigated_to_directory_response = 4;
    FetchFileTreeResponse fetch_file_tree_response = 5;
    FileTreeUpdatePush file_tree_update = 6;
    RepositoryIndexedPush repository_indexed = 7;
  }
}
```

Push messages use an empty `request_id` to distinguish them from request/response pairs.

### 2. Server-side model bootstrap

Update `remote_server::run()` to register the repo metadata singletons:

```rust
AppBuilder::new_headless(...).run(|ctx| {
    ctx.add_singleton_model(DirectoryWatcher::new);
    ctx.add_singleton_model(DetectedRepositories::default_entity);
    ctx.add_singleton_model(|ctx| {
        let mut model = LocalRepoMetadataModel::new(ctx);
        model.set_emit_incremental_updates(true);
        model
    });
    ctx.add_singleton_model(ServerModel::new);
});
```

This automatically wires up the existing `DetectedRepositories` → `LocalRepoMetadataModel` subscription and the watcher → `LocalRepoMetadataModel` update pipeline.

New API needed: `LocalRepoMetadataModel::set_emit_incremental_updates(&mut self, enabled: bool)` (or a builder-style constructor parameter).

### 3. Server-side message handling

#### `NavigatedToDirectory`

When the server receives `NavigatedToDirectory { path }`:

1. Await `detect_possible_git_repo(path)` — this checks the in-memory cache first (instant if already known), otherwise walks up the directory tree checking for `.git` (fast filesystem metadata, not full indexing)
2. If a git repo was found (`Some(git_root)`):
   - Full indexing was already triggered by the `DetectedGitRepo` → `LocalRepoMetadataModel` subscription inside `detect_possible_git_repo`
   - Respond with `{ indexed_path: git_root, is_git: true }`
   - Client waits for `RepositoryIndexedPush` before calling `FetchFileTree`
3. If no git repo (`None`):
   - Call `index_lazy_loaded_path(path)` for first-level-only data
   - Respond with `{ indexed_path: standardized_path, is_git: false }`
   - Client can call `FetchFileTree` immediately

#### `FetchFileTree`

When the server receives `FetchFileTree { repo_path }`:

1. Look up the repository in `LocalRepoMetadataModel` via `get_repository(&repo_path)`
2. If `Indexed`: serialize the full `FileTreeEntry` as one or more `FetchFileTreeResponse` chunks (see section on streaming pagination below)
3. If `Pending`: return `ErrorResponse` — the client retries after receiving `RepositoryIndexedPush`
4. If `Failed` or not found: return `ErrorResponse`

Serialization: Walk the `FileTreeEntry`'s `state_map` and `parent_to_child_map` to produce `FileTreeEntryUpdate` entries. This is the same shape as `RepoMetadataUpdate` but for the full tree.

#### Incremental update push

The `ServerModel` subscribes to `LocalRepoMetadataModel`'s `IncrementalUpdateReady` events. On receiving the event:

1. Convert the `RepoMetadataUpdate` to `FileTreeUpdatePush` proto
2. Send as a `ServerMessage` with empty `request_id`

### 4. Conversion layer: Rust types ↔ Proto

Add a new module `crates/remote_server/src/file_tree_proto.rs` with:

- `RepoMetadataUpdate` → `FileTreeUpdatePush` proto
- `FileTreeEntry` → `FetchFileTreeResponse` proto (full tree serialization with chunking)
- Proto `FetchFileTreeResponse` / `FileTreeUpdatePush` → `RepoMetadataUpdate` for client-side application

These conversions are straightforward because the Rust types in `file_tree_update.rs` were designed to mirror the proto schema 1:1.

### 5. Client-side changes

#### `RemoteServerClient` additions

Add methods to the client:

- `navigate_to_directory(&self, path: String) -> Result<NavigatedToDirectoryResponse>`
- `fetch_file_tree(&self, repo_path: String) -> Result<FetchFileTreeResponse>` (accumulates chunked responses)
- Handle push messages (`FileTreeUpdatePush`, `RepositoryIndexedPush`) in the client's reader loop and emit them as client events

#### `RemoteServerClient` event handling

The client's reader task receives `ServerMessage`s. For push messages (empty `request_id`), route to event emission instead of completing a pending request:

```rust
RemoteServerClientEvent::FileTreeUpdated { update: RepoMetadataUpdate }
RemoteServerClientEvent::RepositoryIndexed { repo_path: String }
```

The downstream consumer (future file tree view integration) subscribes to these events and calls `RemoteRepoMetadataModel::apply_incremental_update()` for `FileTreeUpdated`, and `RemoteRepoMetadataModel::insert_repository()` for the initial tree after calling `fetch_file_tree`.

## Design Decisions

### 1. NavigatedToDirectory: await git detection, then branch

The local `FileTreeView::update_directory_contents` (`view.rs:703`) uses a two-pronged approach: check for a git repo first, fall back to lazy-loading if none is found. The remote server mirrors this but runs git detection synchronously within the request handling so the client gets a definitive answer in one round trip:

1. Server awaits `detect_possible_git_repo(path)` — checks in-memory cache first (instant for known repos), otherwise walks up the directory tree (fast filesystem metadata checks, not full indexing)
2. If git repo found: respond with `{ indexed_path: git_root, is_git: true }`. Full indexing was already triggered by `DetectedGitRepo` → `LocalRepoMetadataModel`. Client waits for `RepositoryIndexedPush` before `FetchFileTree`.
3. If no git repo: server calls `index_lazy_loaded_path(path)` for first-level data, responds with `{ indexed_path: path, is_git: false }`. Client calls `FetchFileTree` immediately.

This avoids the unnecessary eager lazy-load for git repos (which would be thrown away when full indexing completes) and gives the client clear instructions in a single response.

### 2. Initial tree fetch: server-controlled streaming pagination

Pagination is controlled by the server based on actual response size, not tree depth (a flat repo could have huge amounts of data at each level). The protocol:

1. Server serializes the tree top-to-bottom (breadth-first or depth-first pre-order)
2. Each `FetchFileTreeResponse` chunk contains a batch of entries plus a `bool sync_complete` flag
3. The server segments by a target byte budget per chunk (e.g. 256KB)
4. The client renders progressively as chunks arrive, and knows the full tree is loaded when `sync_complete = true`

Multiple `FetchFileTreeResponse` messages are sent for the same `request_id`. The client accumulates them and applies each chunk to the `RemoteRepoMetadataModel` as it arrives.

## End-to-End Flow

### Case A: Directory is inside a git repo

```
                        Client                                      Server
                          │                                           │
  User navigates to       │  NavigatedToDirectory { path }            │
  /home/user/project/src  │ ────────────────────────────────────────> │
                          │                                           │── await detect_possible_git_repo(path)
                          │                                           │   → found git root /home/user/project
                          │                                           │   (full indexing triggered in bg)
                          │  Response { indexed_path: .../project,     │
                          │             is_git: true }                 │
                          │ <──────────────────────────────────────── │
                          │                                           │
  Client waits...         │   ... full repo indexing completes ...     │
                          │                                           │
                          │  RepositoryIndexedPush { repo_path }      │
                          │ <──────────────────────────────────────── │
                          │                                           │
  Now fetch full tree     │  FetchFileTree { repo_path }              │
                          │ ────────────────────────────────────────> │
                          │                                           │
                          │  FetchFileTreeResponse { ... true }       │ ← full tree (chunked)
                          │ <──────────────────────────────────────── │
                          │                                           │
                          │     ... file watcher detects changes ...   │
                          │                                           │
                          │  FileTreeUpdatePush { incremental }       │ ← push, empty request_id
                          │ <──────────────────────────────────────── │
```

### Case B: Directory is NOT a git repo

```
                        Client                                      Server
                          │                                           │
  User navigates to       │  NavigatedToDirectory { path }            │
  /tmp/some-dir           │ ────────────────────────────────────────> │
                          │                                           │── await detect_possible_git_repo → None
                          │                                           │── index_lazy_loaded_path(path)
                          │  Response { indexed_path: /tmp/some-dir,   │
                          │             is_git: false }                │
                          │ <──────────────────────────────────────── │
                          │                                           │
  Fetch immediately       │  FetchFileTree { repo_path }              │
                          │ ────────────────────────────────────────> │
                          │                                           │
                          │  FetchFileTreeResponse { ... true }       │ ← first-level tree
                          │ <──────────────────────────────────────── │
```

## Follow-ups (out of scope)

- Wire the client events to `RemoteRepoMetadataModel` and `FileTreeView`
- `LoadDirectory` request for expanding collapsed directories over the network
- Subscription management (unsubscribe from updates when file tree is closed)
