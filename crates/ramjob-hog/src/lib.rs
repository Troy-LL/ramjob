//! Synthetic memory hog for trim-yield ground truth (SPEC §9.1).
//!
//! Modes:
//! - `forget`: allocate + touch once, then idle (trim-friendly)
//! - `loop`: allocate + keep re-touching (thrash-prone)
//! - `sawtooth`: allocate / free cycles (oscillating WS)

use std::thread;
use std::time::{Duration, Instant};

const PAGE: usize = 4096;
const SAWTOOTH_HALF_MS: u64 = 500;

/// Allocation / touch pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Touch once, then hold without re-touching.
    Forget,
    /// Continuously re-touch pages while holding.
    Loop,
    /// Allocate, hold briefly, free, repeat until hold ends.
    Sawtooth,
}

impl Mode {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "forget" => Some(Self::Forget),
            "loop" => Some(Self::Loop),
            "sawtooth" => Some(Self::Sawtooth),
            _ => None,
        }
    }
}

/// How long the pattern runs after the initial allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hold {
    /// Finite hold (0 = return as soon as the mode's first step finishes).
    Secs(u64),
    /// Run until the process is killed.
    Forever,
}

/// Parsed hog configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HogConfig {
    pub mode: Mode,
    pub mb: usize,
    pub hold: Hold,
}

/// Allocate `mb` MiB and touch every page so the OS commits physical pages.
pub fn alloc_and_touch(mb: usize) -> Vec<u8> {
    let size = mb.saturating_mul(1024 * 1024);
    let mut buf = vec![0u8; size];
    touch_pages(&mut buf);
    buf
}

/// Write one byte per page so pages are resident.
pub fn touch_pages(buf: &mut [u8]) {
    let mut i = 0;
    while i < buf.len() {
        buf[i] = buf[i].wrapping_add(1);
        i += PAGE;
    }
}

fn deadline(hold: Hold) -> Option<Instant> {
    match hold {
        Hold::Forever => None,
        Hold::Secs(s) => Some(Instant::now() + Duration::from_secs(s)),
    }
}

fn still_holding(until: Option<Instant>) -> bool {
    match until {
        None => true,
        Some(t) => Instant::now() < t,
    }
}

fn sleep_while_holding(until: Option<Instant>, slice: Duration) {
    match until {
        None => thread::sleep(slice),
        Some(t) => {
            let remaining = t.saturating_duration_since(Instant::now());
            if !remaining.is_zero() {
                thread::sleep(remaining.min(slice));
            }
        }
    }
}

/// Run the configured hog pattern. Returns when `hold` expires (never if Forever).
pub fn run(config: HogConfig) {
    match config.mode {
        Mode::Forget => run_forget(config.mb, config.hold),
        Mode::Loop => run_loop(config.mb, config.hold),
        Mode::Sawtooth => run_sawtooth(config.mb, config.hold),
    }
}

fn run_forget(mb: usize, hold: Hold) {
    let buf = alloc_and_touch(mb);
    let until = deadline(hold);
    while still_holding(until) {
        sleep_while_holding(until, Duration::from_millis(100));
    }
    std::hint::black_box(&buf);
}

fn run_loop(mb: usize, hold: Hold) {
    let mut buf = alloc_and_touch(mb);
    let until = deadline(hold);
    while still_holding(until) {
        touch_pages(&mut buf);
        sleep_while_holding(until, Duration::from_millis(1));
    }
    std::hint::black_box(&buf);
}

fn run_sawtooth(mb: usize, hold: Hold) {
    let until = deadline(hold);
    let half = Duration::from_millis(SAWTOOTH_HALF_MS);
    while still_holding(until) {
        let buf = alloc_and_touch(mb);
        sleep_while_holding(until, half);
        drop(buf);
        if !still_holding(until) {
            break;
        }
        sleep_while_holding(until, half);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forget_allocates_without_panic() {
        run(HogConfig {
            mode: Mode::Forget,
            mb: 8,
            hold: Hold::Secs(0),
        });
    }

    #[test]
    fn alloc_and_touch_size_matches_mb() {
        let buf = alloc_and_touch(2);
        assert_eq!(buf.len(), 2 * 1024 * 1024);
        assert_ne!(buf[0], 0);
    }

    #[test]
    fn mode_parse_accepts_known_names() {
        assert_eq!(Mode::parse("forget"), Some(Mode::Forget));
        assert_eq!(Mode::parse("loop"), Some(Mode::Loop));
        assert_eq!(Mode::parse("sawtooth"), Some(Mode::Sawtooth));
        assert_eq!(Mode::parse("nope"), None);
    }

    #[test]
    fn loop_and_sawtooth_zero_hold_without_panic() {
        run(HogConfig {
            mode: Mode::Loop,
            mb: 1,
            hold: Hold::Secs(0),
        });
        run(HogConfig {
            mode: Mode::Sawtooth,
            mb: 1,
            hold: Hold::Secs(0),
        });
    }
}
