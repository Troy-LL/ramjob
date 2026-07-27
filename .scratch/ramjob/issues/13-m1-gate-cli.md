# 13 — M1 gate CLI + harness protocol

**Milestone:** M1

**What to build:** `ramjob gate` (or `ramjob trim --measure`) that trims a target group or the hog, waits per protocol (3 s for Ry_live; bench path records Available), prints Ry_bench and Ry_live, and writes `.superpowers/sdd/m1-gate-results.md` with pass/marginal/fail vs SPEC §9.2 thresholds on Ry_bench.

**Blocked by:** 11 — Synthetic hog; 12 — Yield metrics

**Status:** ready-for-agent

- [ ] CLI command documented in `--help`
- [ ] Can target hog by image name or PID group
- [ ] Results file includes at least one synthetic-hog run with both metrics
- [ ] Classifies Pass (≥0.5) / Marginal (0.3–0.5) / Fail (<0.3) on Ry_bench
- [ ] Does NOT silently pivot product on Fail — only reports

**Verify:** `. .\scripts\dev-env.ps1; cargo test --workspace; cargo run -p ramjob-cli -- gate --help` and a recorded hog gate run in m1-gate-results.md

**Notes:** Real Brave/VS Code corpus optional if time; synthetic hog is required. Pause for human if Fail/Marginal.
