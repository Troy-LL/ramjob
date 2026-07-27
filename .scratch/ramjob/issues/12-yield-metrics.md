# 12 — Ry_bench / Ry_live measurement

**Milestone:** M1

**What to build:** After a trim, compute `Ry_live = (ΔGF − ΔCompressStore) / ΔGF` using on-demand samples and member intersection. Expose `Ry_bench = Δ(Available MBytes) / Δ(GF)` for quiesced gate runs. Sample Memory Compression process WS from the same NtQSI sweep.

**Blocked by:** 10 — Soft trim pass

**Status:** done

- [x] `accountant::measure_yield(...)` or `yield_math` module with pure functions tested from synthetic numbers
- [x] CompressStore = WS of Memory Compression system process when present
- [x] ΔGF uses intersection of members by pid+ctime; private WS only
- [x] Division-by-zero safe when ΔGF == 0

**Verify:** `cargo test -p ramjob-core yield`

**Notes:** Runtime cutoff remains placeholder *set at M1* after gate data.
