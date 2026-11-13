//! Refreshes `Cargo.lock` after members were added. Covered by `tests/integration/scaffold.rs`,
//! which runs the binary against a fixture workspace.

use std::path::Path;
use std::process::Command;

use super::Fault;

/// Runs `cargo update --workspace --offline` in the workspace root. Dependency-free crates need no
/// registry, so this adds the new packages to the lock file without network access.
pub(crate) fn refresh(root: &Path) -> Result<(), Fault> {
    let output = Command::new("cargo")
        .args(["update", "--workspace", "--offline"])
        .current_dir(root)
        .output()
        .map_err(Fault::RunCargo)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(Fault::Lock(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}
