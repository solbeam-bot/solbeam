# Fixtures

Two artefacts, one per half of Phase 1. Both are small, both are records of
something that was *measured* rather than assumed.

## `deposit_1.json` — Phase 1A

A complete, self-contained deposit proof produced on the synthetic chain:

- the checkpoint and 16 block headers
- the deposit transaction
- a **202-byte mint instruction**, with a committed `sha256d` hash

It is **byte-deterministic** across runs (re-run `check_bsv_pegin.py` and the
bytes are identical), and the checker re-verifies it **from the file alone** —
so it stands on its own rather than depending on the process that produced it.

This is what Phase 2 consumes: the Solana program must verify *these* bytes.

## `node_merkleproof_raw.json` — Phase 1B

The **unmodified response from a real SV Node v1.1.1** to:

```
getmerkleproof2 "<blockhash>" "<txid>"
```

Recorded by the first live run of `check_bsv_node.py`, and committed for the
same reason the rest of the suite exists: so the format is a fact in the repo
rather than something someone remembers. It is what corrected four separate
wrong assumptions about this RPC (argument order, the `nodes` key, the encodings
of the hashes, and the type of `target`).

Reproduce it against any SV Node in regtest:

```bash
poc/scripts/regtest-up.sh
export SOLBEAM_RPC=http://127.0.0.1:18443 SOLBEAM_RPC_USER=solbeam SOLBEAM_RPC_PASS=solbeam
python3 poc/checks/check_bsv_node.py     # rewrites this file
```

The exact shape, and how the harness interprets it, is written up in
[`../VERSIONS.md`](../VERSIONS.md#sv-node-rpc-facts).
