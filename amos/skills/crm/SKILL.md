---
name: crm
description: Manage and explain the CRM — sales pipeline (leads → opportunities → win/lose), activities, support tickets, a customer portal (self-signup + sales-assisted invite), and pipeline analytics (open value, weighted forecast, win rate, lead conversion). Use when the user mentions CRM, leads, prospects, sales pipeline, opportunities/deals, forecast, win rate, activities/follow-ups, support tickets, or the customer portal. CRM is an optional add-on that is off until enabled.
license: Proprietary
compatibility: Zavora ERA CRM module (optional, per-tenant feature flag). Driven through the ERP UI (browser tools); figures read from the CRM Overview screen.
allowed-tools: [get_dashboard, browser_navigate, browser_snapshot, browser_click, browser_type, browser_fill_form, browser_select_option, browser_wait_for, showcase_step, plan_tasks, update_task, remember]
metadata:
  author: Zavora AI
  category: crm
  success-criteria:
    check-enabled-first: "Always confirm CRM is enabled (or enable it with the user's OK) before managing pipeline data"
    figures-from-screen: "Every pipeline/forecast figure comes from the CRM Overview screen — never invented"
    confirm-before-win-lose: "Marking a deal Won/Lost, converting a lead, or disabling CRM is confirmed with the user first"
    portal-privacy: "Never expose one customer's data to another; the portal is self-service and per-customer"
---

# CRM

You help a business owner or sales user run the optional **CRM** on Zavora ERA:
capture **leads**, work **opportunities** through a **pipeline**, log **activities**,
handle **support tickets**, onboard customers to the **customer portal**, and read
**pipeline analytics**. CRM is additive and does not touch the accounting ledger.

Everything is done through the ERP UI at **`/crm`** (and the customer portal at
**`/customerportal`**). State pipeline/forecast numbers only from what's on screen.

## First: is CRM on?
CRM is **off by default** for a tenant. Before managing anything:
1. `browser_navigate` to `/crm`, `browser_snapshot`.
2. If you see the **"Turn on CRM"** opt-in card, CRM is disabled. Ask the user:
   *"CRM isn't switched on for this workspace yet — shall I enable it? It's optional and won't affect your accounting."* On **yes**, `browser_click` **Enable CRM** (this seeds a default "Sales Pipeline" with stages Lead In → Qualified → Proposal → Negotiation → Won → Lost).
3. If you see tabs (Overview / Pipeline / Leads / Activities), CRM is already on.

## The model in plain language
- **Lead** — an unqualified prospect (a name/company/email). Leads come from manual entry or **portal self-signups**. A lead is **converted** into an opportunity once it's real.
- **Opportunity (deal)** — a potential sale with an **amount** moving through pipeline **stages**. Each stage has a **probability**; the **weighted forecast** = Σ (amount × probability). A deal ends **Won** or **Lost**.
- **Activity** — a task/call/meeting/email/note, optionally with a due date; mark it done when complete.
- **Ticket** — a customer support request (from the portal or logged by staff), with a message thread.
- **Customer portal** — customers sign in at `/customerportal` to see their invoices/statement and raise tickets. They **self-register** (creates a lead) or are **invited** by sales.

## Decision Tree
```
CRM request
├── "how's the pipeline / what's the forecast / win rate"      → ANALYSE: Overview tab (read figures off screen)
├── "add a lead / new prospect"                                → MANAGE: Leads tab → New Lead
├── "this lead is real / convert <lead>"                       → MANAGE: Leads tab → Convert (confirm)
├── "add a deal / opportunity"                                 → MANAGE: Pipeline tab → New Opportunity
├── "move <deal> to <stage>"                                   → MANAGE: Pipeline tab → card stage dropdown
├── "we won / lost <deal>"                                     → MANAGE: Pipeline tab → Win/Lose (confirm)
├── "log a call/task/follow-up" / "what's due"                 → MANAGE: Activities tab
├── "invite <customer> to the portal"                          → MANAGE: WORKFLOW I (assisted invite)
├── "a customer support ticket / complaint"                    → MANAGE: tickets (portal or staff side)
└── "turn CRM on/off"                                          → MANAGE: Enable/Disable (confirm; data is kept)
```

## ANALYSE workflow (pipeline analytics)
1. `browser_navigate` to `/crm`; ensure the **Overview** tab is selected; `browser_snapshot`.
2. Read the four headline cards: **Open pipeline** (open value + deal count), **Weighted forecast**, **Win rate** (won/lost), **Lead conversion** (converted/total). Plus the **pipeline-by-stage** bars and **avg. won deal / won value / open activities**.
3. Lead with the figure asked for, rounded for speech ("your open pipeline is about eight hundred thousand shillings, with a weighted forecast near four hundred thousand"), then 2–3 supporting numbers.
4. `showcase_step` the Overview so the user sees it.
- Never invent a forecast or win rate — if the screen hasn't loaded, `browser_wait_for` and re-snapshot.

## MANAGE: pipeline
- **New lead:** Leads tab → **New Lead** → fill name (+ company/email/phone/source) → Create.
- **Convert a lead** (confirm first — it opens an opportunity and marks the lead Converted): Leads tab → **Convert** on the row.
- **New opportunity:** Pipeline tab → **New Opportunity** → name + amount (+ expected close) → Create (opens in the first stage).
- **Move a deal:** Pipeline tab → on the card, pick the target **stage** in the dropdown (probability updates automatically).
- **Win / Lose** (confirm with the amount first): Pipeline tab → the green **Won** or red **Lost** button on the card. Won/Lost is a real outcome — always confirm: *"Mark 'Beta pilot' (KES 80,000) as Won?"*
- **Showcase** the updated board with `showcase_step`.

## MANAGE: activities
- Activities tab → **New Activity** → type (Task/Call/Meeting/Email/Note) + subject (+ due) → Create.
- **Mark done** when complete. Use activities for follow-ups ("call Acme next Tuesday").

## WORKFLOW I: invite a customer to the portal (sales-assisted onboarding)
Customers can self-sign-up at `/customerportal`, or sales can invite them:
1. Confirm the customer's email (and which billing account to link, if any).
2. Invite via the CRM API/action `POST /crm/customers/invite-portal` (email [+ display name, customer_id, or a temporary password]). With no password, they get an emailed **set-password** link; with a password, the account is active immediately.
3. Tell the user what was sent. The customer then signs in at `/customerportal` to view invoices/statement and raise tickets.
- **Privacy:** the portal is strictly per-customer — one customer never sees another's invoices or tickets.

## MANAGE: support tickets
- Portal customers raise tickets themselves; staff see them in the CRM tickets view and **reply** on the thread, or set status (Open → Pending → Resolved/Closed).
- Keep replies factual; escalate billing questions to the finance side (invoices/statement live in the ERP, not the ticket).

## MUST DO
- Check CRM is enabled first; if not, offer to enable it (with the user's OK) before doing anything else.
- State pipeline, forecast, win-rate and conversion figures only from the CRM Overview screen.
- Confirm **Convert lead**, **Win/Lose deal**, and **Disable CRM** with the user first — each with the specific name/amount.
- Keep the customer portal private per customer.
- Showcase the relevant screen (Overview / Pipeline) after acting.

## MUST NOT DO
- Never invent a forecast, win rate, deal amount, or pipeline total — read it from the screen.
- Never mark a deal Won/Lost or convert a lead without an explicit yes.
- Never disable CRM without confirming (data is kept, but the module is hidden).
- Never treat CRM as required — it's an optional add-on; core accounting works without it.
- Never expose one customer's invoices, statement or tickets to another customer.
