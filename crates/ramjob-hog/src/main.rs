use ramjob_hog::{run, Hold, HogConfig, Mode};

fn main() {
    match parse_args(std::env::args().skip(1)) {
        Ok(config) => run(config),
        Err(msg) => {
            eprintln!("error: {msg}");
            print_usage();
            std::process::exit(2);
        }
    }
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<HogConfig, String> {
    let mut mode = None;
    let mut mb = None;
    let mut hold = Hold::Forever;

    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            "--mode" => {
                let v = it.next().ok_or_else(|| "missing value for --mode".to_string())?;
                mode = Some(Mode::parse(&v).ok_or_else(|| {
                    format!("invalid --mode '{v}' (expected forget|loop|sawtooth)")
                })?);
            }
            "--mb" => {
                let v = it.next().ok_or_else(|| "missing value for --mb".to_string())?;
                let n: usize = v
                    .parse()
                    .map_err(|_| format!("invalid --mb '{v}' (expected positive integer)"))?;
                if n == 0 {
                    return Err("--mb must be > 0".into());
                }
                mb = Some(n);
            }
            "--hold-secs" => {
                let v = it
                    .next()
                    .ok_or_else(|| "missing value for --hold-secs".to_string())?;
                let n: u64 = v
                    .parse()
                    .map_err(|_| format!("invalid --hold-secs '{v}' (expected integer)"))?;
                hold = Hold::Secs(n);
            }
            other => return Err(format!("unexpected argument '{other}'")),
        }
    }

    let mode = mode.ok_or_else(|| "missing required --mode".to_string())?;
    let mb = mb.ok_or_else(|| "missing required --mb".to_string())?;
    Ok(HogConfig { mode, mb, hold })
}

fn print_usage() {
    println!("Usage: ramjob-hog --mode <forget|loop|sawtooth> --mb <n> [--hold-secs <n>]");
    println!();
    println!("Modes:");
    println!("  forget     Allocate and touch once, then idle (trim-friendly)");
    println!("  loop       Allocate and keep re-touching (thrash-prone)");
    println!("  sawtooth   Allocate / free cycles (oscillating working set)");
    println!();
    println!("Options:");
    println!("  --mode         Allocation pattern (required)");
    println!("  --mb           Size to allocate in mebibytes (required)");
    println!("  --hold-secs    How long to run the pattern (default: until killed)");
    println!("  -h, --help     Print help");
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn parses_forget_with_hold() {
        let cfg = parse_args(
            ["--mode", "forget", "--mb", "64", "--hold-secs", "1"]
                .into_iter()
                .map(String::from),
        )
        .unwrap();
        assert_eq!(
            cfg,
            HogConfig {
                mode: Mode::Forget,
                mb: 64,
                hold: Hold::Secs(1),
            }
        );
    }

    #[test]
    fn hold_defaults_to_forever() {
        let cfg = parse_args(["--mode", "loop", "--mb", "8"].into_iter().map(String::from))
            .unwrap();
        assert_eq!(cfg.hold, Hold::Forever);
    }

    #[test]
    fn rejects_missing_mode() {
        assert!(parse_args(["--mb", "8"].into_iter().map(String::from)).is_err());
    }
}
