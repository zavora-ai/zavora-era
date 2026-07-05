#!/usr/bin/env python3
"""Stage 4: run Zavora's Trial Balance / P&L / Balance Sheet and compare to source."""
import json, sys, urllib.request, urllib.error
QB = "sample_data/migration"
BASE = "http://localhost:8080/api/v1"
maps = json.load(open(f"{QB}/zavora_maps.json"))
targets = json.load(open(f"{QB}/comparison_targets.json"))
token = maps["token"]

def call(method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(BASE + path, data=data, method=method)
    req.add_header("Content-Type", "application/json")
    req.add_header("Authorization", "Bearer " + token)
    try:
        with urllib.request.urlopen(req) as r:
            return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read() or b"null")

def unwrap(d, key):
    if not isinstance(d, dict): return None
    if "content" in d and isinstance(d["content"], dict): d = d["content"]
    return d.get(key)

def report(rtype, params):
    st, d = call("POST", "/reports", {"entity_id": "00000000-0000-0000-0000-000000000000", "report_type": rtype, "parameters": params})
    return unwrap(d, rtype)

def row(label, src, zav):
    q = float(src) if src is not None else 0.0
    z = float(zav) if zav is not None else 0.0
    delta = z - q
    flag = "OK " if abs(delta) < 0.01 else "!! "
    print(f"  {flag}{label:28} source {q:>13,.2f}   Zavora {z:>13,.2f}   Δ {delta:>12,.2f}")

ASAT = "2026-12-31"
tb = report("TrialBalance", {"as_at": ASAT})
pl = report("ProfitAndLoss", {"period_from": "2026-01-01", "period_to": ASAT})
bs = report("BalanceSheet", {"as_at": ASAT})

print("=== TRIAL BALANCE ===")
print(f"  debits {float(tb['total_debits']):,.2f}  credits {float(tb['total_credits']):,.2f}  "
      f"balanced={tb['is_balanced']}  diff {float(tb['difference']):,.2f}")

print("\n=== PROFIT & LOSS  (Zavora vs the source accounting system) ===")
t = targets["pnl"]
row("Total Income", t["total_income"], pl["total_revenue"])
row("Cost of Goods Sold", t["total_cogs"], pl["total_cost_of_sales"])
row("Gross Profit", t["gross_profit"], pl["gross_profit"])
row("Operating Expenses", t["total_expenses"], pl["total_operating_expenses"])
# source "Other Expenses" vs Zavora "other" section
zav_other = -float(pl.get("net_profit", 0)) + (float(pl["gross_profit"]) - float(pl["total_operating_expenses"]))
row("Net Income", t["net_income"], pl["net_profit"])

print("\n=== BALANCE SHEET  (Zavora vs the source accounting system) ===")
b = targets["balance_sheet"]
row("Total Assets", b["total_assets"], bs["total_assets"])
row("Total Liabilities", b["total_liabilities"], bs["total_liabilities"])
# Zavora folds current-year earnings into equity separately
zeq = float(bs["total_equity"]) + float(bs.get("current_year_earnings", 0))
row("Total Equity", b["total_equity"], zeq)
print(f"  (Zavora equity {float(bs['total_equity']):,.2f} + current-year earnings "
      f"{float(bs.get('current_year_earnings',0)):,.2f})")
print(f"  Zavora balanced={bs['is_balanced']}  diff {float(bs['difference']):,.2f}")
