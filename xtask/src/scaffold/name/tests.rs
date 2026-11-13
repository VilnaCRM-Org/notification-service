//! Unit tests of the parent module.

use super::super::Refusal;
use super::{check, valid};

#[test]
fn names_that_match_the_pattern_are_valid() {
    for name in [
        "a",
        "a1",
        "billing",
        "invoice-ledger",
        "a-1",
        "a-1b-22",
        "x9",
    ] {
        assert!(valid(name), "{name}");
    }
}

#[test]
fn names_that_do_not_match_the_pattern_are_invalid() {
    for name in [
        "", "Billing", "billING", "1x", "-a", "a-", "a--b", "a_b", "a b", "a.b", "é", "aé", "a-é",
        "a-B", "-",
    ] {
        assert!(!valid(name), "{name}");
    }
}

#[test]
fn a_valid_pair_is_accepted() {
    assert_eq!(check("billing", "invoice-ledger"), Ok(()));
}

#[test]
fn an_invalid_group_is_refused_and_named_as_a_group() {
    assert_eq!(
        check("Billing", "ledger"),
        Err(Refusal::InvalidName {
            kind: "group",
            name: "Billing".to_owned()
        })
    );
}

#[test]
fn an_invalid_context_is_refused_and_named_as_a_context() {
    assert_eq!(
        check("billing", "a--b"),
        Err(Refusal::InvalidName {
            kind: "context",
            name: "a--b".to_owned()
        })
    );
}

#[test]
fn the_group_is_checked_before_the_context() {
    assert_eq!(
        check("1x", "Bad"),
        Err(Refusal::InvalidName {
            kind: "group",
            name: "1x".to_owned()
        })
    );
}

#[test]
fn a_reserved_context_name_is_refused() {
    for context in ["shared", "service", "xtask", "test-support", "e2e-tests"] {
        assert_eq!(
            check("billing", context),
            Err(Refusal::Reserved(context.to_owned()))
        );
    }
}

#[test]
fn a_reserved_name_is_allowed_for_a_group_and_a_name_that_only_contains_one_is_allowed() {
    assert_eq!(check("service", "ledger"), Ok(()));
    assert_eq!(check("billing", "shared-ledger"), Ok(()));
    assert_eq!(check("billing", "xtask2"), Ok(()));
}

#[test]
fn the_refusals_say_what_is_wrong() {
    let invalid = check("billing", "1x").unwrap_err().to_string();
    let reserved = check("billing", "shared").unwrap_err().to_string();

    assert!(
        invalid.starts_with("the context name `1x` is invalid: use lowercase letters"),
        "{invalid}"
    );
    assert_eq!(
        reserved,
        "`shared` is reserved and cannot name a bounded context"
    );
}
