//! Schema of `protected-config.toml`.

use serde::Deserialize;

use super::Failure;

/// The protected list. An unknown key is an error: a misspelt key would silently protect nothing.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct List {
    /// The label whose presence on a pull request approves a change.
    pub(crate) approval_label: String,
    /// Files that must not change at all.
    #[serde(default)]
    pub(crate) files: Vec<String>,
    /// Directories whose files, as they exist on the base ref, must not change at all.
    #[serde(default)]
    pub(crate) directories: Vec<String>,
    /// Tables of TOML files that must not change.
    #[serde(default)]
    pub(crate) manifest_tables: Vec<ManifestTable>,
    /// Values of JSON files that must not change.
    #[serde(default)]
    pub(crate) json_values: Vec<JsonValues>,
    /// Names of files that would override a protected configuration file for part of the tree.
    #[serde(default)]
    pub(crate) override_names: Vec<String>,
    /// Directories in which such files are allowed.
    #[serde(default)]
    pub(crate) override_exempt: Vec<String>,
    /// Directories that must exist in the working tree.
    #[serde(default)]
    pub(crate) required_directories: Vec<String>,
}

/// A protected table of a TOML file, named by its dotted path.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ManifestTable {
    pub(crate) file: String,
    pub(crate) table: String,
}

/// Protected values of a JSON file, named by RFC 6901 pointers.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JsonValues {
    pub(crate) file: String,
    pub(crate) pointers: Vec<String>,
}

/// Parses the text of the protected list.
pub(crate) fn parse(bytes: &[u8]) -> Result<List, Failure> {
    let text = String::from_utf8_lossy(bytes);
    toml::from_str(&text).map_err(Failure::InvalidList)
}

/// A directory with exactly one trailing slash, so that a prefix test cannot match a sibling.
pub(crate) fn directory(path: &str) -> String {
    format!("{}/", path.trim_end_matches('/'))
}

impl List {
    /// The protected items, one string each: files as their path, directories with a trailing
    /// `/`, tables and JSON values as `<file>#<table or pointer>`.
    pub(crate) fn items(&self) -> Vec<String> {
        let tables = self
            .manifest_tables
            .iter()
            .map(|item| format!("{}#{}", item.file, item.table));
        let values = self.json_values.iter().flat_map(|item| {
            item.pointers
                .iter()
                .map(|pointer| format!("{}#{pointer}", item.file))
        });
        self.files
            .iter()
            .cloned()
            .chain(self.directories.iter().map(|path| directory(path)))
            .chain(tables)
            .chain(values)
            .collect()
    }
}

#[cfg(test)]
mod tests;
