//! A small JSON Schema checker for the documentation tests: the keywords that the documented
//! parameters, frames and problems use, `$ref` into the components of the document, and `pattern`
//! matched by a backtracking matcher of the ECMA-262 subset those patterns are written in (anchors,
//! sets with ranges, groups, alternation and the quantifiers `*`, `+`, `?`, `{n}` and `{m,n}`).
//! Formats are annotations, as in the default vocabulary of JSON Schema 2020-12.

use serde_json::Value;

/// Whether `schema`, whose `$ref`s point into the components of `document`, admits `value`.
pub(super) fn admits(document: &Value, schema: &Value, value: &Value) -> bool {
    let schema = resolve(document, schema);
    type_admits(schema, value)
        && schema["enum"]
            .as_array()
            .is_none_or(|values| values.contains(value))
        && string_admits(document, schema, value)
        && integer_admits(schema, value)
        && array_admits(document, schema, value)
        && object_admits(document, schema, value)
}

/// The schema a `$ref` names, or the schema itself.
fn resolve<'a>(document: &'a Value, schema: &'a Value) -> &'a Value {
    schema["$ref"].as_str().map_or(schema, |reference| {
        let name = reference
            .strip_prefix("#/components/schemas/")
            .expect("a reference to a component schema");
        resolve(document, &document["components"]["schemas"][name])
    })
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(number) if number.is_u64() || number.is_i64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn type_admits(schema: &Value, value: &Value) -> bool {
    let name = type_name(value);
    let admitted =
        |expected: &Value| expected == name || (expected == "number" && name == "integer");
    schema["type"].as_array().map_or_else(
        || schema["type"].is_null() || admitted(&schema["type"]),
        |types| types.iter().any(admitted),
    )
}

fn string_admits(document: &Value, schema: &Value, value: &Value) -> bool {
    let Some(text) = value.as_str() else {
        return true;
    };
    let length = u64::try_from(text.chars().count()).unwrap();
    schema["minLength"].as_u64().is_none_or(|min| length >= min)
        && schema["maxLength"].as_u64().is_none_or(|max| length <= max)
        && schema["pattern"]
            .as_str()
            .is_none_or(|pattern| is_match(pattern, text))
        && schema.get("contentSchema").is_none_or(|content| {
            assert_eq!(schema["contentMediaType"], "application/json");
            serde_json::from_str::<Value>(text)
                .is_ok_and(|decoded| admits(document, content, &decoded))
        })
}

fn integer_admits(schema: &Value, value: &Value) -> bool {
    let Some(number) = value.as_i64() else {
        return true;
    };
    schema["minimum"].as_i64().is_none_or(|min| number >= min)
        && schema["maximum"].as_i64().is_none_or(|max| number <= max)
}

fn array_admits(document: &Value, schema: &Value, value: &Value) -> bool {
    let Some(items) = value.as_array() else {
        return true;
    };
    let count = u64::try_from(items.len()).unwrap();
    schema["minItems"].as_u64().is_none_or(|min| count >= min)
        && schema["maxItems"].as_u64().is_none_or(|max| count <= max)
        && schema
            .get("items")
            .is_none_or(|item| items.iter().all(|value| admits(document, item, value)))
}

fn object_admits(document: &Value, schema: &Value, value: &Value) -> bool {
    let Some(members) = value.as_object() else {
        return true;
    };
    let required = schema["required"].as_array().map_or(&[][..], Vec::as_slice);
    required
        .iter()
        .all(|name| members.contains_key(name.as_str().unwrap()))
        && members.iter().all(|(name, member)| {
            schema["properties"].get(name).map_or_else(
                || schema["additionalProperties"] != false,
                |property| admits(document, property, member),
            )
        })
}

/// One element of a pattern and how often it repeats.
#[derive(Debug)]
struct Piece {
    atom: Atom,
    min: usize,
    max: Option<usize>,
}

#[derive(Debug)]
enum Atom {
    /// `^`
    Start,
    /// `$`
    End,
    /// One character within one of the inclusive ranges: a set, an escape or a literal.
    Set(Vec<(char, char)>),
    /// A group of alternatives, each a sequence.
    Group(Vec<Vec<Piece>>),
}

/// The continuation of a match: called with the position after the part matched so far.
type Next<'a> = dyn FnMut(usize) -> bool + 'a;

/// Whether `pattern` matches somewhere in `text`, as ECMA-262 `RegExp.prototype.test` does.
pub(super) fn is_match(pattern: &str, text: &str) -> bool {
    let source: Vec<char> = pattern.chars().collect();
    let mut at = 0_usize;
    let alternatives = parse_alternatives(&source, &mut at);
    assert_eq!(at, source.len(), "unsupported pattern {pattern}");
    let whole = [Piece {
        atom: Atom::Group(alternatives),
        min: 1,
        max: Some(1),
    }];
    let text: Vec<char> = text.chars().collect();
    (0..=text.len()).any(|start| sequence(&whole, &text, start, &mut |_end| true))
}

fn parse_alternatives(source: &[char], at: &mut usize) -> Vec<Vec<Piece>> {
    let mut alternatives = vec![parse_sequence(source, at)];
    while source.get(*at) == Some(&'|') {
        *at = at.saturating_add(1);
        alternatives.push(parse_sequence(source, at));
    }
    alternatives
}

fn parse_sequence(source: &[char], at: &mut usize) -> Vec<Piece> {
    let mut pieces = Vec::new();
    while let Some(&next) = source.get(*at) {
        if next == '|' || next == ')' {
            break;
        }
        let atom = parse_atom(source, at);
        let (min, max) = parse_quantifier(source, at);
        pieces.push(Piece { atom, min, max });
    }
    pieces
}

fn parse_atom(source: &[char], at: &mut usize) -> Atom {
    let next = source[*at];
    *at = at.saturating_add(1);
    match next {
        '^' => Atom::Start,
        '$' => Atom::End,
        '(' => {
            if source.get(*at..at.saturating_add(2)) == Some(&['?', ':'][..]) {
                *at = at.saturating_add(2);
            }
            let alternatives = parse_alternatives(source, at);
            assert_eq!(source.get(*at), Some(&')'), "an unclosed group");
            *at = at.saturating_add(1);
            Atom::Group(alternatives)
        }
        '[' => parse_set(source, at),
        '\\' => {
            let escaped = class_char(source, at);
            Atom::Set(vec![(escaped, escaped)])
        }
        '.' | '*' | '+' | '?' | '{' => panic!("`{next}` is not supported here"),
        literal => Atom::Set(vec![(literal, literal)]),
    }
}

fn parse_set(source: &[char], at: &mut usize) -> Atom {
    assert_ne!(source[*at], '^', "negated sets are not supported");
    let mut ranges = Vec::new();
    while source[*at] != ']' {
        let low = class_char(source, at);
        let is_range = source[*at] == '-' && source[at.saturating_add(1)] != ']';
        if is_range {
            *at = at.saturating_add(1);
            ranges.push((low, class_char(source, at)));
        } else {
            ranges.push((low, low));
        }
    }
    *at = at.saturating_add(1);
    Atom::Set(ranges)
}

fn class_char(source: &[char], at: &mut usize) -> char {
    if source[*at] == '\\' {
        *at = at.saturating_add(1);
    }
    let character = source[*at];
    *at = at.saturating_add(1);
    character
}

fn parse_quantifier(source: &[char], at: &mut usize) -> (usize, Option<usize>) {
    let bounds = match source.get(*at) {
        Some('*') => (0, None),
        Some('+') => (1, None),
        Some('?') => (0, Some(1)),
        Some('{') => return parse_bounds(source, at),
        _ => return (1, Some(1)),
    };
    *at = at.saturating_add(1);
    bounds
}

fn parse_bounds(source: &[char], at: &mut usize) -> (usize, Option<usize>) {
    let close = source[*at..]
        .iter()
        .position(|&character| character == '}')
        .expect("a closed quantifier")
        .saturating_add(*at);
    let inner: String = source[at.saturating_add(1)..close].iter().collect();
    *at = close.saturating_add(1);
    inner.split_once(',').map_or_else(
        || {
            let count = inner.parse().unwrap();
            (count, Some(count))
        },
        |(min, max)| (min.parse().unwrap(), Some(max.parse().unwrap())),
    )
}

fn sequence(pieces: &[Piece], text: &[char], at: usize, next: &mut Next<'_>) -> bool {
    match pieces.split_first() {
        None => next(at),
        Some((piece, rest)) => repeat(piece, 0, text, at, &mut |end| {
            sequence(rest, text, end, next)
        }),
    }
}

/// Greedy: one more repetition first while the maximum allows, then the rest of the pattern once
/// the minimum is reached. A repetition beyond the minimum must consume a character.
fn repeat(piece: &Piece, count: usize, text: &[char], at: usize, next: &mut Next<'_>) -> bool {
    let more = piece.max.is_none_or(|max| count < max)
        && atom(&piece.atom, text, at, &mut |end| {
            (end > at || count < piece.min)
                && repeat(piece, count.saturating_add(1), text, end, next)
        });
    more || (count >= piece.min && next(at))
}

fn atom(atom: &Atom, text: &[char], at: usize, next: &mut Next<'_>) -> bool {
    match atom {
        Atom::Start => at == 0 && next(at),
        Atom::End => at == text.len() && next(at),
        Atom::Set(ranges) => {
            text.get(at).is_some_and(|character| {
                ranges
                    .iter()
                    .any(|(low, high)| (low..=high).contains(&character))
            }) && next(at.saturating_add(1))
        }
        Atom::Group(alternatives) => alternatives
            .iter()
            .any(|alternative| sequence(alternative, text, at, next)),
    }
}

#[test]
fn the_matcher_reads_the_constructs_of_the_documented_patterns() {
    assert!(is_match(
        "^[a-z][a-z0-9]*([._-][a-z0-9]+)*$",
        "user.created"
    ));
    assert!(is_match("^[a-z][a-z0-9]*([._-][a-z0-9]+)*$", "a-b_c.d9"));
    assert!(!is_match("^[a-z][a-z0-9]*([._-][a-z0-9]+)*$", "a..b"));
    assert!(!is_match("^[a-z][a-z0-9]*([._-][a-z0-9]+)*$", "a."));
    assert!(!is_match("^[a-z][a-z0-9]*([._-][a-z0-9]+)*$", "Ab"));
    assert!(is_match("^(?:ab|c){2}$", "abc"));
    assert!(!is_match("^(?:ab|c){2}$", "ab"));
    assert!(is_match("^a{1,3}b?$", "aaa"));
    assert!(!is_match("^a{1,3}b?$", "aaaa"));
    assert!(is_match("[Uu][Pp]", "keep-alive, Up"));
    assert!(!is_match("[Uu][Pp]", "keep-alive"));
    assert!(is_match(r"^\.\-$", ".-"));
    assert!(is_match("^x+$", "xx"));
    assert!(!is_match("^x+$", ""));
    assert!(is_match("^$", ""));
}

#[test]
fn the_checker_reads_the_keywords_of_the_documented_schemas() {
    let document = serde_json::json!({"components": {"schemas": {"Count": {"type": "integer", "minimum": 1, "maximum": 3}}}});
    let object = serde_json::json!({
        "type": "object",
        "required": ["n"],
        "properties": {"n": {"$ref": "#/components/schemas/Count"}, "s": {"type": ["string", "null"], "enum": ["a", null]}},
        "additionalProperties": false
    });

    assert!(admits(&document, &object, &serde_json::json!({"n": 2})));
    assert!(admits(
        &document,
        &object,
        &serde_json::json!({"n": 3, "s": null})
    ));
    assert!(!admits(&document, &object, &serde_json::json!({"n": 4})));
    assert!(!admits(&document, &object, &serde_json::json!({"n": 0})));
    assert!(!admits(&document, &object, &serde_json::json!({"s": "a"})));
    assert!(!admits(
        &document,
        &object,
        &serde_json::json!({"n": 1, "s": "b"})
    ));
    assert!(!admits(
        &document,
        &object,
        &serde_json::json!({"n": 1, "x": 1})
    ));
    assert!(!admits(&document, &object, &serde_json::json!([1])));
    let list = serde_json::json!({"type": "array", "minItems": 1, "maxItems": 2, "items": {"type": "string", "minLength": 1, "maxLength": 2}});
    assert!(admits(&document, &list, &serde_json::json!(["a", "bc"])));
    assert!(!admits(&document, &list, &serde_json::json!([])));
    assert!(!admits(
        &document,
        &list,
        &serde_json::json!(["a", "b", "c"])
    ));
    assert!(!admits(&document, &list, &serde_json::json!(["abc"])));
    assert!(!admits(&document, &list, &serde_json::json!([""])));
    let json_text = serde_json::json!({"type": "string", "contentMediaType": "application/json", "contentSchema": {"type": "object", "required": ["a"]}});
    assert!(admits(
        &document,
        &json_text,
        &serde_json::json!(r#"{"a":1}"#)
    ));
    assert!(!admits(
        &document,
        &json_text,
        &serde_json::json!(r#"{"b":1}"#)
    ));
    assert!(!admits(
        &document,
        &json_text,
        &serde_json::json!("not json")
    ));
}
