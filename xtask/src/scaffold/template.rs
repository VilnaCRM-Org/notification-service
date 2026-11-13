//! The six template files, rendered for one context.

/// The three layers of a bounded context.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Layer {
    Domain,
    Application,
    Infrastructure,
}

/// The two files of a layer crate.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Rendered {
    pub(crate) manifest: String,
    pub(crate) lib: String,
}

impl Layer {
    /// Every layer, in the order in which the members are registered.
    pub(crate) const ALL: [Self; 3] = [Self::Domain, Self::Application, Self::Infrastructure];

    /// The directory of the layer crate below the context directory.
    pub(crate) const fn directory(self) -> &'static str {
        match self {
            Self::Domain => "domain",
            Self::Application => "application",
            Self::Infrastructure => "infrastructure",
        }
    }

    const fn manifest(self) -> &'static str {
        match self {
            Self::Domain => include_str!("../../templates/domain/Cargo.toml.tmpl"),
            Self::Application => include_str!("../../templates/application/Cargo.toml.tmpl"),
            Self::Infrastructure => include_str!("../../templates/infrastructure/Cargo.toml.tmpl"),
        }
    }

    const fn lib(self) -> &'static str {
        match self {
            Self::Domain => include_str!("../../templates/domain/lib.rs.tmpl"),
            Self::Application => include_str!("../../templates/application/lib.rs.tmpl"),
            Self::Infrastructure => include_str!("../../templates/infrastructure/lib.rs.tmpl"),
        }
    }
}

/// Fills the templates of `layer` with the context name.
pub(crate) fn render(layer: Layer, context: &str) -> Rendered {
    let fill = |template: &str| template.replace("{{context}}", context);
    Rendered {
        manifest: fill(layer.manifest()),
        lib: fill(layer.lib()),
    }
}

#[cfg(test)]
mod tests;
