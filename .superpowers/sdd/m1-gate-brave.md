# M1 gate results

**Classification (Ry_bench):** Pass

## Synthetic hog run

- group_key: `c:\users\troyl\appdata\local\bravesoftware`
- target_pids: [1808, 2304, 2816, 4072, 5192, 9624, 9768, 10468, 13988, 14936, 16904, 17580, 18736, 19024, 19284, 19700, 20008, 22064, 22520]
- trimmed_pids: [22520, 4072, 17580, 19700, 14936, 2816, 16904, 9768, 10468, 13988, 1808, 5192, 9624, 19284, 2304, 18736, 20008, 19024, 22064]
- excluded_pids: []
- rate_limited: false
- settle: 3 s
- gf0 / gf1 (intersected private WS): 765292544 / 29241344 bytes
- available0 / available1: 882778112 / 1402003456 bytes
- CompressStore cs0 / cs1: 97759232 / 350720000
- **Ry_bench:** 0.7054
- **Ry_live:** 0.6563

## Thresholds (SPEC §9.2)

- Pass: Ry_bench ≥ 0.5
- Marginal: 0.3 ≤ Ry_bench < 0.5
- Fail: Ry_bench < 0.3

## Product pivot

This file reports only. Fail/Marginal does **not** silently change product shape.
