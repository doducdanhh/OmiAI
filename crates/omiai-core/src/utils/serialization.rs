//! Serde-based (de)serialization helpers for persisting knowledge
//! graphs, causal models, and evolved programs.

use std::fs;
use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;

/// Pretty-print JSON.
pub fn to_json_pretty<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(value)
}

/// Parse JSON.
pub fn from_json<T: DeserializeOwned>(s: &str) -> Result<T, serde_json::Error> {
    serde_json::from_str(s)
}

/// Write JSON to a file.
pub fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let s = to_json_pretty(value).map_err(|e| e.to_string())?;
    fs::write(path, s).map_err(|e| e.to_string())
}

/// Read JSON from a file.
pub fn load_json<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let s = fs::read_to_string(path).map_err(|e| e.to_string())?;
    from_json(&s).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[test]
    fn roundtrip() {
        let p = Point { x: 1, y: 2 };
        let s = to_json_pretty(&p).unwrap();
        let q: Point = from_json(&s).unwrap();
        assert_eq!(p, q);
    }
}
