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
| `grammar.ebnf` | the input grammar, which the suite's generators read |
| `template.md` | the narrative, with holes where the facts go |

Nothing states a fact twice. The limits are defined once and read everywhere:
by the prose, by the suite's boundary cases, and by an implementation that
generates its constants from them. To read it as one document:

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

One line per case, then a summary — or `--quiet` for the summary alone:

```
contract 1.0.0: 25 ruled question(s) to enforce
result: 0 case(s) run, 0 passed, 0 failed
```

| Exit code | Meaning |
|---|---|
| 0 | the implementation conforms |
| 1 | the implementation does not conform |
| 2 | the suite could not run: bad arguments, no such implementation, or an incoherent contract |

## Scope

The command-line surface only: text on stdin, text on stdout, diagnostics on
stderr, an exit code.
