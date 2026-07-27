# M0 verify — live `ramjob list`

**When:** 2026-07-27  
**Command:** `cargo run -p ramjob-cli -- list`  
**Format:** `group_key\tmembers\thuman_gf` (GF desc, GF ≥ 50 MB)

## Output summary

8 visible groups on this machine. Top consumers:

| group_key | members | GF |
|-----------|---------|-----|
| `c:\users\troyl\appdata\local\programs\cursor` | 17 | 956 MiB |
| `c:\users\troyl\appdata\local\bravesoftware` | 23 | 950 MiB |
| `c:\program files\windowsapps` | 9 | 350 MiB |
| `c:\users\troyl\.local\bin` | 4 | 144 MiB |
| `c:\windows\system32\windowspowershell\v1.0` | 2 | 126 MiB |
| `c:\users\troyl\appdata\local\programs\orca` | 5 | 118 MiB |
| `c:\users\troyl\.cursor\extensions\...\native-binary` | 2 | 86 MiB |
| `c:\windows\system32` | 10 | 84 MiB |

## Cross-app merge glance (Brave / Spotify / VS Code / Discord)

| App | Seen? | Cross-merge? |
|-----|-------|--------------|
| **Brave** | Yes — `...\local\bravesoftware`, 23 members | **No.** Separate from Cursor, Orca, WindowsApps. |
| **Spotify** | Not in list (not running or GF &lt; 50 MB) | n/a |
| **VS Code** | Not in list (Cursor present instead) | n/a |
| **Discord** | Not in list (not running or GF &lt; 50 MB) | n/a |

Brave and Cursor stay distinct install-root groups. No Brave↔Cursor (or Brave↔other) merge observed.

## Notes

- Sort is GF descending; all printed rows meet the 50 MB floor.
- `windowsapps` is a coarse parent key (everything under `Program Files\WindowsApps` that survived Microsoft.* filter). Expected M0 coarseness, not a Brave merge.
- `%WINDIR%` paths (`system32`, PowerShell) still appear. Grouper §5.2 windir filter may be incomplete for some image paths; flag for a follow-up, not a Task 5 wiring failure.
- M0 accountant unique-shared is 0, so GF understates shared DLL cost.
