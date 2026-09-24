# martian-robots-tests

The contract for the Martian Robots CLI, and the conformance suite that
enforces it.

Both live here because they are one artifact. A rule nobody checks is a
suggestion; a check that cites no rule is a preference. An implementation
depends on this repository — this repository depends on nothing.

## The contract

`contract/` is the contract, as data:

| File | What it holds |
|---|---|
| `limits.toml` | the version, and the numbers the brief gives us |
| `rulings.toml` | every question the brief leaves open, ruled or deliberately open |
| `grammar.ebnf` | the input grammar: the generators read their separators, line endings and vocabularies from it |
| `template.md` | the narrative, with holes where the facts go |

Nothing states a fact twice. The limits are defined once and read everywhere:
by the prose, by the suite's boundary cases, and by an implementation that
generates its constants from them. The same goes for the grammar — what
separates tokens, what ends a line, and which letters the two vocabularies
hold are parsed out of `grammar.ebnf`, so the generators cannot drift from the
published grammar, and the characters it does *not* admit are exactly what the
rejection generator injects. To read it as one document:

```
cargo run --quiet -- --contract
```

That rendering is written to stdout, never to a file in the tree — a generated
file in a repository is a second copy of the truth that can be edited and can
go stale.

## Ruled and open

A question with `status = "ruled"` is contract: an implementation that
disagrees is wrong, and a case is expected to pin it. A question with
`status = "open"` is deliberately undecided: no case may test it, and an
implementation may answer it however it likes. The difference is data rather
than prose so that a tool can act on it.

## Running the suite

```
cargo run --release -- --bin /path/to/martian-robots
```

One line per case, then a summary — or `--quiet` for failures and the summary
alone:

```
ok   the brief's sample, byte for byte
FAIL a grid coordinate past the maximum is refused
      rejected input must produce no stdout, got "1 1 E\n"
      stdin: "51 3\n"
      enforces: R5
contract 1.0.0: 9 of 25 ruled question(s) enforced
result: 18 case(s) run, 17 passed, 1 failed
```

The coverage line is deliberately unflattering: it counts the ruled questions
some case cites, so the distance between the contract and the suite is visible
on every run rather than discoverable by reading both.

## Generated missions

A catalogue is exactly as strong as the shapes someone thought to write down.
After the cases, the suite draws missions and writes each one several legal
ways — different whitespace runs, line endings, leading zeros, blank
separators, a final line with or without its ending — and requires the same
answer from all of them:

```
spelling: 60 mission(s) x 4 rendering(s), seed 1, 0 divergence(s)
```

This asks a program to agree with itself, so it needs no reference
implementation and still bites when the suite and the program share a wrong
belief. `--spelling <n>` sets how many missions; `--seed <n>` replays a
corpus, and a run without one picks a seed and prints it.

The generator checks itself on every run, not only in its own tests: each
rendering must read back as the mission it came from, and must stay out of the
shapes the contract leaves open — mixed line endings within one input (Q1) and
an unterminated final line of only whitespace (Q4). A generator that strayed
would be testing something nobody has decided.

## Invariants

Agreeing with yourself is not the same as being right: a program that reports
every robot at `0 0 N` agrees with itself perfectly. So the suite also states
things that are true of an answer on its own, and checks those:

```
        40 one line per robot, in input order
        40 every line is canonical
        32 every reported position is on the grid
        25 a robot that cannot move reports where it started
         6 a robot that only moves forward stops where the world stops it
        12 no two robots are lost on the same cell
        40 the same input twice gives the same answer
        40 appending a robot does not change the robots before it
properties: 40 mission(s), seed 1, 0 violation(s)
```

None of these consults a second implementation, which is what makes them the
answer to a suite and a program sharing a wrong belief. Two need no simulation
at all: a robot whose instructions contain no `F` cannot have moved, and since
a loss scents the cell it happened on and a scented cell blocks the next
departure, no two robots can ever report a loss on the same cell.

The counts are the point of the display. A predicate that never evaluates
reads exactly like one that always holds, so the suite prints how many
missions each one actually judged, and missions are drawn with degenerate
instruction shapes on purpose — a uniformly random instruction string is
all-`F` about once in 3^n, so without the bias the strongest predicates would
almost never fire. `--properties <n>` sets the corpus size.

## Generated rejections

The last mode breaks valid missions on purpose, one way at a time: a
coordinate past a limit, a start off the world, a letter outside a vocabulary,
a token too many or too few, a separator the grammar does not have.

```
rejections: 31 mutation(s), seed 1, 0 failure(s)
```

Two things make this more than a fuzzer. The expected line and the admissible
rulings are derived from *how the mutation was built*, never from what the
program said, so a program cannot teach the suite to accept its own answer.
And the diagnostic is judged, not just the exit code — a mode that checked
only for a non-zero exit and an empty stdout would let a program reject in
silence over a much larger space than any catalogue.

Each mutation is checked to be genuinely invalid before it is used. A mutation
that quietly left a valid mission behind would turn a rejection test into a
much weaker success test that still reported green. `--rejections <n>` sets
how many to attempt.

## Differential

Last, the suite compares answers with a second implementation written from the
same contract:

```
differential: 60 mission(s), seed 1, 0 disagreement(s)
```

This is the only mode that catches a plainly wrong answer to a mixed
instruction string. A robot that ends one cell east of where it belongs agrees
with itself across every respelling and satisfies every invariant; nothing
short of another implementation notices.

What it proves is **agreement**, and the difference matters. A disagreement is
a defect in the program under test, or a place where the contract admits two
readings and the two sides took different ones — both are findings, and
neither side is automatically the wrong one. The reference is checked against
the one piece of external truth available: the brief's own published sample,
input and output, written by somebody who wrote neither implementation.

| Exit code | Meaning |
|---|---|
| 0 | the implementation conforms |
| 1 | the implementation does not conform |
| 2 | the suite could not run: bad arguments, no such implementation, or an incoherent contract |

## What a case may assume

A case cites the rulings it enforces, and a test refuses a citation to a
question that does not exist or is still open — `status = "open"` means no case
may depend on it, in either direction.

Judgement asks for exactly what the contract fixes and nothing more: output is
byte-exact and stderr is ignored on success (Q2); a rejection must produce no
stdout and some diagnostic, with any non-zero exit; a diagnostic must name its
line where one is attributable and must not name one where none is; help output
must exist, and no ruling constrains its wording, so neither does the suite.

## Scope

The command-line surface only: text on stdin, text on stdout, diagnostics on
stderr, an exit code.
