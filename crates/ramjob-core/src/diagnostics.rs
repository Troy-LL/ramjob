//! Fixed-size diagnostics ring (SPEC §8.1 / M2).

use std::collections::VecDeque;

const CAPACITY: usize = 1024;

#[derive(Debug, Default)]
pub struct DiagnosticsRing {
    buf: VecDeque<String>,
}

impl DiagnosticsRing {
    pub fn new() -> Self {
        Self {
            buf: VecDeque::with_capacity(CAPACITY),
        }
    }

    pub fn push(&mut self, line: impl Into<String>) {
        if self.buf.len() == CAPACITY {
            self.buf.pop_front();
        }
        self.buf.push_back(line.into());
    }

    pub fn lines(&self) -> Vec<&str> {
        self.buf.iter().map(String::as_str).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_oldest_when_over_capacity() {
        let mut r = DiagnosticsRing::new();
        for i in 0..1025 {
            r.push(format!("line-{i}"));
        }
        let lines = r.lines();
        assert_eq!(lines.len(), 1024);
        assert_eq!(lines[0], "line-1");
        assert_eq!(lines[1023], "line-1024");
    }
}
