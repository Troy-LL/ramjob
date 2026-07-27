# single measure owner

## Leading word

single-measure-owner

## When

Building a second trim/yield consumer (daemon tick, gate, tray action) that needs Ry_live or post-trim GF.

## Failure mode

Re-implementing sample→trim→settle→sample next to `gate::run_gate_on_group` / `measure_under_lock`, then stubbing FSM feedback (`refault_hot = false`, `trim_was_ineffective` from the wrong signal). Thermo catches dual §2.3 owners and dead Thrashing/WouldBackstop paths.

## Do this

1. Call the shared gate measurement helper for live soft-trim yield.
2. Derive `refault_hot` / `trim_was_ineffective` / `last_ry_live` from `GateMeasurement` (`gf0`/`gf1`/`ry_live`).
3. Use `ExclusionPolicy::ProtectInteractive` on the daemon path (bench/gate may use `None`).
4. Fail closed on rate-limit / empty trim — no fake `trims_attempted`.

## Done when

- One measurement owner for §2.3 settle window.
- Post-trim FSM inputs come from real samples, not hard-coded false/zero.
- A unit test covers refault/ineffective derivation from gf0/gf1.

## Anti-pattern

A private `measured_soft_trim` that copies gate’s sleep+sample loop “just for the daemon.”
