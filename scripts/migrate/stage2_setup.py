#!/usr/bin/env python3
"""Stage 2: create a fresh Zavora tenant, build a chart of accounts mirroring
the source chart of accounts, set posting config (AR/AP), and load customers/vendors/products.

Writes sample_data/migration/zavora_maps.json with the name->code/id maps that
Stage 3 (transaction replay) consumes.
"""
import json, time, urllib.request, urllib.error, sys

BASE = "http://localhost:8080/api/v1"
QB = "sample_data/migration"

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

# source account type -> (Zavora account_type, code range base). Ranges chosen so
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
    email = f"sample{int(time.time())}@demo.co"
    st, d = call("POST", "/auth/signup", body={
        "organization_name": "Sample Company (Zavora)",
        "organization_type": "private_limited", "kra_pin": "P051234567X",
        "email": email, "display_name": "Sample Owner", "password": "Passw0rd!23"})
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
        if qtype not in source_TO_Z:
            print("  ! unmapped source type:", qtype); continue
        ztype, base = source_TO_Z[qtype]
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

    # ---- point posting config at the source accounts (not just AR/AP) ----
    # Leaving default_bank / default_sales / default_purchase / rounding on the
    # Kenya-seed codes makes fresh postings land in different accounts than the
    # imported source history, fragmenting reports. Remap every default we can to a
    # real source account so new transactions post consistently with replayed data.
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
    # source product types: "Service", "Inventory", "NonInventory". Zavora's
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
        is_inventory = qtype == "inventory"
        ztype = "Goods" if qtype in ("inventory", "noninventory", "non-inventory") else "Service"
        body = {
            "name": p["name"],
            "description": p.get("description") or None,
            "product_type": ztype,
            "vat_treatment": "ZeroRated",  # source sample has no VAT; matches invoice replay
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
                "type": qtype,
                "is_inventory": is_inventory,
                "sku": p.get("sku") or "",
                "description": p.get("description") or "",
                "cogs_account_name": (p.get("cogs_account") or "").split(":")[-1].strip(),
            }
        else:
            # keep the income mapping even if creation failed, so the replay can
            # still resolve the revenue account by name.
            prod_map[p["name"]] = {"income_account_name": leaf, "income_code": income_code,
                                   "type": qtype, "is_inventory": is_inventory}
            if created_products + 1 <= 5:
                print(f"  PRODUCT FAIL {p['name']}: {st} {str(r)[:120]}")
    print("products created:", created_products)

    # ---- inventory items for Inventory-type products ----
    # the source system's export carries no per-item opening quantities. We reconstruct the
    # inventory subledger so that it ties to the GL Inventory Asset to the cent
    # and reproduces source COGS exactly:
    #
    #   GL Inventory Asset = START opening (567.50) + Check 75 (228.75)
    #                        + Bill (205.00) − invoice COGS (405.00) = 596.25
    #
    # For the subledger to end at 596.25 with issues (COGS) of 405.00, opening
    # receipts must total 1001.25. Each *sold* item is seeded at its per-unit
    # COGS cost (Rock Fountain 125, Pump 10, Sprinkler Pipes 10) so the WAC
    # engine books COGS at exactly the source cost; the never-sold "Sprinkler Heads"
    # carries the residual opening value so the subledger total ties to the GL.
    # Per-item opening *quantities* are synthetic (source didn't export them); the
    # inventory *value* and total COGS are exact. (documented constraint)
    # Per-item opening (quantity, unit_cost). unit_cost is set to the item's true
    # per-unit COGS so the WAC engine books COGS on each sale at exactly the source
    # cost. Units sold via "Sales of Product Income" across the whole dataset:
    # Rock Fountain 3, Pump 2, Sprinkler Pipes 2, Sprinkler Heads 1 — so seed at
    # least that many. The opening *value* must total 1001.25 so the inventory
    # subledger ties to the GL Inventory Asset (START 567.50 + Check 228.75 +
    # Bill 205.00 − COGS 405.00 = 596.25 ending; opening 1001.25 − 405 = 596.25).
    # Per-unit COGS by item (from source COGS lines): RF 125, Pump 10, Pipes 5
    # (2 issues totalling the source Pipes COGS of 10), Heads 0 (sold at zero cost).
    # the source system's fractional cost layers make a clean whole-unit split impossible, so
    # Rock Fountain carries a fractional opening qty (units are synthetic — source
    # exported values, not quantities; documented constraint).
    OPENING = {  # name -> (quantity, unit_cost)
        "Rock Fountain":  (7.37, 125.0),  # 921.25
        "Pump":           (6, 10.0),      #  60.00
        "Sprinkler Pipes":(4, 5.0),       #  20.00
        "Sprinkler Heads":(2, 0.0),       #   0.00 (sold at zero cost in source)
    }                                      # total opening value = 1001.25 = GL inv debits
    inv_code = code_for("Asset", "Inventory Asset") or "1201"
    cogs_code = code_for("Expense", "Cost of Goods Sold") or "6001"
    inv_items = {}  # product name -> inventory_item_id
    created_items = 0
    for name, info in prod_map.items():
        if not info.get("is_inventory") or not info.get("id"):
            continue
        sku = info.get("sku") or name.replace(" ", "-")[:20]
        st, r = call("POST", "/inventory", token, {
            "sku": sku,
            "description": name,
            "costing_method": "WeightedAvgCost",
            "gl_inventory": inv_code,
            "gl_cogs": cogs_code,
            "product_id": info["id"],
        })
        if st < 300 and r and r.get("id"):
            item_id = r["id"]
            inv_items[name] = item_id
            info["inventory_item_id"] = item_id
            created_items += 1
            qty, uc = OPENING.get(name, (1, 10.0))
            # Seed opening stock (subledger only; the matching GL value comes from
            # the replayed START/purchase journals in stage 3).
            call("POST", "/inventory/receive", token, {
                "item_id": item_id, "quantity": qty, "unit_cost": uc,
                "date": "2026-05-28",
            })
        else:
            print(f"  INV ITEM FAIL {name}: {st} {str(r)[:120]}")
    print("inventory items created:", created_items)

    maps = {"token_email": email, "token": token, "ar_code": ar_code, "ap_code": ap_code,
            "accounts": acct_map, "customers": cust_map, "vendors": vend_map,
            "products": prod_map, "inventory_items": inv_items,
            "inventory_asset_code": inv_code, "cogs_code": cogs_code}
    json.dump(maps, open(f"{QB}/zavora_maps.json", "w"), indent=1)
    print("saved zavora_maps.json")

    # ---- link products.inventory_item_id (no API for this; set via SQL) ----
    # invoice posting reads products.inventory_item_id to decide whether to issue
    # stock + book COGS. Nothing in the API sets it, so we set it directly.
    link_pairs = [(info["id"], info["inventory_item_id"])
                  for info in prod_map.values()
                  if info.get("is_inventory") and info.get("id") and info.get("inventory_item_id")]
    if link_pairs:
        sql_parts = ["BEGIN;"]
        for pid, iid in link_pairs:
            sql_parts.append(
                f"UPDATE products SET track_inventory=true, inventory_item_id='{iid}' WHERE id='{pid}';")
        sql_parts.append("COMMIT;")
        sql = "\n".join(sql_parts)
        linked = False
        import subprocess
        # Prefer host psql; fall back to the dockerised postgres container.
        for cmd in (
            ["psql", "-h", "localhost", "-p", "5433", "-U", "zavora", "-d", "zavora_era"],
            ["docker", "exec", "-i", "-e", "PGPASSWORD=zavora", "zavora-postgres",
             "psql", "-U", "zavora", "-d", "zavora_era"],
        ):
            try:
                env = {"PGPASSWORD": "zavora", "PATH": "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin"}
                subprocess.run(cmd, input=sql, text=True, check=True, env=env)
                print(f"linked {len(link_pairs)} products to inventory items (track_inventory=true)")
                linked = True
                break
            except Exception:
                continue
        if not linked:
            print("  WARN: could not link products.inventory_item_id automatically. Run manually:\n" + sql)

if __name__ == "__main__":
    main()
