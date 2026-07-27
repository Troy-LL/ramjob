//! `ramjob run` daemon loop (M2).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ramjob_core::config::{load_config_file, parse_config, RamjobConfig, CONFIG_VERSION};
use ramjob_core::pressure::{SimulatedPressure, WinPressure};
use ramjob_core::runtime::Runtime;

pub struct RunArgs {
    pub config: PathBuf,
    pub once: bool,
    pub simulate_armed: bool,
}

pub fn parse_run_args(args: impl IntoIterator<Item = String>) -> Result<RunArgs, String> {
    let mut config = default_config_path();
    let mut once = false;
    let mut simulate_armed = false;
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => return Err("__help__".into()),
            "--once" => once = true,
            "--simulate-armed" => simulate_armed = true,
            "--config" => {
                let p = it
                    .next()
                    .ok_or_else(|| "missing value for --config".to_string())?;
                config = PathBuf::from(p);
            }
            other => return Err(format!("unexpected argument '{other}'")),
        }
    }
    Ok(RunArgs {
        config,
        once,
        simulate_armed,
    })
}

pub fn print_run_help() {
    println!("Usage: ramjob run [OPTIONS]");
    println!();
    println!("Run the M2 policy loop (pressure + per-group soft-trim FSM).");
    println!();
    println!("Options:");
    println!("  --config <path>     Config TOML (default: %APPDATA%\\RamJob\\config.toml)");
    println!("  --once              Single tick then exit");
    println!("  --simulate-armed    Force Armed for the tick(s) (skip OS dwell)");
    println!("  -h, --help          Print help");
}

fn default_config_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("RamJob").join("config.toml")
}

fn ensure_config(path: &Path) -> Result<RamjobConfig, String> {
    if path.exists() {
        return load_config_file(path);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create config dir {}: {e}", parent.display()))?;
    }
    let template = format!(
        "version = {CONFIG_VERSION}\nrunaway_multiplier = 3.0\n# [[group]]\n# key = \"...\"\n# cap_bytes = 0\n# always_enforce = false\n"
    );
    std::fs::write(path, &template)
        .map_err(|e| format!("write config {}: {e}", path.display()))?;
    eprintln!("wrote empty config template at {}", path.display());
    parse_config(&template)
}

pub fn run_daemon(args: RunArgs) {
    let cfg = match ensure_config(&args.config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };
    let mut rt = Runtime::from_config(cfg);
    if args.simulate_armed {
        rt.force_arm_for_test();
    }

    let mut sim = SimulatedPressure {
        low_memory: true,
        high_memory: false,
        hard_faults_per_sec: 40.0,
    };
    let mut win = if args.simulate_armed {
        None
    } else {
        match WinPressure::new() {
            Ok(w) => Some(w),
            Err(e) => {
                eprintln!("warning: WinPressure unavailable ({e}); using simulated Disarmed-leaning source");
                None
            }
        }
    };

    loop {
        let now = Instant::now();
        let result = if let Some(w) = win.as_mut() {
            rt.tick(w, now)
        } else {
            if !args.simulate_armed {
                sim.low_memory = false;
                sim.hard_faults_per_sec = 0.0;
                sim.high_memory = true;
            }
            rt.tick(&mut sim, now)
        };
        match result {
            Ok(out) => {
                println!(
                    "tick system={:?} trims={} diag_lines={}",
                    out.system,
                    out.trims_attempted,
                    rt.diagnostics.lines().len()
                );
            }
            Err(e) => eprintln!("tick error: {e}"),
        }
        if args.once {
            break;
        }
        let sleep = match rt.policy.arm {
            ramjob_core::policy::SystemArm::Armed => Duration::from_secs(1),
            ramjob_core::policy::SystemArm::Disarmed => Duration::from_secs(15),
        };
        std::thread::sleep(sleep);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flags() {
        let a = parse_run_args([
            "--once".into(),
            "--simulate-armed".into(),
            "--config".into(),
            "C:\\tmp\\c.toml".into(),
        ])
        .unwrap();
        assert!(a.once);
        assert!(a.simulate_armed);
        assert_eq!(a.config, PathBuf::from("C:\\tmp\\c.toml"));
    }
}
