// Canonical pricing definition — the single source of truth for plans.
// Both the marketing pricing section and the registration screen import this,
// so prices/features can never drift. Update tiers here only.
//
// Tiering rationale (mapped to the actual module set):
//   Free         — solo/just-starting: invoicing, expenses, core reports.
//   Starter      — micro business: full accounting, VAT/eTIMS, reconciliation, assets.
//   Business     — growing SME: POS, inventory, Kenyan payroll, HR, full tax, FX.
//   Business Pro — groups/multi-branch: procurement, CRM, consolidation, DoA, custom RBAC.

export type PlanKey = 'free' | 'starter' | 'business' | 'business_pro';

export interface PricingPlan {
  key: PlanKey;
  name: string;
  /** Display price, e.g. "KSh 0" or "KSh 6,900". */
  price: string;
  /** Price suffix, e.g. "/mo". */
  per?: string;
  /** Short audience tagline. */
  tag: string;
  /** Feature bullets (shown on the marketing page). */
  features: string[];
  /** Landing CTA label. */
  cta: string;
  /** Highlight as the recommended plan. */
  highlight?: boolean;
}

export const PRICING_PLANS: PricingPlan[] = [
  {
    key: 'free',
    name: 'Free',
    price: 'KSh 0',
    per: '/mo',
    tag: 'Solo & just starting out',
    cta: 'Start free',
    features: [
      '1 user',
      'Invoicing, quotes & expenses',
      'Customers & suppliers',
      'M-Pesa payment links',
      'Core reports (P&L, balance sheet)',
      'Amos AI — text chat (20 tasks/mo)',
    ],
  },
  {
    key: 'starter',
    name: 'Starter',
    price: 'KSh 2,500',
    per: '/mo',
    tag: 'Sole traders & micro businesses',
    cta: 'Start free trial',
    features: [
      'Up to 3 users',
      'Full double-entry accounting',
      'Bank import & reconciliation',
      'Products & price lists',
      'VAT returns & KRA eTIMS',
      'Fixed assets & depreciation',
      'Amos AI — 150 tasks/mo',
    ],
  },
  {
    key: 'business',
    name: 'Business',
    price: 'KSh 6,900',
    per: '/mo',
    tag: 'Growing SMEs',
    highlight: true,
    cta: 'Start free trial',
    features: [
      'Up to 10 users',
      'Point of Sale + eTIMS receipts',
      'Inventory & stock control',
      'Kenyan statutory payroll (PAYE, NSSF, SHA, Housing, HELB)',
      'HR: leave, onboarding & employee self-service',
      'WHT & full tax filing',
      'Multi-currency & FX',
      'Budgets & advanced reports',
      'Amos AI — voice + unlimited chat, document & web AI',
    ],
  },
  {
    key: 'business_pro',
    name: 'Business Pro',
    price: 'KSh 14,900',
    per: '/mo',
    tag: 'Multi-branch, groups & advanced control',
    cta: 'Start free trial',
    features: [
      'Up to 50 users',
      'Procurement suite (requisitions, tenders, POs, 3-way match) + vendor portal',
      'CRM + customer portal',
      'Multi-entity consolidation',
      'Approval limits & delegation of authority',
      'Custom roles & granular permissions',
      'Scheduled & custom reports, dimensional analysis',
      'Priority support',
    ],
  },
];

/** Default selected plan on the registration screen (the recommended tier). */
export const DEFAULT_PLAN_KEY: PlanKey = 'business';

/** Look up a plan by key. */
export const planByKey = (key: string): PricingPlan | undefined =>
  PRICING_PLANS.find((p) => p.key === key);
