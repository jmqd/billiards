#!/usr/bin/env python3
"""Build agent-facing distilled knowledge artifacts from whitepapers/.

Requires:
- pdftotext
- pdfinfo
Both are available from this repo's nix dev shell via `nix develop`.

Outputs under agent_knowledge/:
- README.md
- whitepapers_index.jsonl
- whitepapers_formula_candidates.txt
- whitepapers_corpus.txt
- agent_reading_guide.md
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
from collections import Counter, defaultdict
from dataclasses import asdict, dataclass
from html.parser import HTMLParser
from pathlib import Path
from typing import Iterable

REPO_ROOT = Path(__file__).resolve().parents[1]
WHITEPAPERS_DIR = REPO_ROOT / "whitepapers"
OUT_DIR = REPO_ROOT / "agent_knowledge"
INDEX_PATH = OUT_DIR / "whitepapers_index.jsonl"
FORMULA_PATH = OUT_DIR / "whitepapers_formula_candidates.txt"
CORPUS_PATH = OUT_DIR / "whitepapers_corpus.txt"
GUIDE_PATH = OUT_DIR / "agent_reading_guide.md"
README_PATH = OUT_DIR / "README.md"

PDFTOTEXT = shutil.which("pdftotext")
PDFINFO = shutil.which("pdfinfo")

TEXT_FILE_EXTS = {".rs", ".md", ".org", ".txt", ".toml", ".nix", ".yml", ".yaml", ".json"}
DOC_EXTS = {".pdf", ".html", ".htm", ".txt"}

STOPWORDS = {
    "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "in", "into",
    "is", "it", "of", "on", "or", "that", "the", "their", "this", "to", "with",
}

TOPIC_RULES = {
    "technical_proofs": ["tp ", "tp_", "technical proof"],
    "collisions_and_impacts": [
        "collision", "impact", "restitution", "friction", "cushion", "ball-ball", "carom",
        "hertz", "non-smooth",
    ],
    "cue_ball_motion_and_spin": [
        "cue ball", "draw", "follow", "rolling", "sliding", "spin", "squirt", "swerve",
        "english", "masse", "gear", "drag-enhanced", "tip offset",
    ],
    "aiming_and_potting": [
        "aim", "aiming", "pocket", "potting", "cut angle", "90°", "30°", "30 degree",
        "90 degree", "fractional-ball", "ghost ball", "vision center", "bridge length",
    ],
    "banks_kicks_rails": [
        "bank", "kick", "rail", "corner-5", "plus system", "mirror", "rebound",
    ],
    "strategy_rules_drills": [
        "strategy", "safety", "rules", "drill", "practice", "pattern", "break", "runout",
        "ratings", "exam", "challenge", "stance",
    ],
    "robotics_and_computation": [
        "robot", "robotic", "computational", "simulation", "monte carlo", "workspace",
        "mechatronics", "automation", "pool-playing robot",
    ],
    "history_and_general_physics": [
        "coriolis", "physics of billiards", "art of billiards", "amateur physics",
        "histor", "theory of billiards",
    ],
}

PRIMARY_STARTER_DOCS = [
    "pool_and_billiards_physics_principles_by_coriolis_and_others.pdf",
    "collision_of_billiard_balls_in_3d_with_spin_and_friction.pdf",
    "motions_of_a_billiard_ball_after_a_cue_stroke.pdf",
    "sliding_and_rolling_the_physics_of_a_rolling_ball.pdf",
    "rolling_motion_of_a_ball_spinning_about_a_near_vertical_axis.pdf",
    "the_art_of_billiards_play.html",
    "the_physics_of_billiards.html",
    "tp_a_4_post_impact_cue_ball_trajectory_for_any_cut_angle_speed_and_spin.pdf",
    "tp_4_2_center_of_percussion_of_the_cue_ball.pdf",
    "tp_3_1_90_degree_rule.pdf",
    "tp_3_3_30_degree_rule.pdf",
    "tp_3_4_margin_of_error_based_on_distance_and_cut_angle.pdf",
    "tp_3_5_effective_target_sizes_for_slow_shots_into_a_side_pocket_at_different_angles.pdf",
    "tp_3_6_effective_target_sizes_for_slow_shots_into_a_corner_pocket_at_different_angles.pdf",
    "tp_3_7_effective_target_sizes_for_fast_shots_into_a_side_pocket_at_different_angles.pdf",
    "tp_3_8_effective_target_sizes_for_fast_shots_into_a_corner_pocket_at_different_angles.pdf",
    "tp_a_24_the_effects_of_follow_and_draw_on_throw_and_ob_swerve.pdf",
    "tp_b_1_squirt_angle_pivot_length_and_tip_size.pdf",
    "everything_you_always_wanted_to_know_about_cue_ball_squirt_but_were_afraid_to_ask.pdf",
    "non_smooth_modelling_of_billiard_and_superbilliard_ball_collisions.pdf",
    "a_theoretical_analysis_of_billiard_ball_dynamics_under_cushion_impacts.pdf",
    "numerical_simulations_of_the_frictional_collisions_of_solid_balls_on_a_rough_surface.pdf",
]


class SimpleHTMLTextExtractor(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self._skip_stack: list[str] = []
        self.text_parts: list[str] = []

    def handle_starttag(self, tag: str, attrs) -> None:
        if tag in {"script", "style", "noscript"}:
            self._skip_stack.append(tag)
        if tag in {"p", "div", "section", "article", "br", "li", "h1", "h2", "h3", "h4", "h5", "h6", "tr"}:
            self.text_parts.append("\n")

    def handle_endtag(self, tag: str) -> None:
        if self._skip_stack and self._skip_stack[-1] == tag:
            self._skip_stack.pop()
        if tag in {"p", "div", "section", "article", "li", "h1", "h2", "h3", "h4", "h5", "h6", "tr"}:
            self.text_parts.append("\n")

    def handle_data(self, data: str) -> None:
        if self._skip_stack:
            return
        self.text_parts.append(data)

    def text(self) -> str:
        raw = "".join(self.text_parts)
        raw = raw.replace("\r", "")
        raw = re.sub(r"\n{3,}", "\n\n", raw)
        lines = [re.sub(r"\s+", " ", line).strip() for line in raw.splitlines()]
        lines = [line for line in lines if line]
        return "\n".join(lines).strip()


@dataclass
class DocRecord:
    path: str
    filename: str
    title: str
    extension: str
    size_bytes: int
    topics: list[str]
    cited_by_repo: bool
    cited_in_code: bool
    cited_in_docs: bool
    primary_start: bool
    char_count: int
    line_count: int
    formula_line_count: int
    excerpt: str


def require_tool(name: str, path: str | None) -> str:
    if path:
        return path
    raise SystemExit(
        f"Required tool '{name}' not found in PATH. Run inside `nix develop` or install poppler-utils."
    )


def nice_title_from_filename(path: Path) -> str:
    stem = path.stem
    stem = stem.replace("_", " ").replace("-", " ")
    stem = re.sub(r"\s+", " ", stem).strip()
    return stem


def pretty_title_from_filename(path: Path) -> str:
    stem = path.stem
    words = stem.split("_")
    if len(words) >= 4 and words[0] == "tp":
        if re.fullmatch(r"[0-9]+", words[1]) and re.fullmatch(r"[0-9]+", words[2]):
            rest = " ".join(w.capitalize() if w != "degree" else "degree" for w in words[3:])
            return normalize_title(f"TP {words[1]}.{words[2]} - {rest}")
        if re.fullmatch(r"[a-z]", words[1]) and re.fullmatch(r"[0-9]+", words[2]):
            rest = " ".join(w.capitalize() if w != "degree" else "degree" for w in words[3:])
            return normalize_title(f"TP {words[1].upper()}.{words[2]} - {rest}")
    out = []
    acronyms = {
        "tp": "TP",
        "cb": "CB",
        "ob": "OB",
        "bu": "BU",
        "bhe": "BHE",
        "fhe": "FHE",
        "nv": "NV",
        "hsv": "HSV",
        "vepp": "VEPP",
        "veps": "VEPS",
        "veeb": "VEEB",
        "vent": "VENT",
        "saws": "SAWS",
        "haps": "HAPS",
        "rds": "RDS",
    }
    for word in words:
        if word in acronyms:
            out.append(acronyms[word])
        elif re.fullmatch(r"[0-9]+", word):
            out.append(word)
        elif re.fullmatch(r"[a-z][0-9]+", word):
            out.append(word.upper())
        elif word == "degree":
            out.append("degree")
        else:
            out.append(word.capitalize())
    title = " ".join(out)
    title = title.replace(" 3d ", " 3D ").replace(" 2d ", " 2D ")
    return normalize_title(title)


def title_is_noisy(title: str) -> bool:
    bad_markers = [
        "ILLUSTRATED PRINCIPLES",
        "ARTICLE IN PRESS",
        "JMES",
        "Sports Eng DOI",
        "????cad",
        "technical proof technical proof",
        "Supporting narrated video",
        "Proceedings 01",
        "Microsoft Word",
        "Version Date",
        "http://",
        "https://",
    ]
    lower = title.lower()
    if any(marker.lower() in lower for marker in bad_markers):
        return True
    if title.count("\"") >= 2:
        return True
    if len(title) > 120:
        return True
    if lower.endswith((" into", " for", " with", " and", " of", " on", " at", " to", " the")):
        return True
    return False


def html_title_and_text(path: Path) -> tuple[str, str]:
    source = path
    if path.name == "the_art_of_billiards_play.html":
        inner = WHITEPAPERS_DIR / "art_of_billiards_play_files" / "bil_praa.html"
        if inner.exists():
            source = inner
    text = source.read_text(errors="ignore")
    title_match = re.search(r"<title[^>]*>(.*?)</title>", text, flags=re.IGNORECASE | re.DOTALL)
    title = re.sub(r"\s+", " ", title_match.group(1)).strip() if title_match else nice_title_from_filename(path)
    if path.name == "the_art_of_billiards_play.html":
        h2_match = re.search(r"<h2[^>]*>(.*?)</h2>", text, flags=re.IGNORECASE | re.DOTALL)
        if h2_match:
            t = re.sub(r"<[^>]+>", " ", h2_match.group(1))
            t = re.sub(r"\(.*$", "", re.sub(r"\s+", " ", t)).strip()
            if t:
                title = t
    elif path.name == "inelastic_collision_and_the_hertz_theory_of_impact.html":
        page_title = re.search(r'<div class="pagetitle">(.*?)</div>', text, flags=re.IGNORECASE | re.DOTALL)
        if page_title:
            t = re.sub(r"<[^>]+>", " ", page_title.group(1))
            t = re.sub(r"\s+", " ", t).strip()
            if t:
                title = t
    else:
        h1_match = re.search(r"<h1[^>]*>(.*?)</h1>", text, flags=re.IGNORECASE | re.DOTALL)
        if h1_match:
            t = re.sub(r"<[^>]+>", " ", h1_match.group(1))
            t = re.sub(r"\s+", " ", t).strip()
            if t and t != ".":
                title = t
    parser = SimpleHTMLTextExtractor()
    parser.feed(text)
    extracted = parser.text()
    return normalize_title(title), extracted


def pdf_info_title(path: Path, pdfinfo_path: str) -> str | None:
    try:
        out = subprocess.check_output([pdfinfo_path, str(path)], text=True, stderr=subprocess.DEVNULL)
    except subprocess.CalledProcessError:
        return None
    title = None
    for line in out.splitlines():
        if line.startswith("Title:"):
            title = line.split(":", 1)[1].strip()
            break
    if not title:
        return None
    tl = title.lower()
    bad_prefixes = ("mathcad -", "microsoft word -")
    bad_contains = [".pdf", ".dvi", "doi ", "doi:", "proceedings", "vol.", "journal", "article in press"]
    if tl in {"section 1", "call first"} or tl.startswith(bad_prefixes) or any(x in tl for x in bad_contains):
        return None
    return normalize_title(title)


def pdf_text(path: Path, pdftotext_path: str) -> str:
    try:
        return subprocess.check_output(
            [pdftotext_path, "-layout", str(path), "-"],
            text=True,
            stderr=subprocess.DEVNULL,
        )
    except subprocess.CalledProcessError:
        return ""


def title_from_pdf_text(text: str, fallback: str) -> str:
    lines = [re.sub(r"\s+", " ", line).strip() for line in text.splitlines()[:40]]
    lines = [line for line in lines if line]
    if not lines:
        return fallback
    for i, line in enumerate(lines[:10]):
        if re.fullmatch(r"TP\s+[A-Z0-9.]+", line) and i + 1 < len(lines):
            return normalize_title(f"{line} - {lines[i + 1]}")
    skip_contains = [
        "available online at", "sciencedirect", "abstract", "department of", "received ",
        "accepted ", "http://", "https://", "colorado state university", "david g. alciatore",
        "dr. dave", "university", "argonne pool league", "version date", "copyright",
        "article has been accepted", "ieee transactions",
    ]
    cleaned: list[str] = []
    for line in lines[:20]:
        low = line.lower().strip('"“”')
        if any(s in low for s in skip_contains):
            continue
        if len(low) < 4:
            continue
        if low.startswith("keywords") or low.startswith("subject"):
            continue
        cleaned.append(line.strip('"“”'))
    if not cleaned:
        return fallback
    first = cleaned[0]
    second = cleaned[1] if len(cleaned) > 1 else None
    if second and (
        first.endswith(("of", "in", "on", "for", "to", "with", "and"))
        or (len(first) < 55 and len(second) < 65 and not re.search(r"\b[A-Z]\.\b", second))
    ):
        return normalize_title(f"{first} {second}")
    return normalize_title(first)


def normalize_title(title: str) -> str:
    title = title.replace("–", "-").replace("—", "-").replace("…", "...")
    title = title.replace("“", '"').replace("”", '"').replace("’", "'").replace("‘", "'")
    title = re.sub(r"\s+", " ", title).strip().strip('"')
    title = re.sub(r"\s*[,;:.\-]+\s*$", "", title)
    return title


def classify_topics(title: str, filename: str, text: str) -> list[str]:
    haystack = f"{title} {filename} {text[:4000]}".lower()
    matched = [topic for topic, needles in TOPIC_RULES.items() if any(needle in haystack for needle in needles)]
    if not matched:
        matched = ["uncategorized"]
    return matched


def short_excerpt(text: str, limit: int = 450) -> str:
    lines = [re.sub(r"\s+", " ", line).strip() for line in text.splitlines()]
    lines = [line for line in lines if line]
    chunk = []
    size = 0
    for line in lines:
        if len(line) < 3:
            continue
        if line.lower().startswith(("abstract", "keywords", "contents")):
            continue
        chunk.append(line)
        size += len(line) + 1
        if size >= limit:
            break
    excerpt = " ".join(chunk)
    return excerpt[:limit].strip()


def extract_formula_lines(text: str) -> list[str]:
    out: list[str] = []
    seen: set[str] = set()
    for raw in text.splitlines():
        line = re.sub(r"\s+", " ", raw).strip()
        if len(line) < 4 or len(line) > 220:
            continue
        lower = line.lower()
        if line in seen:
            continue
        if (
            "=" in line
            or any(sym in line for sym in ["ω", "Ω", "μ", "θ", "φ", "α", "β", "γ", "τ", "∝", "≤", "≥"]) 
            or re.search(r"\b(eq\.?|equation|formula|theorem)\b", lower)
            or re.search(r"\b[vwxyztfmnurpi]+\s*=\s*", lower)
            or re.search(r"\b[suvwxyz]\([^)]+\)", line)
            or re.search(r"\bRω\b", line)
            or re.search(r"\b[a-zA-Z]\^\d", line)
        ):
            seen.add(line)
            out.append(line)
    return out[:40]


def parse_todo_aliases() -> dict[str, str]:
    todo = REPO_ROOT / "TODO.org"
    if not todo.exists():
        return {}
    text = todo.read_text(errors="ignore")
    aliases: dict[str, str] = {}
    pattern = re.compile(
        r"=whitepapers/(?P<old>[^=]+)=\s*\n\s*->\s*=whitepapers/(?P<new>[^=]+)=",
        flags=re.MULTILINE,
    )
    for match in pattern.finditer(text):
        aliases[Path(match.group("old")).name] = Path(match.group("new")).name
    return aliases


def gather_repo_citations() -> tuple[set[str], set[str], set[str]]:
    aliases = parse_todo_aliases()
    cited_any: set[str] = set()
    cited_code: set[str] = set()
    cited_docs: set[str] = set()
    for path in REPO_ROOT.rglob("*"):
        if not path.is_file():
            continue
        if any(part in {".git", "whitepapers", "agent_knowledge", ".hive", "target"} for part in path.parts):
            continue
        if path.suffix.lower() not in TEXT_FILE_EXTS and path.name not in {"README", "README.md"}:
            continue
        try:
            text = path.read_text(errors="ignore")
        except Exception:
            continue
        for match in re.findall(r"whitepapers/([^\s`'\"\)\]>]+)", text):
            name = Path(match).name
            name = aliases.get(name, name)
            cited_any.add(name)
            if path.suffix == ".rs":
                cited_code.add(name)
            else:
                cited_docs.add(name)
    return cited_any, cited_code, cited_docs


def iter_docs() -> Iterable[Path]:
    for path in sorted(WHITEPAPERS_DIR.iterdir()):
        if path.is_dir():
            continue
        if path.suffix.lower() in DOC_EXTS:
            yield path


def keyword_signature(title: str) -> list[str]:
    words = re.findall(r"[a-z0-9]+", title.lower())
    sig = []
    for word in words:
        if len(word) < 4 or word in STOPWORDS:
            continue
        sig.append(word)
        if len(sig) >= 8:
            break
    return sig


def render_guide(records: list[DocRecord], topic_map: dict[str, list[DocRecord]]) -> str:
    total_chars = sum(r.char_count for r in records)
    total_formulas = sum(r.formula_line_count for r in records)
    starters = [r for r in records if r.primary_start]
    cited = [r for r in records if r.cited_by_repo]

    def score(record: DocRecord) -> tuple[int, int, int, str]:
        return (
            1 if record.primary_start else 0,
            1 if record.cited_by_repo else 0,
            record.formula_line_count,
            record.title.lower(),
        )

    lines: list[str] = []
    lines.append("# Agent Reading Guide for Billiards Whitepapers")
    lines.append("")
    lines.append("This is the *distilled* agent-facing overview. Use it first, then drop into the")
    lines.append("JSONL/corpus files for deeper retrieval.")
    lines.append("")
    lines.append("## What to read first")
    lines.append("")
    lines.append("Use this order if you are trying to quickly grok the current billiards physics model:")
    lines.append("")
    for r in starters:
        lines.append(f"1. `{r.path}` — {r.title}")
    lines.append("")
    lines.append("## Quick stats")
    lines.append("")
    lines.append(f"- Documents indexed: {len(records)}")
    lines.append(f"- Documents cited by current repo code/docs (including TODO old→new mappings): {len(cited)}")
    lines.append(f"- Approx extracted text size: {total_chars:,} characters")
    lines.append(f"- Formula-like candidate lines harvested: {total_formulas:,}")
    lines.append("")
    lines.append("## Code- and doc-cited sources")
    lines.append("")
    for r in sorted(cited, key=lambda x: (not x.cited_in_code, x.title.lower())):
        tags = []
        if r.cited_in_code:
            tags.append("code")
        if r.cited_in_docs:
            tags.append("docs")
        lines.append(f"- `{r.path}` — {r.title} [{', '.join(tags)}]")
    lines.append("")
    lines.append("## Topic map (top docs only)")
    lines.append("")
    lines.append("Each section shows the highest-signal docs first; use `whitepapers_index.jsonl`")
    lines.append("for the exhaustive list.")
    lines.append("")
    for topic, docs in sorted(topic_map.items()):
        ranked = sorted(docs, key=score, reverse=True)
        shown = ranked[:12]
        lines.append(f"### {topic.replace('_', ' ').title()}")
        lines.append("")
        for r in shown:
            extra = []
            if r.primary_start:
                extra.append("starter")
            if r.cited_in_code:
                extra.append("code-cited")
            elif r.cited_in_docs:
                extra.append("doc-cited")
            if r.formula_line_count:
                extra.append(f"formula-lines:{r.formula_line_count}")
            meta = f" [{' | '.join(extra)}]" if extra else ""
            lines.append(f"- `{r.path}` — {r.title}{meta}")
        if len(ranked) > len(shown):
            lines.append(f"- ... {len(ranked) - len(shown)} more in `whitepapers_index.jsonl`")
        lines.append("")
    lines.append("## Notes")
    lines.append("")
    lines.append("- `whitepapers_corpus.txt` is the full plain-text dump with per-document delimiters.")
    lines.append("- `whitepapers_formula_candidates.txt` is a grep-like list of formula/equation candidates.")
    lines.append("- `whitepapers_index.jsonl` is the machine-readable manifest for tools/agents.")
    return "\n".join(lines).strip() + "\n"


def render_readme(records: list[DocRecord]) -> str:
    return f"""# Agent Knowledge Artifacts

This directory contains a machine-generated, agent-friendly distillation layer over
`whitepapers/`.

Generated by:
- `python scripts/build_agent_knowledge.py`
- best run inside `nix develop` so `pdfinfo` and `pdftotext` are available.

## Files

- `whitepapers_index.jsonl`
  - one JSON object per top-level document in `whitepapers/`
  - includes title, topics, citation status, and short excerpt
- `whitepapers_corpus.txt`
  - full extracted plain-text dump with document delimiters and metadata headers
- `whitepapers_formula_candidates.txt`
  - formula-like lines harvested from extracted text
- `agent_reading_guide.md`
  - curated/generated reading order and topic map for agents

## Scope

Indexed documents: {len(records)} top-level files from `whitepapers/` with extensions
`.pdf`, `.html`, `.htm`, or `.txt`.

## Usage ideas

- ask an agent to read `agent_reading_guide.md` first
- grep `whitepapers_index.jsonl` for relevant topics/titles
- search `whitepapers_formula_candidates.txt` for likely equations
- load only the relevant sections of `whitepapers_corpus.txt` instead of the raw PDFs/HTML
"""


def main() -> None:
    pdftotext_path = require_tool("pdftotext", PDFTOTEXT)
    pdfinfo_path = require_tool("pdfinfo", PDFINFO)
    OUT_DIR.mkdir(parents=True, exist_ok=True)

    cited_any, cited_code, cited_docs = gather_repo_citations()
    records: list[DocRecord] = []
    formula_sections: list[str] = []
    corpus_sections: list[str] = []

    for path in iter_docs():
        title = nice_title_from_filename(path)
        text = ""
        pretty_title = pretty_title_from_filename(path)
        if path.suffix.lower() == ".pdf":
            text = pdf_text(path, pdftotext_path)
            title = pdf_info_title(path, pdfinfo_path) or title_from_pdf_text(text, pretty_title)
        elif path.suffix.lower() in {".html", ".htm"}:
            title, text = html_title_and_text(path)
        else:
            text = path.read_text(errors="ignore")
        text = text.replace("\r", "")
        text = re.sub(r"\n{3,}", "\n\n", text).strip()
        if title_is_noisy(title):
            title = pretty_title

        topics = classify_topics(title, path.name, text)
        formulas = extract_formula_lines(text)
        excerpt = short_excerpt(text)
        record = DocRecord(
            path=f"whitepapers/{path.name}",
            filename=path.name,
            title=title,
            extension=path.suffix.lower(),
            size_bytes=path.stat().st_size,
            topics=topics,
            cited_by_repo=path.name in cited_any,
            cited_in_code=path.name in cited_code,
            cited_in_docs=path.name in cited_docs,
            primary_start=path.name in PRIMARY_STARTER_DOCS,
            char_count=len(text),
            line_count=text.count("\n") + (1 if text else 0),
            formula_line_count=len(formulas),
            excerpt=excerpt,
        )
        records.append(record)

        formula_sections.append(f"## {record.title}\npath: {record.path}\n")
        if formulas:
            formula_sections.extend(f"- {line}\n" for line in formulas)
        else:
            formula_sections.append("- [no formula-like lines detected]\n")
        formula_sections.append("\n")

        corpus_sections.append(
            "\n".join(
                [
                    "===== BEGIN DOCUMENT =====",
                    f"path: {record.path}",
                    f"title: {record.title}",
                    f"topics: {', '.join(record.topics)}",
                    f"cited_by_repo: {'yes' if record.cited_by_repo else 'no'}",
                    "text:",
                    text or "[NO_EXTRACTED_TEXT]",
                    "===== END DOCUMENT =====",
                    "",
                ]
            )
        )

    with INDEX_PATH.open("w") as f:
        for record in records:
            f.write(json.dumps(asdict(record), ensure_ascii=False) + "\n")

    FORMULA_PATH.write_text("".join(formula_sections))
    CORPUS_PATH.write_text("".join(corpus_sections))

    topic_map: dict[str, list[DocRecord]] = defaultdict(list)
    for record in records:
        for topic in record.topics:
            topic_map[topic].append(record)

    GUIDE_PATH.write_text(render_guide(records, topic_map))
    README_PATH.write_text(render_readme(records))

    counts = Counter(topic for r in records for topic in r.topics)
    print(f"indexed_docs={len(records)}")
    print(f"cited_docs={sum(1 for r in records if r.cited_by_repo)}")
    print(f"formula_lines={sum(r.formula_line_count for r in records)}")
    for topic, count in counts.most_common():
        print(f"topic[{topic}]={count}")


if __name__ == "__main__":
    main()
