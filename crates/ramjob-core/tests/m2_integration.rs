//! M2 Task 8 — armed + over-cap hog → SoftTrim once, then rate-limit.

use std::path::PathBuf;
use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

use ramjob_core::config::{GroupConfig, RamjobConfig};
use ramjob_core::gate::find_group_for_pid;
use ramjob_core::grouper::{group_processes, AppGroup, GroupMember};
use ramjob_core::policy::SystemArm;
use ramjob_core::runtime::Runtime;
use ramjob_core::scanner::{enumerate_processes_with_cache, PathCache};

fn workspace_debug_bin(name: &str) -> PathBuf {
    let mut exe = name.to_string();
    if cfg!(windows) && !exe.ends_with(".exe") {
        exe.push_str(".exe");
    }
    if let Ok(td) = std::env::var("CARGO_TARGET_DIR") {
        return PathBuf::from(td).join("debug").join(&exe);
    }
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // crates
    p.pop(); // workspace root
    p.join("target").join("debug").join(exe)
}

fn spawn_hog(mb: u32, hold_secs: u64) -> Child {
    let bin = workspace_debug_bin("ramjob-hog");
    assert!(
        bin.exists(),
        "missing {}; run `cargo build -p ramjob-hog` first",
        bin.display()
    );
    Command::new(&bin)
        .args([
            "--mode",
            "forget",
            "--mb",
            &mb.to_string(),
            "--hold-secs",
            &hold_secs.to_string(),
        ])
        .spawn()
        .unwrap_or_else(|e| panic!("spawn {}: {e}", bin.display()))
}

fn wait_for_hog_group(pid: u32) -> AppGroup {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let mut cache = PathCache::new();
        let procs = enumerate_processes_with_cache(&mut cache).expect("enumerate");
        let groups = group_processes(&procs);
        let group = find_group_for_pid(&groups, pid).cloned().or_else(|| {
            procs.iter().find(|p| p.pid == pid).map(|p| AppGroup {
                group_key: "image:ramjob-hog".into(),
                members: vec![GroupMember {
                    pid: p.pid,
                    create_time: p.create_time,
                    private_working_set_bytes: p.private_working_set_bytes,
                    private_usage_bytes: p.private_usage_bytes,
                }],
            })
        });
        if let Some(g) = group {
            let gf: u64 = g.members.iter().map(|m| m.private_working_set_bytes).sum();
            // Prefer private WS; fall back to process working_set if private is still warming.
            let ws = procs
                .iter()
                .find(|p| p.pid == pid)
                .map(|p| p.working_set_bytes)
                .unwrap_or(0);
            if gf > 1_000_000 || ws > 8_000_000 {
                let mut g = g;
                if gf <= 1_000_000 {
                    // SoftTrim uses live sample_private_ws; member bytes only drive FSM gf.
                    g.members[0].private_working_set_bytes = ws.max(gf);
                }
                return g;
            }
        }
        if Instant::now() >= deadline {
            let detail = procs
                .iter()
                .find(|p| p.pid == pid)
                .map(|p| {
                    format!(
                        "image={} private={} ws={}",
                        p.image_name, p.private_working_set_bytes, p.working_set_bytes
                    )
                })
                .unwrap_or_else(|| "missing".into());
            panic!("timed out waiting for hog pid={pid} ({detail})");
        }
        thread::sleep(Duration::from_millis(250));
    }
}

#[test]
fn armed_over_cap_trims_once_then_rate_limits() {
    let mut child = spawn_hog(32, 120);
    let pid = child.id();

    let result = std::panic::catch_unwind(|| {
        let hog = wait_for_hog_group(pid);

        let cfg = RamjobConfig {
            version: 2,
            runaway_multiplier: 3.0,
            overall_limit_bytes: 0,
            groups: vec![GroupConfig {
                key: hog.group_key.clone(),
                cap_bytes: 1_000_000, // 1 MiB — below hog GF
                always_enforce: false,
            }],
            pause_all: false,
        };
        let mut rt = Runtime::new_inert();
        let now = Instant::now();
        let apps = vec![hog];

        let out1 = rt
            .tick_with_groups(&cfg, SystemArm::Armed, &apps, now)
            .expect("first tick");
        assert!(
            out1.trims_attempted >= 1,
            "expected SoftTrim attempt, got {}",
            out1.trims_attempted
        );

        let out2 = rt
            .tick_with_groups(&cfg, SystemArm::Armed, &apps, now + Duration::from_secs(1))
            .expect("second tick");
        assert_eq!(
            out2.trims_attempted, 0,
            "second tick within 20s must be rate-limited"
        );
    });

    let _ = child.kill();
    let _ = child.wait();
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}
