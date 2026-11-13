//! Validation of the group and context names.

use super::Refusal;

/// Context names that would collide with a crate or a layer that the template already owns.
const RESERVED: [&str; 5] = ["shared", "service", "xtask", "test-support", "e2e-tests"];

/// Accepts names that match `^[a-z][a-z0-9]*(-[a-z0-9]+)*$`; a context name may not be reserved.
pub(crate) fn check(group: &str, context: &str) -> Result<(), Refusal> {
    for (kind, name) in [("group", group), ("context", context)] {
        if !valid(name) {
            return Err(Refusal::InvalidName {
                kind,
                name: name.to_owned(),
            });
        }
    }
    if RESERVED.contains(&context) {
        return Err(Refusal::Reserved(context.to_owned()));
    }
    Ok(())
}

fn valid(name: &str) -> bool {
    let mut words = name.split('-');
    words
        .next()
        .is_some_and(|first| first.starts_with(|c: char| c.is_ascii_lowercase()) && word(first))
        && words.all(|rest| !rest.is_empty() && word(rest))
}

fn word(text: &str) -> bool {
    text.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

#[cfg(test)]
mod tests;
