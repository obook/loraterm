//! history.rs
//! Message history on disk: one JSON record per line, appended and synced before a message
//! is shown, so that a message fetched from the radio is never lost.
//!
//! Author: O. Booklage
//! Date: October 2026
//! Licence: MIT

use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Conversation key of the public channel; other conversations use the contact's prefix in hex.
pub const PUBLIC: &str = "public";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Msg {
    /// Unix seconds.
    pub ts: i64,
    pub conv: String,
    pub from: String,
    pub text: String,
    pub outgoing: bool,
    /// Expected ack tag in hex, for a direct message we sent.
    pub ack: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Record {
    Msg(Msg),
    Ack { tag: String },
}

/// `$XDG_DATA_HOME/meshcore-m5/history.jsonl`, or `~/.local/share/...` without the variable.
pub fn default_path() -> PathBuf {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share")
        });
    data.join("meshcore-m5/history.jsonl")
}

/// Add one record at the end of the file and sync it to disk before returning.
pub fn append(path: &Path, rec: &Record) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(path)?;
    let mut line = serde_json::to_string(rec)?;
    line.push('\n');
    // A line cut by a crash must not swallow this record.
    if file.metadata()?.len() > 0 {
        let mut last = [0u8];
        file.seek(SeekFrom::End(-1))?;
        file.read_exact(&mut last)?;
        if last[0] != b'\n' {
            line.insert(0, '\n');
        }
    }
    file.write_all(line.as_bytes())?;
    file.sync_data()
}

/// Every readable record of the file; a missing file is an empty history.
pub fn load(path: &Path) -> std::io::Result<Vec<Record>> {
    // Bytes, not a String: a line cut inside a multi-byte character must only lose that line.
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e),
    };
    Ok(bytes
        .split(|&b| b == b'\n')
        .filter(|l| !l.is_empty())
        .filter_map(|l| match serde_json::from_slice(l) {
            Ok(rec) => Some(rec),
            Err(e) => {
                tracing::warn!("skipping unreadable history line: {e}");
                None
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn msg(text: &str) -> Record {
        Record::Msg(Msg {
            ts: 1_700_000_000,
            conv: PUBLIC.into(),
            from: "Alice".into(),
            text: text.into(),
            outgoing: false,
            ack: None,
        })
    }

    #[test]
    fn round_trip_keeps_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.jsonl");
        let records = [
            msg("one"),
            Record::Ack {
                tag: "01020304".into(),
            },
            msg("two"),
        ];
        for r in &records {
            append(&path, r).unwrap();
        }
        assert_eq!(load(&path).unwrap(), records);
    }

    #[test]
    fn missing_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(&dir.path().join("none.jsonl")).unwrap(), vec![]);
    }

    #[test]
    fn truncated_last_line_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.jsonl");
        append(&path, &msg("one")).unwrap();
        append(&path, &msg("two")).unwrap();
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        write!(f, "{{\"kind\":\"msg\",\"ts\":").unwrap();
        assert_eq!(load(&path).unwrap(), vec![msg("one"), msg("two")]);
    }

    #[test]
    fn append_after_truncated_line_is_not_lost() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.jsonl");
        append(&path, &msg("one")).unwrap();
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        write!(f, "{{\"kind\":\"msg\",\"ts\":").unwrap();
        append(&path, &msg("two")).unwrap();
        assert_eq!(load(&path).unwrap(), vec![msg("one"), msg("two")]);
    }

    #[test]
    fn line_cut_inside_an_accent_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.jsonl");
        append(&path, &msg("one")).unwrap();
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        f.write_all(
            b"{\"kind\":\"msg\",\"ts\":1,\"conv\":\"public\",\"from\":\"x\",\"text\":\"caf\xC3",
        )
        .unwrap();
        append(&path, &msg("two")).unwrap();
        assert_eq!(load(&path).unwrap(), vec![msg("one"), msg("two")]);
    }

    #[test]
    fn creates_parent_dir() {
        let dir = tempfile::tempdir().unwrap();
        append(&dir.path().join("a/b/history.jsonl"), &msg("one")).unwrap();
    }
}
