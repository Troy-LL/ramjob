//! Enumerate → group → footprint → print visible rows.

use ramjob_core::accountant::{group_footprint, meets_gf_floor};
use ramjob_core::grouper::{group_processes, AppGroup};
use ramjob_core::scanner::{enumerate_processes_with_cache, PathCache};

/// One printed line after GF filter and sort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListRow {
    pub name: String,
    pub member_count: usize,
    pub gf_bytes: u64,
}

/// Build visible rows from groups: GF ≥ 50 MB, sorted by GF descending.
pub fn visible_rows(groups: &[AppGroup]) -> Vec<ListRow> {
    let mut rows: Vec<ListRow> = groups
        .iter()
        .filter_map(|g| {
            let gf = group_footprint(g);
            if !meets_gf_floor(gf) {
                return None;
            }
            Some(ListRow {
                // M0: no friendly display-name map yet; print the stable group key.
                name: g.group_key.clone(),
                member_count: g.members.len(),
                gf_bytes: gf,
            })
        })
        .collect();
    rows.sort_by(|a, b| b.gf_bytes.cmp(&a.gf_bytes).then_with(|| a.name.cmp(&b.name)));
    rows
}

/// Human GF: GiB when ≥ 1 GiB, else MiB.
pub fn format_gf_human(bytes: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    const GIB: u64 = 1024 * 1024 * 1024;
    if bytes >= GIB {
        let gib = bytes as f64 / GIB as f64;
        format!("{gib:.2} GiB")
    } else {
        let mib = bytes as f64 / MIB as f64;
        format!("{mib:.0} MiB")
    }
}

/// Stable smoke line: `name\tmembers\thuman_gf`.
pub fn format_row(row: &ListRow) -> String {
    format!(
        "{}\t{}\t{}",
        row.name,
        row.member_count,
        format_gf_human(row.gf_bytes)
    )
}

/// Live enumerate → group → filter → print.
pub fn run_list() {
    let mut cache = PathCache::new();
    let procs = match enumerate_processes_with_cache(&mut cache) {
        Ok(procs) => procs,
        Err(status) => {
            eprintln!("error: NtQuerySystemInformation failed ({status:?})");
            std::process::exit(1);
        }
    };
    let groups = group_processes(&procs);
    let rows = visible_rows(&groups);
    for row in &rows {
        println!("{}", format_row(row));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramjob_core::grouper::GroupMember;

    fn group(key: &str, members: Vec<(u32, u64)>) -> AppGroup {
        AppGroup {
            group_key: key.into(),
            members: members
                .into_iter()
                .map(|(pid, private_working_set_bytes)| GroupMember {
                    pid,
                    create_time: 1,
                    private_working_set_bytes,
                })
                .collect(),
        }
    }

    #[test]
    fn visible_rows_filters_below_50_mb_and_sorts_desc() {
        let mb = 1024 * 1024;
        let groups = vec![
            group(r"c:\apps\small", vec![(1, 10 * mb)]),
            group(r"c:\apps\medium", vec![(2, 80 * mb)]),
            group(r"c:\apps\large", vec![(3, 200 * mb), (4, 50 * mb)]),
            group(r"c:\apps\edge", vec![(5, 50 * mb)]),
        ];
        let rows = visible_rows(&groups);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].name, r"c:\apps\large");
        assert_eq!(rows[0].gf_bytes, 250 * mb);
        assert_eq!(rows[0].member_count, 2);
        assert_eq!(rows[1].name, r"c:\apps\medium");
        assert_eq!(rows[2].name, r"c:\apps\edge");
        assert_eq!(rows[2].gf_bytes, 50 * mb);
    }

    #[test]
    fn format_gf_human_mib_and_gib() {
        assert_eq!(format_gf_human(80 * 1024 * 1024), "80 MiB");
        assert_eq!(format_gf_human(1536 * 1024 * 1024), "1.50 GiB");
    }

    #[test]
    fn format_row_is_tab_separated() {
        let row = ListRow {
            name: r"c:\program files\brave-browser".into(),
            member_count: 14,
            gf_bytes: 900 * 1024 * 1024,
        };
        assert_eq!(
            format_row(&row),
            "c:\\program files\\brave-browser\t14\t900 MiB"
        );
    }

    /// Live smoke: enumerate → group → footprint on this machine.
    #[test]
    fn live_enumerate_group_footprint_smoke() {
        let mut cache = PathCache::new();
        let procs = enumerate_processes_with_cache(&mut cache).expect("NtQSI");
        let groups = group_processes(&procs);
        let rows = visible_rows(&groups);
        if rows.is_empty() {
            // Quiescent / filtered machine: no group ≥ 50 MB is possible.
            eprintln!("live smoke: 0 visible groups (skip assert)");
            return;
        }
        assert!(rows[0].gf_bytes >= 50 * 1024 * 1024);
        assert!(rows.windows(2).all(|w| w[0].gf_bytes >= w[1].gf_bytes));
    }
}
