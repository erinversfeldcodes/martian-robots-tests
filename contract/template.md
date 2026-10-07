<!--
  Placeholders: {{version}}, {{max_coordinate}}, {{max_instructions}},
  {{grammar}}, {{sample}}, {{rulings}}, {{open_questions}}.
-->
# The Martian Robots CLI — the brief, the I/O contract, and the ambiguity rulings

This document is the complete interface between the problem and every
implementation and test of it. §1 restates the brief; §2 is the formal I/O
contract the command-line program implements; §3 rules on every ambiguity we
have found in the brief; §4 names the questions we have deliberately left open.

It is written for two audiences who never read each other's work: the people
implementing the program, and the people writing the conformance suite that
grades it. The suite is authored from this document and the original problem
statement — never from an implementation's source. That is the point of writing
it down: where the two sides disagree, the disagreement is a defect in this
document, and it is fixed here rather than in whichever side happened to be
written second.

**Contract version: {{version}}.** This version covers the command-line surface
only: text on stdin, text on stdout, diagnostics on stderr, an exit code. Other
ways of reaching the same simulation — an HTTP API, agent tooling, a browser
playground — are out of scope, and would arrive as a later version of this
document rather than as an implementation detail of this one.

---

## 1. The brief (paraphrased)

Mars is a bounded rectangular grid. Robots occupy integer coordinates with an
orientation (N, S, E, W) and execute instruction strings: `L` (turn left 90°),
`R` (turn right 90°), `F` (move one cell forward). North is from `(x, y)`
toward `(x, y+1)`. Provision should be made for additional instruction types in
future.

A robot moving off the grid is lost forever, but leaves a *scent* at the last
on-grid cell it occupied; an instruction that would move a later robot off the
world from a scented cell is ignored. Robots execute sequentially — each
finishes before the next begins.

Input: the first line is the upper-right coordinate of the world (lower-left is
`0 0`); then two lines per robot (initial position and orientation; instruction
string). Coordinates are at most {{max_coordinate}}; instruction strings are
shorter than 100 characters. Output: each robot's final position and
orientation, with ` LOST` appended if it fell off.

Sample input → output, verbatim from the brief:

{{sample}}

## 2. The I/O contract

### 2.1 Grammar

ASCII text on stdin. The productions below are rendered from the grammar in
`contract/contract.toml`, which is also what the conformance suite reads to
generate the inputs it grades with: there is no separate grammar file to keep
in step with them.

{{grammar}}

Semantic constraints, checked after parse: each `number` is at most
{{max_coordinate}} (R5); robot start positions lie on the grid (R1); a robot
carries at most {{max_instructions}} instructions (R6).

### 2.2 Semantics

- The world is the inclusive cell set `(0..=max_x, 0..=max_y)`.
- `L` and `R` rotate 90° in place; `F` translates one cell along the
  orientation.
- An `F` that would leave the world: if the current cell is scented, the
  instruction is ignored (see R9 for the scent model); otherwise the robot is
  lost — it vanishes, the cell becomes scented, and its remaining instructions
  are discarded.
- Blocking world-leaving moves is scent's only effect (R23). Moving onto or
  through a scented cell is unremarkable, and a robot may start on one; scent
  never rejects input, alters movement that stays on-grid, or marks a robot.
- Robots run strictly sequentially, in input order.

### 2.3 Output

For each robot, in input order, one line to stdout: `<x> <y> <orientation>` —
the final on-grid position — with ` LOST` appended if the robot was lost
(position and orientation are those held at the moment of loss). Single ASCII
spaces between tokens, LF line endings, no trailing whitespace, no blank lines.
Numbers are canonical decimal (R16). The sample in §1 is normative.

### 2.4 Failure

Invalid input — a grammar violation or a semantic-constraint violation —
produces diagnostics on stderr naming the offending line and rule, every
violation found in one pass (R25), **no output on stdout, and a non-zero exit
code**. Framing is positional (§2.1), so a missing or extra line shifts the
blocks after it, and the lines it shifts are diagnosed where they fall.

Execution is all-or-nothing. Robot blocks are coupled by scent: running "just
the valid robots" could silently change the outcomes of the robots that *are*
valid, because a robot that was skipped might have scented a cell the next one
walks off. An incomplete answer would not be incomplete; it would be wrong.

Valid input exits 0.

### 2.5 Diagnostics content

A diagnostic names its input line as `line N` (1-based, physical) **when the
violation is attributable to a physical line**. Violations with no such line —
empty or blank-only input, which is missing its grid line — carry no line
reference, and a missing instruction line anchors to the position line that
demanded it (R13).

Where one or more numbered rulings govern a rejection, **at least one**
governing tag appears as `(R#)`. Overlapping rulings need not all be
enumerated: an off-grid coordinate that is also over the maximum may cite
either.

Exact prose is implementation-chosen. The line reference and the ruling tag are
contract; the sentence around them is not.

### 2.6 Invocation

The program is a stdin/stdout filter when invoked bare. Its argument surface is
R20, R21, R24 and R26: a help flag alone prints usage to stdout, exits 0, and
reads no stdin; a version flag alone reports the contract version on the same
terms; anything else — including either flag given alongside another argument —
is a usage error on stderr with a non-zero exit and no stdout.

Input that is not valid UTF-8 is invalid input under §2.4 (R22). That is a rule
about stdin rather than about arguments, and is listed here only because it is
the other thing a program must get right before it has parsed anything.

### 2.7 Versioning

This contract carries a semver version, defined in `contract/limits.toml`, and
an implementation reports the version it implements through the surface R26
gives it.

- A change that alters what input is accepted, or what output a given input
  produces, is a **major** bump. Implementations and suites pinned to the
  previous major stay correct against it.
- Settling a question §4 leaves open, without changing the answer any
  conforming implementation already gives, is a **minor** bump.
- Extending the contract to a surface it does not yet cover is a **minor** bump
  that adds a section rather than rewriting this one.
- Releasing the suite that enforces this contract — a case added, a generator
  widened, a false green closed — is a **patch** bump. None of those change what
  conforms, only what is caught, so an implementation that was correct before a
  patch is correct after it. This is why R26 asks for the major and minor rather
  than the whole version: a suite release must not unconform a correct program.

The contract and the suite that enforces it are released together under this
one version, because they are one artifact — a rule nobody checks is a
suggestion. A consumer pins one number.

## 3. Ambiguity rulings

Every ruling below answers a question the brief leaves open, and every one of
them is testable. An implementation that disagrees with a ruling is wrong; a
*reader* who disagrees has found a defect in this document, and the loop that
fixes it — finding, ruling, revision, cases — is how a contract gets hard
enough to build against.

{{rulings}}

## 4. Open questions

These are known gaps, left open deliberately. The conformance suite does not
test them, and an implementation may answer them however it likes without being
wrong. They are recorded as data rather than as a closing paragraph so that a
tool can tell the difference between a question we answered and one we did not.

{{open_questions}}

## 5. Revision history

| Version | Date | Change |
|---|---|---|
| 1.0.0 | 2026-09-22 | Initial contract for the command-line surface: the grammar and its framing rules, the simulation's semantics, the output format, the failure discipline, the content of diagnostics, the invocation surface, and rulings R1–R25. |
| 2.0.0 | 2026-09-29 | Three places where the rule was not where a reader would look for it. R4 now says spaces and tabs in all three of its clauses and names the grammar, so the rejection of other whitespace no longer rests on Q6's note — Q6 keeps only the characterisation question, which is genuinely open. R12 is stated as a rule about productions rather than about surplus tokens, so a short line and a malformed number are governed rather than arguable. R26 gives §2.7's version obligation a surface to be satisfied by; before it, `--version` was a usage error and the sentence could not be met. Major, by §2.7's own test: R26 changes what an input produces, and R12 narrows the outputs a conforming implementation may give. |
| 2.1.0 | 2026-09-30 | The patch component is given a meaning: it counts releases of the suite, so the contract and the suite it comes with carry one version between them. R26 follows it, asking for the major and minor rather than the whole string, so that a suite release closing a false green cannot unconform an implementation that was already right. Minor, by §2.7's own test: it settles how the third number is read and broadens what R26 accepts without changing the answer any conforming implementation already gives. R7 is scoped where it was being read too widely: its question is about an instruction string or an orientation, so a character that is not a separator on the grid line — a line carrying neither — is governed by R4 and R12 and not by R7. Q6's note says so rather than offering all three everywhere. R25 gains a companion demand: where a rejection carries independent violations, each is expected to cite a ruling that governs it, rather than one tag standing in for both. Both narrow the outputs a conforming implementation may give, which §2.7 makes a major bump — recorded here as a minor one because this is the first tagged release, and the rows above it are revisions of an unpublished document rather than releases anybody could have built against. |
| 2.2.0 | 2026-10-07 | The contract gains a schema. `contract/contract.proto` states what a contract is, and the Rust types the suite uses are generated from it, so the shape is written once and the suite cannot disagree with it; `contract/contract.toml` states this contract's values and is still authored by hand, because the rulings are prose that people read in diffs. The grammar's productions become data and the EBNF a reader sees is rendered from them, which retires `contract/grammar.ebnf` as a file to keep in step. A `separator` production is named so that `ws` is `separator , { separator }` rather than an inline alternation; that admits exactly what it admitted before. Nothing about conformance changes: both conforming implementations score what they scored, and all four generators stay clean. Minor, not major, by §2.7's own test. |
