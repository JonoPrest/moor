# Benchmarks

The "measure before optimising" triggers from ARCHITECTURE §10. Run with

```
cargo bench -p nits-review-core -p nitsd            # 50 000-file synthetic repo
NITS_BENCH_FILES=2000 cargo bench -p nits-review-core   # what CI runs
```

Each bench is a hand-timed realistic operation (median of 5) printed as a
table; nothing fails on a trip. CI runs the small variant only to keep the
benches compiling and honest. Update this file when a number changes
materially or a trigger is acted on.

## 2026-08-27 — MacBook (Apple Silicon), release build, 50 000 files

| case | median | trigger | note |
|------|-------:|--------:|------|
| working-tree snapshot after one edit | 214.8 ms **TRIPPED** | 100 ms | 50000 files |
| changed_files on a directory move | 19.9 ms | 500 ms | 100 renames of 50000 files |
| tree_snapshot | 130.4 ms | 200 ms | 50000 files, 6 MB JSON (size trigger 5 MB — tripped) |
| 200-comment burst (Core::add_comment) | 1364.3 ms **TRIPPED** | 1000 ms | durable redb append per comment |
| 200-comment burst (Daemon::write) | 852.2 ms | 1000 ms | sequential awaits, writer thread + broadcast |

Run-to-run variance on the comment burst is large (it is fsync-bound); the
two burst lines are the same work through two entry points.

### Tripped → candidate fixes (§10), not yet built

1. **Snapshot after one edit.** The temp-index `git add -A` still stats
   50k paths. Fix: the watcher passes the reported paths and the snapshot
   runs `update-index` on those only; ignore rules at the watcher.
2. **Comment burst.** One redb commit (fsync) per event. Fix: batch appends
   per writer-thread tick (drain the job queue into one transaction) and/or
   keep ephemeral state out of the durable log. Durability semantics must
   stay "acked ⇒ on disk" for mutations.
3. **Tree snapshot size.** 6 MB at 50k files is over the 5 MB line but the
   time is under; lazy subtrees (§4.7) only once a real repo of this size
   is in use — the synthetic repo is the worst case (every path unique).
