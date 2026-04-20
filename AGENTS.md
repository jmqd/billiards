# AGENTS

## Whitepaper / research workflow

- For billiards physics research, start with `agent_knowledge/agent_reading_guide.md`.
- Use `agent_knowledge/whitepapers_index.jsonl` to find relevant sources by topic/title/citation status.
- Use `agent_knowledge/whitepapers_formula_candidates.txt` for quick equation/formula skims.
- Use `agent_knowledge/whitepapers_corpus.txt` for extracted plain text instead of reading raw PDFs/HTML when possible.
- Raw source documents remain under `whitepapers/`.

## Regeneration

- `agent_knowledge/` is generated. Do not hand-edit its contents unless explicitly asked.
- To rebuild it, run: `nix develop -c python scripts/build_agent_knowledge.py`
- If the distillation needs to change, edit `scripts/build_agent_knowledge.py` and regenerate.

## API / compatibility

- Backwards compatibility is not a project goal here.
- Prefer the cleanest in-tree API / data-model refactor over compatibility shims.
- It is acceptable to break existing call sites and update them in the same change.
