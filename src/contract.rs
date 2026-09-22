use std::fmt::Write as _;

pub struct Contract {
    pub version: String,
    pub limits: Limits,
    pub rulings: Vec<Ruling>,
}

#[derive(serde::Deserialize)]
pub struct Limits {
    pub max_coordinate: u32,
    pub max_instructions: u32,
}

pub struct Ruling {
    pub id: String,
    pub question: String,
    pub decision: Decision,
}

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
        let limits: raw::Limits =
            toml::from_str(LIMITS).map_err(|error| format!("contract/limits.toml: {error}"))?;
        let rulings: raw::Rulings =
            toml::from_str(RULINGS).map_err(|error| format!("contract/rulings.toml: {error}"))?;

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
        let body = TEMPLATE
            .split_once("-->\n")
            .map_or(TEMPLATE, |(_, rest)| rest);

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
            .replace("{{grammar}}", &grammar_block())
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

fn grammar_block() -> String {
    let productions = GRAMMAR.split_once("*)\n").map_or(GRAMMAR, |(_, rest)| rest);
    format!("```ebnf\n{}\n```", productions.trim())
}
