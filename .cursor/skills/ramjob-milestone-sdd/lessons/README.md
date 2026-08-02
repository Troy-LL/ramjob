# Lessons index

Repeatable approach failures captured as skills or structural checks.

| Leading word | Path or check | Failure mode |
|---|---|---|
| caller-owned cache | [no-premature-global-cache](no-premature-global-cache/SKILL.md) | Process-global PathCache / OnceLock for one-shot CLI; racy parallel tests |
| link env | [windows-msvc-sdk-env](windows-msvc-sdk-env/SKILL.md) | Incomplete Windows Kits → LNK1181 kernel32.lib |
| SAC target dir | [windows-smart-app-control-cargo](windows-smart-app-control-cargo/SKILL.md) | Smart App Control blocks unsigned build scripts (os 4551) |
| fresh vcvars shell | [windows-vcvars-line-length](windows-vcvars-line-length/SKILL.md) | Re-sourcing vcvars64 blows PATH/INCLUDE past cmd limit |
| global invoke | [tauri-with-global-tauri](tauri-with-global-tauri/SKILL.md) | Missing withGlobalTauri → panel stuck on MOCK_SNAPSHOT |
| lock across settle | [trim-lock-covers-settle](trim-lock-covers-settle/SKILL.md) | Released trim_lock before settle; dual ΔGF pipelines; classified no-op trims |
| single measure owner | [single-measure-owner](single-measure-owner/SKILL.md) | Second §2.3 copy in runtime; stubbed FSM refault/ineffective inputs |
| always-on cadence | [always-on-engine-cadence](always-on-engine-cadence/SKILL.md) | Panel hide stops Runtime tick; tooltip-only path |

## How to add

1. One failure mode → one row.
2. Prefer a test/lint/CI assert when enforceable in-repo.
3. Else add `lessons/<slug>/SKILL.md` and link it here.
4. Merge duplicates on the next lesson-capture pass.
