#!/usr/bin/env python3
"""Stage 2: create a fresh Zavora tenant, build a chart of accounts mirroring
Craig's QBO chart, set posting config (AR/AP), and load customers/vendors/products.

Writes sample_data/quickbooks/zavora_maps.json with the name->code/id maps that
Stage 3 (transaction replay) consumes.
"""
import json, time, urllib.request, urllib.error, sys

BASE = "http://localhost:8080/api/v1"
QB = "sample_data/quickbooks"

def call(method, path, token=None, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(BASE + path, data=data, method=method)
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", "Bearer " + token)
    try:
        with urllib.request.urlopen(req) as r:
            raw = r.read()
            return r.status, (json.loads(raw) if raw else None)
    except urllib.error.HTTPError as e:
        raw = e.read()
        return e.code, (json.loads(raw) if raw else None)

# QBO account type -> (Zavora account_type, code range base). Ranges chosen so
# Zavora's P&L buckets correctly: 6xxx=COGS, 7xxx=opex, 8xxx=other.
QBO_TO_Z = {
    "Bank": ("Asset", 1000), "Accounts receivable (A/R)": ("Asset", 1100),
    "Other Current Assets": ("Asset", 1200), "Fixed Assets": ("Asset", 1500),
    "Accounts payable (A/P)": ("Liability", 2000), "Credit Card": ("Liability", 2100),
    "Other Current Liabilities": ("Liability", 2200), "Long Term Liabilities": ("Liability", 2500),
    "Equity": ("Equity", 3000),
    "Income": ("Revenue", 4000), "Other Income": ("Revenue", 4500),
    "Cost of Goods Sold": ("Expense", 6000), "Expenses": ("Expense", 7000),
    "Other Expense": ("Expense", 8000),
}

def main():
    coa = json.load(open(f"{QB}/chart_of_accounts.json"))
    customers = json.load(open(f"{QB}/customers.json"))
    vendors = json.load(open(f"{QB}/vendors.json"))
    products = json.load(open(f"{QB}/products_services.json"))

    # ---- signup ----
    email = f"craig{int(time.time())}@demo.co"
    st, d = call("POST", "/auth/signup", body={
        "organization_name": "Craig's Design and Landscaping (Zavora)",
        "organization_type": "private_limited", "kra_pin": "P051234567X",
        "email": email, "display_name": "Craig", "password": "Passw0rd!23"})
    token = (d or {}).get("access_token") or (d or {}).get("tokens", {}).get("access_token")
    assert token, f"signup failed: {st} {d}"
    print("tenant:", email)

    # ---- existing (seeded) codes to avoid colliding with ----
    st, existing = call("GET", "/accounts", token)
    used = set(a["code"] for a in (existing or []))
    print("seeded accounts:", len(used))

    def next_free(base):
        # first free code at/after base, within the base's 1000-block
        for c in range(base, base + 1000):
            cs = str(c)
            if cs not in used:
                used.add(cs)
                return cs
        raise RuntimeError(f"no free code near {base}")

    # ---- build account code map (dedupe by (ztype, name)), skipping taken codes ----
    acct_map = {}          # "ztype||name" -> {code,name,ztype,qbo_type}
    for a in coa:
        qtype = a["type"]
        if qtype not in QBO_TO_Z:
            print("  ! unmapped QBO type:", qtype); continue
        ztype, base = QBO_TO_Z[qtype]
        key = f"{ztype}||{a['name']}"
        if key in acct_map:
            continue  # duplicate leaf within same type -> merge
        acct_map[key] = {"code": next_free(base), "name": a["name"], "ztype": ztype, "qbo_type": qtype}

    # ---- create accounts ----
    created = 0
    for key, info in acct_map.items():
        is_ctrl = info["qbo_type"] in ("Accounts receivable (A/R)", "Accounts payable (A/P)")
        st, r = call("POST", "/accounts", token, {
            "code": info["code"], "name": info["name"], "account_type": info["ztype"],
            "is_control": is_ctrl, "tags": []})
        if st < 300:
            created += 1
        elif st != 409:
            print(f"  acct {info['code']} {info['name']} -> {st} {r}")
    print(f"accounts created: {created}/{len(acct_map)}")

    def code_for(ztype, name):
        x = acct_map.get(f"{ztype}||{name}")
        return x["code"] if x else None

    ar_code = code_for("Asset", "Accounts Receivable (A/R)")
    ap_code = code_for("Liability", "Accounts Payable (A/P)")
    print("AR code:", ar_code, "AP code:", ap_code)

    # ---- point posting config at the QBO accounts (not just AR/AP) ----
    # Leaving default_bank / default_sales / default_purchase / rounding on the
    # Kenya-seed codes makes fresh postings land in different accounts than the
    # imported QBO history, fragmenting reports. Remap every default we can to a
    # real QBO account so new transactions post consistently with replayed data.
    st, cfg = call("GET", "/settings", token)
    posting = (cfg or {}).get("posting", {})
    posting["accounts_receivable"] = ar_code
    posting["accounts_payable"] = ap_code

    def first_code(ztype, *names):
        for n in names:
            c = code_for(ztype, n)
            if c:
                return c
        return None

    # Operating bank for payments recorded without an explicit bank account.
    bank = first_code("Asset", "Checking", "Savings")
    if bank:
        posting["default_bank"] = bank
    # Fallback income / purchase / COGS accounts.
    sales = first_code("Revenue", "Sales of Product Income", "Services", "Landscaping Services")
    if sales:
        posting["default_sales"] = sales
    cogs = first_code("Expense", "Cost of Goods Sold")
    if cogs:
        posting["default_purchase"] = cogs
    # Dedicated rounding account if the chart has one.
    rounding = first_code("Expense", "Rounding Differences", "Miscellaneous")
    if rounding:
        posting["rounding_adjustment"] = rounding

    st, _ = call("PUT", "/settings", token, {"posting": posting})
    print("posting config patched:", st)

    # ---- customers ----
    cust_map = {}
    for c in customers:
        st, r = call("POST", "/customers", token, {"name": c["name"], "email": [], "phone": []})
        if st < 300 and r and r.get("id"):
            cust_map[c["name"]] = r["id"]
    print("customers created:", len(cust_map))

    # ---- vendors ----
    vend_map = {}
    for v in vendors:
        st, r = call("POST", "/vendors", token, {"name": v["name"], "resident": True,
                     "payment_terms": "Net30", "email": [], "phone": []})
        if st < 300 and r and r.get("id"):
            vend_map[v["name"]] = r["id"]
    print("vendors created:", len(vend_map))

    # ---- products (create them, and map item name -> {id, income code}) ----
    # QBO product types: "Service", "Inventory", "NonInventory". Zavora's
    # ProductType is Service | Goods | Expense (serialised as-is). Inventory and
    # NonInventory both map to Goods; everything else to Service.
    prod_map = {}
    created_products = 0
    for p in products:
        inc = p.get("income_account", "")
        # income account names are sometimes paths "A:B:C" -> use the leaf.
        leaf = inc.split(":")[-1].strip() if inc else ""
        income_code = code_for("Revenue", leaf)
        qtype = (p.get("type") or "").strip().lower()
        ztype = "Goods" if qtype in ("inventory", "noninventory", "non-inventory") else "Service"
        body = {
            "name": p["name"],
            "description": p.get("description") or None,
            "product_type": ztype,
            "vat_treatment": "ZeroRated",  # QBO sample has no VAT; matches invoice replay
        }
        if income_code:
            body["sales_account"] = income_code
        st, r = call("POST", "/products", token, body)
        if st < 300 and r and r.get("id"):
            created_products += 1
            prod_map[p["name"]] = {
                "id": r["id"],
                "income_account_name": leaf,
                "income_code": income_code,
            }
        else:
            # keep the income mapping even if creation failed, so the replay can
            # still resolve the revenue account by name.
            prod_map[p["name"]] = {"income_account_name": leaf, "income_code": income_code}
            if created_products + 1 <= 5:
                print(f"  PRODUCT FAIL {p['name']}: {st} {str(r)[:120]}")
    print("products created:", created_products)

    maps = {"token_email": email, "token": token, "ar_code": ar_code, "ap_code": ap_code,
            "accounts": acct_map, "customers": cust_map, "vendors": vend_map,
            "products": prod_map}
    json.dump(maps, open(f"{QB}/zavora_maps.json", "w"), indent=1)
    print("saved zavora_maps.json")

if __name__ == "__main__":
    main()
