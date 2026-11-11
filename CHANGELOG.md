# Changelog

## 1.4.0 - 2026-11-11

Fixes from the Specula M5 run.

- Summaries may be up to 200 characters (was 100), for `new`, `edit`, and
  `check` alike. About six lanes lost a retry to the old limit. Longer is
  still an error, never a silent cut, and it now says how long the summary
  is and what the limit is. A summary with a line break is refused too
  (before, it broke the note's frontmatter). The limit is in the
  `relatum_new` and `relatum_edit` input schemas (`maxLength: 200`) and
  descriptions. A store whose summaries run past 100 characters fails
  `check` under Relatum 1.3.0 and Examen.
- `relatum_supersede` with a new note that does not exist says to write it
  with `relatum_new` first (that was the failed supersede after a failed
  new).
- `ls --since` / `relatum_ls` `since`: a bare date now means local
  midnight, the calendar `updated` uses. 1.3.0 read it as UTC midnight, so
  east of UTC `since <today>` was in the future for the first hours of the
  day and returned nothing, and notes from early that day were missed (the
  Specula `since: "2026-11-06"` report, at UTC+11). A date-time may now
  carry `+HH:MM` / `-HH:MM` or no zone (local), seconds are optional, and a
  fraction is ignored. `YYYY-MM-DDTHH:MM:SSZ` reads as before.
- `relatum_get` takes `include_superseded`: each returned note also carries
  `superseded`, every note whose supersede chain ends at it (`id`,
  `superseded_by`, `summary`, `body`), oldest first. Handoffs written as
  deltas keep the earlier rounds in reach, whether you ask for the old id
  or the live one. Without it the reply is unchanged.
- Tool descriptions and the initialize instructions say when notes beat
  grep or asking: a handoff note after a context refresh or before you
  stop, an api note for code other agents call. The `relatum_ls` argument
  descriptions lose stray runs of spaces.
- CI and the parity tools read Examen 0.3.0 from the `pre-rust-python-legacy`
  tag; the `python-legacy` branch is gone from GitHub.
- `relatum --version` (and `relatum-mcp`, `swarmnotes`; also `-V`) prints
  the name and version. Help output is unchanged.
- The parity harness drops descriptions and `maxLength` from the
  `tools/list` comparison (Relatum writes its own) and `relatum_get`'s
  `include_superseded` from its schema; CONTRIBUTING.md lists the 1.4.0
  differences.

## 1.3.0 - 2026-11-05

- Agents choose tool groups per project: the first `agent-tools` block in
  the project's AGENTS.md (where `.swarmnotes/` is, else the working
  directory), lines `relatum: tools <list>` / `relatum: disable <list>`.
  Env variables override it per key (`RELATUM_TOOLS` the tools line,
  `RELATUM_DISABLE` the disable line).
- Mistakes in the block never stop the server: the line is ignored and the
  initialize instructions carry one warning line. Env mistakes still exit 2.
- The server advertises `tools.listChanged`, stats AGENTS.md on every
  request, and sends `notifications/tools/list_changed` before the response
  when the exposed tools change, so a mid-session edit applies without a
  restart.
- When an env switch or an AGENTS.md line for relatum is present, the
  initialize instructions end with one line naming the groups on and off
  and how to change them. With neither, the output is byte-identical to
  1.2.0 except `tools.listChanged`, now always `true`.
- A block nested inside another fenced block (a documentation example) is
  text, not config; the README, SPEC, snippet, and skill examples switch
  nothing.
- AGENTS.snippet.md and the skill gain "Choosing groups"; the README a short
  section on the block.

## 1.2.0 - 2026-11-05

- MCP tool groups, per the suite's convention: `core` (`relatum_ls`,
  `relatum_get`, `relatum_new`; always on), `revise` (`relatum_edit`,
  `relatum_supersede`), `lint` (`relatum_check`), `setup`
  (`relatum_init`). `RELATUM_TOOLS` is an allowlist of groups or tools,
  `RELATUM_DISABLE` hides groups or tools after it. A hidden tool is absent
  from `tools/list`, and a call to it returns one line naming the variable
  to change. An unknown name, or disabling `core`, exits 2 at startup with
  one stderr line. The initialize instructions gain one line listing the
  groups on and off when either variable is set. With neither set, the
  MCP output is byte-identical to 1.1.0. The CLI ignores both.

## 1.1.0 - 2026-11-05

- Notes are written whole: a temp file is hard-linked (new) or renamed
  (supersede) into place, so a concurrent reader never sees a torn note.
  On Windows the replace uses POSIX rename semantics, so busy readers no
  longer starve a writer on Rust 1.83.
- MCP tool descriptions drop docstring indentation, tool result text is
  compact JSON, and errors for a missing store, an empty scope, and an
  invalid id say what to call next. The CLI is unchanged.
- The parity harness and benchmark are std-only Rust (`cargo run --example
  parity` / `bench`); the Python scripts are removed.
- README rewritten for newcomers; parity details moved to CONTRIBUTING.md.
- `ls` and `check` read large stores on up to eight threads: on Windows a
  2000-note `ls` or `check` drops from about 150 ms to about 55 ms.
- `ls --limit N`, `--since TIME`, and `--details` (and `relatum_ls` `limit`,
  `since`, `details`): opt-in, default output unchanged.
- `relatum_edit`: a note's author corrects its summary, body, or tags in
  place. Errors for an existing id point to `relatum_edit` and
  `relatum_supersede`.
- MCP argument errors are one line (field, value, expected, how to call
  again) instead of pydantic's report. Intentional difference from Examen.
- The bench's memory row works on Windows and macOS; CI runs the bench on
  all three OSes and checks the table.

## 1.0.1 - 2026-11-04

- npm packages `@iron_adamant/relatum` and five platform packages
  (`linux-x64`, `linux-arm64`, `darwin-x64`, `darwin-arm64`, `windows-x64`).
  The launcher has no npm dependencies. `npx -y @iron_adamant/relatum mcp`
  starts the stdio server.
- `relatum mcp` starts the same stdio server as `relatum-mcp`.
- `server.json` names the server `io.github.IronAdamant/relatum` for the MCP
  Registry.
- The `v*` release workflow still attaches the five archives and
  `SHA256SUMS` to the GitHub Release, then publishes the npm packages. That
  job requires the `NPM_TOKEN` repository secret and fails if it is missing.
  `workflow_dispatch` can publish npm for an existing tag without rebuilding.
- README section "Migrating from Examen (Python)".

## 1.0.0 - 2026-11-04

- Rust port of the Examen 0.3.0 CLI and stdio MCP server. No Cargo crates.
  The note format is unchanged from v0.2 and is treated as stable.
- The crate, the CLI, and the MCP server are named `relatum`. Tools are
  `relatum_init`, `relatum_ls`, `relatum_get`, `relatum_new`,
  `relatum_supersede`, and `relatum_check`. `swarmnotes` remains a CLI alias.
- `.swarmnotes/` and `SWARMNOTES_DIR` are unchanged, so existing stores load
  with no migration. `init` still seeds `author: examen` on
  `_shared/report-format`.
- The Python implementation is the `python-legacy` branch. This tree does
  not publish to crates.io, PyPI, or npm.
- Release workflow on `v*` tags builds linux x64, linux arm64, macOS x64,
  macOS arm64, and Windows x64 binaries with `SHA256SUMS`.

## 0.3.0 - 2026-10-02

- Optional stdio MCP server over the same `.swarmnotes/` files as the CLI.
  Install from GitHub with
  `pip install "examen[mcp] @ git+https://github.com/IronAdamant/Examen"`
  (official MCP Python SDK, Python 3.10+). This project is not published on
  PyPI; that name belongs to a different package. The core `examen` command
  stays zero-dependency.
- `examen_init` creates a store only in the current working directory or at
  `SWARMNOTES_DIR`.
- Tools: `examen_init`, `examen_ls`, `examen_get`, `examen_new`,
  `examen_supersede`, `examen_check`.
- Console script `examen-mcp`, or `python -m examen_mcp`.
- Agent skill and `AGENTS.snippet.md` prefer the MCP tools when a host
  exposes them.

## 0.2.0 - 2026-10-02

- Public CLI: `init`, `new`, `ls`, `get`, `supersede`, `check`.
- `swarmnotes` is an alias of `examen`.
- Pass-by-reference note format ([SPEC.md](SPEC.md)). Notes live in
  `.swarmnotes/`.
- `ls` hides superseded notes unless `--all`. `get` follows `superseded_by`,
  including multi-hop chains.
- Lint fails on broken frontmatter, bad kinds, summaries, tags, dates, empty
  bodies, and supersede chains that do not end on a live note. Bodies over
  300 words warn.
