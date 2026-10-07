use std::fmt::Write as _;

/// The contract's shape, generated from `contract/contract.proto`.
///
/// Nothing in this file restates these types. A field added to the schema
/// appears here without anybody writing it twice, and a field removed stops
/// compiling.
mod schema {
    include!(concat!(env!("OUT_DIR"), "/martian_robots.contract.rs"));
}

pub use schema::ruling::Decision;
pub use schema::{Limits, Open, Production, Ruled, Ruling, Term};

/// The contract, as the suite uses it.
///
/// The generated types say what a contract is; this adds the one thing they
/// cannot, which is the terminal sets the generators draw from. Those are
/// projected out of the productions once, at load time, so every caller sees
/// the same derivation rather than each re-deriving it.
#[derive(Debug)]
pub struct Contract {
    pub version: String,
    pub limits: Limits,
    pub rulings: Vec<Ruling>,
    pub grammar: Grammar,
}

/// The grammar's terminals, projected from its productions, and the
/// productions themselves so the EBNF a reader sees can be rendered from them.
#[derive(Debug)]
pub struct Grammar {
    pub separators: Vec<char>,
    pub line_endings: Vec<String>,
    pub orientations: Vec<char>,
    pub instructions: Vec<char>,
    note: String,
    productions: Vec<Production>,
}

impl Grammar {
    /// Characters a runtime is liable to treat as whitespace, minus the ones
    /// this grammar actually admits as separators or line structure.
    ///
    /// Stated as a principle rather than a list, because a list is a thing
    /// somebody has to remember to extend and a hand-picked one leaves exactly
    /// the characters nobody thought of. The principle is: everything Unicode
    /// calls whitespace, which is what `char::is_whitespace` reports, plus two
    /// families outside that property that runtimes treat as whitespace anyway
    /// — the ASCII information separators, which several languages' split
    /// routines and line readers accept, and the byte-order mark, which text
    /// pipelines strip. A narrower pool let a reader that folded only the
    /// characters outside it pass every gate in this suite.
    ///
    /// The filter is the contract: admit a character in `ws` and it stops
    /// being injected, with nothing to update.
    pub fn not_separators(&self) -> Vec<char> {
        /// No character above this is whitespace by the Unicode property, so
        /// the scan has somewhere to stop. U+3000 is the last one.
        const LAST_WHITESPACE: u32 = 0x3000;
        const ALSO_TREATED_AS_WHITESPACE: [char; 5] =
            ['\u{1c}', '\u{1d}', '\u{1e}', '\u{1f}', '\u{feff}'];

        (0..=LAST_WHITESPACE)
            .filter_map(char::from_u32)
            .filter(|character| character.is_whitespace())
            .chain(ALSO_TREATED_AS_WHITESPACE)
            .filter(|character| {
                !self.separators.contains(character)
                    && !self
                        .line_endings
                        .iter()
                        .any(|ending| ending.contains(*character))
            })
            .collect()
    }

    /// Project the terminals out of the productions.
    ///
    /// A terminal production is an alternation of literals, so the characters
    /// the generators need are exactly the literals of four named productions.
    /// Reading them from the productions rather than from a separate list is
    /// what makes the published grammar and the injected characters the same
    /// facts: add a separator to the grammar and it stops being injected,
    /// with nothing to remember.
    pub fn project(note: String, productions: Vec<Production>) -> Result<Self, String> {
        let literals = |name: &str| -> Result<Vec<String>, String> {
            let production = productions
                .iter()
                .find(|production| production.name == name)
                .ok_or_else(|| format!("the grammar has no {name} production"))?;
            if production.literals.is_empty() {
                return Err(format!("{name} is not an alternation of literals"));
            }
            Ok(production.literals.clone())
        };

        let characters = |name: &str| -> Result<Vec<char>, String> {
            literals(name)?
                .into_iter()
                .map(|literal| {
                    let mut found = literal.chars();
                    match (found.next(), found.next()) {
                        (Some(only), None) => Ok(only),
                        _ => Err(format!(
                            "{name} admits {literal:?}, which is not one character"
                        )),
                    }
                })
                .collect()
        };

        Ok(Self {
            separators: characters("separator")?,
            line_endings: literals("eol")?,
            orientations: characters("orientation")?,
            instructions: characters("instruction")?,
            note,
            productions,
        })
    }

    /// The grammar as a reader sees it: the prose the productions cannot
    /// carry, then the productions.
    ///
    /// The note is here rather than in a comment at the top of a `.ebnf` file
    /// because there is no such file any more. It said things the productions
    /// cannot - which framing rules they fail to express, and where the
    /// numeric bounds live - and losing it with the file would have made the
    /// grammar look more complete than it is.
    pub fn to_markdown(&self) -> String {
        let ebnf = format!("```ebnf\n{}\n```", self.to_ebnf());
        if self.note.trim().is_empty() {
            ebnf
        } else {
            format!("{}\n\n{ebnf}", self.note.trim())
        }
    }

    /// The grammar as EBNF, rendered from the productions.
    ///
    /// The document a reader sees is downstream of the data, the same way
    /// CONTRACT.md is: there is no `.ebnf` file to keep in step, so the text
    /// cannot disagree with the terminals the suite injects.
    pub fn to_ebnf(&self) -> String {
        let width = self
            .productions
            .iter()
            .map(|production| production.name.chars().count())
            .max()
            .unwrap_or(0);

        let mut rendered = String::new();
        for production in &self.productions {
            let right = if production.literals.is_empty() {
                production
                    .terms
                    .iter()
                    .map(|term| {
                        let name = &term.name;
                        if term.is_repeated {
                            format!("{{ {name} }}")
                        } else if term.is_optional {
                            format!("[ {name} ]")
                        } else {
                            name.clone()
                        }
                    })
                    .collect::<Vec<String>>()
                    .join(" , ")
            } else {
                production
                    .literals
                    .iter()
                    .map(|literal| format!("\"{}\"", escape(literal)))
                    .collect::<Vec<String>>()
                    .join(" | ")
            };

            let _ = write!(rendered, "{:width$} = {right} ;", production.name);
            if production.comment.is_empty() {
                rendered.push('\n');
            } else {
                let _ = writeln!(rendered, "  (* {} *)", production.comment);
            }
        }
        rendered.trim_end().to_string()
    }
}

/// A literal as EBNF spells it, so a tab reads as `\t` rather than as a gap.
fn escape(literal: &str) -> String {
    literal
        .chars()
        .map(|character| match character {
            '\t' => "\\t".to_string(),
            '\n' => "\\n".to_string(),
            '\r' => "\\r".to_string(),
            other => other.to_string(),
        })
        .collect()
}

impl Ruling {
    /// Whether this question has been answered. A ruled question is contract;
    /// an open one may not be tested, in either direction.
    pub fn is_ruled(&self) -> bool {
        matches!(self.decision, Some(Decision::Ruled(_)))
    }
}

const INSTANCE: &str = include_str!("../contract/contract.toml");
const TEMPLATE: &str = include_str!("../contract/template.md");

pub const SAMPLE_INPUT: &[u8] = include_bytes!("../contract/sample.input");
pub const SAMPLE_OUTPUT: &[u8] = include_bytes!("../contract/sample.output");

impl Contract {
    pub fn load() -> Result<Self, String> {
        Self::parse(INSTANCE)
    }

    fn parse(instance: &str) -> Result<Self, String> {
        let schema::Contract {
            version,
            limits,
            grammar,
            rulings,
        } = toml::from_str(instance).map_err(|error| format!("contract/contract.toml: {error}"))?;

        // Everything the schema can state, it states. These two it cannot: a
        // proto3 oneof is optional, so "exactly one decision" is only "at most
        // one"; and protobuf has no uniqueness constraint for a repeated
        // field, which a map would give at the cost of the order the document
        // renders in. Status dispatch, a ruled entry with no ruling, an open
        // entry with no note and an unknown status were all checks here until
        // the schema made them unrepresentable.
        let mut seen = Vec::with_capacity(rulings.len());
        for ruling in &rulings {
            if seen.contains(&ruling.id) {
                return Err(format!("{} appears twice", ruling.id));
            }
            seen.push(ruling.id.clone());
            if ruling.decision.is_none() {
                return Err(format!("{} is neither ruled nor open", ruling.id));
            }
        }

        let limits = limits.ok_or("the contract states no limits")?;
        let grammar = grammar.ok_or("the contract states no grammar")?;

        Ok(Self {
            version,
            limits,
            rulings,
            grammar: Grammar::project(grammar.note, grammar.productions)?,
        })
    }

    /// The part of the version that conformance depends on: the major.
    ///
    /// Every other component moves without changing what conforms. The patch
    /// counts releases of the suite, which change what is detected rather than
    /// what is correct. A minor settles an open question without changing any
    /// conforming implementation's answer — a claim that would be false of
    /// this very surface if R26 asked for the minor as well, because then a
    /// minor bump would unconform every implementation built against the
    /// release before it. The mechanical test for a major bump found exactly
    /// that: the previous release's conforming probe failed the new suite on
    /// nothing but its version string.
    pub fn conformance_version(&self) -> String {
        self.version
            .split_once('.')
            .map_or_else(|| self.version.clone(), |(major, _)| major.to_string())
    }

    pub fn ruled(&self) -> impl Iterator<Item = &Ruling> {
        self.rulings.iter().filter(|ruling| ruling.is_ruled())
    }

    pub fn render(&self) -> Result<String, String> {
        self.render_with(TEMPLATE)
    }

    fn render_with(&self, template: &str) -> Result<String, String> {
        let body = template
            .split_once("-->\n")
            .map_or(template, |(_, rest)| rest);

        let rendered = body
            .replace("{{version}}", &self.version)
            .replace(
                "{{max_coordinate}}",
                &self.limits.max_coordinate.to_string(),
            )
            .replace(
                "{{max_instructions}}",
                &self.limits.max_instructions.to_string(),
            )
            .replace("{{grammar}}", &self.grammar.to_markdown())
            .replace("{{sample}}", &sample_block())
            .replace("{{rulings}}", &self.rulings_table())
            .replace("{{open_questions}}", &self.open_questions());

        if let Some(hole) = rendered.find("{{") {
            let tail = &rendered[hole..];
            let name = tail.find("}}").map_or(tail, |end| &tail[..=end + 1]);
            return Err(format!("template has a hole nothing filled: {name}"));
        }

        Ok(rendered)
    }

    fn rulings_table(&self) -> String {
        let mut table =
            String::from("| ID | Question the brief leaves open | Ruling |\n|---|---|---|\n");
        for entry in self.ruled() {
            let Some(Decision::Ruled(ruled)) = &entry.decision else {
                continue;
            };
            let (ruling, rationale) = (&ruled.ruling, &ruled.rationale);
            let rationale = if rationale.is_empty() {
                String::new()
            } else {
                format!(" {rationale}")
            };
            let _ = writeln!(
                table,
                "| {} | {} | {ruling}{rationale} |",
                entry.id, entry.question
            );
        }
        table.trim_end().to_string()
    }

    fn open_questions(&self) -> String {
        let mut list = String::new();
        for entry in &self.rulings {
            if let Some(Decision::Open(open)) = &entry.decision {
                let note = &open.note;
                let _ = writeln!(list, "- **{}** — {} {note}", entry.id, entry.question);
            }
        }
        list.trim_end().to_string()
    }
}

fn sample_block() -> String {
    let input = String::from_utf8_lossy(SAMPLE_INPUT);
    let output = String::from_utf8_lossy(SAMPLE_OUTPUT);
    let mut outputs = output.lines();
    let mut block = String::from("```\n");
    for line in input.lines() {
        match outputs.next() {
            Some(answer) => {
                let _ = writeln!(block, "{line:<15}{answer}");
            }
            None => {
                let _ = writeln!(block, "{line}");
            }
        }
    }
    for answer in outputs {
        let _ = writeln!(block, "{:<15}{answer}", "");
    }
    block.push_str("```");
    block
}

#[cfg(test)]
mod tests {
    use super::{Contract, Decision, Grammar, Production, Term};

    fn terminal(name: &str, literals: &[&str]) -> Production {
        Production {
            name: name.to_string(),
            literals: literals.iter().map(ToString::to_string).collect(),
            terms: Vec::new(),
            comment: String::new(),
        }
    }

    fn sequence(name: &str, terms: &[(&str, bool, bool)]) -> Production {
        Production {
            name: name.to_string(),
            literals: Vec::new(),
            terms: terms
                .iter()
                .map(|(name, is_repeated, is_optional)| Term {
                    name: (*name).to_string(),
                    is_repeated: *is_repeated,
                    is_optional: *is_optional,
                })
                .collect(),
            comment: String::new(),
        }
    }

    fn small() -> Vec<Production> {
        vec![
            terminal("orientation", &["N", "S"]),
            terminal("instruction", &["L", "F"]),
            terminal("separator", &[" "]),
            terminal("eol", &["\n"]),
            sequence(
                "ws",
                &[("separator", false, false), ("separator", true, false)],
            ),
        ]
    }

    #[test]
    fn the_terminals_are_projected_rather_than_restated() {
        let grammar = Grammar::project(String::new(), small()).unwrap();
        assert_eq!(grammar.separators, [' ']);
        assert_eq!(grammar.line_endings, ["\n"]);
        assert_eq!(grammar.orientations, ['N', 'S']);
        assert_eq!(grammar.instructions, ['L', 'F']);
    }

    #[test]
    fn a_grammar_missing_a_terminal_production_is_refused() {
        let without = small()
            .into_iter()
            .filter(|production| production.name != "separator")
            .collect();
        let refused = Grammar::project(String::new(), without);
        assert!(
            refused.is_err_and(|why| why.contains("separator")),
            "a grammar with nothing to separate tokens must not load"
        );
    }

    #[test]
    fn a_terminal_that_is_not_one_character_is_refused() {
        let mut productions = small();
        productions[0] = terminal("orientation", &["NN"]);
        let refused = Grammar::project(String::new(), productions);
        assert!(refused.is_err_and(|why| why.contains("one character")));
    }

    #[test]
    fn the_ebnf_a_reader_sees_is_rendered_from_the_productions() {
        // There is no .ebnf file to keep in step: the document is downstream
        // of the same data the generators draw from.
        let ebnf = Grammar::project(String::new(), small()).unwrap().to_ebnf();
        assert!(ebnf.contains(r#"orientation = "N" | "S" ;"#), "{ebnf}");
        assert!(
            ebnf.contains("ws          = separator , { separator } ;"),
            "{ebnf}"
        );
    }

    #[test]
    fn an_escaped_literal_renders_as_an_escape() {
        let productions = vec![
            terminal("separator", &[" ", "\t"]),
            terminal("eol", &["\n", "\r\n"]),
            terminal("orientation", &["N"]),
            terminal("instruction", &["L"]),
        ];
        let grammar = Grammar::project(String::new(), productions).unwrap();
        assert_eq!(grammar.line_endings, ["\n", "\r\n"]);
        let ebnf = grammar.to_ebnf();
        assert!(ebnf.contains(r#""\t""#), "{ebnf}");
        assert!(ebnf.contains(r#""\r\n""#), "{ebnf}");
    }

    #[test]
    fn the_injected_characters_cover_the_families_a_hand_list_misses() {
        // Each of these is whitespace to some runtime and not to this
        // contract, and none of them was in the hand-picked list this pool
        // replaced. A reader that folded only these to spaces passed every
        // gate in the suite.
        let grammar = Contract::load().unwrap().grammar;
        let pool = grammar.not_separators();
        for (character, what) in [
            ('\u{85}', "the next-line control"),
            ('\u{1f}', "an ASCII information separator"),
            ('\u{2029}', "the paragraph separator"),
            ('\u{202f}', "a narrow no-break space"),
            ('\u{2003}', "an em space"),
            ('\u{feff}', "the byte-order mark"),
        ] {
            assert!(pool.contains(&character), "{what} is not injected");
        }
        assert!(pool.len() >= 20, "only {} character(s)", pool.len());

        for admitted in [' ', '\t', '\n', '\r'] {
            assert!(
                !pool.contains(&admitted),
                "the grammar admits {admitted:?}, so it must not be injected"
            );
        }
    }

    #[test]
    fn changing_what_separates_tokens_changes_what_is_injected() {
        // The generators draw their foreign characters from what the grammar
        // does not admit. Admit one, and it stops being a defect.
        let strict = Grammar::project(String::new(), small()).unwrap();
        assert!(strict.not_separators().contains(&'\u{a0}'));

        let mut lenient = small();
        lenient[2] = terminal("separator", &[" ", "\u{a0}"]);
        let lenient = Grammar::project(String::new(), lenient).unwrap();
        assert!(!lenient.not_separators().contains(&'\u{a0}'));
    }

    #[test]
    fn the_shipped_contract_loads_and_renders() {
        let contract = Contract::load().expect("the contract this suite ships must load");
        contract
            .render()
            .expect("the contract this suite ships must render");
    }

    #[test]
    fn every_question_reaches_the_rendered_document() {
        let contract = Contract::load().unwrap();
        let document = contract.render().unwrap();
        for ruling in &contract.rulings {
            assert!(
                document.contains(&ruling.id),
                "{} is in the contract but not in the document it renders",
                ruling.id
            );
        }
    }

    #[test]
    fn open_questions_stay_out_of_the_rulings_table() {
        let contract = Contract::load().unwrap();
        let table = contract.rulings_table();
        for ruling in &contract.rulings {
            if matches!(ruling.decision, Some(Decision::Open(_))) {
                assert!(
                    !table.contains(&format!("| {} |", ruling.id)),
                    "{} is open and must not appear as a ruling",
                    ruling.id
                );
            }
        }
    }

    fn instance(rulings: &str) -> String {
        format!(
            "version = \"1.0.0\"\n\
             [limits]\nmax_coordinate = 1\nmax_instructions = 1\n\
             [grammar]\nnote = \"\"\n\
             [[grammar.productions]]\nname = \"separator\"\nliterals = [\" \"]\n\
             [[grammar.productions]]\nname = \"eol\"\nliterals = [\"\\n\"]\n\
             [[grammar.productions]]\nname = \"orientation\"\nliterals = [\"N\"]\n\
             [[grammar.productions]]\nname = \"instruction\"\nliterals = [\"L\"]\n\
             {rulings}"
        )
    }

    fn ruled(id: &str) -> String {
        format!(
            "[[rulings]]\nid = \"{id}\"\nquestion = \"q\"\n\
             [rulings.decision.ruled]\nruling = \"r\"\n\n"
        )
    }

    fn open(id: &str) -> String {
        format!(
            "[[rulings]]\nid = \"{id}\"\nquestion = \"q\"\n\
             [rulings.decision.open]\nnote = \"n\"\n\n"
        )
    }

    #[test]
    fn a_ruled_question_needs_a_ruling() {
        // The schema makes this a missing field rather than a check: `ruling`
        // has no presence in proto3, so serde refuses the instance outright.
        let entries = "[[rulings]]\nid = \"R1\"\nquestion = \"q\"\n\
                       [rulings.decision.ruled]\nrationale = \"why\"\n";
        let error = Contract::parse(&instance(entries)).unwrap_err();
        assert!(error.contains("ruling"), "{error}");
    }

    #[test]
    fn an_open_question_needs_a_note() {
        let entries = "[[rulings]]\nid = \"Q1\"\nquestion = \"q\"\n\
                       [rulings.decision.open]\n";
        let error = Contract::parse(&instance(entries)).unwrap_err();
        assert!(error.contains("note"), "{error}");
    }

    #[test]
    fn a_decision_is_ruled_or_open_and_nothing_else() {
        let entries = "[[rulings]]\nid = \"R1\"\nquestion = \"q\"\n\
                       [rulings.decision.maybe]\nruling = \"r\"\n";
        let error = Contract::parse(&instance(entries)).unwrap_err();
        assert!(error.contains("maybe"), "{error}");
    }

    #[test]
    fn a_question_with_no_decision_at_all_is_refused() {
        // The one thing the schema cannot say: a proto3 oneof is optional, so
        // "exactly one decision" is only "at most one".
        let entries = "[[rulings]]\nid = \"R1\"\nquestion = \"q\"\n";
        let error = Contract::parse(&instance(entries)).unwrap_err();
        assert!(error.contains("R1"), "{error}");
        assert!(error.contains("neither"), "{error}");
    }

    #[test]
    fn an_id_cannot_be_used_twice() {
        let entries = format!("{}{}", ruled("R1"), ruled("R1"));
        let error = Contract::parse(&instance(&entries)).unwrap_err();
        assert!(error.contains("R1"), "{error}");
        assert!(error.contains("twice"), "{error}");
    }

    #[test]
    fn a_hole_nothing_fills_is_refused() {
        let contract =
            Contract::parse(&instance(&format!("{}{}", ruled("R1"), open("Q1")))).unwrap();
        let error = contract
            .render_with("-->\n{{rulings}} {{nonexistent}}\n")
            .unwrap_err();
        assert!(error.contains("{{nonexistent}}"), "{error}");
    }

    #[test]
    fn a_contract_with_no_holes_renders_its_data() {
        let contract =
            Contract::parse(&instance(&format!("{}{}", ruled("R1"), open("Q1")))).unwrap();
        let document = contract
            .render_with(
                "-->\n{{version}} {{max_coordinate}} {{max_instructions}}\n{{grammar}}\n{{rulings}}\n{{open_questions}}\n",
            )
            .unwrap();
        assert!(document.contains("1.0.0"), "{document}");
        assert!(document.contains("R1"), "{document}");
        assert!(document.contains("Q1"), "{document}");
        // The grammar block is rendered from the productions, not pasted in,
        // and the names are padded into a column as a reader expects.
        assert!(
            document
                .lines()
                .any(|line| line.starts_with("separator") && line.ends_with("= \" \" ;")),
            "{document}"
        );
    }

    #[test]
    fn the_crate_and_the_contract_are_one_version() {
        // They are released together as one artifact, so a tag means one
        // thing. Nothing can derive the crate version from the contract -
        // Cargo.toml cannot read a file - so the two are written down twice
        // and held equal here, which is the only place the duplication is
        // allowed to be.
        let contract = Contract::load().unwrap();
        assert_eq!(
            contract.version,
            env!("CARGO_PKG_VERSION"),
            "contract/limits.toml and Cargo.toml disagree about what this is"
        );
    }

    #[test]
    fn the_conformance_version_is_the_major_alone() {
        let contract = Contract::load().unwrap();
        assert_eq!(
            contract.conformance_version(),
            contract.version.split_once('.').unwrap().0,
            "every component but the major moves without changing what conforms"
        );
    }

    #[test]
    fn a_version_with_nothing_to_drop_is_left_alone() {
        let mut contract = Contract::load().unwrap();
        contract.version = "3".to_string();
        assert_eq!(contract.conformance_version(), "3");
    }
}
