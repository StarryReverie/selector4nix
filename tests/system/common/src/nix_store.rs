use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result as AnyhowResult, bail};
use tempfile::TempDir;

pub struct NixStoreEntry {
    pub name: String,
    pub hash: String,
}

pub struct NixStore {
    dir: TempDir,
    nix_bin: PathBuf,
    entries: Vec<NixStoreEntry>,
}

impl NixStore {
    pub fn create(nix_bin: PathBuf) -> AnyhowResult<Self> {
        let dir = TempDir::new().context("failed to create temp directory for nix store")?;
        Ok(Self {
            dir,
            nix_bin,
            entries: Vec::new(),
        })
    }

    pub fn add_file(&mut self, name: &str, content: &[u8]) -> AnyhowResult<&NixStoreEntry> {
        let file_path = self.dir.path().join(name);
        std::fs::write(&file_path, content)
            .with_context(|| format!("failed to write file `{name}`"))?;

        let store_uri = format!("file://{}?compression=none", self.dir.path().display());
        let output = Command::new(&self.nix_bin)
            .args(["store", "add-file", "--store", &store_uri])
            .arg(&file_path)
            .env("NIX_CONFIG", "experimental-features = nix-command")
            .output()
            .with_context(|| format!("failed to spawn `nix store add-file` for `{name}`"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("`nix store add-file` failed for `{name}`: {stderr}");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let store_path = stdout.trim();
        let hash = store_path
            .strip_prefix("/nix/store/")
            .and_then(|s| s.split_once('-'))
            .map(|(h, _)| h)
            .with_context(|| {
                format!("unexpected `nix store add-file` output for `{name}`: {store_path}")
            })?;

        self.entries.push(NixStoreEntry {
            name: name.to_string(),
            hash: hash.to_string(),
        });

        Ok(self.entries.last().unwrap())
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn entries(&self) -> impl Iterator<Item = &NixStoreEntry> {
        self.entries.iter()
    }
}
