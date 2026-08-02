# tick-persist-day-bucket

## Leading word

tick-persist-day-bucket

## When

Updating “last seen” or similar bookkeeping from the always-on Runtime / tray tick.

## Failure mode

`save_config_atomic` (or any full config rewrite) whenever `now_unix` changes → ~1 Hz disk I/O with the panel open. Fights SPEC §6 and always-on cadence. M6 thermo I1.

## Do this

1. Update in-memory stamps every tick (cheap).
2. Persist only on a coarse bucket (calendar day / 24h) or dirty debounce.
3. Keep prune-on-startup; do not need second-granularity durability for 90-day prune.

## Done when

- Same-day ticks do not rewrite config.
- Cross-day (or debounce) path still persists and is tested.

## Related

`always-on-engine-cadence` — tick must keep running; this lesson is about keeping that tick cheap.
