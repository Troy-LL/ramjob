mod gate;
mod list;
mod run;

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None | Some("list") => list::run_list(),
        Some("gate") => match gate::parse_gate_args(args) {
            Ok(ga) => gate::run_gate(ga),
            Err(msg) => {
                eprintln!("error: {msg}");
                gate::print_gate_help();
                std::process::exit(2);
            }
        },
        Some("run") => match run::parse_run_args(args) {
            Ok(ra) => run::run_daemon(ra),
            Err(msg) if msg == "__help__" => run::print_run_help(),
            Err(msg) => {
                eprintln!("error: {msg}");
                run::print_run_help();
                std::process::exit(2);
            }
        },
        Some("-h") | Some("--help") => print_usage(),
        Some(other) => {
            eprintln!("error: unexpected argument '{other}'");
            print_usage();
            std::process::exit(2);
        }
    }
}

fn print_usage() {
    println!("Usage: ramjob [COMMAND] [OPTIONS]");
    println!();
    println!("Commands:");
    println!("  list          Enumerate apps and print Group Footprint (default)");
    println!("  gate          Run M1 compression gate (Ry_bench / Ry_live)");
    println!("  run           M2 policy loop (pressure + FSM soft trim)");
    println!();
    println!("Options:");
    println!("  -h, --help    Print help");
    println!();
    println!("Run `ramjob gate --help` or `ramjob run --help` for command options.");
}
