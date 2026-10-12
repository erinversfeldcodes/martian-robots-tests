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
    let names = martian_robots_verify::modes::predicate_names();
    assert!(
        names.len() >= 8,
        "only {} predicate(s) to watch",
        names.len()
    );
    for name in names {
        assert!(
            readme.contains(name),
            "the README does not list the invariant {name:?}"
        );
    }
}

#[test]
fn the_rendered_contract_names_no_file_that_is_not_there() {
    // Three normative sentences named `grammar.ebnf` and `limits.toml` for two
    // releases after those files were deleted. The schema validates keys and
    // treats the prose as opaque strings, and the prose is where this
    // contract's substance lives — so the one thing it cannot check is the
    // one thing that went stale.
    let contract = Contract::load().expect("a coherent contract");
    let rendered = contract.render().expect("the contract to render");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    // Everything above the revision history. A history describes the past, so
    // a row that names the file it retired is correct and must stay sayable.
    let document = rendered
        .split_once("## 5. Revision history")
        .map_or(rendered.as_str(), |(before, _)| before);

    let mut checked = 0;
    for mention in document.split('`').skip(1).step_by(2) {
        let looks_like_a_path = mention.contains('/') || mention.contains('.');
        if !looks_like_a_path || mention.contains(' ') || mention.contains('{') {
            continue;
        }
        let Some(stem) = mention.split('/').next_back() else {
            continue;
        };
        if !stem.contains('.') || stem.starts_with('.') {
            continue;
        }
        // Only names that look like files this repository would hold.
        if !["toml", "proto", "ebnf", "md", "rs", "input", "output"]
            .contains(&stem.rsplit('.').next().unwrap_or(""))
        {
            continue;
        }
        let relative = mention.strip_prefix("contract/").unwrap_or(mention);
        assert!(
            root.join(mention).exists() || root.join("contract").join(relative).exists(),
            "the contract names {mention}, which does not exist"
        );
        checked += 1;
    }

    assert!(
        checked > 0,
        "no file is named, so this test watches nothing"
    );
}
