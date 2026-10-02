# config — the parameter sheet and its projections

**`config/params.json` is the single source of truth.** `python3 config/gen.py` projects it, with
nothing retyped, into:

| projection | who reads it |
|---|---|
| `poc/solana/programs/solbeam/src/params.rs` | the compiled program |
| `poc/solana/tests/params.json` | the test suite |
| `docs/parameters.csv` | people and scripts |
| `docs/06-parameters.md` (the fenced regions only) | people |

The generator computes every projection before it writes any of them and refuses if a value cell,
a doc row or a legend count disagrees with the sheet, so a failure cannot leave one projection
updated and another stale.

```
python3 config/gen.py          # rewrite the projections
python3 config/gen.py --check  # verify only; fail if anything is stale
```

## Profiles: testnet without a second sheet

`config/params.json` describes the mainnet deployment. A **profile** is an **overlay** on it —
`config/params.<name>.json` — that carries **only the rows that differ**, so every value still has
exactly one source of truth and there is no 60-row copy to keep in sync.

```
python3 config/gen.py --profile testnet          # write the testnet projections
python3 config/gen.py --check                    # verify mainnet AND every profile
python3 config/gen.py --check --profile testnet  # verify just that profile
config/check.sh --profile testnet                # regenerate + verify that profile
```

Each overlay row must name a row that exists, set only fields that row already has, actually
change something, and carry a non-empty **`_what`** saying why it differs. `gen.py` refuses an
overlay row that breaks any of those, so a typo cannot silently leave the mainnet value in place.

### Where a profile's output goes

A profile is projected into **`config/out/<profile>/`**, never over the mainnet files:

```
config/out/testnet/params.rs          # Rust constants, banner says @profile testnet
config/out/testnet/tests-params.json  # the suite mirror, with $profile
config/out/testnet/parameters.csv     # opens with "# profile: testnet" comment lines
config/out/testnet/06-parameters.md   # the profile's tables behind a banner
```

Every artifact names its profile in a banner or header. These files are **committed** so
`gen.py --check` can compare them, and they are the reason `--check` covers profiles: a profile's
numbers are as load-bearing as the mainnet ones, they are just read by people deploying, not by
the program.

**A profile is not compiled.** The program and the suite read the mainnet `params.rs` and
`tests/params.json`; `config/out/<profile>/params.rs` is read by nothing. Deploying a profile
means copying its `params.rs` over the program's and rebuilding, which is a deliberate action —
and a `git diff` will show it if it is ever done by accident.

## `config/check.sh` — the change gate

```
config/check.sh                  # regenerate mainnet, verify mainnet + all profiles,
                                 # run the suite if config/params.json differs from HEAD
config/check.sh --profile NAME   # regenerate + verify that profile only
config/check.sh --suite          # force the suite
config/check.sh --no-suite       # refuse if the sheet is dirty
```

`gen.py --check` compares the projections **with each other**, never with the **compiled
program**. `check.sh` is the thing that runs the suite, which is why it exists.

**The suite is never run for a profile**, and that is deliberate: the compiled program reads the
mainnet `params.rs`, so `anchor test` could not see a profile change at all. A profile run says so
in its summary instead of implying a verification it did not perform.

**This is a convention, not a control.** There is no CI in this repository and no `.github/`.
`check.sh` runs when a maintainer runs it.
