---
name: management-accounts
description: Produce the monthly management accounts pack — P&L vs budget vs prior month vs same month last year, margins, KPIs (DSO/DPO/stock turns/current ratio), department split, and disciplined variance commentary. Use when the user asks for management accounts, a board pack, "how did we do vs budget?", KPIs, or a monthly performance review.
license: Proprietary
compatibility: Requires mcp-erp (zavora backend).
allowed-tools: [get_dashboard, run_report, list_budgets, set_budget, list_fiscal_periods, list_invoices, list_bills, showcase_step, plan_tasks, update_task]
metadata:
  author: Zavora AI
  category: management-accounting
  success-criteria:
    traceability: "Every figure in the pack comes from a report run in this session"
    commentary: "Every variance >10% (or > KES 100k) is explained by its driver, or explicitly flagged unexplained"
    kpis: "Ratios computed from the formulas below — never estimated"
---

# Monthly Management Accounts

The pack a finance manager tables: not just the numbers, but what moved, why,
and what to do. Figures come from reports run NOW; commentary follows the rules
below — insight, never filler.

## The pack, in order
1. **Headline** — revenue, gross profit (+%), net profit (+%) for the month, each with: vs budget, vs prior month, vs same month last year.
2. **Variance table** — `run_report BudgetVsActual` for the month; then P&L twice more (prior month; same month last year) for the movements.
3. **Margins** — gross margin % = gross_profit/revenue; net margin % = net_profit/revenue; comment on direction.
4. **KPIs** (formulas — compute, don't estimate):
   - **DSO** = (AR balance ÷ revenue for the period) × days-in-period — "how long customers take to pay". Healthy Kenyan SME: ≤ 45 days; >60 = chase harder.
   - **DPO** = (AP balance ÷ (COGS + operating expenses)) × days — paying faster than you collect burns cash.
   - **Stock turns** (if stock-tracked) = COGS ÷ InventoryValuation; low turns = cash sitting on shelves.
   - **Current ratio** = current assets ÷ current liabilities (BalanceSheet); < 1 = stress, flag it.
   - **Cash cover** = cash ÷ average monthly operating expenses — months of runway.
5. **Department/branch split** — `run_report DimensionalAnalysis` when dimensions are in use; name the best and worst performer.
6. **Commentary** — see rules.
7. Close with the 2–3 decisions the numbers suggest (chase X, review pricing on Y, defer Z).

## Commentary rules (this is the craft)
- Explain EVERY variance beyond ±10% of budget or ± KES 100,000, naming the driver (volume? price? one-off? timing?). Use GlDetail / IncomeByCustomer / ExpenseByVendor to find it — don't speculate.
- One-offs are labelled one-offs; recurring shifts are labelled trends.
- If you cannot find the driver, say "unexplained — needs review", never invent one.
- Numbers in the text carry direction and magnitude ("up 18% / KES 240k"), not adjectives alone.

## Budget maintenance
- "How are we tracking?" → BudgetVsActual for the period, YTD view if asked.
- "Set next year's budget" → propose per-account figures from this year's actuals (P&L by account via TrialBalance/GlDetail) adjusted by the user's growth/inflation assumptions; confirm the table, then `set_budget` per account × period. Never write budget figures the user hasn't seen.
- No budget loaded? Say so and offer to build one — a variance report against nothing is noise.

## MUST NOT DO
- Never present a KPI without its formula inputs available in this session.
- Never smooth or reclassify figures to "look better" — the pack reports, the user decides.
- Never set budgets unconfirmed.
