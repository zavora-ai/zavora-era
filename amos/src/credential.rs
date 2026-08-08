//! Private, per-session bearer storage for delegated MCP calls.
//!
//! The bearer is never placed in model-visible arguments or on the MCP wire.
//! Amos injects only this file's opaque path after model generation; mcp-erp
//! validates the path and reads the current short-lived token for that call.

use anyhow::{Context, Result, bail};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static CREDENTIAL_ROOT: OnceLock<Result<PathBuf, String>> = OnceLock::new();

pub fn root() -> Result<PathBuf> {
    match CREDENTIAL_ROOT.get_or_init(|| {
        let path = std::env::var("AMOS_MCP_CREDENTIAL_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::temp_dir().join(format!(
                    "amos-mcp-credentials-{}-{}",
                    std::process::id(),
                    uuid::Uuid::new_v4()
                ))
            });
        create_private_dir(&path)
            .and_then(|()| {
                path.canonicalize()
                    .context("failed to resolve Amos MCP credential directory")
            })
            .map_err(|error| error.to_string())
    }) {
        Ok(path) => Ok(path.clone()),
        Err(error) => bail!(error.clone()),
    }
}

#[cfg(unix)]
fn create_private_dir(path: &Path) -> Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    if !path.exists() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)?;
    }
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
        bail!("AMOS_MCP_CREDENTIAL_DIR must be a private directory (mode 0700)");
    }
    Ok(())
}

#[cfg(not(unix))]
fn create_private_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    Ok(())
}

pub struct SessionCredential {
    path: PathBuf,
}

impl SessionCredential {
    pub fn create(token: &str) -> Result<Self> {
        let path = root()?.join(format!("{}.jwt", uuid::Uuid::new_v4()));
        let credential = Self { path };
        credential.refresh(token)?;
        Ok(credential)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Atomically replace the token so an in-flight MCP read sees either the
    /// old valid JWT or the new valid JWT, never a truncated intermediate.
    pub fn refresh(&self, token: &str) -> Result<()> {
        if token.trim().is_empty() || token.chars().any(char::is_whitespace) {
            bail!("refusing to store an empty or malformed access token");
        }
        let next = self
            .path
            .with_extension(format!("next-{}", uuid::Uuid::new_v4()));
        write_private_file(&next, token.as_bytes())?;
        if let Err(error) = std::fs::rename(&next, &self.path) {
            let _ = std::fs::remove_file(&next);
            return Err(error).context("failed to rotate delegated MCP credential");
        }
        Ok(())
    }
}

impl Drop for SessionCredential {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(unix)]
fn write_private_file(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_rotation_is_atomic_and_drop_removes_file() {
        let credential = SessionCredential::create("aaa.bbb.ccc").unwrap();
        let path = credential.path().to_path_buf();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "aaa.bbb.ccc");
        credential.refresh("ddd.eee.fff").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "ddd.eee.fff");
        drop(credential);
        assert!(!path.exists());
    }
}
