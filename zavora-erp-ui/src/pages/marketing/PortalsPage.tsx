import { Link } from 'react-router-dom';
import {
  ArrowRight, Check, ShieldCheck, Building2, Users, BadgeCheck, Gavel,
  FileText, Receipt, MessageSquare, CalendarDays, Wallet,
} from 'lucide-react';
import Logo from '../../components/brand/Logo';
import MarketingFooter from './Footer';

const SIGNUP = '/login?signup=1';

/** Dedicated public page for the self-service portals — supplier, customer
 * and employee surfaces that live outside the back office. */
export default function PortalsPage() {
  return (
    <div className="min-h-screen bg-white text-slate-900 antialiased selection:bg-indigo-200/60">
      <header className="sticky top-0 z-50 backdrop-blur-lg bg-white/80 border-b border-slate-100">
        <div className="mx-auto max-w-7xl px-5 h-16 flex items-center justify-between">
          <Link to="/" className="flex items-center gap-2.5"><Logo /></Link>
          <nav className="hidden md:flex items-center gap-8 text-sm font-medium text-slate-600">
            <Link to="/amos-ai" className="hover:text-slate-900">Amos AI</Link>
            <a href="/#modules" className="hover:text-slate-900">Modules</a>
            <Link to="/portals" className="text-slate-900 font-semibold">Portals</Link>
            <a href="/#pricing" className="hover:text-slate-900">Pricing</a>
          </nav>
          <div className="flex items-center gap-3">
            <Link to="/login" className="text-sm font-semibold text-slate-700 hover:text-slate-900">Sign in</Link>
            <Link to={SIGNUP} className="text-sm font-semibold text-white bg-slate-900 hover:bg-slate-800 rounded-full px-4 py-2 transition">Start free</Link>
          </div>
        </div>
        <div className="border-t border-slate-100 bg-white/60">
          <nav className="mx-auto max-w-7xl px-5 h-10 flex items-center gap-6 text-[13px] font-medium text-slate-500 overflow-x-auto">
            <span className="text-slate-400 uppercase tracking-wider text-[11px] shrink-0">Portals</span>
            <a href="#suppliers" className="hover:text-slate-900 whitespace-nowrap">Suppliers</a>
            <a href="#customers" className="hover:text-slate-900 whitespace-nowrap">Customers</a>
            <a href="#employees" className="hover:text-slate-900 whitespace-nowrap">Employees</a>
            <a href="#security" className="hover:text-slate-900 whitespace-nowrap">Isolation &amp; security</a>
          </nav>
        </div>
      </header>

      <Hero />
      <Suppliers />
      <Customers />
      <Employees />
      <Security />
      <Cta />
      <MarketingFooter />
    </div>
  );
}

function Frame({ src, alt }: { src: string; alt: string }) {
  return (
    <div className="relative rounded-2xl border border-slate-200 bg-white shadow-2xl shadow-slate-200/70 overflow-hidden">
      <div className="flex items-center gap-1.5 px-4 h-9 bg-slate-100 border-b border-slate-200">
        <span className="w-2.5 h-2.5 rounded-full bg-red-400/80" />
        <span className="w-2.5 h-2.5 rounded-full bg-amber-400/80" />
        <span className="w-2.5 h-2.5 rounded-full bg-emerald-400/80" />
        <span className="ml-3 text-[11px] text-slate-500">erp.zavora.ai</span>
      </div>
      <img src={src} alt={alt} className="w-full block bg-slate-50" loading="lazy" onError={(e) => ((e.target as HTMLImageElement).style.opacity = '0')} />
    </div>
  );
}

function Flow({ steps }: { steps: string[] }) {
  return (
    <ol className="mt-6 flex flex-wrap items-center gap-y-2 text-[13px] font-medium">
      {steps.map((s, i) => (
        <li key={s} className="flex items-center">
          <span className="rounded-full bg-slate-100 text-slate-700 px-3 py-1.5">{s}</span>
          {i < steps.length - 1 && <ArrowRight className="w-4 h-4 mx-1.5 text-slate-300 shrink-0" />}
        </li>
      ))}
    </ol>
  );
}

function Hero() {
  return (
    <section className="relative overflow-hidden bg-slate-950 text-white">
      <div className="pointer-events-none absolute -top-40 left-1/2 -translate-x-1/2 h-[420px] w-[900px] rounded-full bg-gradient-to-r from-emerald-600/30 via-indigo-600/30 to-fuchsia-600/25 blur-[120px]" />
      <div className="relative mx-auto max-w-7xl px-5 pt-20 pb-16 text-center">
        <div className="inline-flex items-center gap-2 rounded-full border border-white/15 bg-white/5 px-3.5 py-1.5 text-xs font-medium text-indigo-200 mb-7">
          <Building2 className="w-3.5 h-3.5" /> Self-service portals · built into Zavora ERP
        </div>
        <h1 className="mx-auto max-w-4xl text-4xl sm:text-6xl font-extrabold tracking-tight leading-[1.05]">
          Your suppliers, customers and staff <span className="bg-gradient-to-r from-emerald-400 via-indigo-400 to-fuchsia-400 bg-clip-text text-transparent">serve themselves</span>.
        </h1>
        <p className="mx-auto max-w-2xl mt-6 text-lg text-slate-300">
          Three dedicated portals write straight into your books — suppliers bid and lodge invoices,
          customers see their statements and raise tickets, employees request leave and pick up payslips.
          No email ping-pong, no re-keying, no extra seats to buy.
        </p>
        <div className="mt-9 flex flex-col sm:flex-row items-center justify-center gap-3">
          <Link to={SIGNUP} className="group inline-flex items-center gap-2 rounded-full bg-white text-slate-900 font-semibold px-6 py-3.5 hover:bg-slate-100 transition">
            Start free <ArrowRight className="w-4 h-4 group-hover:translate-x-0.5 transition" />
          </Link>
          <a href="#suppliers" className="inline-flex items-center gap-2 rounded-full border border-white/20 px-6 py-3.5 font-semibold hover:bg-white/5 transition">
            See the portals
          </a>
        </div>
      </div>
    </section>
  );
}

function Suppliers() {
  return (
    <section id="suppliers" className="mx-auto max-w-7xl px-5 py-20 grid lg:grid-cols-2 gap-12 items-center">
      <div>
        <div className="inline-flex items-center gap-2 rounded-full bg-emerald-50 text-emerald-700 px-3 py-1 text-xs font-semibold mb-4">
          <Gavel className="w-3.5 h-3.5" /> Supplier portal
        </div>
        <h2 className="text-3xl font-bold tracking-tight">Procurement without the paperwork chase</h2>
        <p className="mt-4 text-slate-600">
          Suppliers register themselves, bid on your tenders, receive LPOs the moment you issue them,
          lodge invoices against those LPOs, and check their own statement — every step landing directly
          in your procurement workflow and, once approved, your ledger.
        </p>
        <Flow steps={['Self-register', 'You approve', 'Bid on tenders', 'Receive the LPO', 'Lodge invoice', 'Statement']} />
        <ul className="mt-6 space-y-2.5 text-sm text-slate-600">
          <li className="flex items-start gap-2.5"><Check className="w-4 h-4 text-emerald-600 mt-0.5 shrink-0" />Registrations queue for <strong>your approval</strong> — nobody enters your supply chain unvetted</li>
          <li className="flex items-start gap-2.5"><Check className="w-4 h-4 text-emerald-600 mt-0.5 shrink-0" />Lodged invoices arrive pre-linked to their LPO, ready for <strong>3-way match</strong> against goods received</li>
          <li className="flex items-start gap-2.5"><Check className="w-4 h-4 text-emerald-600 mt-0.5 shrink-0" />The supplier's statement is <strong>your AP ledger's view</strong> — one version of the truth ends invoice disputes</li>
        </ul>
      </div>
      <div className="space-y-6">
        <Frame src="/marketing/portal-vendor-tenders.png" alt="Supplier portal: open tenders with a live RFQ and Submit bid" />
        <Frame src="/marketing/portal-vendor-orders.png" alt="Supplier portal: purchase orders received from the buyer" />
      </div>
    </section>
  );
}

function Customers() {
  return (
    <section id="customers" className="bg-slate-50 border-y border-slate-100">
      <div className="mx-auto max-w-7xl px-5 py-20 grid lg:grid-cols-2 gap-12 items-center">
        <div className="order-2 lg:order-1">
          <Frame src="/marketing/portal-customer.png" alt="Customer portal: invoices, balances and statement for a real customer" />
        </div>
        <div className="order-1 lg:order-2">
          <div className="inline-flex items-center gap-2 rounded-full bg-indigo-50 text-indigo-700 px-3 py-1 text-xs font-semibold mb-4">
            <Receipt className="w-3.5 h-3.5" /> Customer portal
          </div>
          <h2 className="text-3xl font-bold tracking-tight">"Can you resend that invoice?" — never again</h2>
          <p className="mt-4 text-slate-600">
            Every customer gets a sign-in to their own slice of your books: live invoices and balances,
            a statement of account that always matches yours, and a support thread your team answers
            from inside the ERP.
          </p>
          <Flow steps={['Invite (or self-sign-up)', 'Set password', 'Invoices & balances', 'Statement', 'Support tickets']} />
          <ul className="mt-6 space-y-2.5 text-sm text-slate-600">
            <li className="flex items-start gap-2.5"><Check className="w-4 h-4 text-indigo-600 mt-0.5 shrink-0" />Invite existing customers in one click, or let new ones <strong>self-onboard</strong> — each signup lands as a CRM lead</li>
            <li className="flex items-start gap-2.5"><MessageSquare className="w-4 h-4 text-indigo-600 mt-0.5 shrink-0" /><strong>Support tickets with a message thread</strong> — questions about an invoice stay attached to the account, not lost in email</li>
            <li className="flex items-start gap-2.5"><Check className="w-4 h-4 text-indigo-600 mt-0.5 shrink-0" />Statements read from the live ledger — what the customer sees is exactly what your books say</li>
          </ul>
        </div>
      </div>
    </section>
  );
}

function Employees() {
  return (
    <section id="employees" className="mx-auto max-w-7xl px-5 py-20">
      <div className="max-w-3xl">
        <div className="inline-flex items-center gap-2 rounded-full bg-amber-50 text-amber-700 px-3 py-1 text-xs font-semibold mb-4">
          <Users className="w-3.5 h-3.5" /> Employee self-service
        </div>
        <h2 className="text-3xl font-bold tracking-tight">HR stops being the middleman</h2>
        <p className="mt-4 text-slate-600">
          Staff get their own portal for the things they'd otherwise queue at the office for —
          requesting leave against a live balance, and picking up payslips the moment payroll commits.
        </p>
      </div>
      <div className="mt-8 grid md:grid-cols-3 gap-6">
        {[
          [CalendarDays, 'Leave, self-served', 'Requests route to the right approver with the balance checked up front — accruals by tenure, carryover and Kenyan public holidays already handled.'],
          [Wallet, 'Payslips on demand', 'PDF payslips per pay run, with PAYE/NSSF/SHA/Housing Levy itemised — no more month-end print-and-distribute.'],
          [BadgeCheck, 'Invited, not provisioned', 'HR sends an invite; the employee sets a password. Leavers lose access the day they leave the payroll.'],
        ].map(([Icon, t, d]: any) => (
          <div key={t} className="rounded-2xl border border-slate-200 p-6">
            <span className="flex h-10 w-10 items-center justify-center rounded-xl bg-amber-50 text-amber-600 mb-4"><Icon className="w-5 h-5" /></span>
            <h3 className="font-semibold mb-2">{t}</h3>
            <p className="text-sm text-slate-600">{d}</p>
          </div>
        ))}
      </div>
    </section>
  );
}

function Security() {
  return (
    <section id="security" className="bg-slate-950 text-white">
      <div className="mx-auto max-w-7xl px-5 py-20">
        <div className="text-center mb-12">
          <div className="inline-flex items-center gap-2 rounded-full bg-emerald-500/15 text-emerald-300 px-3 py-1 text-xs font-semibold mb-4">
            <ShieldCheck className="w-3.5 h-3.5" /> Isolation by design
          </div>
          <h2 className="text-3xl font-bold tracking-tight">Outside users never touch the inside</h2>
          <p className="mt-4 text-slate-400 max-w-2xl mx-auto">
            Each portal is a separate class of account with its own sign-in — not a restricted view of your back office.
          </p>
        </div>
        <div className="grid md:grid-cols-3 gap-6 max-w-5xl mx-auto">
          {[
            ['Separate principals', 'Supplier, customer and employee accounts are distinct account types with their own credentials — never rows in your staff user list.'],
            ['Hard API boundary', 'A portal token opens portal endpoints only. Every back-office API — and Amos — rejects it outright, by construction.'],
            ['Scoped to their own data', 'A supplier sees their POs and statement; a customer sees their invoices; an employee sees their payslips. Nothing else exists for them.'],
          ].map(([t, d]) => (
            <div key={t} className="rounded-2xl bg-white/5 border border-white/10 p-6">
              <h3 className="font-semibold mb-2 flex items-center gap-2"><Check className="w-4 h-4 text-emerald-400" />{t}</h3>
              <p className="text-sm text-slate-400">{d}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

function Cta() {
  return (
    <section className="mx-auto max-w-4xl px-5 py-20 text-center">
      <h2 className="text-3xl sm:text-4xl font-bold tracking-tight">One ledger. Everyone on it. Nobody in it.</h2>
      <p className="mt-4 text-slate-600">Portals are included — suppliers, customers and staff don't count against your seats.</p>
      <div className="mt-8 flex flex-col sm:flex-row items-center justify-center gap-3">
        <Link to={SIGNUP} className="inline-flex items-center gap-2 rounded-full bg-slate-900 text-white font-semibold px-7 py-3.5 hover:bg-slate-800 transition">
          Start free <ArrowRight className="w-4 h-4" />
        </Link>
        <a href="/#pricing" className="inline-flex items-center gap-2 rounded-full border border-slate-300 px-7 py-3.5 font-semibold text-slate-700 hover:bg-slate-50 transition">
          <FileText className="w-4 h-4" /> See pricing
        </a>
      </div>
    </section>
  );
}
