# always-on engine cadence

## Leading word

always-on-engine-cadence

## When

Wiring a tray/panel visibility flag into the daemon tick loop, or adding a “cheap path” while the UI is hidden.

## Failure mode

Gating `Runtime::tick` / enumerate / FSM / trim on `window.is_visible()`. Closed panel then freezes enforcement; only a tooltip refresh runs. Violates SPEC §6.1 (panel closed backs the sweep **down**, not off). Thermo M3 Critical #1.

## Do this

1. One always-on cadence owner calls `Runtime::tick` regardless of panel visibility.
2. Visibility only changes sleep interval / history density (open ≈ 1 s; closed ≈ 30 s armed / 120 s disarmed).
3. Tray tooltip may use cheaper samples **in addition**, never **instead of**, the policy tick.

## Done when

- Hidden panel still advances FSM and can trim.
- Sleep intervals match SPEC §6.1 ladder.
- No second enumerate/`PathCache` beside `Runtime`.

## Anti-pattern

`if panel_visible { run_tick() } else { refresh_tooltip_only() }`.
