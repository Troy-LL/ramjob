# M1 gate results

**Classification (Ry_bench):** Pass

## Synthetic hog run

- group_key: `c:\program files\windowsapps`
- target_pids: [4828, 8328, 9260, 11156, 13040, 16156, 16216, 17304, 17876]
- trimmed_pids: [17876, 9260, 16156, 16216, 13040, 17304, 11156, 8328, 4828]
- excluded_pids: []
- rate_limited: false
- settle: 3 s
- gf0 / gf1 (intersected private WS): 233095168 / 152834048 bytes
- available0 / available1: 370155520 / 524615680 bytes
- CompressStore cs0 / cs1: 132096000 / 164507648
- **Ry_bench:** 1.9245
- **Ry_live:** 0.5962

## Thresholds (SPEC §9.2)

- Pass: Ry_bench ≥ 0.5
- Marginal: 0.3 ≤ Ry_bench < 0.5
- Fail: Ry_bench < 0.3

## Product pivot

This file reports only. Fail/Marginal does **not** silently change product shape.
