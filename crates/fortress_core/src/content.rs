use std::path::{Path, PathBuf};

use crate::events::Event;

#[derive(Debug, thiserror::Error)]
pub enum ContentError {
    #[error("content directory not found (tried $FORTRESS_CONTENT, ./content/events, workspace-relative)")]
    DirNotFound,
    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse {path}: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
}

pub fn default_content_dir() -> Result<PathBuf, ContentError> {
    if let Ok(dir) = std::env::var("FORTRESS_CONTENT") {
        let p = PathBuf::from(dir);
        if p.is_dir() {
            return Ok(p);
        }
    }
    let cwd = PathBuf::from("content/events");
    if cwd.is_dir() {
        return Ok(cwd);
    }
    // A shipped binary carries `content/` next to the executable; this finds it
    // however (and from wherever) the game was launched.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let beside_exe = exe_dir.join("content/events");
            if beside_exe.is_dir() {
                return Ok(beside_exe);
            }
        }
    }
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/events");
    if workspace.is_dir() {
        return Ok(workspace);
    }
    Err(ContentError::DirNotFound)
}

pub fn load_events(dir: &Path) -> Result<Vec<Event>, ContentError> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|source| ContentError::Io { path: dir.to_path_buf(), source })?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();

    let mut events = Vec::new();
    for path in paths {
        let json = std::fs::read_to_string(&path)
            .map_err(|source| ContentError::Io { path: path.clone(), source })?;
        let batch: Vec<Event> = serde_json::from_str(&json)
            .map_err(|source| ContentError::Parse { path: path.clone(), source })?;
        events.extend(batch);
    }
    Ok(events)
}
