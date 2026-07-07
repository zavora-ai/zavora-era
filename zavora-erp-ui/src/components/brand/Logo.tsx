// The official Zavora logo (red swirl mark + "zavora" wordmark) with an "ERP"
// suffix for the product name. Two knockouts live in /public:
//   zavora-logo.png        — full colour, for light surfaces
//   zavora-logo-white.png  — red mark + white wordmark, for dark surfaces
// (both white-background-removed from sample_data/zavora/logo).

export default function Logo({
  variant = 'dark',
  className = '',
  showWord = true,
}: {
  variant?: 'dark' | 'light';
  className?: string;
  showWord?: boolean; // show the "ERP" product suffix
}) {
  const src = variant === 'light' ? '/zavora-logo-white.png' : '/zavora-logo.png';
  const erp = variant === 'light' ? 'text-white' : 'text-[#38356e]';
  return (
    <span className={`inline-flex items-center gap-1.5 ${className}`}>
      <img src={src} alt="Zavora" className="h-6 w-auto shrink-0" />
      {showWord && <span className={`font-bold text-[15px] tracking-tight ${erp} relative -top-px`}>ERP</span>}
    </span>
  );
}
