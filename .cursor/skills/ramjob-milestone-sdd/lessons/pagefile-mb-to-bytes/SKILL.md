# pagefile-mb-to-bytes

## Leading word

pagefile-mb-to-bytes

## When

Reading Windows registry or API fields that document sizes in megabytes (e.g. `PagingFiles` max).

## Failure mode

Multiply MB by GiB (`1024³`) or treat MB as bytes. §5.4 `Small` never fires; mock tests that feed bytes hide the bug. M6 thermo C1.

## Do this

1. Convert with a named `mb_to_bytes` = `× 1024²` (MiB).
2. Unit-test with **registry-shaped MB inputs** (e.g. 512 → Small), not only byte mocks.
3. Prefer distinct types/fn names so “MB” and “bytes” cannot share a bare `u64` call site silently.

## Done when

- Registry path classifies Small/Ok correctly under SPEC §5.4.
- At least one test feeds MB literals through the real conversion.

## Anti-pattern

```rust
Some(total_mb) => Ok(Some(total_mb * ONE_GIB)) // 512 MB → 512 GiB
```
