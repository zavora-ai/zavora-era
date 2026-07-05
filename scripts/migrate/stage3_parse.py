#!/usr/bin/env python3
"""Stage 3 (parse): turn the source P&L Detail + Balance Sheet Detail CSVs into
per-transaction records, tracking the account each line hits.

Outputs sample_data/migration/transactions.json and prints sanity totals to
verify against the source system's summary before any replay.
"""
import csv, json, sys
sys.path.insert(0, "scripts/migrate")
from _parse import money

QB = "sample_data/migration"

# Top-group -> Zavora account type (drives which side of the books / P&L bucket).
PNL_GROUP_ZTYPE = {
    "Income": "Revenue", "Other Income": "Revenue",
    "Cost of Goods Sold": "Expense", "Expenses": "Expense", "Other Expense": "Expense",
}
PNL_TOP_GROUPS = set(PNL_GROUP_ZTYPE) | {"Ordinary Income/Expenses"}

BS_GROUP_ZTYPE = {
    "ASSETS": "Asset", "Assets": "Asset", "LIABILITIES AND EQUITY": "Liability",
    "Liabilities": "Liability", "Equity": "Equity",
}


def parse_detail(path, group_ztype, top_groups):
    """Walk a source detail CSV. Returns list of line dicts with the resolved account
    and its Zavora type, plus the transaction identity."""
    rows = list(csv.reader(open(path)))
    lines = []
    current_account = None
    current_ztype = None
    group_stack = []  # track top group for ztype
    for r in rows:
        cells = [c.strip() for c in r]
        nonempty = [(j, c) for j, c in enumerate(cells) if c]
        if not nonempty:
            continue
        first_idx, first = nonempty[0]
        # header / total rows live in column 0
        if first_idx == 0:
            if first.startswith("Total for") or first.startswith("Total "):
                continue
            if first in ("Transaction date",):  # header artifact
                continue
            # a section header: could be a top group or a leaf account
            if first in top_groups or first in group_ztype:
                # a leaf account can share its parent group's name (e.g. the
                # "Cost of Goods Sold" account under the "Cost of Goods Sold"
                # group) — the repeat is the account, not another group.
                if group_stack and group_stack[-1] == first:
                    current_account = first
                else:
                    group_stack.append(first)
                    if first in group_ztype:
                        current_ztype = group_ztype[first]
                    current_account = None  # entering a group
            else:
                current_account = first  # leaf account
            continue
        # data row: col1=date col2=type col3=num col4=name col7=desc col8=split col9=amount
        if len(cells) > 9 and cells[2] and cells[2] != "Transaction type":
            # derive ztype from the nearest group with a known ztype
            zt = current_ztype
            for g in reversed(group_stack):
                if g in group_ztype:
                    zt = group_ztype[g]; break
            lines.append({
                "date": cells[1], "txn_type": cells[2], "num": cells[3], "name": cells[4],
                "description": cells[7], "split": cells[8], "amount": money(cells[9]),
                "account": current_account, "ztype": zt,
            })
    return lines


def txn_key(l):
    return (l["txn_type"], l["num"], l["date"], l["name"])


def main():
    pnl = parse_detail(f"{QB}/pnl_detail.csv", PNL_GROUP_ZTYPE, PNL_TOP_GROUPS)
    bs = parse_detail(f"{QB}/balancesheet_detail.csv", BS_GROUP_ZTYPE, set(BS_GROUP_ZTYPE))

    # group into transactions
    txns = {}
    for src, lines in (("pnl", pnl), ("bs", bs)):
        for l in lines:
            k = txn_key(l)
            t = txns.setdefault(k, {"txn_type": l["txn_type"], "num": l["num"],
                                    "date": l["date"], "name": l["name"],
                                    "pnl_lines": [], "bs_lines": []})
            (t["pnl_lines"] if src == "pnl" else t["bs_lines"]).append(l)

    out = list(txns.values())
    json.dump(out, open(f"{QB}/transactions.json", "w"), indent=1)

    # sanity totals
    from collections import Counter, defaultdict
    print("P&L detail lines:", len(pnl), " BS detail lines:", len(bs))
    print("transactions grouped:", len(out))
    print("by type:", dict(Counter(t["txn_type"] for t in out)))

    inc = sum(l["amount"] for l in pnl if l["ztype"] == "Revenue")
    exp = sum(l["amount"] for l in pnl if l["ztype"] == "Expense")
    print(f"\nP&L detail income sum (Revenue lines): {inc:,.2f}")
    print(f"P&L detail expense sum (Expense lines): {exp:,.2f}")
    print(f"implied net: {inc - exp:,.2f}   (source Net Income = 1,642.46)")

    # invoices income (for the invoice-flow replay)
    inv_income = defaultdict(float)
    for l in pnl:
        if l["txn_type"] == "Invoice":
            inv_income[l["num"]] += l["amount"]
    print(f"\ninvoices with income lines: {len(inv_income)}  total invoice income: {sum(inv_income.values()):,.2f}")


if __name__ == "__main__":
    main()
