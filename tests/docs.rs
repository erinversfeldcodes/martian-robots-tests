//! What the README says about the suite, checked against the suite.
//!
//! Prose goes stale silently. A case id quoted in a README is a promise that
//! something can be run, and renaming a case breaks it without breaking
//! anything a compiler or a test would notice — which is how a document ends
//! up describing a repository that no longer exists.

use martian_robots_verify::cases;
use martian_robots_verify::contract::Contract;

fn readme() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md");
    std::fs::read_to_string(path).expect("a README to check")
}

#[test]
fn every_case_id_the_readme_quotes_is_a_case() {
    let contract = Contract::load().expect("a coherent contract");
    let catalogue = cases::catalogue(&contract);
    let ids: Vec<&str> = catalogue.iter().map(|case| case.id.as_str()).collect();

    // The groups come from the ids themselves, so a new group is covered
    // without this test being told about it.
    let mut groups: Vec<&str> = ids
        .iter()
        .filter_map(|id| id.split_once('/').map(|(group, _)| group))
        .collect();
    groups.sort_unstable();
    groups.dedup();

    let readme = readme();
    let mut found = 0;
    for group in groups {
        let marker = format!("{group}/");
        let mut rest = readme.as_str();
        while let Some(at) = rest.find(&marker) {
            let quoted: String = rest[at..]
                .chars()
                .take_while(|character| {
                    character.is_ascii_lowercase()
                        || character.is_ascii_digit()
                        || *character == '-'
                        || *character == '/'
                })
                .collect();
            assert!(
                ids.contains(&quoted.as_str()),
                "the README quotes {quoted:?}, which is not a case id; \
                 `--cases` lists them"
            );
            found += 1;
            rest = &rest[at + marker.len()..];
        }
    }

    assert!(
        found > 0,
        "no case id appears in the README, so this test is watching nothing"
    );
}

#[test]
fn every_predicate_the_readme_lists_is_a_predicate() {
    // The invariants are quoted by name, and a renamed one would leave the
    // README describing a check the suite no longer makes.
    let readme = readme();
    for name in martian_robots_verify::properties::NAMES {
        assert!(
            readme.contains(name),
            "the README does not list the invariant {name:?}"
        );
    }
}
