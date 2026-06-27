#!/usr/bin/env python3
"""Stage 3b: replay the QBO transactions into the Zavora tenant via real flows.

- Invoices -> POST /invoices (+post)        [income lines, zero-rated]
- Bills    -> POST /bills (+approve+post)    [expense lines, zero-rated]
- Customer Payment / Bill Payment -> POST /payments (FIFO apply to open docs)
- Everything else -> one balanced journal per transaction (P&L + BS lines)
Control accounts (A/R 1101 / A/P 2001) only move via the real flows above; any
stray journal that would touch them is skipped and logged.

Run AFTER stage2_setup.py (uses its saved token). Sales tax neutralized.
"""
import json, sys, urllib.request, urllib.error
from collections import defaultdict
sys.path.insert(0, "scripts/qbo")

BASE = "http://localhost:8080/api/v1"
QB = "sample_data/quickbooks"
AR_NAME = "Accounts Receivable (A/R)"
AP_NAME = "Accounts Payable (A/P)"

def call(method, path, token, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(BASE + path, data=data, method=method)
    req.add_header("Content-Type", "application/json")
    req.add_header("Authorization", "Bearer " + token)
    try:
        with urllib.request.urlopen(req) as r:
            raw = r.read(); return r.status, (json.loads(raw) if raw else None)
    except urllib.error.HTTPError as e:
        raw = e.read()
        try: return e.code, (json.loads(raw) if raw else None)
        except Exception: return e.code, {"error": raw.decode()[:200]}

maps = json.load(open(f"{QB}/zavora_maps.json"))
txns = json.load(open(f"{QB}/transactions.json"))
ACC, CUST, VEND = maps["accounts"], maps["customers"], maps["vendors"]
token = maps["token"]
AR_CODE = ACC[f"Asset||{AR_NAME}"]["code"]
AP_CODE = ACC[f"Liability||{AP_NAME}"]["code"]

def code_for(zt, name):
    x = ACC.get(f"{zt}||{name}"); return x["code"] if x else None

def resolve_party(table, name, kind):
    for cand in (name, name.split(":")[-1].strip(), name.split(":")[0].strip()):
        if cand in table: return table[cand]
    body = {"name": name, "email": [], "phone": []} if kind == "customer" else \
           {"name": name, "resident": True, "payment_terms": "Net30", "email": [], "phone": []}
    st, r = call("POST", f"/{kind}s", token, body)
    if st < 300 and r and r.get("id"): table[name] = r["id"]; return r["id"]
    return None

def to_dr_cr(zt, amt):
    if zt in ("Asset", "Expense"): return (amt, 0.0) if amt >= 0 else (0.0, -amt)
    return (0.0, amt) if amt >= 0 else (-amt, 0.0)

def jline(code, d, c, desc=""):
    return {"account_code": code, "debit": (f"{d:.2f}" if d else None),
            "credit": (f"{c:.2f}" if c else None), "currency": "KES", "fx_rate": "1",
            "description": desc[:120]}

def iso(d):
    m, dd, y = d.split("/"); return f"{y}-{m.zfill(2)}-{dd.zfill(2)}"

# cash GL accounts that payments may land in -> we create bank_accounts for them
CASH_ACCOUNTS = {  # QBO name : (ztype, ledger label)
    "Checking": "Asset", "Savings": "Asset", "Undeposited Funds": "Asset",
    "Mastercard": "Liability", "Visa": "Liability",
}

_seq = [0]
def post_journal(date, ref, desc, lines, log):
    lines = [l for l in lines if (l["debit"] or l["credit"])]
    if any(l["account_code"] in (AR_CODE, AP_CODE) for l in lines):
        log["skip_control_journal"] += 1; return
    if len(lines) < 2: return
    _seq[0] += 1
    ref = f"{ref} #{_seq[0]}"  # Zavora enforces unique journal references
    td = sum(float(l["debit"] or 0) for l in lines)
    tc = sum(float(l["credit"] or 0) for l in lines)
    diff = round(td - tc, 2)
    if abs(diff) > 0.005:
        plug = code_for("Equity", "Opening Balance Equity")
        lines.append(jline(plug, 0, diff) if diff > 0 else jline(plug, -diff, 0))
        log["plugged"] += 1
    st, r = call("POST", "/journal-entries", token,
                 {"date": date, "source": "Manual", "reference": ref[:60],
                  "description": desc[:120], "post_immediately": True, "lines": lines})
    if st >= 300:
        log["journal_fail"] += 1
        if log["journal_fail"] <= 10: print(f"  JRNL FAIL {ref} {st}: {str(r)[:150]}")
    else:
        log["journal_ok"] += 1

def main():
    log = defaultdict(int); unresolved = set()
    open_inv = defaultdict(list)   # customer_id -> [ {id,bal} ]
    open_bill = defaultdict(list)  # vendor_id  -> [ {id,bal} ]
    bank_ids = {}                  # qbo cash account name -> bank_account id

    # periods
    for y in range(2014, 2027):
        call("POST", "/periods", token, {"fiscal_year": y, "year_start_month": 1})

    # bank accounts for cash/CC GLs
    for name, zt in CASH_ACCOUNTS.items():
        code = code_for(zt, name)
        if not code: continue
        st, r = call("POST", "/bank-accounts", token,
                     {"name": name, "bank_name": name, "account_number": name.replace(" ", ""),
                      "gl_account": code})
        if st < 300 and r and r.get("id"): bank_ids[name] = r["id"]
    print("bank accounts:", list(bank_ids))

    def cash_line_of(t):
        """return (qbo cash account name, amount, ztype) for a payment's cash side."""
        for l in t["bs_lines"]:
            if l["account"] in CASH_ACCOUNTS:
                return l["account"], l["amount"], CASH_ACCOUNTS[l["account"]]
        return None, 0, None

    def apply_fifo(docs, amount):
        apps, rem = [], round(amount, 2)
        for d in docs:
            if rem <= 0.005: break
            a = round(min(d["bal"], rem), 2)
            if a > 0.005:
                apps.append({"document_id": d["id"], "amount": a}); d["bal"] -= a; rem -= a
        return apps

    # process in date order; invoices/bills before payments on the same day
    prio = {"Invoice": 0, "Bill": 0, "Sales Receipt": 1, "Credit Memo": 2}
    txns.sort(key=lambda t: (iso(t["date"]), prio.get(t["txn_type"], 5)))

    for t in txns:
        typ, date, name = t["txn_type"], iso(t["date"]), t["name"]
        ref = f"{typ} {t['num']}".strip()

        if typ == "Invoice":
            cid = resolve_party(CUST, name, "customer")
            if not cid: log["cust_unresolved"] += 1; continue
            lines, total = [], 0.0
            for l in t["pnl_lines"]:
                code = code_for("Revenue", l["account"])
                if not code: unresolved.add(("Revenue", l["account"])); continue
                lines.append({"description": (l["description"] or l["account"]), "quantity": 1,
                              "unit_price": l["amount"], "account_code": code, "vat_treatment": "ZeroRated"})
                total += l["amount"]
            if not lines: continue
            st, r = call("POST", "/invoices", token, {"customer_id": cid, "issue_date": date, "lines": lines})
            if st >= 300 or not (r and r.get("id")):
                log["invoice_fail"] += 1
                if log["invoice_fail"] <= 8: print(f"  INV FAIL {ref} {st}: {str(r)[:150]}")
                continue
            call("POST", f"/invoices/{r['id']}/post", token)
            log["invoice_ok"] += 1
            if total > 0.005: open_inv[cid].append({"id": r["id"], "bal": round(total, 2)})

        elif typ == "Bill":
            vid = resolve_party(VEND, name, "vendor")
            if not vid: log["vend_unresolved"] += 1; continue
            lines, total = [], 0.0
            for l in t["pnl_lines"]:
                code = code_for("Expense", l["account"])
                if not code: unresolved.add(("Expense", l["account"])); continue
                lines.append({"description": (l["description"] or l["account"]), "quantity": 1,
                              "unit_price": l["amount"], "account_code": code, "vat_treatment": "ZeroRated"})
                total += l["amount"]
            if not lines: continue
            st, r = call("POST", "/bills", token, {"vendor_id": vid, "issue_date": date, "due_date": date, "lines": lines})
            if st >= 300 or not (r and r.get("id")):
                log["bill_fail"] += 1
                if log["bill_fail"] <= 8: print(f"  BILL FAIL {ref} {st}: {str(r)[:150]}")
                continue
            call("POST", f"/bills/{r['id']}/approve", token)
            call("POST", f"/bills/{r['id']}/post", token)
            log["bill_ok"] += 1
            if total > 0.005: open_bill[vid].append({"id": r["id"], "bal": round(total, 2)})

        elif typ == "Payment":
            cid = resolve_party(CUST, name, "customer")
            cash_name, cash_amt, _ = cash_line_of(t)
            amount = abs(cash_amt) if cash_amt else sum(-l["amount"] for l in t["bs_lines"] if l["account"] == AR_NAME)
            if not cid or amount <= 0.005: log["payment_skip"] += 1; continue
            apps = apply_fifo(open_inv[cid], amount)
            st, r = call("POST", "/payments", token, {
                "payment_type": "customer_payment", "party_id": cid, "payment_date": date,
                "amount": f"{amount:.2f}", "method": {"BankTransfer": {"reference": ref}},
                "bank_account_id": bank_ids.get(cash_name or "Undeposited Funds"),
                "applications": apps})
            if st >= 300: log["payment_fail"] += 1;
            else: log["payment_ok"] += 1
            if st >= 300 and log["payment_fail"] <= 8: print(f"  PAY FAIL {ref} {st}: {str(r)[:150]}")

        elif typ.startswith("Bill Payment"):
            vid = resolve_party(VEND, name, "vendor")
            cash_name, cash_amt, _ = cash_line_of(t)
            amount = abs(cash_amt) if cash_amt else sum(l["amount"] for l in t["bs_lines"] if l["account"] == AP_NAME)
            if not vid or amount <= 0.005: log["billpay_skip"] += 1; continue
            apps = apply_fifo(open_bill[vid], amount)
            st, r = call("POST", "/payments", token, {
                "payment_type": "vendor_payment", "party_id": vid, "payment_date": date,
                "amount": f"{amount:.2f}", "method": {"BankTransfer": {"reference": ref}},
                "bank_account_id": bank_ids.get(cash_name or "Checking"),
                "applications": apps})
            if st >= 300: log["billpay_fail"] += 1
            else: log["billpay_ok"] += 1
            if st >= 300 and log["billpay_fail"] <= 8: print(f"  BILLPAY FAIL {ref} {st}: {str(r)[:150]}")

        else:
            jl = []
            for l in t["pnl_lines"] + t["bs_lines"]:
                code = code_for(l["ztype"], l["account"])
                if not code: unresolved.add((l["ztype"], l["account"])); continue
                d_, c_ = to_dr_cr(l["ztype"], l["amount"])
                jl.append(jline(code, d_, c_, l["description"] or l["account"]))
            post_journal(date, ref, f"{typ} {name}", jl, log)

    print("\n=== LOAD REPORT ===")
    for k in sorted(log): print(f"  {k}: {log[k]}")
    if unresolved:
        print("\n  UNRESOLVED ACCOUNTS:")
        for zt, n in sorted(unresolved): print(f"    {zt} :: {n}")

if __name__ == "__main__":
    main()
