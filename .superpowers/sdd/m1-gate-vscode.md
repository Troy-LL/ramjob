# M1 gate results

**Classification (Ry_bench):** Pass

## Synthetic hog run

- group_key: `c:\users\troyl\appdata\local\programs\microsoft vs code`
- target_pids: [3448, 5200, 5292, 6208, 17224, 18596, 19464, 21332, 22572, 23360]
- trimmed_pids: [18596, 3448, 19464, 17224, 5200, 6208, 5292, 22572, 23360, 21332]
- excluded_pids: []
- rate_limited: false
- settle: 3 s
- gf0 / gf1 (intersected private WS): 231362560 / 19398656 bytes
- available0 / available1: 980631552 / 1137602560 bytes
- CompressStore cs0 / cs1: 136413184 / 221896704
- **Ry_bench:** 0.7406
- **Ry_live:** 0.5967

## Thresholds (SPEC §9.2)

- Pass: Ry_bench ≥ 0.5
- Marginal: 0.3 ≤ Ry_bench < 0.5
- Fail: Ry_bench < 0.3

## Product pivot

This file reports only. Fail/Marginal does **not** silently change product shape.
