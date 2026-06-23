"""Shared helpers for parsing playwright-cli --raw eval output (double-encoded JSON)."""
import json, sys


def load_raw(path):
    """playwright-cli --raw eval returns a JSON-encoded JSON string; decode both layers."""
    s = open(path).read().strip()
    d = json.loads(s)
    if isinstance(d, str):
        d = json.loads(d)
    return d


def money(x):
    """'$1,201.00' / '-$3,621.93' / '' -> float."""
    if not x:
        return 0.0
    neg = x.strip().startswith("-") or x.strip().startswith("(")
    n = x.replace("$", "").replace(",", "").replace("(", "").replace(")", "").replace("-", "").strip()
    if not n:
        return 0.0
    v = float(n)
    return -v if neg else v


if __name__ == "__main__":
    # quick: python _parse.py file.json  -> pretty print rows
    d = load_raw(sys.argv[1])
    print(json.dumps(d, indent=1)[:2000])
