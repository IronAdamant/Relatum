# Contributing

The note format in [SPEC.md](SPEC.md) is the product. Briefs stay
`Task` / `Read first` / `Your scope` / `Done when`. The CLI and the MCP
server are thin conveniences over the same `.swarmnotes/` files.

- Rust standard library only. `Cargo.toml` has no dependencies and no
  dev-dependencies. Do not add a crate.
- The package `name` in `Cargo.toml` is the rename point for the CLI, the
  `{name}-mcp` binary, and the `{name}_*` tools. `swarmnotes` stays as the
  legacy alias.
- Do not rename `.swarmnotes/`. `SWARMNOTES_DIR` stays the environment
  variable. Stores written by Examen 0.3.0 must keep loading.
- No hosted service, account, or second store.
- Do not publish to crates.io or PyPI. `publish` stays false in Cargo.toml.
  npm packages under `npm/` are published by the release workflow.
- Docs use Australian spelling and plain ASCII.

## Tests

```sh
cargo test --locked
cargo build --release --locked
cargo run --release --locked --example parity -- target/release/relatum target/release/relatum-mcp
```

On Windows add `.exe` to both binary paths. Without MSVC build tools, use
the `x86_64-pc-windows-gnu` toolchain; if another MinGW `gcc` is on `PATH`,
build with `RUSTFLAGS="-C link-self-contained=yes"` so Rust links with its
bundled linker.

## Parity with Examen 0.3.0

`parity/harness.rs` (the `parity` example) runs this binary and Examen 0.3.0
side by side and exits 1 on any difference. `parity/bench.rs` (the `bench`
example) prints the benchmark table in the README on Linux, macOS, and
Windows; the memory row is the peak resident set (`/proc` VmHWM, `wait4`
ru_maxrss, or the peak working set). On Windows point `PARITY_PYTHON` at a
real interpreter, not a venv's `python.exe`, which is a launcher. CI runs
the harness and the bench on all three and checks every table row. Both
are std only. The frozen Python release is
the reference oracle: the harness loads `examen.py` from the
`pre-rust-python-legacy` tag (`git fetch origin tag pre-rust-python-legacy`;
it was the `python-legacy` branch) into a Python 3
process, and runs `python -m examen_mcp`, which needs
`pip install "mcp==2.3.0"`. Set `PARITY_PYTHON` to choose the interpreter;
otherwise `python3`, then `python`. Nothing uses the network after that.
GitHub Actions runs `cargo test` and the harness on Linux, macOS, and
Windows.

The harness compares CLI exit codes, stdout, stderr, and note file bytes,
and the MCP responses, stderr log, and files. It normalises only these:

- Examen's program name is forced to `relatum` or `swarmnotes`. MCP text
  has `relatum_` rewritten to `examen_`, then `Relatum` to `Examen`, then
  `relatum` to `examen`. `1.0.1` and `1.0.0` become `0.3.0`
  (`serverInfo.version`).
- **Intentional difference: argument validation errors.** Examen printed
  pydantic's multi-line report with documentation URLs. Relatum prints one
  line, `invalid arguments: <field> must be <type>, got <value>. Call
  <tool> again with <field> like <example>.` or `invalid arguments:
  missing required argument(s) <fields>. <tool> needs <all required>.`
  The harness compares these by shape only: an error result whose text is
  pydantic's report on one side and one `invalid arguments:` line on the
  other becomes the same placeholder. `isError`, the tool name, and the
  `rejected arguments: [...]` line both servers log to stderr (the exact
  fields) are still compared. CLI argparse errors are unchanged.
- The final component of each temp directory becomes `$DIR`.
- CR and LF are folded on CLI pipes and MCP stderr. On Windows both
  programs write CRLF there, as CPython's text-mode streams do.
- `relatum_edit` (Relatum only) is dropped from `tools/list`.
- `relatum_ls`'s opt-in `limit`, `since`, and `details` arguments and
  `relatum_get`'s `include_superseded` are dropped from the input schemas.
  `relatum ls -h` lists the matching CLI flags; its usage line, which
  argparse errors print, is unchanged.
- **Intentional difference: descriptions.** Tool and argument descriptions,
  and the summary's `maxLength`, are dropped from `tools/list` on both
  sides. Relatum writes its own: when to use each tool, and (1.4.0) the
  200-character summary limit. The initialize instructions are not part of
  the comparison; Relatum's add a paragraph on when to reach for notes.
- **Intentional differences in 1.4.0, not hit by the harness session:** a
  summary may be up to 200 characters (Examen: 100) and must be one line;
  the length error says how long it is and what the limit is; a bare
  `--since` date is local midnight (Examen had no `--since`); and
  `relatum_supersede` to a missing new note says to write it first; and
  `--version` alone prints the version (Examen's argparse rejected it).
  `tests/notes.rs`, `tests/ls_options.rs`, and `tests/mcp_agent.rs` cover
  them.
- A tool result's JSON text is compared as data. Relatum writes it compact;
  Examen indented it. `structuredContent` is compared unchanged.
- Relatum's MCP errors add a next step for a missing store, a scope with no
  notes, and an invalid id. The session the harness drives does not hit
  those three cases; `tests/mcp_agent.rs` covers them.
- For the `relatum_init` path rules, Examen's `examen_init` is called
  in-process and a leading `Error executing tool ...:` is dropped from
  Relatum's tool text.

`init` still writes `author: examen` on `_shared/report-format`, so a store
created on the same local date matches byte for byte.
