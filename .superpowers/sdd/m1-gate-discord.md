# M1 gate results

**Classification (Ry_bench):** Pass

## Synthetic hog run

- group_key: `c:\users\troyl\appdata\local\discord`
- target_pids: [2560, 12836, 14064, 17504, 19320, 19648]
- trimmed_pids: [14064, 19320, 2560, 19648, 17504, 12836]
- excluded_pids: []
- rate_limited: false
- settle: 3 s
- gf0 / gf1 (intersected private WS): 272601088 / 37085184 bytes
- available0 / available1: 323178496 / 623329280 bytes
- CompressStore cs0 / cs1: 201367552 / 57802752
- **Ry_bench:** 1.2744
- **Ry_live:** 1.6096

## Thresholds (SPEC §9.2)

- Pass: Ry_bench ≥ 0.5
- Marginal: 0.3 ≤ Ry_bench < 0.5
- Fail: Ry_bench < 0.3

## Product pivot

This file reports only. Fail/Marginal does **not** silently change product shape.
