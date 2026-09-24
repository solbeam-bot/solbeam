"""
SOLBEAM — shared BSV primitives (Python standard library only).

One implementation of everything the peg needs on the BSV side, used by the
checkers in this directory. The Go/Rust ports must reproduce these byte for
byte; the checkers are what proves the Python reference is right, against real
chain data.

Contents
  hashing        sha256d, hash160, endianness helpers
  serialisation  varints, legacy transactions, txids
  scripts        push parsing, P2PKH, OP_RETURN
  addresses      Base58Check, P2PKH address encode/decode
  signing        SIGHASH_FORKID digest, secp256k1 ECDSA (verify + deterministic sign)

Note: BSV has no SegWit — no marker, no flag, no witness commitment, and
CLTV/CSV are no-ops from the Genesis upgrade onward. Code here assumes that.
"""

import hashlib
import hmac

SIGHASH_ALL = 0x01
SIGHASH_NONE = 0x02
SIGHASH_SINGLE = 0x03
SIGHASH_FORKID = 0x40
SIGHASH_ANYONECANPAY = 0x80

OP_RETURN = 0x6A
OP_DUP = 0x76
OP_HASH160 = 0xA9
OP_EQUALVERIFY = 0x88
OP_CHECKSIG = 0xAC

MAINNET_P2PKH = 0x00
TESTNET_P2PKH = 0x6F

# ---------------------------------------------------------------------------
# hashing / endianness
# ---------------------------------------------------------------------------

def sha256d(b: bytes) -> bytes:
    return hashlib.sha256(hashlib.sha256(b).digest()).digest()


def hash160(b: bytes) -> bytes:
    return hashlib.new("ripemd160", hashlib.sha256(b).digest()).digest()


def le(display_hex: str) -> bytes:
    """Explorer hex (big-endian display) -> internal little-endian bytes."""
    return bytes.fromhex(display_hex)[::-1]


def display(be_bytes: bytes) -> str:
    return be_bytes[::-1].hex()


# ---------------------------------------------------------------------------
# serialisation
# ---------------------------------------------------------------------------

def read_varint(buf: bytes, i: int):
    first = buf[i]
    if first < 0xFD:
        return first, i + 1
    if first == 0xFD:
        return int.from_bytes(buf[i + 1:i + 3], "little"), i + 3
    if first == 0xFE:
        return int.from_bytes(buf[i + 1:i + 5], "little"), i + 5
    return int.from_bytes(buf[i + 1:i + 9], "little"), i + 9


def write_varint(n: int) -> bytes:
    if n < 0xFD:
        return bytes([n])
    if n <= 0xFFFF:
        return b"\xfd" + n.to_bytes(2, "little")
    if n <= 0xFFFFFFFF:
        return b"\xfe" + n.to_bytes(4, "little")
    return b"\xff" + n.to_bytes(8, "little")


def parse_tx(raw: bytes) -> dict:
    """Parse a legacy (pre-SegWit) Bitcoin-family transaction."""
    i = 0
    tx = {"version": int.from_bytes(raw[i:i + 4], "little", signed=True)}; i += 4
    nin, i = read_varint(raw, i)
    tx["vin"] = []
    for _ in range(nin):
        txid = raw[i:i + 32]; i += 32
        vout = int.from_bytes(raw[i:i + 4], "little"); i += 4
        slen, i = read_varint(raw, i)
        script = raw[i:i + slen]; i += slen
        sequence = int.from_bytes(raw[i:i + 4], "little"); i += 4
        tx["vin"].append({"txid": txid, "vout": vout, "script": script, "sequence": sequence})
    nout, i = read_varint(raw, i)
    tx["vout"] = []
    for _ in range(nout):
        o = {"value": int.from_bytes(raw[i:i + 8], "little")}; i += 8
        slen, i = read_varint(raw, i)
        o["script"] = raw[i:i + slen]; i += slen
        tx["vout"].append(o)
    tx["locktime"] = int.from_bytes(raw[i:i + 4], "little"); i += 4
    if i != len(raw):
        raise ValueError(f"trailing bytes: consumed {i} of {len(raw)}")
    return tx


def serialise_outpoint(v: dict) -> bytes:
    return v["txid"] + int(v["vout"]).to_bytes(4, "little")


def serialise_output(o: dict) -> bytes:
    return int(o["value"]).to_bytes(8, "little") + write_varint(len(o["script"])) + o["script"]


def serialise_tx(tx: dict) -> bytes:
    out = int(tx["version"]).to_bytes(4, "little", signed=True)
    out += write_varint(len(tx["vin"]))
    for v in tx["vin"]:
        out += serialise_outpoint(v) + write_varint(len(v["script"])) + v["script"]
        out += int(v["sequence"]).to_bytes(4, "little")
    out += write_varint(len(tx["vout"]))
    out += b"".join(serialise_output(o) for o in tx["vout"])
    out += int(tx["locktime"]).to_bytes(4, "little")
    return out


def txid_of(raw: bytes) -> str:
    return sha256d(raw)[::-1].hex()


def new_tx(version: int = 1, locktime: int = 0) -> dict:
    return {"version": version, "vin": [], "vout": [], "locktime": locktime}


def add_input(tx: dict, prev_txid: bytes, vout: int, sequence: int = 0xFFFFFFFF):
    tx["vin"].append({"txid": prev_txid, "vout": vout, "script": b"", "sequence": sequence})


def add_output(tx: dict, value: int, script: bytes):
    tx["vout"].append({"value": value, "script": script})


# ---------------------------------------------------------------------------
# scripts
# ---------------------------------------------------------------------------

def push_data(data: bytes) -> bytes:
    n = len(data)
    if n < 0x4C:
        return bytes([n]) + data
    if n <= 0xFF:
        return b"\x4c" + bytes([n]) + data
    if n <= 0xFFFF:
        return b"\x4d" + n.to_bytes(2, "little") + data
    return b"\x4e" + n.to_bytes(4, "little") + data


def parse_pushes(script: bytes) -> list:
    pushes, i = [], 0
    while i < len(script):
        op = script[i]; i += 1
        if op < 0x4C:
            n = op
        elif op == 0x4C:
            n = script[i]; i += 1
        elif op == 0x4D:
            n = int.from_bytes(script[i:i + 2], "little"); i += 2
        elif op == 0x4E:
            n = int.from_bytes(script[i:i + 4], "little"); i += 4
        else:
            raise ValueError(f"non-push opcode 0x{op:02x} in scriptSig")
        if i + n > len(script):
            raise ValueError("push runs past end of script")
        pushes.append(script[i:i + n]); i += n
    return pushes


def p2pkh_script(h160: bytes) -> bytes:
    """<DUP> <HASH160> <20-byte hash> <EQUALVERIFY> <CHECKSIG>"""
    if len(h160) != 20:
        raise ValueError("hash160 must be 20 bytes")
    return bytes([OP_DUP, OP_HASH160, 0x14]) + h160 + bytes([OP_EQUALVERIFY, OP_CHECKSIG])


def p2pkh_hash(script: bytes):
    """Return the 20-byte hash from a P2PKH scriptPubKey, else None."""
    if len(script) == 25 and script[0] == OP_DUP and script[1] == OP_HASH160 \
            and script[2] == 0x14 and script[23] == OP_EQUALVERIFY and script[24] == OP_CHECKSIG:
        return script[3:23]
    return None


def op_return_script(payload: bytes) -> bytes:
    """OP_RETURN carrying arbitrary data. BSV's data-carrier limit is effectively
    unlimited post-Genesis, so a 32-byte Solana address is unremarkable."""
    return bytes([OP_RETURN]) + push_data(payload)


def parse_op_return(script: bytes):
    """Return the payload of an OP_RETURN output, or None."""
    if not script or script[0] != OP_RETURN:
        return None
    pushes = parse_pushes(script[1:])
    return pushes[0] if pushes else None


# ---------------------------------------------------------------------------
# addresses
# ---------------------------------------------------------------------------

B58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


def b58encode(raw: bytes) -> str:
    n = int.from_bytes(raw, "big")
    out = ""
    while n:
        n, r = divmod(n, 58)
        out = B58_ALPHABET[r] + out
    for byte in raw:
        if byte == 0:
            out = "1" + out
        else:
            break
    return out


def b58decode(text: str) -> bytes:
    n = 0
    for ch in text:
        if ch not in B58_ALPHABET:
            raise ValueError(f"invalid base58 character {ch!r}")
        n = n * 58 + B58_ALPHABET.index(ch)
    pad = len(text) - len(text.lstrip("1"))
    body = n.to_bytes((n.bit_length() + 7) // 8, "big") if n else b""
    return b"\x00" * pad + body


def b58check_encode(payload: bytes) -> str:
    return b58encode(payload + sha256d(payload)[:4])


def b58check_decode(text: str) -> bytes:
    raw = b58decode(text)
    if len(raw) < 5 or sha256d(raw[:-4])[:4] != raw[-4:]:
        raise ValueError("bad Base58Check checksum")
    return raw[:-4]


def p2pkh_address(h160: bytes, version: int = MAINNET_P2PKH) -> str:
    return b58check_encode(bytes([version]) + h160)


def address_to_hash160(address: str):
    """Return (version, hash160) for a P2PKH address."""
    payload = b58check_decode(address)
    if len(payload) != 21:
        raise ValueError("not a P2PKH address")
    return payload[0], payload[1:]


# ---------------------------------------------------------------------------
# signature hash (SIGHASH_FORKID / BIP143-style, no witness)
# ---------------------------------------------------------------------------

def sighash_forkid(tx: dict, index: int, prevout_value: int,
                   prevout_script: bytes, sighash_type: int = SIGHASH_ALL | SIGHASH_FORKID) -> bytes:
    """Fields: version, hashPrevouts, hashSequence, outpoint, scriptCode,
    amount, nSequence, hashOutputs, nLockTime, sighashType (4 bytes LE)."""
    if not sighash_type & SIGHASH_FORKID:
        raise ValueError("BSV requires SIGHASH_FORKID (0x40)")
    base = sighash_type & 0x1F
    anyonecanpay = bool(sighash_type & SIGHASH_ANYONECANPAY)

    hash_prevouts = bytes(32) if anyonecanpay else sha256d(
        b"".join(serialise_outpoint(v) for v in tx["vin"]))
    hash_sequence = bytes(32) if (anyonecanpay or base in (SIGHASH_NONE, SIGHASH_SINGLE)) else sha256d(
        b"".join(int(v["sequence"]).to_bytes(4, "little") for v in tx["vin"]))
    if base == SIGHASH_ALL:
        hash_outputs = sha256d(b"".join(serialise_output(o) for o in tx["vout"]))
    elif base == SIGHASH_SINGLE and index < len(tx["vout"]):
        hash_outputs = sha256d(serialise_output(tx["vout"][index]))
    else:
        hash_outputs = bytes(32)

    vin = tx["vin"][index]
    preimage = (
        int(tx["version"]).to_bytes(4, "little", signed=True)
        + hash_prevouts + hash_sequence + serialise_outpoint(vin)
        + write_varint(len(prevout_script)) + prevout_script
        + int(prevout_value).to_bytes(8, "little")
        + int(vin["sequence"]).to_bytes(4, "little")
        + hash_outputs
        + int(tx["locktime"]).to_bytes(4, "little")
        + sighash_type.to_bytes(4, "little")
    )
    return sha256d(preimage)


# ---------------------------------------------------------------------------
# DER
# ---------------------------------------------------------------------------

def der_decode(sig: bytes):
    if sig[0] != 0x30:
        raise ValueError("DER: missing SEQUENCE")
    i = 2
    if sig[1] & 0x80:
        i = 2 + (sig[1] & 0x7F)
    if sig[i] != 0x02:
        raise ValueError("DER: missing r INTEGER")
    rlen = sig[i + 1]
    r = int.from_bytes(sig[i + 2:i + 2 + rlen], "big")
    i += 2 + rlen
    if sig[i] != 0x02:
        raise ValueError("DER: missing s INTEGER")
    slen = sig[i + 1]
    s = int.from_bytes(sig[i + 2:i + 2 + slen], "big")
    return r, s


def der_encode(r: int, s: int) -> bytes:
    def integer(x: int) -> bytes:
        b = x.to_bytes(max(1, (x.bit_length() + 7) // 8), "big")
        if b[0] & 0x80:
            b = b"\x00" + b
        return b"\x02" + bytes([len(b)]) + b
    body = integer(r) + integer(s)
    return b"\x30" + bytes([len(body)]) + body


# ---------------------------------------------------------------------------
# secp256k1
# ---------------------------------------------------------------------------

P = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC2F
N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
GX = 0x79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798
GY = 0x483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8
G = (GX, GY)


def _inv(a: int, m: int) -> int:
    return pow(a, m - 2, m)


def pt_add(p1, p2):
    if p1 is None:
        return p2
    if p2 is None:
        return p1
    x1, y1 = p1
    x2, y2 = p2
    if x1 == x2 and (y1 + y2) % P == 0:
        return None
    if p1 == p2:
        lam = (3 * x1 * x1) * _inv(2 * y1, P) % P
    else:
        lam = (y2 - y1) * _inv((x2 - x1) % P, P) % P
    x3 = (lam * lam - x1 - x2) % P
    return (x3, (lam * (x1 - x3) - y1) % P)


def pt_mul(k: int, pt):
    result = None
    while k:
        if k & 1:
            result = pt_add(result, pt)
        pt = pt_add(pt, pt)
        k >>= 1
    return result


def pubkey_from_priv(priv: int, compressed: bool = True) -> bytes:
    x, y = pt_mul(priv, G)
    if compressed:
        return bytes([0x02 if y % 2 == 0 else 0x03]) + x.to_bytes(32, "big")
    return b"\x04" + x.to_bytes(32, "big") + y.to_bytes(32, "big")


def parse_pubkey(b: bytes):
    if len(b) == 65 and b[0] == 0x04:
        return (int.from_bytes(b[1:33], "big"), int.from_bytes(b[33:], "big"))
    if len(b) == 33 and b[0] in (0x02, 0x03):
        x = int.from_bytes(b[1:], "big")
        y = pow((pow(x, 3, P) + 7) % P, (P + 1) // 4, P)
        if (y % 2 == 1) != (b[0] == 0x03):
            y = P - y
        return (x, y)
    raise ValueError("unsupported public key encoding")


def ecdsa_verify(pub, z: int, r: int, s: int) -> bool:
    if not (1 <= r < N and 1 <= s < N):
        return False
    w = _inv(s, N)
    pt = pt_add(pt_mul((z * w) % N, G), pt_mul((r * w) % N, pub))
    return pt is not None and pt[0] % N == r


def _rfc6979_k(priv: int, z: int) -> int:
    """Deterministic nonce (RFC 6979). Test-grade: adequate for the harness,
    and the same construction production signers should use."""
    x = priv.to_bytes(32, "big")
    h1 = z.to_bytes(32, "big")
    v, k = b"\x01" * 32, b"\x00" * 32
    k = hmac.new(k, v + b"\x00" + x + h1, hashlib.sha256).digest()
    v = hmac.new(k, v, hashlib.sha256).digest()
    k = hmac.new(k, v + b"\x01" + x + h1, hashlib.sha256).digest()
    v = hmac.new(k, v, hashlib.sha256).digest()
    while True:
        v = hmac.new(k, v, hashlib.sha256).digest()
        candidate = int.from_bytes(v, "big")
        if 1 <= candidate < N:
            return candidate
        k = hmac.new(k, v + b"\x00", hashlib.sha256).digest()
        v = hmac.new(k, v, hashlib.sha256).digest()


def ecdsa_sign(priv: int, digest: bytes) -> bytes:
    """Return a DER signature (low-S) plus the sighash byte appended."""
    z = int.from_bytes(digest, "big")
    k = _rfc6979_k(priv, z)
    r = pt_mul(k, G)[0] % N
    s = (_inv(k, N) * (z + r * priv)) % N
    if s > N // 2:
        s = N - s
    return der_encode(r, s)


def sign_input(tx: dict, index: int, priv: int, prevout_value: int,
               prevout_script: bytes, sighash_type: int = SIGHASH_ALL | SIGHASH_FORKID) -> bytes:
    """Produce a P2PKH scriptSig for input `index`."""
    digest = sighash_forkid(tx, index, prevout_value, prevout_script, sighash_type)
    sig = ecdsa_sign(priv, digest) + bytes([sighash_type])
    return push_data(sig) + push_data(pubkey_from_priv(priv))
