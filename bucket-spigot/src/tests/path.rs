// Copyright (C) 2021-2024  Daniel Lambert. Licensed under GPL-3.0-or-later, see /COPYING file for details

use crate::path::{Error, Path, RemovedSelf};
use std::str::FromStr as _;

/// Exposes the structural contents of [`Path`], ignoring custom serialization
#[derive(serde::Serialize, Debug)]
#[serde(transparent)]
struct PathStructural(Vec<usize>);

fn path(input: &str) -> Result<Path, Error> {
    let result = Path::from_str(input);
    if let Ok(path_elems) = &result {
        // verify Display <==> FromStr
        assert_eq!(
            path_elems.to_string(),
            input,
            "from_str.to_string does not match input"
        );
    }
    result
}
fn path_elems(input: &str) -> Result<PathStructural, Error> {
    path(input).map(|path| PathStructural(path.into_iter().collect()))
}

fn json_de_elems(input: &str) -> PathStructural {
    let elems = {
        let path: Path = serde_json::from_str(input).expect("test JSON input should be valid");
        let elems: Vec<_> = path.into_iter().collect();
        elems
    };
    let recreated_path: Path = elems.iter().copied().collect();
    // verify Serialize <==> Deserialize
    // (note: restricts flexibility of test JSON inputs)
    assert_eq!(
        serde_json::to_string(&recreated_path).expect("should serialize OK"),
        input
    );
    PathStructural(elems)
}

#[test]
fn inner_structure_from_str() {
    insta::assert_ron_snapshot!(path_elems(""), @"Err(MissingStartDelim)");
    insta::assert_ron_snapshot!(path_elems("invalid"), @"Err(MissingStartDelim)");

    insta::assert_ron_snapshot!(path_elems("invalid."), @"Err(MissingStartDelim)");
    insta::assert_ron_snapshot!(path_elems(".invalid"), @r###"
    Err(InvalidNumber(
      input: "invalid",
    ))
    "###);

    insta::assert_ron_snapshot!(path_elems("."), @"Ok([])");
    insta::assert_ron_snapshot!(path_elems(".1"), @r###"
    Ok([
      1,
    ])
    "###);
    insta::assert_ron_snapshot!(path_elems(".1.2.3.4.5"), @r###"
    Ok([
      1,
      2,
      3,
      4,
      5,
    ])
    "###);
    insta::assert_ron_snapshot!(path_elems(".234.32.9"), @r###"
    Ok([
      234,
      32,
      9,
    ])
    "###);
}

#[test]
fn public_ser() {
    insta::assert_ron_snapshot!(Path::from(vec![]), @r###"".""###);
    insta::assert_ron_snapshot!(Path::from(vec![1, 2, 3]), @r###"".1.2.3""###);

    insta::assert_ron_snapshot!(path(".1.2.3"), @r###"Ok(".1.2.3")"###);
}
#[test]
fn public_de() {
    insta::assert_ron_snapshot!(json_de_elems("\".\""), @"[]");
    insta::assert_ron_snapshot!(json_de_elems("\".1\""), @r###"
    [
      1,
    ]
    "###);
    insta::assert_ron_snapshot!(json_de_elems("\".1.2.3.4.5\""), @r###"
    [
      1,
      2,
      3,
      4,
      5,
    ]
    "###);
    insta::assert_ron_snapshot!(json_de_elems("\".59.2.393904\""), @r###"
    [
      59,
      2,
      393904,
    ]
    "###);
}

fn check_remove(original: &str, other: &str) -> Result<Option<&'static str>, RemovedSelf> {
    // allow space-justifying tests
    let original = original.trim();
    let other = other.trim();

    let mut target: Path = original
        .parse()
        .expect("test target Path input should be valid");

    let other: Path = other
        .parse()
        .expect("test other Path input should be valid");

    target.modify_for_removed(other.as_ref())?;
    let modified_str = target.to_string();
    let changed: Option<&str> = (original != modified_str).then_some(modified_str.leak());
    Ok(changed)
}

#[test]
fn modify_removed_simple() {
    assert_eq!(check_remove(".0  ", ".1  "), Ok(None));
    assert_eq!(check_remove(".0.0", ".0.1"), Ok(None));

    assert_eq!(check_remove(".1  ", ".0  "), Ok(Some(".0")));
    assert_eq!(check_remove(".0.1", ".0.0"), Ok(Some(".0.0")));
}
#[test]
fn modify_removed_complex() {
    assert_eq!(check_remove(".2.3.4", ".0    "), Ok(Some(".1.3.4")));
    assert_eq!(check_remove(".2.3.4", ".2.0  "), Ok(Some(".2.2.4")));
    assert_eq!(check_remove(".2.3.4", ".2.3.0"), Ok(Some(".2.3.3")));

    let tgt = ".5.5.5.5.5";

    assert_eq!(check_remove(tgt, ".0        "), Ok(Some(".4.5.5.5.5")));
    assert_eq!(check_remove(tgt, ".5.1      "), Ok(Some(".5.4.5.5.5")));
    assert_eq!(check_remove(tgt, ".5.5.2    "), Ok(Some(".5.5.4.5.5")));
    assert_eq!(check_remove(tgt, ".5.5.5.3  "), Ok(Some(".5.5.5.4.5")));
    assert_eq!(check_remove(tgt, ".5.5.5.5.4"), Ok(Some(".5.5.5.5.4")));
    assert_eq!(check_remove(tgt, ".5.5.5.5.5"), Err(RemovedSelf));
    assert_eq!(check_remove(tgt, ".5.5.5.5.6"), Ok(None));
    assert_eq!(check_remove(tgt, ".5.5.5.7  "), Ok(None));
    assert_eq!(check_remove(tgt, ".5.5.8    "), Ok(None));
    assert_eq!(check_remove(tgt, ".5.9      "), Ok(None));
    assert_eq!(check_remove(tgt, ".10       "), Ok(None));

    assert_eq!(check_remove(tgt, ".5.5.5.5.5.1      "), Ok(None));
    assert_eq!(check_remove(tgt, ".5.5.5.5.5.1.1    "), Ok(None));
    assert_eq!(check_remove(tgt, ".5.5.5.5.5.1.1.1  "), Ok(None));
    assert_eq!(check_remove(tgt, ".5.5.5.5.5.1.1.1.1"), Ok(None));
}
