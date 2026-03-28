use super::{LocalFileStat, LocalFsState, PATH_GRANT_TTL_SECONDS};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AccessMode {
    Read,
    Write,
    ReadWrite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PathGrantSource {
    AllowedRootsOnly,
    TrustedDialog,
}

#[derive(Debug)]
pub(super) struct PathGrant {
    pub target_path: PathBuf,
    pub mode: AccessMode,
    pub expires_at: Instant,
}

impl AccessMode {
    pub fn from_str(raw: &str) -> Result<Self, String> {
        match raw {
            "read" => Ok(Self::Read),
            "write" => Ok(Self::Write),
            "read_write" => Ok(Self::ReadWrite),
            other => Err(format!("Unsupported access mode: {}", other)),
        }
    }

    pub fn allows_read(self) -> bool {
        matches!(self, Self::Read | Self::ReadWrite)
    }

    pub fn allows_write(self) -> bool {
        matches!(self, Self::Write | Self::ReadWrite)
    }
}

pub(super) fn normalize_path(path: &str) -> PathBuf {
    PathBuf::from(path)
}

pub(super) fn default_allowed_roots() -> Vec<PathBuf> {
    Vec::new()
}

pub(super) fn canonical_target_path(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!(
            "Local path must be absolute: {}",
            path.to_string_lossy()
        ));
    }

    if path.exists() {
        return fs::canonicalize(path).map_err(|e| e.to_string());
    }

    let parent = path.parent().ok_or_else(|| {
        format!(
            "Path is outside allowed roots or has no existing parent: {}",
            path.display()
        )
    })?;
    let canonical_parent = fs::canonicalize(parent).map_err(|e| e.to_string())?;
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("Path has no file name: {}", path.to_string_lossy()))?;
    Ok(canonical_parent.join(file_name))
}

pub(super) fn path_access_anchor(path: &Path) -> Result<PathBuf, String> {
    let mut current = Some(path);
    while let Some(candidate) = current {
        if candidate.exists() {
            return fs::canonicalize(candidate).map_err(|e| e.to_string());
        }
        current = candidate.parent();
    }

    Err(format!(
        "Path is outside allowed roots or has no existing parent: {}",
        path.display()
    ))
}

pub(super) fn ensure_path_in_allowed_roots(
    state: &LocalFsState,
    path: &Path,
) -> Result<(), String> {
    let anchor = path_access_anchor(path)?;
    if state
        .allowed_roots
        .iter()
        .any(|root| anchor.starts_with(root))
    {
        return Ok(());
    }

    Err(format!(
        "Access to local path is denied: {}",
        path.to_string_lossy()
    ))
}

pub(super) fn cleanup_expired_grants(state: &LocalFsState) -> Result<(), String> {
    let now = Instant::now();
    let mut grants = state.path_grants.lock().map_err(|e| e.to_string())?;
    grants.retain(|_, grant| grant.expires_at > now);
    Ok(())
}

pub(super) fn issue_path_grant(
    state: &LocalFsState,
    path: &Path,
    mode: AccessMode,
    source: PathGrantSource,
) -> Result<String, String> {
    if matches!(source, PathGrantSource::AllowedRootsOnly) {
        ensure_path_in_allowed_roots(state, path)?;
    }
    cleanup_expired_grants(state)?;
    let token = Uuid::new_v4().to_string();
    let canonical = canonical_target_path(path)?;
    let grant = PathGrant {
        target_path: canonical,
        mode,
        expires_at: Instant::now() + Duration::from_secs(PATH_GRANT_TTL_SECONDS),
    };

    let mut grants = state.path_grants.lock().map_err(|e| e.to_string())?;
    grants.insert(token.clone(), grant);
    Ok(token)
}

pub(crate) fn issue_dialog_path_grant(
    state: &LocalFsState,
    path: &Path,
    mode_raw: &str,
) -> Result<String, String> {
    let mode = AccessMode::from_str(mode_raw.trim())?;
    issue_path_grant(state, path, mode, PathGrantSource::TrustedDialog)
}

pub(super) fn consume_path_grant(
    state: &LocalFsState,
    access_token: &str,
    path: &Path,
    required_mode: AccessMode,
) -> Result<Option<PathBuf>, String> {
    cleanup_expired_grants(state)?;
    let canonical = canonical_target_path(path)?;
    let mut grants = state.path_grants.lock().map_err(|e| e.to_string())?;
    let Some(grant) = grants.get(access_token) else {
        return Ok(None);
    };

    if grant.target_path != canonical {
        return Ok(None);
    }

    let mode_ok = match required_mode {
        AccessMode::Read => grant.mode.allows_read(),
        AccessMode::Write => grant.mode.allows_write(),
        AccessMode::ReadWrite => grant.mode.allows_read() && grant.mode.allows_write(),
    };
    if !mode_ok {
        return Ok(None);
    }

    grants.remove(access_token);
    Ok(Some(canonical))
}

pub(super) fn authorize_path(
    state: &LocalFsState,
    path: &Path,
    required_mode: AccessMode,
    access_token: Option<&str>,
) -> Result<PathBuf, String> {
    if ensure_path_in_allowed_roots(state, path).is_ok() {
        return canonical_target_path(path);
    }

    if let Some(token) = access_token {
        if let Some(canonical) = consume_path_grant(state, token, path, required_mode)? {
            return Ok(canonical);
        }
    }

    Err(format!(
        "Access to local path is denied: {}",
        path.to_string_lossy()
    ))
}

pub(super) fn stat_with_optional_access(
    state: &LocalFsState,
    path: &Path,
    access_mode: Option<String>,
) -> Result<LocalFileStat, String> {
    if let Some(mode_raw) = access_mode {
        let mode = AccessMode::from_str(mode_raw.trim())?;
        let size = match fs::metadata(path) {
            Ok(metadata) => metadata.len(),
            Err(err) if mode.allows_write() && err.kind() == std::io::ErrorKind::NotFound => 0,
            Err(err) => return Err(err.to_string()),
        };
        let token = issue_path_grant(state, path, mode, PathGrantSource::AllowedRootsOnly)?;
        return Ok(LocalFileStat {
            size,
            access_token: Some(token),
        });
    }

    ensure_path_in_allowed_roots(state, path)?;
    let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
    Ok(LocalFileStat {
        size: metadata.len(),
        access_token: None,
    })
}
