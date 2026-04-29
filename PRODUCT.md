# APP-3792: Remote Codebase Indexing

Linear: [APP-3792](https://linear.app/warpdotdev/issue/APP-3792)

## Summary

When a Warp user is SSH'd into a remote host, the agent's codebase-search tool works against the repository on that remote host with the same quality, latency profile, and status visibility as local indexing. The user can opt in (or have it auto-opt-in) per repo, see clear status about indexing progress and errors, and get retrieval results from the remote tree without any additional configuration.

## Figma

Figma: none provided. The user-visible surface is the existing codebase-indexing settings page and speedbump banner, extended to carry a remote indicator.

## Problem

The `SearchCodebase` agent tool is disabled for SSH sessions because every piece of the indexing pipeline — filesystem walk, tree building, fragment chunking, filesystem watching, snapshot persistence — is wired to the local filesystem via local-only APIs. Users working on a remote host can use `ReadFiles` and `ApplyFileDiffs` (after APP-3790) but can't ask the agent for semantic codebase search, which is the primary way agents locate relevant code in large repos.

## Behavior

### Enablement and opt-in

1. When a user SSHes into a remote host that has a running remote-server daemon and navigates to a git repo, Warp offers to index that repo. The offer is surfaced the same way as local indexing today (speedbump banner in the active terminal block), with a visible indicator that this is a remote repository (e.g. a host badge near the repo name).

2. If the user has the global "always allow automatic indexing" setting enabled, remote repos are indexed automatically without showing the speedbump, matching local behavior.

3. The user can decline the speedbump for a specific repo (dismisses for that repo), or decline globally (disables remote auto-indexing). Declining has the same semantics as declining local indexing.

4. Declining for one repo on one host does not affect other repos on the same host or the same repo on other hosts.

5. Codebase indexing enablement is per `(Warp user, host, repo)`. Two different Warp users SSH'd into the same host see independent indexing state. See "Per-user scoping" below.

### Indexing status visibility

6. The codebase-indexing settings page lists remote repos alongside local ones. Each remote entry is labeled with the host it belongs to, so a user SSH'd into multiple hosts can distinguish them.

7. Each entry surfaces one of these status states, visible both in the settings page and in any inline status affordance (banner, status dot, tooltip):
    - **Indexing** (pending / in-progress), with progress info when available: "Discovering N files" during tree build, "Syncing M/N nodes" during embedding sync.
    - **Ready** — indexing is complete and retrieval works.
    - **Stale** — indexing has completed at least once, and the tree is currently being re-synced after a filesystem change. Retrieval still works against the last-known root hash.
    - **Failed** — indexing failed with a user-readable reason (repo too large, filesystem inaccessible, backend unreachable from the daemon, index sync error).
    - **Disabled** — the user opted out for this repo.

8. Status transitions are pushed from the remote side in near real-time. The user does not have to refresh the settings page; status updates without any action.

9. Error states include a retry affordance. Clicking "retry" re-kicks-off the indexing pipeline for that repo on the remote host.

10. When the user disconnects from the remote host (SSH drops, tab closes), the status for that host's repos becomes "Unavailable" in the settings page but is not deleted. When the user reconnects (same host, same Warp user), prior status resumes.

### Agent retrieval

11. When the agent runs in an SSH session that has a remote-server connection and at least one ready remote index, the `SearchCodebase` tool is exposed to the LLM and is invokable just like local `SearchCodebase`.

12. Invoking `SearchCodebase` from a remote session returns relevant file fragments from the remote host. The agent sees the same `CodeContextLocation` shape (whole files + fragment ranges) it gets from local retrieval.

13. If `SearchCodebase` is invoked while the remote index is `Indexing` (not yet ready), the tool returns a clear "index is still syncing, retry later" error message rather than silently failing or returning partial results.

14. If `SearchCodebase` is invoked when the remote index is in a `Failed` state, the tool returns the failure reason to the LLM so the agent can choose to fall back to other tools (e.g. `Grep`, `FileGlob`).

15. Retrieval latency for a remote repo on a typical developer SSH link should be within ~200 ms of the equivalent local-repo latency. Users should not perceive a meaningful slowdown relative to local search.

### Persistence and reconnection

16. Once a repo has been indexed, its tree and associated embeddings persist across SSH session drops, remote-server daemon grace-period expirations, and daemon restarts. When the user reconnects and navigates back to the same repo, retrieval works immediately without re-running the full index build.

17. When the remote filesystem has changed while the user was disconnected, the daemon detects this on reconnect and runs an incremental re-sync. Status moves to `Stale` during the re-sync and back to `Ready` when complete. The prior root hash remains usable for retrieval throughout.

18. If the daemon's on-disk persistence is wiped (host reinstall, cache cleared, storage lost), the next connection rebuilds from scratch. The client's view does not assume persistence is durable across all host-side lifecycle events.

### Per-user scoping

19. Each Warp user on a host gets their own codebase index. A second user connecting to the same host sees no shared state with the first — they separately opt in (or auto-index), separately pay the tree-build cost, and separately observe status.

20. Users with different filesystem read permissions on the same host (e.g. user A can read `/srv/secrets/`, user B cannot) never see each other's indexed content. Indexing respects the OS-level permissions of the user running the daemon.

21. The same user SSH'd into two different hosts has two separate indices. Settings UI surfaces them as separate entries, labeled with the host.

### Backend unreachability from the daemon

22. If the remote-server daemon cannot reach `app.warp.dev` (firewall, egress policy, etc.), the repo status transitions to `Failed` with a user-readable reason ("couldn't reach Warp backend from the remote host") and a clickable retry affordance.

23. The daemon reports the unreachability status as soon as it discovers it (first failed sync call), not on a delayed schedule.

24. Retrieval is gated on a ready index, so backend-unreachable repos remain un-searchable until either the network issue is resolved or indexing succeeds via retry.

### Interaction with other tools and surfaces

25. `ReadFiles` and `ApplyFileDiffs` (from APP-3790) on remote sessions continue to work unchanged, regardless of whether codebase indexing is enabled. Remote codebase indexing does not gate, block, or alter their behavior.

26. When `SearchCodebase` returns `CodeContextLocation`s that reference files on the remote host, the subsequent file-content hydration step uses the existing remote file-read path (`ReadFileContext`). The LLM sees file context in the same shape as local.

27. The agent's visible tool list (in `get_supported_tools`) reflects the current state: when the feature flag is on, a remote session is connected, and the daemon is reachable, `SearchCodebase` is listed. Otherwise it is absent and the LLM does not attempt to call it.

### Settings and controls

28. The codebase-indexing settings page has a top-level toggle for "Enable automatic indexing on remote hosts" (independent of the local toggle) so users can allow local auto-indexing but gate remote indexing behind an explicit opt-in, or vice versa.

29. Individual remote repos can be "dropped" from the settings page the same way local repos can. Dropping a remote repo deletes the index on the daemon (clearing its cache) and stops future re-sync.

### Invariants that must not regress

30. Local codebase indexing behavior is unchanged by this feature. Local users see identical status, speedbump, and retrieval behavior as before.

31. A session that cannot establish a remote-server connection (binary install failed, SSH drop, feature flag off) gracefully falls back: no remote indexing, `SearchCodebase` is not exposed for that session, no error or blocker for the rest of the agent's tools.

32. Agent conversations started in a remote session do not bundle or expose the remote user's auth credentials. All backend calls remain authenticated per-user via APP-3801.
