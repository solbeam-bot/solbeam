#!/usr/bin/env python3
"""One canonical parameter sheet, projected into the program and the tests.

`config/params.json` is the single source of truth.  `docs/parameters.csv` is
its human-readable projection, `poc/solana/programs/solbeam/src/params.rs` is
its Rust projection, and `poc/solana/tests/params.json` is the projection the
test suite reads.  Nothing is retyped: change a value in `config/params.json`,
run this script, and the program and the suite move together.

The check is the point.  If `config/params.json` and `docs/parameters.csv`
disagree on any value, name, status or description, this exits non-zero and
writes nothing -- so the document, the CSV and the code cannot drift.

Usage:
    python3 config/gen.py              # verify, then (re)write the generated files
    python3 config/gen.py --check      # verify only; fail if a generated file is stale
    python3 config/gen.py --from-csv   # bootstrap config/params.json from the CSV
    python3 config/gen.py --write-csv  # also regenerate docs/parameters.csv

`--write-csv` is deliberately not the default: the CSV is checked in, and the
default run proves that the checked-in bytes are exactly what this script would
emit from `config/params.json`.
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
PARAMS_RS = ROOT / "poc" / "solana" / "programs" / "solbeam" / "src" / "params.rs"
TESTS_JSON = ROOT / "poc" / "solana" / "tests" / "params.json"

CSV_COLUMNS = ["id", "name", "value", "status", "description"]

# Parameter IDs whose value is not a plain leading number and need a spelling.
EXPLICIT_VALUES = {
    "lc.daa": "cw-144",
    "fed.script": "2-of-2 OP_CHECKMULTISIG",
    "pi.op_return_layout": "version | cluster_id | program_hash | flags | recipient",
}

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
]


def die(message: str) -> "NoReturn":  # type: ignore[name-defined]
    print(f"gen.py: error: {message}", file=sys.stderr)
    sys.exit(1)


# ---------------------------------------------------------------------------
# CSV <-> params.json
# ---------------------------------------------------------------------------

def read_csv(path: Path) -> list[dict[str, str]]:
    raw = path.read_bytes()
    text = raw.decode("utf-8")
    rows = list(csv.DictReader(io.StringIO(text, newline="")))
    if not rows:
        die(f"{path} has no rows")
    return rows


def csv_bytes(rows: list[dict[str, str]]) -> bytes:
    """The exact bytes the checked-in CSV has: CRLF, minimal quoting."""
    buf = io.StringIO(newline="")
    writer = csv.DictWriter(buf, fieldnames=CSV_COLUMNS, lineterminator="\r\n")
    writer.writeheader()
    for row in rows:
        writer.writerow({k: row[k] for k in CSV_COLUMNS})
    return buf.getvalue().encode("utf-8")


def typed_value(pid: str, csv_value: str):
    """The machine value of a row, from its human `value` cell.

    This is the only place a value cell is interpreted, and it is used both to
    bootstrap `params.json` and to check it, so the two cannot disagree.
    """
    if pid in EXPLICIT_VALUES:
        return EXPLICIT_VALUES[pid]
    text = csv_value.strip()
    low = text.lower()
    if low == "true dec" or low == "true":
        return True
    if low == "false dec" or low == "false":
        return False
    if re.search(r"\bsuperseded\b", low) or low.startswith("open"):
        return None
    match = re.match(r"^[<>]?\s*([0-9][0-9,]*)", text)
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
# the check
# ---------------------------------------------------------------------------

def load_params() -> dict:
    if not PARAMS_JSON.exists():
        die(f"{PARAMS_JSON} does not exist (bootstrap it with --from-csv)")
    return json.loads(PARAMS_JSON.read_text(encoding="utf-8"))


def check_against_csv(params: dict, csv_rows: list[dict[str, str]]) -> None:
    csv_ids = [r["id"] for r in csv_rows]
    own_ids = [p["id"] for p in params["parameters"]]
    if csv_ids != own_ids:
        missing = [i for i in csv_ids if i not in own_ids]
        extra = [i for i in own_ids if i not in csv_ids]
        die("config/params.json and docs/parameters.csv list different IDs "
            f"(csv-only={missing}, json-only={extra})")

    for row, param in zip(csv_rows, params["parameters"]):
        pid = row["id"]
        for field in ("name", "status", "description"):
            if param.get(field) != row[field]:
                die(f"{pid}: {field} differs\n"
                    f"  csv : {row[field]!r}\n  json: {param.get(field)!r}")
        if param.get("csv_value") != row["value"]:
            die(f"{pid}: value differs\n"
                f"  csv : {row['value']!r}\n  json: {param.get('csv_value')!r}")
        expected = typed_value(pid, row["value"])
        if param.get("value") != expected:
            die(f"{pid}: machine value {param.get('value')!r} does not match "
                f"{row['value']!r} (which means {expected!r})")

    # The strongest form of the same statement: the checked-in CSV must be
    # byte-for-byte what this JSON projects.
    regenerated = csv_bytes([
        {
            "id": p["id"], "name": p["name"], "value": p["csv_value"],
            "status": p["status"], "description": p["description"],
        }
        for p in params["parameters"]
    ])
    if regenerated != PARAMS_CSV.read_bytes():
        die("docs/parameters.csv is not byte-identical to the CSV this "
            "config/params.json projects (regenerate with --write-csv)")

    check_arithmetic(params)
    check_code_constants(params, csv_rows)


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


def check_code_constants(params: dict, csv_rows: list[dict[str, str]]) -> None:
    by_id = {r["id"]: r for r in csv_rows}
    for cc in params.get("code_constants", []):
        row = by_id.get(cc["parameter"])
        if row is None:
            die(f"code constant {cc['name']} names unknown parameter "
                f"{cc['parameter']!r}")
        if cc["name"] not in row["description"]:
            die(f"code constant {cc['name']} is not named in the "
                f"{cc['parameter']} row it claims to come from")
        if str(cc["value"]) not in row["description"]:
            die(f"code constant {cc['name']} = {cc['value']} does not appear "
                f"in the {cc['parameter']} row text")
    for name, _ty, pid, _expr in RUST_CONSTANTS:
        if not isinstance(param_value(params, pid), int):
            die(f"Rust constant {name} maps to {pid}, whose value is not an "
                "integer")


# ---------------------------------------------------------------------------
# the projections
# ---------------------------------------------------------------------------

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
        "//! edit by hand.  `config/params.json` is the single source;",
        "//! `docs/parameters.csv` is its human-readable projection and this file",
        "//! is its Rust projection.  The generator refuses to run if the JSON and",
        "//! the CSV disagree, so the document, the CSV and the code cannot drift.",
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
        lines.append(f"/// `{pid}` -- {param['name']} -- `{param['csv_value']}`.")
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
# bootstrap
# ---------------------------------------------------------------------------

def bootstrap() -> dict:
    rows = read_csv(PARAMS_CSV)
    return {
        "schema": "solbeam.params/1",
        "note": ("The single canonical parameter sheet. docs/parameters.csv, "
                 "programs/solbeam/src/params.rs and tests/params.json are all "
                 "generated from this file by config/gen.py."),
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
                "description": r["description"],
                "csv_value": r["value"],
            }
            for r in rows
        ],
    }


# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------

def write_if_changed(path: Path, content: bytes, check: bool) -> None:
    rel = path.relative_to(ROOT)
    if path.exists() and path.read_bytes() == content:
        print(f"gen.py: {rel} is up to date")
        return
    if check:
        die(f"{rel} is stale; run `python3 config/gen.py`")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)
    print(f"gen.py: wrote {rel}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true",
                        help="verify only; fail if a generated file is stale")
    parser.add_argument("--from-csv", action="store_true",
                        help="bootstrap config/params.json from docs/parameters.csv")
    parser.add_argument("--write-csv", action="store_true",
                        help="regenerate docs/parameters.csv from config/params.json")
    args = parser.parse_args()

    if args.from_csv:
        data = bootstrap()
        PARAMS_JSON.write_text(
            json.dumps(data, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8")
        print(f"gen.py: wrote {PARAMS_JSON.relative_to(ROOT)} from "
              f"{PARAMS_CSV.relative_to(ROOT)}")

    params = load_params()
    csv_rows = read_csv(PARAMS_CSV)
    check_against_csv(params, csv_rows)

    if args.write_csv:
        PARAMS_CSV.write_bytes(csv_bytes([
            {
                "id": p["id"], "name": p["name"], "value": p["csv_value"],
                "status": p["status"], "description": p["description"],
            }
            for p in params["parameters"]
        ]))
        print(f"gen.py: wrote {PARAMS_CSV.relative_to(ROOT)}")

    write_if_changed(PARAMS_RS, emit_rust(params).encode("utf-8"), args.check)
    write_if_changed(TESTS_JSON, emit_tests_json(params).encode("utf-8"),
                     args.check)
    print("gen.py: config/params.json agrees with docs/parameters.csv")


if __name__ == "__main__":
    main()
