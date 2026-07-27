# M1 gate results

**Classification (Ry_bench):** Pass

## Synthetic hog run

- group_key: `image:ramjob-hog`
- target_pids: [12884]
- trimmed_pids: [12884]
- excluded_pids: []
- rate_limited: false
- settle: 3 s
- gf0 / gf1 (intersected private WS): 268840960 / 118784 bytes
- available0 / available1: 1319227392 / 1604898816 bytes
- CompressStore cs0 / cs1: 52203520 / 55668736
- **Ry_bench:** 1.0631
- **Ry_live:** 0.9871

## Thresholds (SPEC §9.2)

- Pass: Ry_bench ≥ 0.5
- Marginal: 0.3 ≤ Ry_bench < 0.5
- Fail: Ry_bench < 0.3

## Product pivot

This file reports only. Fail/Marginal does **not** silently change product shape.
