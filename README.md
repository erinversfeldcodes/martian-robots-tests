# martian-robots-tests

This project contains the data contract and schema for the Martian Robots application and the test suite that enforces them. While it might be conceptually cleaner to break the tests and the contracts into their own repositories they are both contained here for simplicity. The implementation of the Martian Robots application depends on this project.

## The contract

For any project the problem statement is the human-understandble and human-created description of the problem we want to design solutions for. It is a combination of clear requirements and ambiguous statements that either need clarification through continued investigation and questions or definitive rulings on. Humans navigate problem statements naturally and with intuition, but dependable code generation requires translating these into contracts that can grow, change and track our decisions.

`contract/` contains this contract as code, split by what each file is for:

| File | What it holds |
|---|---|
| `contract.proto` | the schema: what a contract *is*. The Rust types the suite uses are generated from it |
| `contract.toml` | this contract's values: the version, the limits, the grammar's productions, and every question the problem statement leaves open |
| `template.md` | the narrative, with holes where the facts go |

The schema is the shape and the instance is the content, so neither restates the other. Nothing hand-writes the contract's Rust types: `build.rs` compiles the schema with `protox` and generates them with `prost-build`, which means a field added to the schema appears in the suite without anybody writing it twice, and a field removed stops compiling. A consumer in any language `protoc` targets generates its own types from the same schema.

The instance stays TOML and stays authored by hand, because the rulings are prose that people read and argue over in diffs, and no binary or comment-free format supports that. The schema is what makes a misspelled key an error instead of a silently missing value.

The limits are defined once and read everywhere: by the document, by the suite's boundary cases, and by an implementation that derives its constants from them. The grammar is productions rather than text, so the EBNF a reader sees is rendered from the same data the generators draw their terminals from — there is no grammar file to keep in step, and the characters the grammar does not admit are exactly what the rejection generator injects. To read the contract as a single, coherent document:

```
cargo run --quiet -- --contract
```

That rendering is written to stdout rather than to a tracked file to avoid needing to update the file for every change.

## Ruled and open questions

A question with `status = "ruled"` becomes part of the contract: an implementation that disagrees with the contract is wrong, and a test case is expected to pin the rule. A question with `status = "open"` is deliberately undecided: no case may test it, and an implementation may answer it however it likes. The difference is data rather than prose so that a tool can act on it.

## Getting it

Releases are tagged and can be pinned:

```
cargo install --git https://github.com/erinversfeldcodes/martian-robots-tests \
  --tag v2.1.0 --locked --bin martian-robots-verify
martian-robots-verify --version
```

`--bin` matters: the crate also builds `probe` and `misbehave`, which are implementations written to conform and to be wrong in one specific way, and neither belongs on the PATH of somebody writing their own.

An implementation can derive its own constants from the contract rather than restating them, which is the point of defining the limits once. The contract travels inside the crate, so a build dependency on a pinned tag gives an implementation exactly the limits, grammar and vocabularies the suite will grade it against:

```
[build-dependencies]
martian-robots-verify = { git = "https://github.com/erinversfeldcodes/martian-robots-tests", tag = "v2.1.0" }
```

```rust
let contract = martian_robots_verify::contract::Contract::load()?;
// contract.limits.max_coordinate, contract.grammar.instructions, ...
```

There is one version shared by both the contract and the suite as they're a single artefact. The major and minor versions are what conformance ultimately depends on, and they move by the policy in §2.7 of the contract: a change to what input is accepted or what output it produces is major, settling an open question is typically minor. The patch counts releases of the suite: a case added, a generator widened, or some other issue resolved. None of those change what conforms, only what is caught, so an implementation that was right before a patch is still right after it — which is why R26 asks a program to report the major and minor rather than the whole string.

Each release carries `CONTRACT.md`, rendered from `contract/` at that tag. No prebuilt binaries are attached yet. They would be a CI-speed optimisation for a consumer, and as yet there is no consumer whose CI time can be measured.

## Running the suite

```
cargo run --release -- --bin /path/to/martian-robots
```

One line per case, then one line per generator — or `--quiet` for failures and the summary lines alone:

```
ok   the brief's sample, byte for byte
FAIL a grid coordinate past the maximum is refused
      rejected input must produce no stdout, got "1 1 E\n"
      stdin: "51 3\n"
      case: boundaries/a-grid-coordinate-past-the-maximum-is-refused (--case <id>)
      enforces: R5
contract <version>: <n> of <n> ruled question(s) enforced
result: <n> case(s) run, <n> passed, <n> failed
spelling: <n> mission(s) x <n> rendering(s), seed <n>, <n> divergence(s), <n> refused
properties: <n> mission(s), seed <n>, <n> violation(s)
rejections: <n> of <n> attempted, seed <n>, <n> failure(s)
differential: <n> mission(s), seed <n>, <n> disagreement(s)
```

A case is a fixed input and the output the contract expects of it. Each case records which rulings it enforces, and tests refuse citations to questions that do not exist or are still open. Cases ask for what the contract fixes: a rejection must produce no stdout and some diagnostic, with any non-zero exit; a diagnostic must indicate the line where it is attributable; help output must exist. The four lines after `result:` are generators, which supply input for the tests. Three of them produce valid missions, one invalid: `--spelling` requires the same answer to one mission written several legal ways, `--properties` checks statements that must hold of any correct answer, and `--differential` compares against a second implementation written from the same contract. The fourth, `--rejections`, produces invalid input instead, where the question is whether it is refused with a diagnostic somebody can act on.

The generator seed is fixed by default so two runs of the same code grade the same corpus. The seed is documented in `tests/corpus.rs`. `--seed random` draws a wider corpus and prints the seed it chose, which replays through `--seed <n>`. In CI the fixed seed is the gate; a scheduled random run finds the next case worth adding.

### Spelling

This generator draws missions and renders each one several legal ways, for example: varying whitespace runs, line endings, leading zeros, blank separators and whether the final line is terminated. Every rendering must produce the same answer:

```
spelling: <n> mission(s) x <n> rendering(s), seed <n>, <n> divergence(s)
```

### Invariants

This generator checks statements that must hold of any correct answer, printing how many missions evaluated each one:

```
      one line per robot
      every line is canonical
      every reported position is on the grid
      a robot that cannot move reports where it started
      a robot that only moves forward stops where the world stops it
      no two robots are lost on the same cell
      the same input twice gives the same answer
      appending a robot does not change the robots before it
```

### Rejections

This generator breaks valid missions on purpose: a coordinate past a limit, a start off the world, a letter outside a vocabulary, a token too many or too few, a line removed or inserted, bytes that are not text, two independent problems on two robots, and a character the grammar does not admit as whitespace.

```
rejections: <n> of <n> attempted, seed <n>, <n> failure(s)
```

The expected line and the admissible rulings come from how the mutation was built rather than from what the implementation said, so an implementation cannot teach the suite to accept its own answer. Every mutation is checked against the published grammar to be genuinely invalid before it is used.

### Differential

This generator compares answers with a second implementation written from the same contract:

```
differential: <n> mission(s), seed <n>, <n> disagreement(s)
```

| Exit code | Meaning |
|---|---|
| 0 | the implementation conforms |
| 1 | the implementation does not conform |
| 2 | the suite could not run: bad arguments, no such implementation, or an incoherent contract |
