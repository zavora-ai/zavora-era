# Manufacturing Roadmap — v2 and beyond

**Status of v1 (shipped 2026-07-11, PR #97).** Light manufacturing on top of the
existing inventory (WAC) + warehousing + double-entry engine:

- **Bills of materials** — a recipe of component items (+ optional labour/overhead
  per batch) for a finished-good product (`boms`, `bom_lines`; migration `061`).
- **Work orders**, two-step lifecycle:
  - **start** → issues the scaled components into **Work in Progress**:
    `DR 1510 WIP / CR component inventory` (at weighted-average cost).
  - **complete** → receives finished goods out of WIP at their rolled-up unit
    cost: `DR finished-good inventory / CR 1510 WIP / CR 6300 Manufacturing
    Overhead`. WIP nets to zero on completion.
- Reuses `inventory::{issue,receive}_inventory_in_tx`, so the warehouse ledger
  stays consistent (`SUM(warehouse_stock) == on_hand`) and WAC is recomputed
  correctly. Code: `zavora-erp-core/src/services/manufacturing.rs`,
  `zavora-erp-api/src/routes/manufacturing.rs`, UI
  `zavora-erp-ui/src/pages/inventory/ManufacturingPage.tsx`.

This document tracks what v1 deliberately **left out**, why, and the shape each
piece would take so it slots cleanly onto the v1 foundation without breaking the
invariants above.

> **Design principles carried from v1 (do not regress):**
> 1. The **ledger is the source of truth** — every stock/cost movement posts a
>    balanced journal entry through `create_and_post_in_tx`.
> 2. The **warehouse invariant** `SUM(warehouse_stock.quantity) == inventory_items.on_hand`
>    must hold after every operation (use `warehousing::apply_stock_delta_tx`).
> 3. **WIP is a real, reconciling account** — anything that enters WIP must leave
>    it (to finished goods, scrap, or a variance account); WIP should not
>    silently accumulate.
> 4. **Additive, non-breaking migrations** with backfills (see
>    `docs/BACKUP_RUNBOOK.md` §5).
> 5. Costing stays **weighted-average by default**; FIFO is opt-in per item.

Legend: **P1** high-value / commonly requested · **P2** fast-follow · **P3** advanced.

---

## 1. Routing, operations & work centres — **P1**

**What.** Break a work order into ordered **operations** (cut → assemble →
finish), each performed at a **work centre** (a machine, cell, or team) with a
standard time and cost rate. Today a work order is a single black-box step with
one flat overhead figure.

**Why deferred.** v1's goal was correct material + overhead costing for
assemble-to-stock SMEs; multi-operation routing is only needed once shops want
per-stage tracking, WIP-by-operation, and labour absorption by work centre.

**Data model (additive).**
- `work_centres` (id, entity_id, code, name, cost_rate_per_hour, overhead_rate,
  default_labour_account, default_overhead_account, is_active).
- `routings` + `routing_operations` (bom_id or product_id → ordered operations:
  seq, work_centre_id, description, standard_minutes, setup_minutes).
- `work_order_operations` (work_order_id, seq, work_centre_id, status
  [pending|in_progress|done], actual_minutes, labour_cost, overhead_cost,
  started_at, completed_at).

**Accounting.** Each operation absorbs labour + overhead into WIP as it
completes: `DR 1510 WIP / CR labour applied (e.g. 6310) / CR 6300 overhead
applied`, using the work centre's rates × actual (or standard) minutes. The v1
flat `overhead_cost` becomes the sum of per-operation applied overhead. WIP still
nets to zero at completion.

**API/UI.** `/work-centres` CRUD; routing editor on the BOM; a work-order
"operations" panel to start/finish each step and capture actual time; a
per-operation cost breakdown.

**Builds on v1:** the WIP posting pattern generalises — v1's single start→WIP is
operation 0; v2 adds N operations each crediting labour/overhead into WIP.

---

## 2. Capacity planning & scheduling — **P2**

**What.** A finite/infinite-capacity schedule: given open work orders and work
centre calendars, compute start/finish dates, load per work centre, and flag
overloads. A Gantt/board view of the shop floor.

**Why deferred.** Requires routing + work centres (§1) as a prerequisite, and is
planning (decision-support) rather than ledger correctness.

**Data model.** `work_centre_calendars` (available hours/shifts per day);
scheduling is mostly computed — persist a `work_order_operations.scheduled_start/
scheduled_end` and a materialised load view.

**Accounting.** None — scheduling posts nothing to the ledger. Keep it strictly
advisory so a wrong schedule can never corrupt the books.

**API/UI.** `POST /manufacturing/schedule` (recompute); a capacity board +
per-work-centre load chart. Consider running the recompute in the hourly
scheduler tick (like reminders/depreciation) rather than synchronously.

---

## 3. MRP — material requirements planning — **P1/P2**

**What.** From demand (sales orders / forecasts / reorder points) explode BOMs to
compute **net requirements** per component and suggest **planned work orders** and
**planned purchase orders** (respecting on-hand, on-order, lead times, and safety
stock).

**Why deferred.** High value for make-to-order/merchandising shops but a
substantial planning engine; v1 focused on execution + costing. Also depends on
multi-level BOM explosion (§5) to be fully correct.

**Data model.** `mrp_runs` (snapshot + parameters); `mrp_suggestions`
(item_id, required_qty, required_date, source [demand|reorder], action
[make|buy], suggested_order_ref). Reuse existing `products.reorder_point/
reorder_quantity`, procurement (POs), and sales/estimates as demand.

**Accounting.** None until a suggestion is **converted** into a real work order or
PO (which then post through the existing paths).

**API/UI.** `POST /mrp/run`, `GET /mrp/suggestions`, convert-to-WO / convert-to-PO
actions; a planning worksheet grouped by item with the netting logic shown.

**Builds on v1 + procurement:** conversions land on existing `create_work_order`
and the procurement `create_direct_po` paths — MRP only *suggests*.

---

## 4. Scrap & yield variance — **P1**

**What.** Capture that a run consumed more material than standard (scrap) or
produced fewer good units than expected (yield loss), and post the difference to
a **variance** account instead of burying it in unit cost.

**Why deferred.** v1 assumes the BOM quantities are actual; real production has
waste. Doing this right needs a standard-vs-actual comparison at completion.

**Data model.** `work_orders` gains `expected_quantity` vs `good_quantity` +
`scrapped_quantity`; `work_order_consumptions` already holds actual consumption,
so material variance = actual − (standard × good_quantity).

**Accounting.** At completion:
- Good units → finished goods at standard/rolled cost (v1 behaviour).
- **Material usage variance** and **yield variance** → a dedicated P&L account
  (e.g. `6320 Production Variances`, new): `DR 6320 / CR 1510 WIP` for
  unfavourable, reversed for favourable — so WIP still clears to zero and the
  waste is visible in the P&L rather than inflating inventory cost.
- Optional scrap with salvage value → `DR scrap inventory / CR WIP`.

**API/UI.** Completion form captures good vs scrapped quantities; the work-order
detail shows a standard-vs-actual variance breakdown.

**Guard the invariant:** the sum of finished-goods value + variance + scrap must
equal the WIP balance so 1510 nets to zero every time.

---

## 5. Multi-level BOM explosion — **P2**

**What.** A finished good whose components are themselves manufactured
sub-assemblies (BOM within a BOM). Producing the top level should explode through
all levels, and cost should roll up bottom-up.

**Why deferred.** v1 enforces one BOM per product and treats every component as a
purchased/stock item (single level). Multi-level needs recursive explosion +
cycle detection + level-by-level costing.

**Data model.** No new tables — a `bom_lines.component_item_id` can already point
at an item that *has its own BOM*. Add: recursive resolution in the service, a
`low_level_code` per item (standard MRP technique) to order cost roll-up, and a
**cycle guard** (A→B→A must be rejected at BOM save).

**Accounting.** Two options, make it a setting:
1. **Explode & auto-produce** — one top-level work order spawns child work orders
   for sub-assemblies (each posts its own WIP cycle). Cleanest audit trail.
2. **Phantom/flatten** — sub-assemblies marked "phantom" are exploded to their raw
   components in a single work order (no intermediate stock/JE). Fewer postings.

**API/UI.** BOM editor shows the exploded tree; work-order create shows the full
component requirement across levels; costing shows the roll-up per level.

**Prerequisite for:** accurate MRP (§3).

---

## 6. Subcontracting (outwork) — **P2**

**What.** Send components to a third party who performs an operation (e.g.
plating, stitching) and returns them, billing a service fee. The materials are
still yours while out; the fee is a production cost.

**Why deferred.** Cross-cuts procurement (a subcontract PO), inventory (stock at
a vendor location), and manufacturing (an operation done off-site).

**Data model.** A **3PL-style vendor "warehouse"** already exists from the
warehousing feature (#92, `kind='3pl'`) — reuse it to hold "stock at
subcontractor". Add `subcontract_orders` linking a work-order operation to a
vendor + a PO for the service fee.

**Accounting.**
- Transfer components to the subcontractor location (a warehouse transfer — no
  P&L impact, invariant preserved).
- The subcontractor's fee arrives as a **bill** (existing AP path) coded to WIP:
  `DR 1510 WIP / CR AP`, so the outwork cost lands in the finished-good cost.
- Returned goods re-enter via the normal receipt into WIP/finished goods.

**API/UI.** A "subcontract" operation type on the routing; issue-to-vendor +
receive-from-vendor actions; link the service bill to the work order.

**Builds on:** warehousing 3PL locations + procurement bills + routing (§1).

---

## 7. Per-warehouse costing & FIFO layers — **P2/P3**

**What.** Two related upgrades to costing:
- **Per-warehouse / per-location valuation** — today WAC is a single per-item
  cost; some operations want cost tracked per warehouse.
- **FIFO cost layers** — issue at the cost of the oldest receipt rather than the
  weighted average. The model already hints at FIFO
  (`inventory/mod.rs CostingMethod::Fifo`) but only WAC is implemented.

**Why deferred.** WAC is correct and sufficient for most SMEs, is simpler, and
avoids layer bookkeeping. FIFO matters for perishables, high-value serialised
goods, or where regulation/audit prefers it. This is also listed in
`REMAINING.md` §3 (Inventory FIFO) as a standing follow-up.

**Data model.** `inventory_layers` (item_id, warehouse_id, receipt_date, qty
_remaining, unit_cost, source_movement_id). Receipts create a layer; issues
consume layers oldest-first and compute COGS/consumption cost from the layers
consumed. `inventory_items.costing_method` selects WAC vs FIFO per item.

**Accounting.** Same journal shapes — only the **unit cost** of an issue changes
(FIFO layer cost vs WAC). Manufacturing consumption (`issue_inventory_in_tx`) and
COGS at sale both read the chosen method. WIP/finished-goods postings are
unaffected in structure.

**API/UI.** Item setting for costing method; a layer/valuation report; ensure the
`InventoryValuation` report reflects layers when FIFO is on.

**Care:** this touches the hot path (every issue/sale). Gate behind the per-item
`costing_method`, keep WAC the default, and add golden-value tests comparing WAC
vs FIFO on the same movement history before enabling.

---

## Suggested sequence

1. **Routing / operations / work centres (§1)** — unlocks per-stage costing and
   is the prerequisite for scheduling and richer subcontracting.
2. **Scrap & yield variance (§4)** — small, high-value correctness win; makes the
   P&L honest about waste.
3. **Multi-level BOM explosion (§5)** — needed for real manufacturers and for MRP.
4. **MRP (§3)** — the planning payoff, once explosion (§5) is in.
5. **Subcontracting (§6)** and **capacity scheduling (§2)** — depend on §1.
6. **Per-warehouse costing / FIFO (§7)** — only when a merchandising/perishable
   tenant needs it; treat as a costing-engine project with its own test suite.

## Non-goals (for now)

- Serial/lot/batch traceability and expiry (a separate inventory-traceability
  track, though FIFO layers (§7) are a stepping stone).
- Quality management (inspection plans, non-conformance) beyond simple scrap.
- Shop-floor data collection hardware / barcode terminals.
- Full APS (advanced planning & scheduling) optimisation.

---

_See also: `docs/BACKUP_RUNBOOK.md` (migration safety), `REMAINING.md`
(cross-module backlog), `CHANGELOG.md` (2026-07-11 — Manufacturing v1)._
