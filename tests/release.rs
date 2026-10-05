//! What a release promises, checked.
//!
//! A tag is the only thing a consumer pins, and the workflow that publishes one
//! reads the version out of the binary to check the tag against it. That makes
//! the shape of one printed line part of the release machinery, where nothing
//! would otherwise notice it changing.

use std::process::Command;

fn ask(argument: &str) -> (String, bool) {
    let output = Command::new(env!("CARGO_BIN_EXE_martian-robots-verify"))
        .arg(argument)
        .output()
        .expect("to run the suite");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        output.status.success(),
    )
}

#[test]
fn the_version_is_the_second_word_of_one_line() {
    // `.github/workflows/release.yml` takes the tag apart with
    // `awk '{ print $2 }'`. If this line grows a prefix, a tag stops being
    // checked against anything and the check still passes.
    let (said, exited_zero) = ask("--version");
    assert!(exited_zero, "--version must exit 0");
    assert_eq!(said.lines().count(), 1, "one line: {said:?}");
    assert_eq!(
        said.split_whitespace().nth(1),
        Some(env!("CARGO_PKG_VERSION")),
        "the release workflow reads the second word: {said:?}"
    );
}

#[test]
fn the_readme_points_at_the_repository_the_manifest_names() {
    // An install command naming the wrong repository is worse than none: it
    // works, and installs something else.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).expect("a manifest");
    let repository = manifest
        .lines()
        .find_map(|line| line.strip_prefix("repository = "))
        .expect("a repository field")
        .trim()
        .trim_matches('"')
        .to_string();

    let readme = std::fs::read_to_string(root.join("README.md")).expect("a README");
    assert!(
        readme.contains(&repository),
        "the README never names {repository}"
    );
}

#[test]
fn every_tag_the_readme_shows_is_the_version_in_the_tree() {
    // The install command names a tag, and a README that tells somebody to
    // install last release is worse than one that tells them nothing: it
    // works, quietly, against an older contract.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = std::fs::read_to_string(root.join("README.md")).expect("a README");
    let wanted = format!("v{}", env!("CARGO_PKG_VERSION"));

    // Only inside fenced blocks: prose illustrating what a patch means says
    // `--tag v2.1.1` on purpose, and that is an example rather than an
    // instruction. A command somebody will copy lives in a fence.
    let fenced: String = readme
        .split("```")
        .skip(1)
        .step_by(2)
        .collect::<Vec<&str>>()
        .join("\n");

    let mut found = 0;
    for marker in ["--tag v", "tag = \"v"] {
        found += tags_match(&fenced, marker, &wanted);
    }

    assert!(
        found > 0,
        "the README shows no tag to install, so this test is watching nothing"
    );
}

/// Every tag spelled with `marker` inside fenced blocks must be `wanted`.
fn tags_match(fenced: &str, marker: &str, wanted: &str) -> usize {
    let mut found = 0;
    let mut rest = fenced;
    while let Some(at) = rest.find(marker) {
        let from = at + marker.len() - 1;
        let shown: String = rest[from..]
            .chars()
            .take_while(|character| character.is_ascii_alphanumeric() || *character == '.')
            .collect();
        assert_eq!(
            shown, wanted,
            "the README names {shown}, and this tree is {wanted}"
        );
        found += 1;
        rest = &rest[at + marker.len()..];
    }
    found
}
