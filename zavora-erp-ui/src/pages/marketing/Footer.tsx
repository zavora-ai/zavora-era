import { Link } from 'react-router-dom';
import Logo from '../../components/brand/Logo';

type L = { label: string; to: string; hash?: boolean };
const PRODUCT: L[] = [
  { label: 'Amos AI', to: '/amos-ai' },
  { label: 'Point of Sale', to: '/#modules', hash: true },
  { label: 'Invoicing', to: '/#modules', hash: true },
  { label: 'Payroll', to: '/#modules', hash: true },
  { label: 'Reports', to: '/#modules', hash: true },
  { label: 'Pricing', to: '/#pricing', hash: true },
];
const COMPANY: L[] = [
  { label: 'About', to: '/about' },
      { label: "What's new", to: '/updates' },
  { label: 'Careers', to: '/careers' },
  { label: 'Contact', to: '/contact' },
];
const LEGAL: L[] = [
  { label: 'Privacy', to: '/privacy' },
  { label: 'Terms', to: '/terms' },
  { label: 'Security', to: '/security' },
  { label: 'Data Protection', to: '/data-protection' },
];

function FooterLink({ l }: { l: L }) {
  const cls = 'hover:text-white transition';
  return l.hash ? (
    <a href={l.to} className={cls}>{l.label}</a>
  ) : (
    <Link to={l.to} className={cls}>{l.label}</Link>
  );
}

function Col({ title, links }: { title: string; links: L[] }) {
  return (
    <div>
      <h4 className="text-white font-semibold text-sm mb-3">{title}</h4>
      <ul className="space-y-2 text-sm">
        {links.map((l) => <li key={l.label}><FooterLink l={l} /></li>)}
      </ul>
    </div>
  );
}

export default function MarketingFooter() {
  return (
    <footer className="bg-slate-950 text-slate-400">
      <div className="mx-auto max-w-7xl px-5 py-14 grid sm:grid-cols-2 lg:grid-cols-4 gap-10">
        <div>
          <Logo variant="light" />
          <p className="mt-4 text-sm max-w-xs">The AI-native business platform for Kenyan SMEs. Sales, stock, payroll, tax — and Amos.</p>
          <p className="mt-4 text-sm">
            <a href="mailto:hello@zavora.ai" className="hover:text-white transition">hello@zavora.ai</a>
          </p>
        </div>
        <Col title="Product" links={PRODUCT} />
        <Col title="Company" links={COMPANY} />
        <Col title="Legal" links={LEGAL} />
      </div>
      <div className="border-t border-white/5">
        <div className="mx-auto max-w-7xl px-5 py-6 flex flex-col sm:flex-row items-center justify-between gap-3 text-xs">
          <span>© {new Date().getFullYear()} Zavora Technologies Ltd. All rights reserved.</span>
          <span>Made in Kenya 🇰🇪</span>
        </div>
      </div>
    </footer>
  );
}
