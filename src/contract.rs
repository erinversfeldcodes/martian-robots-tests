use std::fmt::Write as _;

#[derive(Debug)]
pub struct Contract {
    pub version: String,
    pub limits: Limits,
    pub rulings: Vec<Ruling>,
}

#[derive(Debug, serde::Deserialize)]
pub struct Limits {
    pub max_coordinate: u32,
    pub max_instructions: u32,
}

#[derive(Debug)]
pub struct Ruling {
    pub id: String,
    pub question: String,
    pub decision: Decision,
}

#[derive(Debug)]
pub enum Decision {
    Ruled {
        ruling: String,
        rationale: Option<String>,
    },
    Open {
        note: String,
    },
}

impl Ruling {
    pub fn is_ruled(&self) -> bool {
        matches!(self.decision, Decision::Ruled { .. })
    }
}

const LIMITS: &str = include_str!("../contract/limits.toml");
const RULINGS: &str = include_str!("../contract/rulings.toml");
const GRAMMAR: &str = include_str!("../contract/grammar.ebnf");
const TEMPLATE: &str = include_str!("../contract/template.md");

pub const SAMPLE_INPUT: &[u8] = include_bytes!("../contract/sample.input");
pub const SAMPLE_OUTPUT: &[u8] = include_bytes!("../contract/sample.output");

mod raw {
    #[derive(serde::Deserialize)]
    pub struct Limits {
        pub version: String,
        pub limits: super::Limits,
    }

    #[derive(serde::Deserialize)]
    pub struct Rulings {
        pub ruling: Vec<Ruling>,
    }

    #[derive(serde::Deserialize)]
    pub struct Ruling {
        pub id: String,
        pub status: String,
        pub question: String,
        #[serde(rename = "ruling")]
        pub statement: Option<String>,
        pub rationale: Option<String>,
        pub note: Option<String>,
    }
}

impl Contract {
    pub fn load() -> Result<Self, String> {
        Self::parse(LIMITS, RULINGS)
    }

    fn parse(limits: &str, rulings: &str) -> Result<Self, String> {
        let limits: raw::Limits =
            toml::from_str(limits).map_err(|error| format!("contract/limits.toml: {error}"))?;
        let rulings: raw::Rulings =
            toml::from_str(rulings).map_err(|error| format!("contract/rulings.toml: {error}"))?;

        let mut seen = Vec::new();
        let mut parsed = Vec::with_capacity(rulings.ruling.len());
        for entry in rulings.ruling {
            if seen.contains(&entry.id) {
                return Err(format!("{} appears twice", entry.id));
            }
            seen.push(entry.id.clone());

            let decision = match entry.status.as_str() {
                "ruled" => Decision::Ruled {
                    ruling: entry
                        .statement
                        .ok_or_else(|| format!("{} is ruled but has no ruling", entry.id))?,
                    rationale: entry.rationale,
                },
                "open" => Decision::Open {
                    note: entry
                        .note
                        .ok_or_else(|| format!("{} is open but has no note", entry.id))?,
                },
                other => {
                    return Err(format!(
                        "{} has status {other:?}, which is neither \"ruled\" nor \"open\"",
                        entry.id
                    ));
                }
            };

            parsed.push(Ruling {
                id: entry.id,
                question: entry.question,
                decision,
            });
        }

        Ok(Self {
            version: limits.version,
            limits: limits.limits,
            rulings: parsed,
        })
    }

    pub fn ruled(&self) -> impl Iterator<Item = &Ruling> {
        self.rulings.iter().filter(|ruling| ruling.is_ruled())
    }

    pub fn render(&self) -> Result<String, String> {
        self.render_with(TEMPLATE, GRAMMAR)
    }

    fn render_with(&self, template: &str, grammar: &str) -> Result<String, String> {
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
            .replace("{{grammar}}", &grammar_block(grammar))
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
            let Decision::Ruled { ruling, rationale } = &entry.decision else {
                continue;
            };
            let rationale = rationale
                .as_deref()
                .map_or_else(String::new, |rationale| format!(" {rationale}"));
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
            if let Decision::Open { note } = &entry.decision {
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

fn grammar_block(grammar: &str) -> String {
    let productions = grammar.split_once("*)\n").map_or(grammar, |(_, rest)| rest);
    format!("```ebnf\n{}\n```", productions.trim())
}

#[cfg(test)]
mod tests {
    use super::{Contract, Decision};

    const LIMITS: &str =
        "version = \"1.0.0\"\n[limits]\nmax_coordinate = 1\nmax_instructions = 1\n";

    fn ruled(id: &str) -> String {
        format!(
            "[[ruling]]\nid = \"{id}\"\nstatus = \"ruled\"\nquestion = \"q\"\nruling = \"r\"\n\n"
        )
    }

    fn open(id: &str) -> String {
        format!("[[ruling]]\nid = \"{id}\"\nstatus = \"open\"\nquestion = \"q\"\nnote = \"n\"\n\n")
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
            if matches!(ruling.decision, Decision::Open { .. }) {
                assert!(
                    !table.contains(&format!("| {} |", ruling.id)),
                    "{} is open and must not appear as a ruling",
                    ruling.id
                );
            }
        }
    }

    #[test]
    fn a_ruled_question_needs_a_ruling() {
        let entries = "[[ruling]]\nid = \"R1\"\nstatus = \"ruled\"\nquestion = \"q\"\n";
        let error = Contract::parse(LIMITS, entries).unwrap_err();
        assert!(error.contains("R1"), "{error}");
        assert!(error.contains("no ruling"), "{error}");
    }

    #[test]
    fn an_open_question_needs_a_note() {
        let entries = "[[ruling]]\nid = \"Q1\"\nstatus = \"open\"\nquestion = \"q\"\n";
        let error = Contract::parse(LIMITS, entries).unwrap_err();
        assert!(error.contains("Q1"), "{error}");
        assert!(error.contains("no note"), "{error}");
    }

    #[test]
    fn a_status_is_ruled_or_open_and_nothing_else() {
        let entries = "[[ruling]]\nid = \"R1\"\nstatus = \"maybe\"\nquestion = \"q\"\n";
        let error = Contract::parse(LIMITS, entries).unwrap_err();
        assert!(error.contains("R1"), "{error}");
        assert!(error.contains("maybe"), "{error}");
    }

    #[test]
    fn an_id_cannot_be_used_twice() {
        let entries = format!("{}{}", ruled("R1"), ruled("R1"));
        let error = Contract::parse(LIMITS, &entries).unwrap_err();
        assert!(error.contains("R1"), "{error}");
        assert!(error.contains("twice"), "{error}");
    }

    #[test]
    fn a_hole_nothing_fills_is_refused() {
        let contract = Contract::parse(LIMITS, &format!("{}{}", ruled("R1"), open("Q1"))).unwrap();
        let error = contract
            .render_with("-->\n{{rulings}} {{nonexistent}}\n", "*)\nx = \"y\" ;\n")
            .unwrap_err();
        assert!(error.contains("{{nonexistent}}"), "{error}");
    }

    #[test]
    fn a_contract_with_no_holes_renders_its_data() {
        let contract = Contract::parse(LIMITS, &format!("{}{}", ruled("R1"), open("Q1"))).unwrap();
        let document = contract
            .render_with(
                "-->\n{{version}} {{max_coordinate}} {{max_instructions}}\n{{grammar}}\n{{rulings}}\n{{open_questions}}\n",
                "*)\nx = \"y\" ;\n",
            )
            .unwrap();
        assert!(document.contains("1.0.0"), "{document}");
        assert!(document.contains("R1"), "{document}");
        assert!(document.contains("Q1"), "{document}");
        assert!(document.contains("x = \"y\" ;"), "{document}");
    }
}
