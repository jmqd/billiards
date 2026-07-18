# Deduplication: Cargo artifact messages

## Rank and scope

12 of 12. Wasm build artifact discovery in `xtask`.

## Problem

`build_wasm_artifact` identifies Cargo compiler-artifact lines through raw substring searches, then uses hand-written partial JSON routines for string arrays and string escapes. This duplicates mature JSON parsing and couples correctness to field order/text layout. Escaped filenames, reordered fields, or unrelated embedded `name` strings can be mishandled.

## Decision

Add direct `serde`/`serde_json` dependencies to `xtask`. Define a narrow deserialization model for Cargo messages and targets. Parse each stdout line at the boundary, select `reason == "compiler-artifact"` and `target.name == "billiards"`, then preserve existing `.wasm` filtering, sort/dedup, and exactly-one-artifact checks. Delete the custom JSON string and array parsers.

Malformed matching artifact messages must remain explicit errors; unrelated non-JSON/non-artifact output should retain current tolerance.

## Files

- `xtask/src/main.rs`
- `xtask/Cargo.toml`
- `Cargo.lock`
- Existing `xtask` unit tests

## Invariants

- Exactly one billiards `.wasm` artifact is required.
- Unrelated Cargo targets and message reasons are ignored.
- Filename escapes, whitespace, and field order are handled by JSON semantics.
- Zero or multiple artifacts retain current errors.
- No general command/process abstraction is introduced.

## Verification

Run focused `cargo test -p xtask` cases for reordered fields, escaped filenames, unrelated messages, missing filenames, malformed matching records, and zero/one/multiple wasm paths. Smoke `wasm-preview --no-serve` only when the wasm toolchain is available.

## Commit boundary

Only Cargo message parsing and required direct dependencies. Xtask argument parsing and broader build orchestration remain unchanged.