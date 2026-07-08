import { Link, useLocation } from 'react-router-dom';
import { ArrowLeft } from 'lucide-react';
import Logo from '../../components/brand/Logo';
import MarketingFooter from './Footer';

type Section = { h: string; p?: string[]; list?: string[] };
type Page = { title: string; subtitle: string; updated?: string; sections: Section[]; contact?: boolean };

const UPDATED = 'July 2026';
const CONTACT_EMAIL = 'hello@zavora.ai';

const PAGES: Record<string, Page> = {
  updates: {
    title: 'What’s new in Zavora ERP',
    subtitle: 'Recent releases, straight from the changelog.',
    updated: UPDATED,
    sections: [
      { h: 'July 2026 — Amos works while you sleep', list: [
        'Ambient Ops: Amos now runs your accounting calendar — a morning cash briefing, a daily eTIMS compliance sweep, a Monday receivables chase list, Friday reconciliation checks, VAT & PAYE prep on the KRA deadline calendar, and a month-end close pack. Reports arrive in your notification inbox; anything that posts still waits for your say-so.',
        'Run-now and pause controls for every routine, right inside Amos — and the ERP alerts Amos the moment an eTIMS transmission fails so it retries within minutes.',
        'Amos now covers the full accountant job: raising customer invoices (with eTIMS checks), stock adjustments and transfers, bank reconciliation, period close, and VAT/PAYE/WHT filing workflows.',
        'Attach a photo or PDF of any invoice, receipt or statement — Amos reads it and books it. Ask about live KRA/CBK rates and it searches the web with cited sources.' ] },
      { h: 'July 2026 — KRA eTIMS, built in', list: [
        'Real-time eTIMS (OSCU/VSCU) invoice transmission: posted invoices and POS sales go to KRA automatically, with the signed SCU receipt and verification QR on every fiscal document.',
        'Credit notes transmit as credit/refund receipts referencing the original invoice; items register with KRA automatically.',
        'Every conversation with Amos is kept as an auditable record, and its memory of your business is yours to inspect and correct.' ] },
      { h: 'Earlier in 2026', list: [
        'Full procure-to-pay: requisitions, approvals, LPOs, goods receipt with 3-way match, debit notes and expense claims — plus a supplier portal.',
        'Point of Sale with shift sessions and Z-reports, mobile selling on M-Pesa, and stock operations from the shop floor.',
        'Enterprise payroll: effective-dated PAYE/NSSF/SHA/Housing Levy config, batch runs, department-split GL postings, payslips and statutory reports.',
        'Optional CRM with a sales pipeline and a customer self-service portal.',
        'Granular role-based access control with custom roles.',
        'Explore with sample data: new workspaces can start with a realistic Kenyan-SME dataset to click around before entering real books.' ] },
    ],
  },
  about: {
    title: 'About Zavora ERP',
    subtitle: 'The AI-native business platform built for Kenyan SMEs, by Zavora Technologies Ltd.',
    sections: [
      { h: 'Why we exist', p: [
        'Kenyan businesses run on a patchwork of spreadsheets, a standalone till, a WhatsApp group and a shoebox of receipts — then pay an accountant to make sense of it at month-end. The global software wasn’t built for eTIMS, M-Pesa or Kenyan payroll, and none of it comes with an accountant inside.',
        'Zavora ERP is one platform for the whole business — invoicing, POS, inventory, procurement, payroll and reporting — with Amos, an AI accountant that posts, reconciles and prepares taxes while you run the business.' ] },
      { h: 'What makes us different', list: [
        'Amos, a genuine AI accountant that works inside your live ledger — not a bolt-on chatbot.',
        'Kenya-first: eTIMS tax invoices, M-Pesa, and PAYE/NSSF/SHA/Housing Levy/HELB payroll built in.',
        'One system where sales, stock, payroll and tax all talk to the same books.' ] },
      { h: 'Where we are', p: [ 'Zavora ERP is built by Zavora Technologies Ltd in Nairobi, Kenya, for businesses across East Africa. Want to talk? We’d love to hear from you.' ] },
    ],
  },
  careers: {
    title: 'Careers at Zavora',
    subtitle: 'Help us put every Kenyan business’s books on autopilot.',
    sections: [
      { h: 'Building the future of business software in Africa', p: [
        'We’re a small, senior team shipping fast at the intersection of accounting, AI and the realities of doing business in Kenya. If that excites you, we want to meet you.' ] },
      { h: 'How we work', list: [
        'Ship real things that customers use — weekly, not quarterly.',
        'Deep ownership: you own problems end to end.',
        'Remote-friendly, anchored in Nairobi.' ] },
      { h: 'Open roles', p: [
        'We hire ahead of specific openings. Send us who you are and what you’d want to work on — engineering, product, design, or go-to-market — and we’ll take it from there.' ] },
    ],
    contact: true,
  },
  contact: {
    title: 'Talk to us',
    subtitle: 'Sales questions, support, or just saying hello — we reply fast.',
    sections: [
      { h: 'Sales & general', p: [ 'For pricing, demos, or anything about the product, email us and a human will get back to you within one business day.' ] },
      { h: 'Support', p: [ 'Already using Zavora? Reach the same address and we’ll route you to support — or ask Amos in-app.' ] },
      { h: 'Where we are', p: [ 'Nairobi, Kenya.' ] },
    ],
    contact: true,
  },
  privacy: {
    title: 'Privacy Policy',
    subtitle: 'How Zavora ERP collects, uses and protects your information.',
    updated: UPDATED,
    sections: [
      { h: 'Who we are', p: [ 'Zavora Technologies Ltd ("Zavora", "we") provides Zavora ERP, our business platform. This policy explains how we handle personal data for visitors to our website and users of the product, in line with Kenya’s Data Protection Act, 2019.' ] },
      { h: 'Information we collect', list: [
        'Account data: your name, email, organization details and KRA PIN (if provided).',
        'Business data you enter: invoices, bills, payments, payroll, customers and suppliers.',
        'Usage data: device, browser and interaction logs used to run and improve the service.' ] },
      { h: 'How we use it', list: [
        'To provide, secure and improve the service.',
        'To process transactions and generate your reports and tax documents.',
        'To communicate with you about your account and support requests.' ] },
      { h: 'Sharing', p: [ 'We do not sell your data. We share it only with processors that help us run the service (e.g. cloud hosting and AI inference), under contract, and where required by law.' ] },
      { h: 'AI processing', p: [ 'When you use Amos, the relevant content is processed by our AI provider to generate a response. We do not use your business data to train third-party models.' ] },
      { h: 'Retention & your rights', p: [ 'We keep your data for as long as your account is active and as required by Kenyan tax and company law. You may request access, correction, or deletion of your personal data at any time.' ] },
      { h: 'Contact', p: [ `Questions about privacy? Email ${CONTACT_EMAIL}.` ] },
    ],
  },
  terms: {
    title: 'Terms of Service',
    subtitle: 'The agreement between you and Zavora.',
    updated: UPDATED,
    sections: [
      { h: 'The service', p: [ 'Zavora Technologies Ltd provides Zavora ERP, a cloud business-management and accounting platform. By creating an account you agree to these terms.' ] },
      { h: 'Your account', list: [
        'You are responsible for your organization’s data and for keeping credentials secure.',
        'You must have authority to act for the organization you register.',
        'You are responsible for the accuracy of the records you enter and file.' ] },
      { h: 'Acceptable use', p: [ 'Don’t misuse the service: no unlawful activity, no attempts to breach security, and no use that infringes others’ rights.' ] },
      { h: 'Plans & billing', p: [ 'Paid plans are billed as described at sign-up. You can change or cancel your plan; fees already due are non-refundable except where required by law.' ] },
      { h: 'Compliance disclaimer', p: [ 'Zavora ERP helps you prepare compliant records and returns, but you remain responsible for your tax and statutory filings. Zavora ERP is not a substitute for professional accounting or legal advice.' ] },
      { h: 'Liability', p: [ 'The service is provided "as is". To the extent permitted by law, Zavora Technologies Ltd’s liability is limited to the fees you paid in the preceding 12 months.' ] },
      { h: 'Governing law', p: [ 'These terms are governed by the laws of Kenya.' ] },
    ],
  },
  security: {
    title: 'Security',
    subtitle: 'How we protect your business and its data.',
    updated: UPDATED,
    sections: [
      { h: 'Encryption', p: [ 'Data is encrypted in transit (TLS) and at rest. Credentials are hashed with a modern, salted algorithm — never stored in plain text.' ] },
      { h: 'Access control', list: [
        'Role-based access (Owner, Admin, Accountant, Approver, Editor, Viewer).',
        'Approval limits and delegation-of-authority on financial actions.',
        'Every posting is attributed to a user and captured in an immutable audit trail.' ] },
      { h: 'Tenant isolation', p: [ 'Each organization’s data is isolated. Amos is bound to a single tenant and refuses any session that doesn’t belong to it.' ] },
      { h: 'AI guardrails', p: [ 'Amos screens input for prompt-injection and exfiltration, enforces per-session scopes, and requires your explicit confirmation before any financial posting.' ] },
      { h: 'Backups & availability', p: [ 'Data is backed up regularly and hosted on reputable cloud infrastructure with redundancy.' ] },
      { h: 'Reporting an issue', p: [ `Found a vulnerability? Please email ${CONTACT_EMAIL} — we take reports seriously and will respond quickly.` ] },
    ],
  },
  'data-protection': {
    title: 'Data Protection',
    subtitle: 'Our commitments under Kenya’s Data Protection Act, 2019.',
    updated: UPDATED,
    sections: [
      { h: 'Controller & processor', p: [ 'For your account and website data, Zavora Technologies Ltd is the data controller. For the business data you enter about your own customers and staff, you are the controller and Zavora Technologies Ltd is your processor — we process it only on your instructions to provide the service.' ] },
      { h: 'Lawful basis', p: [ 'We process personal data to perform our contract with you, to comply with legal obligations (including tax and company law), and for our legitimate interest in running and securing the service.' ] },
      { h: 'Your rights', list: [
        'Access the personal data we hold about you.',
        'Correct inaccurate data or complete incomplete data.',
        'Request deletion, subject to legal retention requirements.',
        'Object to or restrict certain processing.' ] },
      { h: 'Cross-border transfers', p: [ 'Some processors (e.g. cloud and AI providers) may process data outside Kenya. Where they do, we rely on appropriate safeguards consistent with the Act.' ] },
      { h: 'Breach notification', p: [ 'If a personal-data breach is likely to cause real risk, we will notify the affected parties and the Data Commissioner as required by the Act.' ] },
      { h: 'Contact', p: [ `To exercise your rights or ask a question, email ${CONTACT_EMAIL}.` ] },
    ],
  },
};

function ContactBlock() {
  return (
    <div className="mt-8 rounded-2xl border border-slate-200 bg-slate-50 p-6">
      <p className="text-slate-700">Email us at{' '}
        <a href={`mailto:${CONTACT_EMAIL}`} className="font-semibold text-indigo-600 hover:underline">{CONTACT_EMAIL}</a>
      </p>
      <a href={`mailto:${CONTACT_EMAIL}`} className="mt-4 inline-flex items-center gap-2 rounded-full bg-slate-900 text-white font-semibold px-5 py-2.5 hover:bg-slate-800 transition">
        Send us a message
      </a>
    </div>
  );
}

export default function InfoPage() {
  const path = useLocation().pathname.replace(/^\//, '').replace(/\/$/, '');
  const page = PAGES[path];

  if (!page) {
    return (
      <div className="min-h-screen bg-white flex flex-col">
        <Header />
        <main className="flex-1 mx-auto max-w-3xl px-5 py-24 text-center">
          <h1 className="text-3xl font-bold">Page not found</h1>
          <p className="mt-3 text-slate-600">The page you’re looking for doesn’t exist.</p>
          <Link to="/" className="mt-6 inline-block text-indigo-600 font-semibold hover:underline">← Back to home</Link>
        </main>
        <MarketingFooter />
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-white flex flex-col">
      <Header />
      <main className="flex-1 mx-auto max-w-3xl px-5 py-16 w-full">
        <Link to="/" className="inline-flex items-center gap-1.5 text-sm text-slate-500 hover:text-slate-900 transition mb-8">
          <ArrowLeft className="w-4 h-4" /> Back to home
        </Link>
        <h1 className="text-4xl font-extrabold tracking-tight">{page.title}</h1>
        <p className="mt-3 text-lg text-slate-600">{page.subtitle}</p>
        {page.updated && <p className="mt-2 text-xs text-slate-400">Last updated {page.updated}</p>}
        <div className="mt-10 space-y-8">
          {page.sections.map((s) => (
            <section key={s.h}>
              <h2 className="text-xl font-bold tracking-tight text-slate-900">{s.h}</h2>
              {s.p?.map((para, i) => <p key={i} className="mt-3 text-slate-600 leading-relaxed">{para}</p>)}
              {s.list && (
                <ul className="mt-3 space-y-2">
                  {s.list.map((li) => (
                    <li key={li} className="flex items-start gap-2.5 text-slate-600">
                      <span className="mt-2 h-1.5 w-1.5 shrink-0 rounded-full bg-indigo-500" />{li}
                    </li>
                  ))}
                </ul>
              )}
            </section>
          ))}
        </div>
        {page.contact && <ContactBlock />}
      </main>
      <MarketingFooter />
    </div>
  );
}

function Header() {
  return (
    <header className="sticky top-0 z-50 backdrop-blur-lg bg-white/80 border-b border-slate-100">
      <div className="mx-auto max-w-7xl px-5 h-16 flex items-center justify-between">
        <Link to="/"><Logo /></Link>
        <div className="flex items-center gap-3">
          <Link to="/login" className="text-sm font-semibold text-slate-700 hover:text-slate-900">Sign in</Link>
          <Link to="/login?signup=1" className="text-sm font-semibold text-white bg-slate-900 hover:bg-slate-800 rounded-full px-4 py-2 transition">Start free</Link>
        </div>
      </div>
    </header>
  );
}
