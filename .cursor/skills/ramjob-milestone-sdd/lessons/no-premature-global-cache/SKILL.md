---
name: no-premature-global-cache
description: Avoid process-global caches until a real multi-call owner needs them. Use when adding PathCache, OnceLock, or static Mutex state in RamJob.
---

# No premature global cache

Leading word: **caller-owned cache**.

## When

Adding memoization, path caches, or resolve-once state in Rust engine code.

## Steps

1. Put the cache in the caller's stack (CLI poll loop, engine struct field, or function parameter).
2. Prefer `fn foo_with_cache(cache: &mut Cache)` plus a thin wrapper only when a long-lived owner exists.
3. Never use `OnceLock`/`lazy_static` Mutex for a one-shot CLI path.
4. Tests must construct a local cache. Never share process-global counters across parallel tests.

**Done when:** no new `static` cache lands without a named multi-call owner in the same PR, and tests pass under `cargo test` default parallelism.
