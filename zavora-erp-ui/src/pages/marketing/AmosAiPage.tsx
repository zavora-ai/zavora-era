import { Link } from 'react-router-dom';
import {
  ArrowRight, Check, Bot, ShieldCheck, FileText, Wallet, Landmark,
  BarChart3, CalendarClock, Receipt, Sparkles, Camera, Paperclip, Globe, Brain,
} from 'lucide-react';
import Logo from '../../components/brand/Logo';
import MarketingFooter from './Footer';

const SIGNUP = '/login?signup=1';

/** Dedicated public page for Amos — the AI accountant. */
export default function AmosAiPage() {
  return (
    <div className="min-h-screen bg-white text-slate-900 antialiased selection:bg-indigo-200/60">
      <header className="sticky top-0 z-50 backdrop-blur-lg bg-white/80 border-b border-slate-100">
        {/* Main site navigation — identical to the landing page's. */}
        <div className="mx-auto max-w-7xl px-5 h-16 flex items-center justify-between">
          <Link to="/" className="flex items-center gap-2.5"><Logo /></Link>
          <nav className="hidden md:flex items-center gap-8 text-sm font-medium text-slate-600">
            <Link to="/amos-ai" className="text-slate-900 font-semibold">Amos AI</Link>
            <a href="/#modules" className="hover:text-slate-900">Modules</a>
            <a href="/#compliance" className="hover:text-slate-900">Kenya-ready</a>
            <a href="/#pricing" className="hover:text-slate-900">Pricing</a>
          </nav>
          <div className="flex items-center gap-3">
            <Link to="/login" className="text-sm font-semibold text-slate-700 hover:text-slate-900">Sign in</Link>
            <Link to={SIGNUP} className="text-sm font-semibold text-white bg-slate-900 hover:bg-slate-800 rounded-full px-4 py-2 transition">Start free</Link>
          </div>
        </div>
        {/* Amos page sections — a slim sub-nav under the main one. */}
        <div className="border-t border-slate-100 bg-white/60">
          <nav className="mx-auto max-w-7xl px-5 h-10 flex items-center gap-6 text-[13px] font-medium text-slate-500 overflow-x-auto">
            <span className="text-slate-400 uppercase tracking-wider text-[11px] shrink-0">Amos</span>
            <a href="#capabilities" className="hover:text-slate-900 whitespace-nowrap">Capabilities</a>
            <a href="#routines" className="hover:text-slate-900 whitespace-nowrap">Proactive by design</a>
            <a href="#trust" className="hover:text-slate-900 whitespace-nowrap">Trust &amp; control</a>
          </nav>
        </div>
      </header>

      <Hero />
      <Capabilities />
      <Routines />
      <HowItWorks />
      <Trust />
      <Cta />
      <MarketingFooter />
    </div>
  );
}

function Frame({ src, alt }: { src: string; alt: string }) {
  return (
    <div className="relative rounded-2xl border border-white/10 bg-slate-900/60 shadow-2xl shadow-indigo-950/50 overflow-hidden">
      <div className="flex items-center gap-1.5 px-4 h-9 bg-slate-800/80 border-b border-white/5">
        <span className="w-2.5 h-2.5 rounded-full bg-red-400/80" />
        <span className="w-2.5 h-2.5 rounded-full bg-amber-400/80" />
        <span className="w-2.5 h-2.5 rounded-full bg-emerald-400/80" />
        <span className="ml-3 text-[11px] text-slate-400">erp.zavora.ai/amos</span>
      </div>
      <img src={src} alt={alt} className="w-full block bg-slate-100" loading="lazy" onError={(e) => ((e.target as HTMLImageElement).style.opacity = '0')} />
    </div>
  );
}

function Hero() {
  return (
    <section className="relative overflow-hidden bg-slate-950 text-white">
      <div className="pointer-events-none absolute -top-40 left-1/2 -translate-x-1/2 h-[500px] w-[900px] rounded-full bg-gradient-to-r from-indigo-600/40 via-purple-600/40 to-fuchsia-600/30 blur-[120px]" />
      <div className="relative mx-auto max-w-7xl px-5 pt-20 pb-16 text-center">
        <div className="inline-flex items-center gap-2 rounded-full border border-white/15 bg-white/5 px-3.5 py-1.5 text-xs font-medium text-indigo-200 mb-7">
          <Bot className="w-3.5 h-3.5" /> Amos · the AI accountant inside Zavora ERP
        </div>
        <h1 className="mx-auto max-w-4xl text-4xl sm:text-6xl font-extrabold tracking-tight leading-[1.05]">
          The accountant that <span className="bg-gradient-to-r from-indigo-400 via-purple-400 to-fuchsia-400 bg-clip-text text-transparent">never sleeps</span> — built into your books.
        </h1>
        <p className="mx-auto max-w-2xl mt-6 text-lg text-slate-300">
          Amos AI Accountant lives inside your general ledger. It raises invoices, books bills, reconciles the bank,
          runs payroll, prepares your KRA returns and tables the monthly management accounts —
          and every posting waits for your yes.
        </p>
        <div className="mt-9 flex flex-col sm:flex-row items-center justify-center gap-3">
          <Link to={SIGNUP} className="group inline-flex items-center gap-2 rounded-full bg-white text-slate-900 font-semibold px-6 py-3.5 hover:bg-slate-100 transition">
            Try Amos free <ArrowRight className="w-4 h-4 group-hover:translate-x-0.5 transition" />
          </Link>
          <a href="#capabilities" className="inline-flex items-center gap-2 rounded-full border border-white/20 px-6 py-3.5 font-semibold hover:bg-white/5 transition">
            See what it can do
          </a>
        </div>
        <div className="relative mx-auto mt-14 max-w-5xl">
          <div className="absolute -inset-4 bg-gradient-to-r from-indigo-600/30 to-fuchsia-600/30 blur-2xl rounded-3xl" />
          <Frame src="/marketing/amos-chat.png" alt="Amos producing June management accounts with KPIs and variance commentary" />
        </div>
        <p className="mt-4 text-xs text-slate-400">Real product. Real ledger. The management pack above is what Amos tables on the 5th of every month.</p>
      </div>
    </section>
  );
}

const CAPS: { icon: any; title: string; items: string[] }[] = [
  { icon: FileText, title: 'Sell & get paid', items: [
    'Raises customer invoices with VAT-verified totals and KRA eTIMS transmission checks',
    'Reads any invoice, receipt or statement from a photo or PDF',
    'Monday chase list — then emails overdue customers their statements on your say-so',
  ]},
  { icon: Wallet, title: 'Buy & pay', items: [
    'Books supplier bills with duplicate checks and correct FX',
    'Runs the full procure-to-pay loop: requisition → LPO → goods receipt → 3-way match',
    'Prepares a cash-safe weekly payment run — statutory first, your approval always',
  ]},
  { icon: Landmark, title: 'Books & compliance', items: [
    'Bank & M-Pesa reconciliation: imports the statement, matches, locks — to the cent',
    'Payroll end-to-end with PAYE/NSSF/SHA/Housing Levy, and the KRA deadline calendar',
    'VAT, WHT and corporation-tax installments computed from the ledger, never guessed',
  ]},
  { icon: BarChart3, title: 'Insight', items: [
    'Monthly management accounts: vs budget, vs last month, vs last year — with commentary that names the driver',
    'KPIs computed, not estimated: DSO, DPO, margins, cash cover',
    '13-week cash forecast that flags the crunch week before it arrives',
  ]},
];

function Capabilities() {
  return (
    <section id="capabilities" className="mx-auto max-w-7xl px-5 py-20">
      <div className="text-center mb-12">
        <h2 className="text-3xl sm:text-4xl font-bold tracking-tight">A real accountant's job description</h2>
        <p className="mt-4 text-slate-600 max-w-2xl mx-auto">
          Sixteen playbooks cover the work an accountant actually does for a Kenyan business —
          each one a disciplined procedure with duplicate checks, confirmation gates and evidence.
        </p>
      </div>
      <div className="grid md:grid-cols-2 gap-6">
        {CAPS.map((c) => (
          <div key={c.title} className="rounded-2xl border border-slate-200 p-6 hover:shadow-lg hover:shadow-slate-100 transition">
            <div className="flex items-center gap-3 mb-4">
              <span className="flex h-10 w-10 items-center justify-center rounded-xl bg-indigo-50 text-indigo-600"><c.icon className="w-5 h-5" /></span>
              <h3 className="font-bold text-lg">{c.title}</h3>
            </div>
            <ul className="space-y-2.5">
              {c.items.map((i) => (
                <li key={i} className="flex items-start gap-2.5 text-sm text-slate-600">
                  <Check className="w-4 h-4 text-indigo-500 mt-0.5 shrink-0" />{i}
                </li>
              ))}
            </ul>
          </div>
        ))}
      </div>
      <div className="mt-10 flex flex-wrap items-center justify-center gap-x-8 gap-y-3 text-sm text-slate-500">
        <span className="inline-flex items-center gap-2"><Paperclip className="w-4 h-4" /> Reads documents</span>
        <span className="inline-flex items-center gap-2"><Globe className="w-4 h-4" /> Cited web research (KRA/CBK rates)</span>
        <span className="inline-flex items-center gap-2"><Brain className="w-4 h-4" /> Remembers your business across sessions</span>
        <span className="inline-flex items-center gap-2"><Camera className="w-4 h-4" /> Files screenshot evidence of its work</span>
        <span className="inline-flex items-center gap-2"><Sparkles className="w-4 h-4" /> Voice or chat</span>
      </div>
    </section>
  );
}

const ROUTINE_ROWS: [string, string, string][] = [
  ['Morning briefing', 'Every day, 7:00', 'Cash position, what’s due today, what went overdue'],
  ['eTIMS compliance sweep', 'Every day, 18:00', 'Finds untransmitted invoices, retries, reports to KRA-ready'],
  ['AR chase list', 'Mondays', 'Who owes you, how late, statements ready to send'],
  ['Payment-run proposal', 'Thursdays', 'A cash-safe batch of what to pay — statutory first'],
  ['Reconciliation check', 'Fridays', 'Which bank/M-Pesa accounts need reconciling'],
  ['PAYE & VAT prep', '5th & 14th', 'Figures staged ahead of the KRA 9th/20th deadlines'],
  ['Month-end close pack', '3rd', 'Trial-balance proof, statements, close checklist'],
  ['Management accounts', '5th', 'The board pack: variance, KPIs, commentary, cash outlook'],
  ['CIT installment check', '10th', 'Corporation-tax installments due by the 20th, estimated from the ledger'],
];

function Routines() {
  return (
    <section id="routines" className="relative bg-slate-950 text-white overflow-hidden">
      <div className="pointer-events-none absolute left-0 top-1/3 h-[400px] w-[500px] rounded-full bg-indigo-600/20 blur-[120px]" />
      <div className="relative mx-auto max-w-7xl px-5 py-20 grid lg:grid-cols-2 gap-14 items-center">
        <div>
          <div className="inline-flex items-center gap-2 rounded-full bg-indigo-500/15 text-indigo-300 px-3 py-1 text-xs font-semibold mb-5">
            <CalendarClock className="w-3.5 h-3.5" /> Ambient operations
          </div>
          <h2 className="text-3xl sm:text-4xl font-bold tracking-tight">Amos AI doesn't wait to be asked.</h2>
          <p className="mt-5 text-slate-300 text-lg">
            Amos moves first: it runs your accounting calendar — eleven scheduled routines — and
            reacts the moment something needs attention, delivering every report to your
            notification inbox before you thought to ask. Anything that posts money still waits for you.
          </p>
          <div className="mt-7 overflow-hidden rounded-xl border border-white/10">
            <table className="w-full text-sm">
              <tbody>
                {ROUTINE_ROWS.map(([name, when, what]) => (
                  <tr key={name} className="border-b border-white/5 last:border-0">
                    <td className="px-4 py-2.5 font-medium text-slate-100 whitespace-nowrap">{name}</td>
                    <td className="px-3 py-2.5 text-indigo-300 whitespace-nowrap text-xs">{when}</td>
                    <td className="px-4 py-2.5 text-slate-400 text-xs">{what}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="mt-4 text-sm text-slate-400">
            Run any routine on demand, pause the schedule for the holidays — and when an eTIMS
            transmission fails, the ERP wakes Amos to fix it within minutes.
          </p>
        </div>
        <div className="relative">
          <div className="absolute -inset-3 bg-gradient-to-tr from-indigo-600/40 to-fuchsia-600/30 blur-2xl rounded-3xl" />
          <Frame src="/marketing/amos-routines-full.png" alt="Amos routines panel: schedule, last outcomes and Run-now controls" />
        </div>
      </div>
    </section>
  );
}

const STEPS = [
  ['You ask — or the calendar fires', 'Type, talk, attach a document, or let a scheduled routine start the job.'],
  ['Amos plans in the open', 'A visible task list appears; it loads the matching playbook and works step by step.'],
  ['Nothing posts without you', 'Drafts are free; every ledger write stops for your explicit confirmation.'],
  ['Evidence, filed', 'It navigates the real ERP, screenshots the result, and keeps the conversation on record.'],
];

function HowItWorks() {
  return (
    <section className="mx-auto max-w-7xl px-5 py-20">
      <h2 className="text-3xl font-bold tracking-tight text-center mb-12">How a job gets done</h2>
      <div className="grid md:grid-cols-4 gap-6">
        {STEPS.map(([t, d], i) => (
          <div key={t} className="rounded-2xl border border-slate-200 p-6">
            <div className="w-8 h-8 rounded-full bg-slate-900 text-white text-sm font-bold flex items-center justify-center mb-4">{i + 1}</div>
            <h3 className="font-semibold mb-2">{t}</h3>
            <p className="text-sm text-slate-600">{d}</p>
          </div>
        ))}
      </div>
    </section>
  );
}

const TRUST = [
  ['Your approval gates every posting', 'Amos drafts freely but cannot write to the ledger, send a statement, or lock a period without an explicit yes.'],
  ['Scoped to your role, audited always', 'Sessions carry your ERP permissions — a viewer’s Amos cannot post. Every tool call lands in an audit trail.'],
  ['One tenant, one Amos', 'Each business gets its own isolated Amos: your ledger, your memory, your evidence — nobody else’s.'],
  ['Honest about its limits', 'iTax filing, moving money and CPA sign-off stay human. Amos prepares; you (or your accountant) execute — and it says so.'],
];

function Trust() {
  return (
    <section id="trust" className="bg-slate-50 border-y border-slate-100">
      <div className="mx-auto max-w-7xl px-5 py-20">
        <div className="text-center mb-12">
          <div className="inline-flex items-center gap-2 rounded-full bg-emerald-50 text-emerald-700 px-3 py-1 text-xs font-semibold mb-4">
            <ShieldCheck className="w-3.5 h-3.5" /> Built for financial trust
          </div>
          <h2 className="text-3xl font-bold tracking-tight">An AI accountant you can audit</h2>
        </div>
        <div className="grid md:grid-cols-2 gap-6 max-w-4xl mx-auto">
          {TRUST.map(([t, d]) => (
            <div key={t} className="rounded-2xl bg-white border border-slate-200 p-6">
              <h3 className="font-semibold mb-2 flex items-center gap-2"><Check className="w-4 h-4 text-emerald-600" />{t}</h3>
              <p className="text-sm text-slate-600">{d}</p>
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
      <h2 className="text-3xl sm:text-4xl font-bold tracking-tight">Your books, on autopilot — with you in command.</h2>
      <p className="mt-4 text-slate-600">Start free with sample data, or bring your real books. Amos is included in the Business plan.</p>
      <div className="mt-8 flex flex-col sm:flex-row items-center justify-center gap-3">
        <Link to={SIGNUP} className="inline-flex items-center gap-2 rounded-full bg-slate-900 text-white font-semibold px-7 py-3.5 hover:bg-slate-800 transition">
          Start free <ArrowRight className="w-4 h-4" />
        </Link>
        <a href="/#pricing" className="inline-flex items-center gap-2 rounded-full border border-slate-300 px-7 py-3.5 font-semibold text-slate-700 hover:bg-slate-50 transition">
          <Receipt className="w-4 h-4" /> See pricing
        </a>
      </div>
    </section>
  );
}
