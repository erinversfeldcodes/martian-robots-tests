//! Generate the contract's Rust types from the contract's schema.
//!
//! `contract/contract.proto` says what a contract is. These types are its
//! shape in Rust, and nothing in `src/` restates them: a field added to the
//! schema appears here without anybody writing it twice, and a field removed
//! stops compiling.
//!
//! The serde derives are injected rather than written into the schema, because
//! the schema is published for consumers in other languages and should not
//! carry one language's annotations. `deny_unknown_fields` is what turns a
//! misspelled key in the authored instance into an error instead of a silently
//! missing value.

fn main() {
    println!("cargo:rerun-if-changed=contract/contract.proto");

    let descriptors =
        protox::compile(["contract/contract.proto"], ["."]).expect("the schema to compile");

    prost_build::Config::new()
        .type_attribute(".", "#[derive(serde::Deserialize)]")
        .type_attribute(".", "#[serde(deny_unknown_fields)]")
        .type_attribute(
            "martian_robots.contract.Ruling.decision",
            "#[serde(rename_all = \"snake_case\")]",
        )
        // Only the fields that are genuinely optional get a default. Every
        // other field stays required, so a missing `ruling` is a load error
        // rather than an empty string nobody notices — which is what the
        // hand-written loader used to check for.
        .field_attribute(
            "martian_robots.contract.Ruled.rationale",
            "#[serde(default)]",
        )
        .field_attribute(
            "martian_robots.contract.Production.literals",
            "#[serde(default)]",
        )
        .field_attribute(
            "martian_robots.contract.Production.terms",
            "#[serde(default)]",
        )
        .field_attribute(
            "martian_robots.contract.Production.comment",
            "#[serde(default)]",
        )
        .field_attribute(
            "martian_robots.contract.Term.is_repeated",
            "#[serde(default)]",
        )
        .field_attribute(
            "martian_robots.contract.Term.is_optional",
            "#[serde(default)]",
        )
        .compile_fds(descriptors)
        .expect("the schema to generate Rust types");
}
