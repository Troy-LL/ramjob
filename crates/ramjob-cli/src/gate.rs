//! `ramjob gate` — M1 compression gate harness.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ramjob_core::gate::{run_live_gate, GateTarget, GATE_SETTLE};

const DEFAULT_OUT: &str = ".superpowers/sdd/m1-gate-results.md";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateArgs {
    pub target: GateTarget,
    pub out: PathBuf,
    pub settle: Duration,
}

pub fn parse_gate_args(args: impl IntoIterator<Item = String>) -> Result<GateArgs, String> {
    let mut image = None;
    let mut pid = None;
    let mut out = PathBuf::from(DEFAULT_OUT);
    let mut settle = GATE_SETTLE;

    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_gate_help();
                std::process::exit(0);
            }
            "--image" => {
                let v = it
                    .next()
                    .ok_or_else(|| "missing value for --image".to_string())?;
                image = Some(v);
            }
            "--pid" => {
                let v = it
                    .next()
                    .ok_or_else(|| "missing value for --pid".to_string())?;
                let n: u32 = v
                    .parse()
                    .map_err(|_| format!("invalid --pid '{v}' (expected unsigned integer)"))?;
                pid = Some(n);
            }
            "--out" => {
                let v = it
                    .next()
                    .ok_or_else(|| "missing value for --out".to_string())?;
                out = PathBuf::from(v);
            }
            "--wait-secs" => {
                let v = it
                    .next()
                    .ok_or_else(|| "missing value for --wait-secs".to_string())?;
                let n: u64 = v
                    .parse()
                    .map_err(|_| format!("invalid --wait-secs '{v}' (expected integer)"))?;
                settle = Duration::from_secs(n);
            }
            other => return Err(format!("unexpected argument '{other}'")),
        }
    }

    let target = match (image, pid) {
        (Some(img), None) => GateTarget::Image(img),
        (None, Some(p)) => GateTarget::Pid(p),
        (Some(_), Some(_)) => return Err("use either --image or --pid, not both".into()),
        (None, None) => return Err("missing required --image <name> or --pid <n>".into()),
    };

    Ok(GateArgs {
        target,
        out,
        settle,
    })
}

pub fn print_gate_help() {
    println!("Usage: ramjob gate (--image <name> | --pid <n>) [OPTIONS]");
    println!();
    println!("Run the M1 compression gate (SPEC §2.3 / §9.2) against one group.");
    println!("Prints Ry_bench, Ry_live, and classification; writes results markdown.");
    println!();
    println!("Target (one required):");
    println!("  --image <name>   Group containing a process image (e.g. ramjob-hog)");
    println!("  --pid <n>        Group containing this PID");
    println!();
    println!("Options:");
    println!("  --out <path>     Results markdown (default: {DEFAULT_OUT})");
    println!("  --wait-secs <n>  Post-trim settle before post-sample (default: 3)");
    println!("  -h, --help       Print help");
}

pub fn run_gate(args: GateArgs) {
    let label = match &args.target {
        GateTarget::Image(name) => format!("image '{name}'"),
        GateTarget::Pid(pid) => format!("pid {pid}"),
    };
    eprintln!(
        "gate: resolving {label} from pre-sample, settle {}s …",
        args.settle.as_secs()
    );

    let measurement = match run_live_gate(args.target, args.settle) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: gate failed: {e}");
            std::process::exit(1);
        }
    };

    println!(
        "Ry_bench:\t{}",
        measurement
            .ry_bench
            .map(|v| format!("{v:.4}"))
            .unwrap_or_else(|| "n/a".into())
    );
    println!(
        "Ry_live:\t{}",
        measurement
            .ry_live
            .map(|v| format!("{v:.4}"))
            .unwrap_or_else(|| "n/a".into())
    );
    println!(
        "Classification:\t{}",
        measurement
            .verdict
            .map(|v| v.as_str().to_string())
            .unwrap_or_else(|| "n/a".into())
    );
    println!("trimmed_pids:\t{:?}", measurement.trimmed_pids);
    println!(
        "ΔGF:\t{} bytes",
        measurement.gf0 as i64 - measurement.gf1 as i64
    );

    if let Err(e) = write_results(&args.out, &measurement.to_markdown()) {
        eprintln!("error: writing {}: {e}", args.out.display());
        std::process::exit(1);
    }
    eprintln!("wrote {}", args.out.display());
}

fn write_results(path: &Path, body: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_image_and_defaults() {
        let args = parse_gate_args(["--image".into(), "ramjob-hog".into()]).unwrap();
        assert_eq!(args.target, GateTarget::Image("ramjob-hog".into()));
        assert_eq!(args.out, PathBuf::from(DEFAULT_OUT));
        assert_eq!(args.settle, GATE_SETTLE);
    }

    #[test]
    fn parse_pid_out_wait() {
        let args = parse_gate_args([
            "--pid".into(),
            "1234".into(),
            "--out".into(),
            "tmp/out.md".into(),
            "--wait-secs".into(),
            "1".into(),
        ])
        .unwrap();
        assert_eq!(args.target, GateTarget::Pid(1234));
        assert_eq!(args.out, PathBuf::from("tmp/out.md"));
        assert_eq!(args.settle, Duration::from_secs(1));
    }

    #[test]
    fn rejects_both_targets() {
        assert!(parse_gate_args([
            "--image".into(),
            "hog".into(),
            "--pid".into(),
            "1".into()
        ])
        .is_err());
    }

    #[test]
    fn rejects_missing_target() {
        assert!(parse_gate_args(Vec::<String>::new()).is_err());
    }
}
