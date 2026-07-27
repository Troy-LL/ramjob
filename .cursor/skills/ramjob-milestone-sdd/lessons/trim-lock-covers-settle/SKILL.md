---
name: trim-lock-covers-settle
description: Hold the global trim lock across sample→trim→settle→sample. Use when implementing or changing RamJob yield measurement / gate protocol.
---

# Trim lock covers settle

Leading word: **lock across settle**.

## When

Changing soft trim, gate measurement, or Ry_live / Ry_bench sampling.

## Steps

1. Acquire the process-wide trim lock before the first sample.
2. Keep it held through trim, the settle wait (3 s for Ry_live), and the second sample.
3. Release only after both samples and yield math inputs are collected.
4. Soft-trim itself stays trim-only when the gate owns the measurement protocol. Do not run a second competing ΔGF pipeline under a shorter lock.

**Done when:** a single owned path matches SPEC §2.3 protocol, and tests cover fail-closed behavior on rate-limited or empty trims.
