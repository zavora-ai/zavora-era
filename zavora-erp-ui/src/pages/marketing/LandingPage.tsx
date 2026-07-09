import { useState, useEffect, useRef } from 'react';
import { Link } from 'react-router-dom';
import {
  Sparkles, ArrowRight, Check, Menu, X, ShoppingCart, FileText, Boxes,
  Wallet, BarChart3, ShieldCheck, Landmark, Bot, Zap, Receipt, Building2,
  Paperclip, Camera,
} from 'lucide-react';
import Logo from '../../components/brand/Logo';
import MarketingFooter from './Footer';
import { PRICING_PLANS } from '../../config/pricing';

/** Where every "Start free" CTA points — the create-organization flow. */
const SIGNUP = '/login?signup=1';

/** Public marketing landing page shown to unauthenticated visitors. */
export default function LandingPage() {
  const [menu, setMenu] = useState(false);
  return (
    <div className="min-h-screen bg-white text-slate-900 antialiased selection:bg-indigo-200/60">
      <Nav menu={menu} setMenu={setMenu} />
      <Hero />
      <LogosStrip />
      <AmosSpotlight />
      <ModulesShowcase />
      <ComplianceBand />
      <Metrics />
      <Comparison />
      <Pricing />
      <FinalCta />
      <MarketingFooter />
    </div>
  );
}

/* ─────────────────────────────  NAV  ───────────────────────────── */
function Nav({ menu, setMenu }: { menu: boolean; setMenu: (v: boolean) => void }) {
  return (
    <header className="sticky top-0 z-50 backdrop-blur-lg bg-white/80 border-b border-slate-100">
      <div className="mx-auto max-w-7xl px-5 h-16 flex items-center justify-between">
        <a href="#top" className="flex items-center gap-2.5">
          <Logo />
        </a>
        <nav className="hidden md:flex items-center gap-8 text-sm font-medium text-slate-600">
          <Link to="/amos-ai" className="hover:text-slate-900">Amos AI</Link>
          <a href="#modules" className="hover:text-slate-900">Modules</a>
          <a href="#compliance" className="hover:text-slate-900">Kenya-ready</a>
          <a href="#pricing" className="hover:text-slate-900">Pricing</a>
        </nav>
        <div className="hidden md:flex items-center gap-3">
          <Link to="/login" className="text-sm font-semibold text-slate-700 hover:text-slate-900">Sign in</Link>
          <Link to={SIGNUP} className="text-sm font-semibold text-white bg-slate-900 hover:bg-slate-800 rounded-full px-4 py-2 transition">Start free</Link>
        </div>
        <button className="md:hidden p-2" onClick={() => setMenu(!menu)}>{menu ? <X /> : <Menu />}</button>
      </div>
      {menu && (
        <div className="md:hidden border-t border-slate-100 px-5 py-4 space-y-3 bg-white">
          <Link to="/amos-ai" className="block text-slate-700" onClick={() => setMenu(false)}>Amos AI</Link>
          <a href="#modules" className="block text-slate-700" onClick={() => setMenu(false)}>Modules</a>
          <a href="#pricing" className="block text-slate-700" onClick={() => setMenu(false)}>Pricing</a>
          <Link to="/login" className="block font-semibold text-indigo-600">Sign in →</Link>
        </div>
      )}
    </header>
  );
}

/* ─────────────────────────────  HERO  ───────────────────────────── */
function Hero() {
  return (
    <section id="top" className="relative overflow-hidden bg-slate-950 text-white">
      {/* glow */}
      <div className="pointer-events-none absolute -top-40 left-1/2 -translate-x-1/2 h-[500px] w-[900px] rounded-full bg-gradient-to-r from-indigo-600/40 via-purple-600/40 to-fuchsia-600/30 blur-[120px]" />
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_50%_-20%,rgba(99,102,241,0.25),transparent_60%)]" />
      <div className="relative mx-auto max-w-7xl px-5 pt-20 pb-16 text-center">
        <div className="inline-flex items-center gap-2 rounded-full border border-white/15 bg-white/5 px-3.5 py-1.5 text-xs font-medium text-indigo-200 mb-7">
          <Sparkles className="w-3.5 h-3.5" /> Meet Amos — the AI accountant built into your books
        </div>
        <h1 className="mx-auto max-w-4xl text-4xl sm:text-6xl font-extrabold tracking-tight leading-[1.05]">
          The ERP that <span className="bg-gradient-to-r from-indigo-400 via-purple-400 to-fuchsia-400 bg-clip-text text-transparent">runs your accounting</span> for you.
        </h1>
        <p className="mx-auto max-w-2xl mt-6 text-lg text-slate-300">
          Zavora ERP is a full business platform for Kenyan SMEs — invoicing, POS, procurement, payroll and reports —
          with <span className="text-white font-medium">Amos</span>, an AI accountant that posts, reconciles and files taxes while you run the business.
        </p>
        <div className="mt-9 flex flex-col sm:flex-row items-center justify-center gap-3">
          <Link to={SIGNUP} className="group inline-flex items-center gap-2 rounded-full bg-white text-slate-900 font-semibold px-6 py-3.5 hover:bg-slate-100 transition">
            Start free <ArrowRight className="w-4 h-4 group-hover:translate-x-0.5 transition" />
          </Link>
          <a href="#amos" className="inline-flex items-center gap-2 rounded-full border border-white/20 px-6 py-3.5 font-semibold hover:bg-white/5 transition">
            See Amos in action
          </a>
        </div>
        <p className="mt-4 text-xs text-slate-400">No card required · KRA eTIMS &amp; M-Pesa ready · Set up in minutes</p>

        {/* Hero shot */}
        <div className="relative mx-auto mt-14 max-w-5xl">
          <div className="absolute -inset-4 bg-gradient-to-r from-indigo-600/30 to-fuchsia-600/30 blur-2xl rounded-3xl" />
          <BrowserFrame src="/marketing/dashboard.png" alt="Zavora ERP dashboard" />
        </div>
      </div>
    </section>
  );
}

/* Chrome-style browser frame wrapper for screenshots. */
function BrowserFrame({ src, alt }: { src: string; alt: string }) {
  return (
    <div className="relative rounded-2xl border border-white/10 bg-slate-900/60 shadow-2xl shadow-indigo-950/50 overflow-hidden">
      <div className="flex items-center gap-1.5 px-4 h-9 bg-slate-800/80 border-b border-white/5">
        <span className="w-2.5 h-2.5 rounded-full bg-red-400/80" />
        <span className="w-2.5 h-2.5 rounded-full bg-amber-400/80" />
        <span className="w-2.5 h-2.5 rounded-full bg-emerald-400/80" />
        <span className="ml-3 text-[11px] text-slate-400">app.zavora.ai</span>
      </div>
      <img src={src} alt={alt} className="w-full block bg-slate-100" loading="lazy" onError={(e) => ((e.target as HTMLImageElement).style.opacity = '0')} />
    </div>
  );
}

/* ────────────────────────────  LOGOS  ──────────────────────────── */
function LogosStrip() {
  return (
    <section className="border-b border-slate-100 bg-white">
      <div className="mx-auto max-w-7xl px-5 py-8">
        <p className="text-center text-xs font-semibold tracking-widest text-slate-400 uppercase mb-5">Built for how business really runs in Kenya</p>
        <div className="flex flex-wrap items-center justify-center gap-x-10 gap-y-4 text-slate-500 font-semibold">
          <span className="inline-flex items-center gap-2"><Receipt className="w-4 h-4" /> KRA eTIMS</span>
          <span className="inline-flex items-center gap-2"><Wallet className="w-4 h-4" /> M-Pesa</span>
          <span className="inline-flex items-center gap-2"><Landmark className="w-4 h-4" /> PAYE · NSSF · SHA</span>
          <span className="inline-flex items-center gap-2"><Building2 className="w-4 h-4" /> Multi-branch</span>
          <span className="inline-flex items-center gap-2"><ShieldCheck className="w-4 h-4" /> Audit-ready</span>
        </div>
      </div>
    </section>
  );
}

/* ────────────────────────────  AMOS  ──────────────────────────── */
function AmosSpotlight() {
  const bullets = [
    'Records supplier bills and customer invoices from a sentence',
    'Reconciles the bank and M-Pesa till automatically',
    'Runs payroll and prepares KRA returns on the deadline calendar',
    'Acts before you ask — morning briefings, eTIMS sweeps, month-end packs, on schedule',
    'Answers "how is the business doing?" in plain language',
  ];
  return (
    <section id="amos" className="relative bg-slate-950 text-white overflow-hidden">
      <div className="pointer-events-none absolute right-0 top-1/2 -translate-y-1/2 h-[400px] w-[500px] rounded-full bg-indigo-600/20 blur-[120px]" />
      <div className="relative mx-auto max-w-7xl px-5 py-24 grid lg:grid-cols-2 gap-14 items-center">
        <div>
          <div className="inline-flex items-center gap-2 rounded-full bg-indigo-500/15 text-indigo-300 px-3 py-1 text-xs font-semibold mb-5">
            <Bot className="w-3.5 h-3.5" /> Your AI Accountant
          </div>
          <h2 className="text-3xl sm:text-4xl font-bold tracking-tight">Amos does the accounting. You run the business.</h2>
          <p className="mt-5 text-slate-300 text-lg">
            Amos lives inside your ledger — not a bolt-on chatbot. It sees every invoice, bill and payment, and
            takes action with your approval. A qualified accountant that never sleeps — already part of your ERP.
          </p>
          <ul className="mt-7 space-y-3">
            {bullets.map((b) => (
              <li key={b} className="flex items-start gap-3 text-slate-200">
                <span className="mt-0.5 flex h-5 w-5 items-center justify-center rounded-full bg-indigo-500/20 text-indigo-300"><Check className="w-3.5 h-3.5" /></span>
                {b}
              </li>
            ))}
          </ul>
          <div className="mt-8 flex flex-wrap items-center gap-4">
            <Link to={SIGNUP} className="inline-flex items-center gap-2 rounded-full bg-white text-slate-900 font-semibold px-6 py-3 hover:bg-slate-100 transition">
              Try Amos free <ArrowRight className="w-4 h-4" />
            </Link>
            <Link to="/amos-ai" className="inline-flex items-center gap-2 font-semibold text-indigo-300 hover:text-indigo-200 transition">
              Everything Amos can do <ArrowRight className="w-4 h-4" />
            </Link>
          </div>
        </div>
        <div className="relative">
          <div className="absolute -inset-3 bg-gradient-to-tr from-indigo-600/40 to-fuchsia-600/30 blur-2xl rounded-3xl" />
          <AmosDemo />
        </div>
      </div>
    </section>
  );
}

/* ── Animated Amos demo — auto-plays through real prompt scenarios ─────────── */
type Msg = { who: 'user' | 'amos'; node: React.ReactNode; attach?: { name: string } };
const SCENES: { chapter: string; messages: Msg[] }[] = [
  {
    chapter: 'Record a bill from a sentence',
    messages: [
      { who: 'user', node: 'Record the Brookside milk delivery — 40 crates at 320, dated today.' },
      { who: 'amos', node: (
        <>Done ✓ Posted <b>BILL-2026-0043</b> to Brookside Dairy — <b>KSh 14,848</b> (incl. 16% VAT).
          <div className="mt-2 rounded-lg border border-slate-200 bg-white p-2.5 text-[12px]">
            <div className="flex justify-between"><span className="text-slate-500">DR</span><span>Cost of Sales · 12,800</span></div>
            <div className="flex justify-between"><span className="text-slate-500">DR</span><span>VAT Input · 2,048</span></div>
            <div className="flex justify-between"><span className="text-slate-500">CR</span><span>Accounts Payable · 14,848</span></div>
          </div>
          Want me to schedule payment for the 30th?</>
      ) },
    ],
  },
  {
    chapter: 'Snap a receipt — Amos reads it',
    messages: [
      { who: 'user', node: 'Log this fuel expense please.', attach: { name: 'shell-receipt.jpg' } },
      { who: 'amos', node: (
        <>Read it 📸 — <b>Shell Westlands</b>, KSh 4,500 fuel on 6 Jul.
          <br />Posted to <b>Motor Vehicle — Fuel</b> (VAT input KSh 620) and attached the photo to the transaction for your records.</>
      ) },
    ],
  },
  {
    chapter: 'Instant answers, in plain English',
    messages: [
      { who: 'user', node: 'How did we do this month?' },
      { who: 'amos', node: (
        <>Revenue <b>KSh 33,030</b>, up <span className="text-emerald-600 font-medium">12%</span> on last month. Cash is healthy at KSh 110k.
          <br />⚠️ 5 invoices (<b>KSh 39,782</b>) are overdue — shall I send M-Pesa reminders?</>
      ) },
    ],
  },
  {
    chapter: 'Reconcile the M-Pesa till',
    messages: [
      { who: 'user', node: 'Reconcile yesterday’s M-Pesa till.' },
      { who: 'amos', node: (
        <>Matched <b>38 of 40</b> M-Pesa payments to open invoices and marked them paid.
          <br />2 payments need a quick look — want me to open them?</>
      ) },
    ],
  },
];

function AmosDemo() {
  const [scene, setScene] = useState(0);
  const [shown, setShown] = useState(0);
  const [typing, setTyping] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const msgs = SCENES[scene].messages;

  useEffect(() => {
    let t: ReturnType<typeof setTimeout>;
    if (shown < msgs.length) {
      const next = msgs[shown];
      if (next.who === 'amos') {
        setTyping(true);
        t = setTimeout(() => { setTyping(false); setShown((s) => s + 1); }, 1200);
      } else {
        t = setTimeout(() => setShown((s) => s + 1), shown === 0 ? 500 : 800);
      }
    } else {
      t = setTimeout(() => { setShown(0); setScene((s) => (s + 1) % SCENES.length); }, 3200);
    }
    return () => clearTimeout(t);
  }, [scene, shown, msgs.length]);

  useEffect(() => { scrollRef.current?.scrollTo({ top: 9999, behavior: 'smooth' }); }, [shown, typing]);

  return (
    <div className="relative rounded-2xl bg-white shadow-2xl shadow-indigo-950/40 overflow-hidden border border-white/10">
      <div className="flex items-center gap-3 px-4 h-14 bg-gradient-to-r from-indigo-600 to-purple-600 text-white">
        <div className="w-8 h-8 rounded-lg bg-white/20 flex items-center justify-center"><Sparkles className="w-4 h-4" /></div>
        <div className="flex-1"><p className="font-semibold text-sm leading-tight">Amos</p><p className="text-[11px] text-indigo-100 flex items-center gap-1"><span className="w-1.5 h-1.5 rounded-full bg-emerald-300 inline-block animate-pulse" /> AI Accountant · online</p></div>
        <span className="text-[10px] font-semibold text-white/70 bg-white/10 rounded-full px-2 py-1">LIVE DEMO</span>
      </div>
      {/* chapter label */}
      <div className="px-4 pt-3 pb-1 bg-slate-50 text-[11px] font-semibold uppercase tracking-wider text-indigo-500">{SCENES[scene].chapter}</div>
      <div ref={scrollRef} className="px-4 pb-4 pt-1 space-y-3 bg-slate-50 text-[13px] h-[380px] overflow-hidden">
        {msgs.slice(0, shown).map((m, i) => <Bubble key={i} who={m.who} attach={m.attach}>{m.node}</Bubble>)}
        {typing && <TypingBubble />}
      </div>
      {/* progress dots */}
      <div className="flex items-center justify-center gap-1.5 pb-2 bg-slate-50">
        {SCENES.map((_, i) => <span key={i} className={`h-1.5 rounded-full transition-all ${i === scene ? 'w-5 bg-indigo-600' : 'w-1.5 bg-slate-300'}`} />)}
      </div>
      <div className="flex items-center gap-2 px-4 py-3 border-t border-slate-100 bg-white">
        <Paperclip className="w-4 h-4 text-slate-400" />
        <div className="flex-1 rounded-full bg-slate-100 px-4 py-2 text-slate-400 text-[13px]">Ask Amos or attach a document…</div>
        <div className="w-8 h-8 rounded-full bg-indigo-600 flex items-center justify-center text-white"><ArrowRight className="w-4 h-4" /></div>
      </div>
    </div>
  );
}
function Bubble({ who, children, attach }: { who: 'user' | 'amos'; children: React.ReactNode; attach?: { name: string } }) {
  const user = who === 'user';
  return (
    <div className={`flex ${user ? 'justify-end' : 'justify-start'} animate-[fadeInUp_0.3s_ease]`}>
      <div className={`max-w-[86%] rounded-2xl px-3.5 py-2.5 ${user ? 'bg-indigo-600 text-white rounded-br-sm' : 'bg-white border border-slate-200 text-slate-700 rounded-bl-sm shadow-sm'}`}>
        {attach && (
          <div className={`mb-2 flex items-center gap-2 rounded-lg px-2.5 py-2 ${user ? 'bg-white/15' : 'bg-slate-100'}`}>
            <span className={`flex h-8 w-8 items-center justify-center rounded ${user ? 'bg-white/20' : 'bg-white'}`}><Camera className={`w-4 h-4 ${user ? 'text-white' : 'text-slate-500'}`} /></span>
            <span className="text-[12px] font-medium truncate">{attach.name}</span>
          </div>
        )}
        {children}
      </div>
    </div>
  );
}
function TypingBubble() {
  return (
    <div className="flex justify-start">
      <div className="rounded-2xl rounded-bl-sm bg-white border border-slate-200 px-4 py-3 shadow-sm">
        <div className="flex gap-1">
          <span className="w-1.5 h-1.5 rounded-full bg-slate-400 animate-bounce" style={{ animationDelay: '0ms' }} />
          <span className="w-1.5 h-1.5 rounded-full bg-slate-400 animate-bounce" style={{ animationDelay: '150ms' }} />
          <span className="w-1.5 h-1.5 rounded-full bg-slate-400 animate-bounce" style={{ animationDelay: '300ms' }} />
        </div>
      </div>
    </div>
  );
}

/* ───────────────────────────  MODULES  ─────────────────────────── */
const MODULES = [
  { icon: FileText, title: 'Invoicing & receivables', img: '/marketing/invoicing.png', color: 'text-blue-600',
    tag: 'Get paid faster',
    desc: 'Send professional, branded invoices in seconds and let Amos chase the money — so cash lands sooner.',
    points: [
      'KRA eTIMS tax invoices with automatic 16% VAT and WHT',
      'Recurring invoices, quotes/estimates and credit notes',
      'Customer statements, ageing and a self-service customer portal',
      'Automatic payment reminders over email &amp; M-Pesa',
    ] },
  { icon: ShoppingCart, title: 'Point of Sale', img: '/marketing/pos.png', color: 'text-emerald-600',
    tag: 'Sell anywhere',
    desc: 'A fast, mobile till that posts straight to your books — no end-of-day export, no reconciliation headache.',
    points: [
      'Cash &amp; M-Pesa tender with automatic change and receipts',
      'ETR/eTIMS thermal receipt with a KRA verification QR',
      'Shift sessions with a Z-report that reconciles the drawer',
      'Every sale updates stock and the ledger in real time',
    ] },
  { icon: Boxes, title: 'Inventory & procurement', img: '/marketing/procurement.png', color: 'text-amber-600',
    tag: 'Control every shilling',
    desc: 'From request to receipt — a full procure-to-pay flow with the controls that stop overspend and fraud.',
    points: [
      'Requisitions → tenders or POs → goods receipt → 3-way match',
      'Live stock levels, valuations and mobile stock counts',
      'Open-commitment register &amp; budget-vs-actual control',
      'Vendor portal, debit notes and approval spend-limits',
    ] },
  { icon: Wallet, title: 'Payroll — Kenya statutory', img: '/marketing/payroll.png', color: 'text-fuchsia-600',
    tag: 'Compliant, automatically',
    desc: 'Run payroll in minutes with every Kenyan deduction computed correctly and every return ready to file.',
    points: [
      'PAYE, NSSF, SHA, Housing Levy &amp; HELB — effective-dated rates',
      'Prepare → review → approve → post pay-run workflow',
      'Payslips, P9s, the statutory schedule and the bank/EFT file',
      'Leave, employee self-service and onboarding built in',
    ] },
  { icon: BarChart3, title: 'Reports & analytics', img: '/marketing/reports.png', color: 'text-indigo-600',
    tag: 'Always know where you stand',
    desc: 'Books that are always closed. Every statement is live — and Amos explains it in plain language.',
    points: [
      'Trial balance, P&amp;L, balance sheet, cash flow &amp; equity',
      'AR/AP ageing, general ledger and custom report builder',
      'Budgets, dimensions/cost-centres and multi-entity consolidation',
      'Scheduled reports emailed to you automatically',
    ] },
  { icon: Landmark, title: 'Banking & reconciliation', img: '/marketing/banking.png', color: 'text-cyan-600',
    tag: 'Close the books on autopilot',
    desc: 'Connect your bank and M-Pesa, and let Amos match the transactions so month-end takes minutes, not days.',
    points: [
      'Import statements (PDF/CSV) with smart transaction matching',
      'M-Pesa till payments captured and reconciled automatically',
      'Multi-currency accounts with automatic FX revaluation',
      'Complete-and-lock periods with a full audit trail',
    ] },
];

function ModulesShowcase() {
  return (
    <section id="modules" className="bg-slate-50">
      <div className="mx-auto max-w-7xl px-5 py-24">
        <div className="text-center max-w-2xl mx-auto">
          <p className="text-sm font-semibold text-indigo-600 uppercase tracking-widest">One platform</p>
          <h2 className="mt-2 text-3xl sm:text-4xl font-bold tracking-tight">Everything your business runs on</h2>
          <p className="mt-4 text-slate-600 text-lg">Replace the spreadsheets, the standalone POS and the shoebox of receipts. It's all here — and it all talks to your ledger.</p>
        </div>
        <div className="mt-14 space-y-20">
          {MODULES.map((m, i) => (
            <div key={m.title} className={`grid lg:grid-cols-2 gap-10 items-center ${i % 2 ? 'lg:[direction:rtl]' : ''}`}>
              <div className="[direction:ltr]">
                <div className={`inline-flex items-center justify-center w-11 h-11 rounded-xl bg-white shadow-sm border border-slate-100 ${m.color} mb-4`}><m.icon className="w-5 h-5" /></div>
                <p className={`text-xs font-bold uppercase tracking-widest ${m.color}`}>{m.tag}</p>
                <h3 className="mt-1 text-2xl font-bold tracking-tight">{m.title}</h3>
                <p className="mt-3 text-slate-600 text-lg">{m.desc}</p>
                <ul className="mt-5 space-y-2.5">
                  {m.points.map((p) => (
                    <li key={p} className="flex items-start gap-2.5 text-[15px] text-slate-700">
                      <span className={`mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-slate-100 ${m.color}`}><Check className="w-3 h-3" /></span>
                      <span dangerouslySetInnerHTML={{ __html: p }} />
                    </li>
                  ))}
                </ul>
              </div>
              <div className="[direction:ltr]">
                <div className="rounded-2xl border border-slate-200 shadow-xl shadow-slate-200/60 overflow-hidden bg-white">
                  <img src={m.img} alt={m.title} className="w-full block" loading="lazy" onError={(e) => ((e.target as HTMLImageElement).closest('div')!.style.display = 'none')} />
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

/* ─────────────────────────  COMPLIANCE  ────────────────────────── */
function ComplianceBand() {
  const items = [
    { icon: Receipt, t: 'KRA eTIMS', d: 'Tax invoices & POS receipts with control-unit QR — on the buy and sell side.' },
    { icon: Wallet, t: 'M-Pesa native', d: 'Till payments captured and reconciled straight into the ledger.' },
    { icon: Landmark, t: 'Statutory payroll', d: 'PAYE, NSSF, SHA, Housing Levy & HELB, effective-dated to the rates.' },
    { icon: ShieldCheck, t: 'Audit trail', d: 'Every posting attributed and immutable — approvals, limits, the lot.' },
  ];
  return (
    <section id="compliance" className="bg-white">
      <div className="mx-auto max-w-7xl px-5 py-20">
        <div className="text-center max-w-2xl mx-auto mb-12">
          <h2 className="text-3xl sm:text-4xl font-bold tracking-tight">Compliant in Kenya, out of the box</h2>
          <p className="mt-3 text-slate-600 text-lg">No plugins, no consultants. The rules are built in.</p>
        </div>
        <div className="grid sm:grid-cols-2 lg:grid-cols-4 gap-5">
          {items.map((it) => (
            <div key={it.t} className="rounded-2xl border border-slate-100 bg-slate-50/50 p-6">
              <div className="w-10 h-10 rounded-lg bg-indigo-600/10 text-indigo-600 flex items-center justify-center mb-4"><it.icon className="w-5 h-5" /></div>
              <h3 className="font-semibold text-lg">{it.t}</h3>
              <p className="mt-1.5 text-sm text-slate-600">{it.d}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

/* ──────────────────────────  METRICS  ─────────────────────────── */
function Metrics() {
  const stats = [
    { n: '1 platform', l: 'Sales, stock, payroll, tax & AI' },
    { n: '10+ hrs', l: 'saved on bookkeeping every week' },
    { n: 'Real-time', l: 'books that are always closed' },
    { n: '24/7', l: 'Amos never takes leave' },
  ];
  return (
    <section className="bg-slate-950 text-white">
      <div className="mx-auto max-w-7xl px-5 py-16 grid grid-cols-2 lg:grid-cols-4 gap-8 text-center">
        {stats.map((s) => (
          <div key={s.l}>
            <div className="text-3xl sm:text-4xl font-extrabold bg-gradient-to-r from-indigo-400 to-fuchsia-400 bg-clip-text text-transparent">{s.n}</div>
            <div className="mt-2 text-sm text-slate-400">{s.l}</div>
          </div>
        ))}
      </div>
    </section>
  );
}

/* ─────────────────────────  COMPARISON  ────────────────────────── */
type Cell = boolean | 'partial' | 'addon';
const COMPARE_COLS = ['QuickBooks', 'Xero', 'Sage'];
const COMPARE_ROWS: { feature: string; z: Cell; c: Cell[] }[] = [
  { feature: 'Built-in AI accountant (Amos)', z: true, c: [false, false, false] },
  { feature: 'KRA eTIMS tax invoices & receipts', z: true, c: ['partial', false, 'partial'] },
  { feature: 'M-Pesa payments, native', z: true, c: [false, false, false] },
  { feature: 'Kenyan statutory payroll (PAYE/NSSF/SHA/Housing/HELB)', z: true, c: [false, 'addon', 'addon'] },
  { feature: 'Point of Sale, built in', z: true, c: ['addon', false, 'addon'] },
  { feature: 'Procurement & 3-way match', z: true, c: [false, false, 'partial'] },
  { feature: 'Inventory & stock', z: true, c: [true, true, true] },
  { feature: 'Multi-currency & consolidation', z: true, c: [true, 'partial', true] },
  { feature: 'Made for Kenya out of the box', z: true, c: [false, false, 'partial'] },
];

function CompareMark({ v }: { v: Cell }) {
  if (v === true) return <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-emerald-100 text-emerald-600"><Check className="w-3.5 h-3.5" /></span>;
  if (v === 'partial') return <span className="text-[11px] font-medium text-amber-600">Partial</span>;
  if (v === 'addon') return <span className="text-[11px] font-medium text-slate-400">Add-on</span>;
  return <span className="inline-flex items-center justify-center w-6 h-6 rounded-full bg-slate-100 text-slate-300"><X className="w-3.5 h-3.5" /></span>;
}

function Comparison() {
  return (
    <section className="bg-white">
      <div className="mx-auto max-w-6xl px-5 py-24">
        <div className="text-center max-w-2xl mx-auto mb-12">
          <p className="text-sm font-semibold text-indigo-600 uppercase tracking-widest">Why Zavora</p>
          <h2 className="mt-2 text-3xl sm:text-4xl font-bold tracking-tight">One platform that does what four others can't</h2>
          <p className="mt-4 text-slate-600 text-lg">The global tools weren't built for Kenya — and none of them come with an accountant inside. Here's how we compare.</p>
        </div>
        <div className="overflow-x-auto rounded-2xl border border-slate-200 shadow-sm">
          <table className="w-full min-w-[640px] text-sm">
            <thead>
              <tr className="border-b border-slate-200">
                <th className="text-left font-semibold text-slate-500 px-5 py-4 w-[40%]">Capability</th>
                <th className="px-4 py-4">
                  <div className="inline-flex flex-col items-center">
                    <Logo className="scale-90" />
                    <span className="mt-1 text-[10px] font-bold text-indigo-600 uppercase tracking-wide">You are here</span>
                  </div>
                </th>
                {COMPARE_COLS.map((c) => <th key={c} className="px-4 py-4 text-slate-500 font-semibold">{c}</th>)}
              </tr>
            </thead>
            <tbody>
              {COMPARE_ROWS.map((r, i) => (
                <tr key={r.feature} className={i % 2 ? 'bg-slate-50/60' : ''}>
                  <td className="px-5 py-3.5 text-slate-700 font-medium">{r.feature}</td>
                  <td className="px-4 py-3.5 text-center bg-indigo-50/60 border-x border-indigo-100"><CompareMark v={r.z} /></td>
                  {r.c.map((v, j) => <td key={j} className="px-4 py-3.5 text-center"><CompareMark v={v} /></td>)}
                </tr>
              ))}
              <tr className="border-t-2 border-slate-200">
                <td className="px-5 py-4 text-slate-700 font-semibold">Typical SME price / month</td>
                <td className="px-4 py-4 text-center bg-indigo-50/60 border-x border-indigo-100 font-bold text-indigo-700">KSh 6,900</td>
                <td className="px-4 py-4 text-center text-slate-500">~KSh 6,500</td>
                <td className="px-4 py-4 text-center text-slate-500">~KSh 5,900</td>
                <td className="px-4 py-4 text-center text-slate-500">KSh 7,500+</td>
              </tr>
            </tbody>
          </table>
        </div>
        <p className="mt-3 text-center text-xs text-slate-400">Competitor pricing and capabilities are indicative, based on publicly available Kenya plans, and payroll/POS often require paid add-ons or separate products.</p>
      </div>
    </section>
  );
}

/* ──────────────────────────  PRICING  ─────────────────────────── */
function Pricing() {
  const tiers = PRICING_PLANS;
  return (
    <section id="pricing" className="bg-slate-50">
      <div className="mx-auto max-w-7xl px-5 py-24">
        <div className="text-center max-w-2xl mx-auto mb-12">
          <h2 className="text-3xl sm:text-4xl font-bold tracking-tight">Simple pricing that scales with you</h2>
          <p className="mt-3 text-slate-600 text-lg">Start free. Upgrade when you're ready. Cancel anytime.</p>
        </div>
        <div className="grid md:grid-cols-2 lg:grid-cols-4 gap-6 items-start">
          {tiers.map((t) => (
            <div key={t.name} className={`rounded-3xl p-7 border ${t.highlight ? 'border-indigo-600 bg-white shadow-2xl shadow-indigo-200/60 relative lg:-mt-4' : 'border-slate-200 bg-white'}`}>
              {t.highlight && <span className="absolute -top-3 left-1/2 -translate-x-1/2 bg-indigo-600 text-white text-xs font-semibold px-3 py-1 rounded-full">Most popular</span>}
              <p className="text-sm font-semibold text-slate-500">{t.tag}</p>
              <h3 className="mt-1 text-xl font-bold">{t.name}</h3>
              <div className="mt-4 flex items-end gap-1"><span className="text-4xl font-extrabold">{t.price}</span>{t.per && <span className="text-slate-500 mb-1">{t.per}</span>}</div>
              <ul className="mt-6 space-y-3">
                {t.features.map((f) => <li key={f} className="flex items-start gap-2.5 text-sm text-slate-700"><Check className="w-4 h-4 text-indigo-600 mt-0.5 shrink-0" /> {f}</li>)}
              </ul>
              <Link to={t.cta === 'Contact sales' ? '/contact' : `${SIGNUP}&plan=${t.key}`} className={`mt-8 block text-center rounded-full font-semibold py-3 transition ${t.highlight ? 'bg-indigo-600 text-white hover:bg-indigo-500' : 'bg-slate-900 text-white hover:bg-slate-800'}`}>{t.cta}</Link>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

/* ──────────────────────────  FINAL CTA  ────────────────────────── */
function FinalCta() {
  return (
    <section className="bg-white">
      <div className="mx-auto max-w-7xl px-5 py-20">
        <div className="relative overflow-hidden rounded-3xl bg-gradient-to-br from-indigo-600 via-purple-600 to-fuchsia-600 px-8 py-16 text-center text-white">
          <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_30%_20%,rgba(255,255,255,0.2),transparent_50%)]" />
          <Zap className="relative w-10 h-10 mx-auto mb-4" />
          <h2 className="relative text-3xl sm:text-4xl font-bold tracking-tight">Put your books on autopilot.</h2>
          <p className="relative mt-4 text-indigo-100 text-lg max-w-xl mx-auto">Join Kenyan businesses running sales, stock, payroll and tax on one platform — with Amos doing the heavy lifting.</p>
          <Link to={SIGNUP} className="relative mt-8 inline-flex items-center gap-2 rounded-full bg-white text-indigo-700 font-semibold px-7 py-3.5 hover:bg-slate-100 transition">
            Start free today <ArrowRight className="w-4 h-4" />
          </Link>
        </div>
      </div>
    </section>
  );
}

