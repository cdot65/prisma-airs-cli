# airs-history-store

The server-side runtime history archive: private SQLite per cloud identity, reconciliation, retry work, receipts, sync state, transfer, compaction and recovery.

Only `airs-mcp` links it. The `airs` CLI never does, and the crate is not published.

```sh
cargo test -p airs-history-store
```
