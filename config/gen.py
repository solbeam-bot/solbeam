#!/usr/bin/env python3
"""One canonical parameter sheet, projected into the program, the tests and the docs.

`config/params.json` is the single source of truth.  Everything else is a
projection of it:

* `poc/solana/programs/solbeam/src/params.rs`  -- the program's constants;
* `poc/solana/tests/params.json`               -- what the test suite reads;
* `docs/parameters.csv`                        -- the machine-readable table;
* `docs/06-parameters.md`                      -- the human tables and the
  status/provenance legend (the fenced, generated regions only).

The workflow is one file and one command:

    edit `value` (or a row's text) in `config/params.json`
    python3 config/gen.py            # verify, then rewrite every projection
    python3 config/gen.py --check    # verify only; fail if a projection is stale

Nothing is retyped.  A row's human spellings are templates over its machine
`value`, so a value edited once lands in all four projections:

* `csv_value`  -- the CSV's value cell.  `%v%` is the value as written,
  `%V%` the thousands-separated spelling, `%bars%` the value with `|`
  rendered as `‖` (the markdown-safe bar), `%stripped%` the value with the
  status/provenance marker words removed.  A `csv_value` with no token is
  the literal cell, used only where the cell is history rather than a
  restatement of the value (the superseded `pi.max_used` row).
* `csv_note`   -- optional; text the CSV carries after the value that the
  doc's own columns already say (`fed.script`'s shape clause).
* `doc_value`  -- optional; the doc-06 value cell where markdown differs from
  the CSV cell (an interior code span, text after the bolded value, a token
  that must not be bolded).  Absent, the cell is `**` + the CSV cell with the
  status/provenance marker words removed + `**`.
* `doc_name` / `doc_description` -- optional; the doc-06 Name/Description cell
  where the table's markdown emphasis is not in the plain text.
* `sections`   -- the doc-06 grouping and order: one heading and its row ids
  per group.  `legend` holds the status/provenance vocabulary and its wording;
  the counts are computed from the rows.

Every row carries **two independent facts**, and both are checked:

* `status` -- *where the value lives*: `built-mutable`, `built-frozen`,
  `built-shape`, `designed`, `open`, `superseded`;
* `provenance` -- *how it was chosen*: `measured`, `decided`, `placeholder`,
  `derived`.

The check is the point.  Before anything is written, every projection is
computed in memory and checked against the JSON: the CSV cell must still mean
the row's machine `value`, the doc rows must round-trip to the same rows, the
legend counts must equal the rows they describe, and the arithmetic must hold.
If any check fails, **nothing is written**, so a failure cannot leave one
projection updated and another stale.  `--check` runs the same computation and
fails if any checked-in projection differs from the bytes this script would
emit.

The legend's counts are generated with the legend tables, so they cannot drift
from the rows either.

Usage:
    python3 config/gen.py              # verify, then (re)write the projections
    python3 config/gen.py --check      # verify only; fail if a projection is stale
    python3 config/gen.py --from-csv   # bootstrap config/params.json from the CSV
"""

from __future__ import annotations

import argparse
import csv
import io
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PARAMS_JSON = ROOT / "config" / "params.json"
PARAMS_CSV = ROOT / "docs" / "parameters.csv"
PARAMS_MD = ROOT / "docs" / "06-parameters.md"
PARAMS_RS = ROOT / "poc" / "solana" / "programs" / "solbeam" / "src" / "params.rs"
TESTS_JSON = ROOT / "poc" / "solana" / "tests" / "params.json"

CSV_COLUMNS = ["id", "name", "value", "status", "provenance", "description"]

# The doc-06 tables' columns, in order.  A parameter table is the one whose
# header is exactly this, so the document can keep using tables for prose.
DOC_COLUMNS = ["id", "name", "value", "status", "provenance", "description"]

# The status and provenance vocabularies, checked against the legend.
STATUSES = ["built-mutable", "built-frozen", "built-shape", "designed", "open",
            "superseded"]
PROVENANCES = ["measured", "decided", "placeholder", "derived"]

# The words the human value cells use for status and provenance.  They are
# stripped when a doc-06 value cell is compared with the JSON, because the
# split moved them into their own columns.  `der·superseded` first: it
# contains `der`.  The bare word `superseded` is *not* in this list: on
# `fed.total_bond` it is the whole cell, and it is what tells `typed_value`
# that the row has no machine value.
MARKER_WORDS = ["der·superseded", "der", "mea", "dec", "ph", "built"]

# The fenced regions of doc 06 that this script owns.  Each pair is
# (begin, end); the text between them is generated and must not be hand-edited.
DOC_BEGIN = "<!-- BEGIN GENERATED: %s -- edit config/params.json and run `python3 config/gen.py` -->"
DOC_END = "<!-- END GENERATED: %s -->"
DOC_STATUS_LEGEND = "status-legend"
DOC_PROVENANCE_LEGEND = "provenance-legend"
DOC_PARAMETER_TABLES = "parameter-tables"

# How the CSV and doc value cells are read, per parameter id.  The default
# for a text-valued row is `text` (the cell with marker words removed, the two
# bar spellings folded); the default otherwise is `number` (the cell's leading
# number).  Only the rows where the obvious reading is wrong are declared.
def spell_meaning(pid: str, text: str, value: object):
    """What a value cell means, given the row's machine value.

    A text row's cell is the row's text in its table spelling (marker words
    removed so `dec`/`mea`/`built` do not count as part of the value, the two
    bar spellings folded).  A numeric row's cell is a number with units around
    it, so the leading number is the meaning.  A row with no machine value --
    an `open` placeholder or a `superseded` legacy row -- means no value, and
    the cell is checked only for being non-empty.
    """
    if value is None:
        return None
    if isinstance(value, bool):
        return typed_value(pid, text)
    if isinstance(value, str):
        return value_core(text)
    match = re.match(r"^[<>]?\s*([0-9][0-9,]*)", " ".join(text.split()))
    if not match:
        die(f"{pid}: value cell {text!r} does not start with a number")
    return int(match.group(1).replace(",", ""))


# `%v%` the value as written, `%V%` the thousands-separated spelling, `%bars%`
# the value with `|` rendered as `‖`, `%stripped%` the value with the
# status/provenance marker words removed.  Longest alternative first.
TOKEN = re.compile(r"%(stripped|bars|V|v)%")
TOKEN_ANY = re.compile(r"%(stripped|bars|V|v)%")

# The Rust constants the program turns on.  (name, type, parameter id, expression)
# `expression` overrides the literal, for a constant that is derived from another
# generated constant; the generator still checks the row's own value against the
# arithmetic.
RUST_CONSTANTS = [
    ("WINDOW_HOURS", "u64", "lc.window_hours", None),
    ("SECONDS_PER_BLOCK", "u64", "lc.seconds_per_block", None),
    ("WINDOW", "usize", "lc.window",
     "(WINDOW_HOURS * 3600 / SECONDS_PER_BLOCK) as usize"),
    ("HEADER_RECORD_SIZE", "usize", "lc.record_size", None),
    ("LOOKBACK_RECORDS", "u64", "lc.lookback", None),
    ("SEED_RECORDS", "usize", "lc.lookback", None),
    ("MIN_CONFIRMATIONS", "u64", "lc.floor", None),
    ("MAX_STALENESS_SLOTS", "u64", "lc.max_staleness_slots", None),
    ("MAX_FORK_BATCH", "usize", "lc.max_fork_batch", None),
    ("MAX_ACCOUNT_CREATE", "usize", "lc.max_account_create", None),
    ("DAA_CLAMP_LOW_MULTIPLIER", "i64", "lc.daa_clamp_low", None),
    ("DAA_CLAMP_HIGH_MULTIPLIER", "i64", "lc.daa_clamp_high", None),
    ("DEFAULT_MATURITY_BLOCKS", "u64", "v.maturity_blocks", None),
    ("TIMELOCK_SLOTS", "u64", "gov.authority_timelock", None),
    ("TOKEN_DECIMALS", "u8", "m.token_decimals", None),
    # The peg-out's numbers.  `fee.redeem_bp` is grouped with the other fees and
    # the `po.*` rows with each other, so the generated file keeps its sections
    # in the sheet's order.
    ("REDEEM_FEE_BP", "u64", "fee.redeem_bp", None),
    ("PAYOUT_CONFIRMATIONS", "u64", "po.payout_confirmations", None),
    ("CHALLENGE_WINDOW", "u64", "po.challenge_window", None),
    ("REDEEM_DEADLINE_SLOTS", "u64", "po.deadline", None),
    ("CANCEL_GRACE_SLOTS", "u64", "po.cancel_grace", None),
    ("REDEEM_D_MIN", "u64", "po.d_min", None),
    ("MAX_PENDING_REDEMPTIONS", "u64", "po.max_pending", None),
    # The peg-in floor. The row's machine value is already in the unit the
    # program compares -- the base units `DepositClaim::amount` carries -- so
    # this is a literal projection, not a unit conversion kept in step by hand.
    # 1 BSV = 100,000,000 base units at 8 decimals.
    ("MIN_PEG_IN", "u64", "pi.min_peg_in", None),
]


def die(message: str) -> "NoReturn":  # type: ignore[name-defined]
    print(f"gen.py: error: {message}", file=sys.stderr)
    sys.exit(1)


# ---------------------------------------------------------------------------
# value templates: one machine value, many human spellings
# ---------------------------------------------------------------------------

def render(template: str, value: object) -> str:
    """A human cell from a template and the row's machine value."""
    def sub(match: "re.Match[str]") -> str:
        token = match.group(1)
        if token == "bars":
            assert isinstance(value, str)
            return value.replace("|", "\u2016")
        if token == "stripped":
            assert isinstance(value, str)
            text = value
            for word in MARKER_WORDS:
                text = re.sub(rf"\b{re.escape(word)}\b", " ", text)
            return " ".join(text.split())
        if value is None:
            return "open"
        if isinstance(value, bool):
            return "true" if value else "false"
        if isinstance(value, int):
            return f"{value:,}" if token == "V" else str(value)
        return str(value)
    return TOKEN.sub(sub, template)


def has_token(template: str) -> bool:
    return TOKEN_ANY.search(template) is not None


def csv_cell(param: dict) -> str:
    """The CSV's value cell: `csv_value` rendered, plus any `csv_note`.

    `csv_note` is text the CSV carries that the doc's own columns and prose
    already say (the shape clause on `fed.script`); it is not part of the
    machine value, so the round-trip check compares the value alone.
    """
    cell = render(param["csv_value"], param["value"])
    note = param.get("csv_note")
    return f"{cell} {note}" if note else cell


def check_template(param: dict) -> None:
    """A `csv_value` template must be renderable from the row's machine value.

    The semantic check -- that the rendered cell still *means* the machine
    value -- is `check_csv_round_trip`, which has the whole row in hand.
    """
    pid = param["id"]
    template = param["csv_value"]
    for token in ("bars", "stripped"):
        if f"%{token}%" in template and not isinstance(param["value"], str):
            die(f"{pid}: csv_value uses %{token}% but the value is not text")
    if param.get("doc_value") is not None and not isinstance(
            param["doc_value"], str):
        die(f"{pid}: doc_value must be text")
    if "doc_value" in param:
        for token in ("bars", "stripped"):
            if f"%{token}%" in param["doc_value"] and not isinstance(
                    param["value"], str):
                die(f"{pid}: doc_value uses %{token}% but the value is not text")


def typed_value(pid: str, csv_value: str) -> object:
    """The machine value a human value cell means.

    This is the only place a value cell is interpreted, and it is used both to
    bootstrap `params.json` and to check it, so the two cannot disagree.  The
    status/provenance marker words are removed first, because a cell that still
    ends in `mea` or `built` means the value in front of it.
    """
    text = csv_value.strip()
    for word in MARKER_WORDS:
        text = re.sub(rf"\b{re.escape(word)}\b", " ", text)
    text = " ".join(text.split())
    low = text.lower()
    if low == "true dec" or low == "true":
        return True
    if low == "false dec" or low == "false":
        return False
    if re.search(r"\bsuperseded\b", low) or low.startswith("open"):
        return None
    if "0x" in low:
        # A hex spelling is a name for the value, not a decimal to parse.
        return text
    # A cell is a number only if the number is the whole cell; `2-of-2` and
    # `4-of-N` are names, not the integers 2 and 4.
    match = re.match(r"^[<>]?\s*([0-9][0-9,]*)\s*$", text)
    if match:
        return int(match.group(1).replace(",", ""))
    return text


def rust_int(value: int) -> str:
    digits = str(value)
    groups = []
    while len(digits) > 3:
        groups.insert(0, digits[-3:])
        digits = digits[:-3]
    groups.insert(0, digits)
    return "_".join(groups)


# ---------------------------------------------------------------------------
# markdown helpers
# ---------------------------------------------------------------------------

def plain_cell(cell: str) -> str:
    """A markdown table cell as plain text, with whitespace collapsed.

    `\u2016` is how the doc's table spells the layout's `|` separator without
    ending the markdown row early, so it is folded back to `|` here.
    """
    text = cell.replace("**", "").replace("*", "").replace("`", "")
    text = text.replace("\u2016", "|")
    return " ".join(text.split())


def strip_markers(text: str) -> str:
    """Marker words gone, and the punctuation they were glued to with them.

    `2-of-2 OP_CHECKMULTISIG dec, shape accepted in code built` is the value
    plus a marker plus a clause; removing the marker must not leave its comma
    behind in the value.
    """
    for word in MARKER_WORDS:
        text = re.sub(rf"\b{re.escape(word)}\b[\s,;:]*", " ", text)
    return " ".join(text.split())


def value_core(cell: str) -> str:
    """A human value cell with the marker words removed."""
    return strip_markers(plain_cell(cell))


def markdown_row(line: str) -> list[str]:
    line = line.strip()
    if not line.startswith("|"):
        return []
    return [cell.strip() for cell in line.strip("|").split("|")]


def doc_cell(cell: str) -> str:
    """A generated value cell, safe to put inside a markdown table row."""
    return cell.replace("|", "\u2016")


def value_cell(param: dict) -> str:
    """The doc-06 Value cell for a row.

    With an explicit `doc_value` template the cell is that template rendered.
    Otherwise it is the CSV cell with the status/provenance marker words
    removed, bolded -- the doc keeps one value column and no marker words.
    """
    if "doc_value" in param:
        return doc_cell(render(param["doc_value"], param["value"]))
    core = render(param["csv_value"], param["value"])
    for word in MARKER_WORDS:
        core = re.sub(rf"\b{re.escape(word)}\b", " ", core)
    return doc_cell("**" + " ".join(core.split()) + "**")


# ---------------------------------------------------------------------------
# the projections
# ---------------------------------------------------------------------------

def load_params(require_sheet: bool = True) -> dict:
    if not PARAMS_JSON.exists():
        die(f"{PARAMS_JSON.relative_to(ROOT)} does not exist "
            "(bootstrap it with --from-csv)")
    params = json.loads(PARAMS_JSON.read_text(encoding="utf-8"))
    if require_sheet:
        check_sheet(params)
    return params


def check_sheet(params: dict) -> None:
    """The JSON's own shape, before any projection is looked at."""
    ids = [p["id"] for p in params["parameters"]]
    if len(set(ids)) != len(ids):
        die("config/params.json lists a parameter id twice")
    sections = params.get("sections")
    if not sections:
        die("config/params.json has no `sections`; the doc-06 grouping lives "
            "there")
    planned = [pid for s in sections for pid in s["parameters"]]
    if planned != ids:
        missing = [i for i in ids if i not in planned]
        extra = [i for i in planned if i not in ids]
        if missing or extra:
            die("config/params.json sections and parameters list different IDs "
                f"(section-only={extra}, parameter-only={missing})")
        die("config/params.json sections list the parameters in a different "
            "order than the parameters themselves")
    legend = params.get("legend") or {}
    for axis, words in (("status", STATUSES), ("provenance", PROVENANCES)):
        entries = legend.get(axis)
        if not entries:
            die(f"config/params.json has no `legend.{axis}`")
        names = [e["name"] for e in entries]
        if names != words:
            die(f"config/params.json legend.{axis} lists {names}, expected "
                f"{words}")
        if [e["short"] for e in entries] != [e["short"] for e in entries]:
            die(f"config/params.json legend.{axis} has a duplicate short word")
    for param in params["parameters"]:
        pid = param["id"]
        for axis, words in (("status", STATUSES), ("provenance", PROVENANCES)):
            if param.get(axis) not in words:
                die(f"{pid}: {axis} {param.get(axis)!r} is not one of "
                    f"{', '.join(words)}")
        doc_name = param.get("doc_name")
        if doc_name is not None and value_core(doc_name) != \
                value_core(param["name"]):
            die(f"{pid}: doc_name {doc_name!r} is not the plain name "
                f"{param['name']!r}")
        doc_desc = param.get("doc_description")
        if doc_desc is not None and not doc_desc.strip():
            die(f"{pid}: doc_description is empty")
        check_template(param)


def emit_csv(params: dict) -> bytes:
    """The exact bytes of the checked-in CSV: CRLF, minimal quoting."""
    buf = io.StringIO(newline="")
    writer = csv.DictWriter(buf, fieldnames=CSV_COLUMNS, lineterminator="\r\n")
    writer.writeheader()
    for param in params["parameters"]:
        writer.writerow({
            "id": param["id"], "name": param["name"],
            "value": csv_cell(param), "status": param["status"],
            "provenance": param["provenance"],
            "description": param["description"],
        })
    return buf.getvalue().encode("utf-8")


def check_csv_round_trip(params: dict) -> None:
    """Each generated CSV cell must still mean the row's machine value.

    This is the `csv_value` trap made exact: the CSV cell legitimately differs
    from `value` (units, a percentage, a literal 200 on a superseded row), but
    what it *means* may not.
    """
    rows = list(csv.DictReader(io.StringIO(
        emit_csv(params).decode("utf-8"), newline="")))
    if [r["id"] for r in rows] != [p["id"] for p in params["parameters"]]:
        die("the generated CSV and config/params.json list different IDs")
    for row, param in zip(rows, params["parameters"]):
        pid = row["id"]
        for field in ("name", "status", "provenance", "description"):
            if param[field] != row[field]:
                die(f"{pid}: {field} differs between the generated CSV and "
                    f"config/params.json")
        value_part = render(param["csv_value"], param["value"])
        if csv_cell(param) != row["value"]:
            die(f"{pid}: generated CSV cell {row['value']!r} is not what "
                f"csv_value projects ({csv_cell(param)!r})")
        if param.get("csv_note"):
            # The note is text the CSV carries after the value; it is not part
            # of the machine value, and the doc's own columns say it.
            note = param["csv_note"]
            if not row["value"].endswith(note):
                die(f"{pid}: csv_note {note!r} is not at the end of its cell "
                    f"{row['value']!r}")
        meant = spell_meaning(pid, value_part, param["value"])
        if not value_equivalent(param["value"], meant):
            die(f"{pid}: machine value {param['value']!r} does not match its "
                f"CSV cell {row['value']!r} (which means {meant!r})")


def emit_doc_parameter_tables(params: dict) -> str:
    """The generated region of doc 06: one table per section, in JSON order."""
    by_id = {p["id"]: p for p in params["parameters"]}
    out: list[str] = []
    for section in params["sections"]:
        out.append(section["heading"])
        out.append("")
        out.append("| " + " | ".join(DOC_COLUMNS) + " |")
        out.append("|" + "|".join(["---"] * len(DOC_COLUMNS)) + "|")
        for pid in section["parameters"]:
            param = by_id[pid]
            out.append("| " + " | ".join([
                f"`{param['id']}`",
                doc_cell(param.get("doc_name", param["name"])),
                value_cell(param),
                f"`{param['status']}`",
                f"`{param['provenance']}`",
                doc_cell(param.get("doc_description", param["description"])),
            ]) + " |")
        out.append("")
    # Drop the trailing blank line; the fence follows immediately.
    while out and out[-1] == "":
        out.pop()
    return "\n".join(out)


def emit_doc_legend(params: dict, axis: str) -> str:
    """One legend table, with the counts computed from the rows it describes."""
    counts: dict[str, int] = {e["name"]: 0 for e in params["legend"][axis]}
    for param in params["parameters"]:
        counts[param[axis]] += 1
    rows = [
        "| " + axis.capitalize() + " | Meaning | Count |",
        "|---|---|---|",
    ]
    for entry in params["legend"][axis]:
        rows.append(f"| **`{entry['name']}`** | {entry['meaning']} | "
                    f"**{counts[entry['name']]}** |")
    return "\n".join(rows)


def check_legend_covers(params: dict) -> None:
    """Every row's status/provenance must appear in the generated legend."""
    for axis in ("status", "provenance"):
        words = {e["name"] for e in params["legend"][axis]}
        for param in params["parameters"]:
            if param[axis] not in words:
                die(f"{param['id']}: {axis} {param[axis]!r} is missing from "
                    f"the generated legend, so its count would be wrong")


def read_doc_rows(path: Path = PARAMS_MD) -> list[dict[str, str]]:
    """Every row of every parameter table in `docs/06-parameters.md`, in order.

    A parameter table is the one whose header is exactly `DOC_COLUMNS`; the
    document's other tables (the legends, the superseded IDs, the revenue
    sketch) are left alone.
    """
    if not path.exists():
        die(f"{path.relative_to(ROOT)} does not exist")
    lines = path.read_text(encoding="utf-8").splitlines()
    header = [c.lower() for c in DOC_COLUMNS]
    rows: list[dict[str, str]] = []
    i = 0
    while i < len(lines):
        if [plain_cell(c).lower() for c in markdown_row(lines[i])] != header:
            i += 1
            continue
        i += 1
        # The `|---|---|` rule, then rows until the table ends.
        if i < len(lines) and set(lines[i].strip()) <= set("|-: "):
            i += 1
        while i < len(lines):
            cells = markdown_row(lines[i])
            if len(cells) != len(DOC_COLUMNS):
                break
            rows.append(dict(zip(DOC_COLUMNS, cells)))
            i += 1
    if not rows:
        die(f"{path.relative_to(ROOT)} has no parameter table with the columns "
            f"{', '.join(DOC_COLUMNS)}")
    return rows


def check_doc_round_trip(params: dict) -> None:
    """The generated tables, re-parsed, must be the JSON again."""
    text = emit_doc_parameter_tables(params)
    rows: list[dict[str, str]] = []
    lines = text.splitlines()
    i = 0
    while i < len(lines):
        if [plain_cell(c).lower() for c in markdown_row(lines[i])] != \
                [c.lower() for c in DOC_COLUMNS]:
            i += 1
            continue
        i += 2
        while i < len(lines):
            cells = markdown_row(lines[i])
            if len(cells) != len(DOC_COLUMNS):
                break
            rows.append(dict(zip(DOC_COLUMNS, cells)))
            i += 1
    by_id = {p["id"]: p for p in params["parameters"]}
    if [plain_cell(r["id"]) for r in rows] != [p["id"] for p in params["parameters"]]:
        die("the generated doc-06 tables do not round-trip to the parameters")
    for row in rows:
        param = by_id[plain_cell(row["id"])]
        pid = param["id"]
        spelled = {"name": row["name"], "status": row["status"],
                   "provenance": row["provenance"]}
        for field in ("name", "status", "provenance"):
            if param[field] != plain_cell(spelled[field]):
                die(f"{pid}: generated doc-06 {field} does not round-trip")
        doc_name = param.get("doc_name")
        if doc_name is not None and plain_cell(row["name"]) != \
                plain_cell(doc_name):
            die(f"{pid}: generated doc-06 name does not round-trip the "
                f"styled spelling")
        if not plain_cell(row["value"]) and param["value"] is not None:
            die(f"{pid}: generated doc-06 value cell is empty")
        if "doc_value" not in param:
            got = plain_cell(row["value"])
            value_part = render(param["csv_value"], param["value"])
            if not value_equivalent(
                    spell_meaning(pid, value_part, param["value"]),
                    spell_meaning(pid, got, param["value"])):
                die(f"{pid}: generated doc-06 value {got!r} does not mean "
                    f"{param['value']!r}")


def value_equivalent(machine: object, spelled: object) -> bool:
    """Does a human spelling carry the row's machine value?

    A prose row may spell its own provenance in the value itself
    (`gov.authority_threshold`), and the layout separator is written `|` in the
    machine value and `‖` in the tables, so text is compared with the marker
    words removed and the two bar spellings folded together.
    """
    if isinstance(machine, str) and isinstance(spelled, str):
        return value_core(machine) == value_core(spelled)
    return machine == spelled


def param_value(params: dict, pid: str):
    for p in params["parameters"]:
        if p["id"] == pid:
            return p["value"]
    die(f"unknown parameter id {pid!r}")


def check_arithmetic(params: dict) -> None:
    hours = param_value(params, "lc.window_hours")
    spacing = param_value(params, "lc.seconds_per_block")
    window = param_value(params, "lc.window")
    derived = hours * 3600 // spacing
    if window != derived:
        die(f"lc.window ({window}) != lc.window_hours * 3600 / "
            f"lc.seconds_per_block ({derived})")


def check_code_constants(params: dict) -> None:
    by_id = {p["id"]: p for p in params["parameters"]}
    for cc in params.get("code_constants", []):
        param = by_id.get(cc["parameter"])
        if param is None:
            die(f"code constant {cc['name']} names unknown parameter "
                f"{cc['parameter']!r}")
        text = param["description"]
        if cc["name"] not in text:
            die(f"code constant {cc['name']} is not named in the "
                f"{cc['parameter']} row it claims to come from")
        if str(cc["value"]) not in text:
            die(f"code constant {cc['name']} = {cc['value']} does not appear "
                f"in the {cc['parameter']} row text")
    for name, _ty, pid, _expr in RUST_CONSTANTS:
        if not isinstance(param_value(params, pid), int):
            die(f"Rust constant {name} maps to {pid}, whose value is not an "
                "integer")


def all_constants(params: dict) -> list[tuple[str, str, str, object]]:
    """(rust name, rust type, parameter id, value) for every generated const."""
    out = []
    for name, ty, pid, _expr in RUST_CONSTANTS:
        out.append((name, ty, pid, param_value(params, pid)))
    for cc in params.get("code_constants", []):
        out.append((cc["name"], cc["type"], cc["parameter"], cc["value"]))
    return out


def rust_literal(name: str, value: object, expr: str | None) -> str:
    if expr is not None:
        return expr
    assert isinstance(value, int)
    if name == "TOKEN_DECIMALS":
        return str(value)
    return rust_int(value)


def emit_rust(params: dict) -> str:
    by_id = {p["id"]: p for p in params["parameters"]}
    lines: list[str] = [
        "//! The SOLBEAM parameter sheet, as Rust constants.",
        "//!",
        "//! @generated by `config/gen.py` from `config/params.json` -- do not",
        "//! edit by hand.  `config/params.json` is the single source; the human",
        "//! tables in `docs/parameters.csv` and `docs/06-parameters.md`, this file",
        "//! and `tests/params.json` are all projections of it.  The generator",
        "//! computes every projection before it writes any of them, and refuses if",
        "//! the JSON and a projection disagree, so the document, the CSV and the",
        "//! code cannot drift.",
        "//!",
        "//! Regenerate with `python3 config/gen.py`.",
        "//!",
        "//! `difficulty.rs` includes this file by relative `#[path]` as well, so",
        "//! the cw-144 rule's spacing, clamps and lookback come from the same",
        "//! generated values without giving that module a Solana dependency (it",
        "//! is `#[path]`-included by the plain `cargo test` vectors crate).",
        "#![allow(dead_code)]",
        "",
    ]
    group = None
    for name, ty, pid, value in all_constants(params):
        param = by_id[pid]
        prefix = pid.split(".", 1)[0]
        if prefix != group:
            group = prefix
            lines += [f"// -- {prefix} --", ""]
        expr = next((e for n, _t, p, e in RUST_CONSTANTS if n == name), None)
        # `expr` is only valid on the constant it was written for.
        if name != "WINDOW":
            expr = None
        lines.append(f"/// `{pid}` -- {param['name']} -- `{csv_cell(param)}`.")
        lines.append("///")
        lines.append(f"/// {param['description']}")
        lines.append(f"pub const {name}: {ty} = {rust_literal(name, value, expr)};")
        lines.append("")
    return "\n".join(lines)


def emit_tests_json(params: dict) -> str:
    constants = {
        name: value for name, _ty, _pid, value in all_constants(params)
    }
    mirror = {
        "$generated": "config/gen.py from config/params.json -- DO NOT EDIT",
        "constants": constants,
        "parameters": {p["id"]: p["value"] for p in params["parameters"]},
    }
    return json.dumps(mirror, indent=2, ensure_ascii=False) + "\n"


# ---------------------------------------------------------------------------
# writing doc 06: only the fenced regions are ours
# ---------------------------------------------------------------------------

def replace_fence(text: str, name: str, body: str) -> str:
    """Swap the body between this region's begin/end fences."""
    begin = DOC_BEGIN % name
    end = DOC_END % name
    lines = text.splitlines()
    starts = [i for i, l in enumerate(lines) if l.strip() == begin]
    ends = [i for i, l in enumerate(lines) if l.strip() == end]
    if len(starts) != 1 or len(ends) != 1 or ends[0] < starts[0]:
        die(f"{PARAMS_MD.relative_to(ROOT)}: expected exactly one "
            f"`{name}` fenced region (BEGIN/END pair)")
    new = lines[:starts[0] + 1] + body.splitlines() + lines[ends[0]:]
    return "\n".join(new) + ("\n" if text.endswith("\n") else "")


def emit_doc(params: dict, current: str) -> str:
    text = replace_fence(current, DOC_STATUS_LEGEND,
                         emit_doc_legend(params, "status"))
    text = replace_fence(text, DOC_PROVENANCE_LEGEND,
                         emit_doc_legend(params, "provenance"))
    text = replace_fence(text, DOC_PARAMETER_TABLES,
                         emit_doc_parameter_tables(params))
    return text


def check_doc_fences(text: str) -> None:
    """The checked-in doc must carry every fenced region this script owns."""
    for name in (DOC_STATUS_LEGEND, DOC_PROVENANCE_LEGEND,
                 DOC_PARAMETER_TABLES):
        if (DOC_BEGIN % name) not in text or (DOC_END % name) not in text:
            die(f"{PARAMS_MD.relative_to(ROOT)} is missing the `{name}` "
                "fenced region; add the BEGIN/END markers around the "
                "generated tables")


# ---------------------------------------------------------------------------
# bootstrap
# ---------------------------------------------------------------------------

def bootstrap() -> dict:
    rows = read_csv(PARAMS_CSV)
    return {
        "schema": "solbeam.params/1",
        "note": ("The single canonical parameter sheet. docs/parameters.csv, "
                 "docs/06-parameters.md, programs/solbeam/src/params.rs and "
                 "tests/params.json are all generated from this file by "
                 "config/gen.py."),
        "source": "docs/parameters.csv",
        "code_constants": [
            {
                "name": "MAX_SCRIPT_LEN",
                "type": "usize",
                "value": 71,
                "parameter": "fed.script",
                "description": ("The length of the 2-of-2 deposit script "
                                "`fed.script` describes, asserted against that "
                                "row's own text."),
            }
        ],
        "parameters": [
            {
                "id": r["id"],
                "name": r["name"],
                "value": typed_value(r["id"], r["value"]),
                "status": r["status"],
                "provenance": r["provenance"],
                "description": r["description"],
                "csv_value": r["value"],
            }
            for r in rows
        ],
    }


def read_csv(path: Path) -> list[dict[str, str]]:
    raw = path.read_bytes()
    text = raw.decode("utf-8")
    rows = list(csv.DictReader(io.StringIO(text, newline="")))
    if not rows:
        die(f"{path} has no rows")
    return rows


# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------

def plan_outputs(params: dict) -> dict[Path, bytes]:
    """Every projection, as bytes, without touching the filesystem."""
    doc_before = PARAMS_MD.read_text(encoding="utf-8")
    check_doc_fences(doc_before)
    doc_after = emit_doc(params, doc_before)
    return {
        PARAMS_CSV: emit_csv(params),
        PARAMS_MD: doc_after.encode("utf-8"),
        PARAMS_RS: emit_rust(params).encode("utf-8"),
        TESTS_JSON: emit_tests_json(params).encode("utf-8"),
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true",
                        help="verify only; fail if a generated file is stale")
    parser.add_argument("--from-csv", action="store_true",
                        help="bootstrap config/params.json from docs/parameters.csv "
                             "(one-shot; re-apply the value templates afterwards)")
    parser.add_argument("--write-csv", action="store_true",
                        help="kept for compatibility; the default run already "
                             "regenerates every projection")
    args = parser.parse_args()

    if args.from_csv:
        data = bootstrap()
        PARAMS_JSON.write_text(
            json.dumps(data, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8")
        print(f"gen.py: wrote {PARAMS_JSON.relative_to(ROOT)} from "
              f"{PARAMS_CSV.relative_to(ROOT)}")

    params = load_params()
    if not PARAMS_MD.exists():
        die(f"{PARAMS_MD.relative_to(ROOT)} does not exist")
    if not PARAMS_MD.read_text(encoding="utf-8"):
        die(f"{PARAMS_MD.relative_to(ROOT)} is empty")

    # Verify everything before writing anything.  Every projection is computed
    # first and checked against the JSON; a failure here writes no file at all.
    check_csv_round_trip(params)
    check_doc_round_trip(params)
    check_legend_covers(params)
    check_arithmetic(params)
    check_code_constants(params)
    outputs = plan_outputs(params)

    # ... and check the checked-in bytes against the computed ones.
    stale: list[Path] = []
    for path, content in outputs.items():
        if not path.exists() or path.read_bytes() != content:
            stale.append(path)
    if args.check:
        for path in stale:
            print(f"gen.py: {path.relative_to(ROOT)} is stale; run "
                  "`python3 config/gen.py`", file=sys.stderr)
        if stale:
            die(f"{len(stale)} generated file(s) are stale")
        for path in outputs:
            print(f"gen.py: {path.relative_to(ROOT)} is up to date")
        print("gen.py: config/params.json agrees with docs/parameters.csv "
              "and docs/06-parameters.md")
        return

    for path, content in outputs.items():
        if path in stale:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)
            print(f"gen.py: wrote {path.relative_to(ROOT)}")
        else:
            print(f"gen.py: {path.relative_to(ROOT)} is up to date")
    print("gen.py: config/params.json agrees with docs/parameters.csv "
          "and docs/06-parameters.md")


if __name__ == "__main__":
    main()