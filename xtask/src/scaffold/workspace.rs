//! Registers the new crates in `[workspace].members` of the root manifest. The edit goes through
//! `toml_edit`, so comments and the layout of the file stay as they are.

use toml_edit::{Array, DocumentMut, RawString, Value};

use super::Fault;

/// Returns the manifest text with `paths` added to `[workspace].members`, each at its sorted
/// position and in the layout of the existing entries.
pub(crate) fn add_members(manifest: &str, paths: &[String]) -> Result<String, Fault> {
    let mut document: DocumentMut = manifest.parse()?;
    let members = document
        .get_mut("workspace")
        .and_then(|workspace| workspace.get_mut("members"))
        .and_then(|members| members.as_array_mut())
        .ok_or(Fault::NoMembers)?;
    for path in paths {
        insert(members, path);
    }
    Ok(document.to_string())
}

/// The entries of `[workspace].members`, as written. A manifest that does not parse or has no
/// array lists none; `add_members` reports that case.
pub(crate) fn members(manifest: &str) -> Vec<String> {
    manifest
        .parse::<DocumentMut>()
        .ok()
        .and_then(|document| {
            let members = document.get("workspace")?.get("members")?.as_array()?;
            Some(
                members
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect(),
            )
        })
        .unwrap_or_default()
}

fn insert(members: &mut Array, path: &str) {
    let Some(last) = members.iter().last() else {
        members.push(path);
        return;
    };
    let separator = Some(prefix(last))
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| " ".to_owned());
    let position = members
        .iter()
        .position(|member| member.as_str().is_some_and(|listed| listed > path))
        .unwrap_or(members.len());
    let value = Value::from(path);
    if separator.contains('\n') {
        insert_on_a_line(members, position, value, &indentation(&separator));
    } else {
        insert_inline(members, position, value, separator);
    }
}

/// Inserts into an array with one entry per line. A comment that follows the comma of the previous
/// entry on the same line belongs to that entry, so it moves in front of the new one.
fn insert_on_a_line(members: &mut Array, position: usize, mut value: Value, indent: &str) {
    let text = following(members, position);
    let (head, tail) = match text.split_once('\n') {
        Some((head, rest)) => (head.to_owned(), format!("\n{rest}")),
        None => (String::new(), text),
    };
    value.decor_mut().set_prefix(format!("{head}{indent}"));
    if let Some(last) = position
        .checked_sub(1)
        .filter(|_| position == members.len())
    {
        move_suffix(members, last, &mut value);
    }
    members.insert_formatted(position, value);
    set_following(members, position.saturating_add(1), tail);
}

/// Inserts into an array on one line. The first entry has no leading space, so a new first entry
/// takes that prefix and the old one takes the separator.
fn insert_inline(members: &mut Array, position: usize, mut value: Value, separator: String) {
    if position == 0 {
        value
            .decor_mut()
            .set_prefix(members.iter().next().map_or_else(String::new, prefix));
        members.insert_formatted(0, value);
        set_following(members, 1, separator);
    } else {
        value.decor_mut().set_prefix(separator);
        members.insert_formatted(position, value);
    }
}

/// Moves the text after the last entry (the line break before `]` of an array without a trailing
/// comma) behind the new last entry.
fn move_suffix(members: &mut Array, last: usize, value: &mut Value) {
    if let Some(previous) = members.get_mut(last) {
        let suffix = previous.decor().suffix().cloned().unwrap_or_default();
        previous.decor_mut().set_suffix("");
        value.decor_mut().set_suffix(suffix);
    }
}

/// The text after the entry before `position`: the prefix of the entry at `position`, or the
/// trailing text of the array when `position` is past the last entry.
fn following(members: &Array, position: usize) -> String {
    members.get(position).map_or_else(
        || members.trailing().as_str().unwrap_or_default().to_owned(),
        prefix,
    )
}

fn set_following(members: &mut Array, position: usize, text: String) {
    match members.get_mut(position) {
        Some(next) => next.decor_mut().set_prefix(text),
        None => members.set_trailing(text),
    }
}

/// The text before a value: whitespace and, in a multi-line array, the comments above it.
fn prefix(value: &Value) -> String {
    value
        .decor()
        .prefix()
        .and_then(RawString::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// The line break and indentation that end a prefix, without the comments that it may contain.
fn indentation(prefix: &str) -> String {
    format!("\n{}", prefix.rsplit('\n').next().unwrap_or_default())
}

#[cfg(test)]
mod tests;
