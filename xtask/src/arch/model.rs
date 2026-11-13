//! Facts about the workspace, independent of where they were read from.

/// Every workspace member.
#[derive(Debug, Default)]
pub(crate) struct Workspace {
    pub(crate) crates: Vec<Crate>,
    /// Local crates below the workspace root that the dependency graph uses but that are not
    /// members (for example listed in `[workspace] exclude`), so no layer or lint rule sees them.
    pub(crate) outside: Vec<Outside>,
}

/// A local crate that the workspace depends on without it being a member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Outside {
    pub(crate) name: String,
    /// Manifest path relative to the workspace root.
    pub(crate) manifest: String,
}

/// One workspace member.
#[derive(Debug)]
pub(crate) struct Crate {
    pub(crate) name: String,
    /// Manifest path relative to the workspace root.
    pub(crate) manifest: String,
    /// Whether the manifest has `[lints] workspace = true`.
    pub(crate) inherits_lints: bool,
    pub(crate) dependencies: Vec<Dependency>,
    /// Every package reachable through non-dev edges of the resolved dependency graph, by name.
    pub(crate) reach: Vec<Reach>,
}

/// A package that a workspace crate reaches through normal and build dependencies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Reach {
    /// The reached package.
    pub(crate) name: String,
    /// A shortest chain of packages from the crate's direct dependency to `name` (inclusive).
    pub(crate) path: Vec<String>,
}

/// A direct dependency declared in the manifest.
#[derive(Debug)]
pub(crate) struct Dependency {
    pub(crate) name: String,
    /// Declared under `[dev-dependencies]`.
    pub(crate) dev: bool,
    /// A workspace member rather than a third-party crate.
    pub(crate) internal: bool,
}
