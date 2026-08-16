// Single-colour inline SVG driven by currentColor, matching the window controls — no icon
// dependency, and crisp at every scale factor.
const STROKE = {
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.6,
  strokeLinecap: "round",
  strokeLinejoin: "round",
} as const;

interface IconProps {
  className?: string | undefined;
}

function Frame({ className, children }: IconProps & { children: React.ReactNode }) {
  return (
    <svg className={className} viewBox="0 0 24 24" aria-hidden="true">
      {children}
    </svg>
  );
}

export function SquarePlus({ className }: IconProps) {
  return (
    <Frame className={className}>
      <rect x="3.5" y="3.5" width="17" height="17" rx="2.5" {...STROKE} />
      <path d="M12 8v8M8 12h8" {...STROKE} />
    </Frame>
  );
}

export function SquareMinus({ className }: IconProps) {
  return (
    <Frame className={className}>
      <rect x="3.5" y="3.5" width="17" height="17" rx="2.5" {...STROKE} />
      <path d="M8 12h8" {...STROKE} />
    </Frame>
  );
}

export function Trash({ className }: IconProps) {
  return (
    <Frame className={className}>
      <path d="M4 7h16M10 4h4M6 7l1 13h10l1-13M10 11v6M14 11v6" {...STROKE} />
    </Frame>
  );
}

export function Pencil({ className }: IconProps) {
  return (
    <Frame className={className}>
      <path d="M4 20h4L19 9a2.8 2.8 0 0 0-4-4L4 16v4Z" {...STROKE} />
      <path d="M13.5 6.5 17.5 10.5" {...STROKE} />
    </Frame>
  );
}

export function ArrowBadgeUp({ className }: IconProps) {
  return (
    <Frame className={className}>
      <path d="M5 15l7-7 7 7" {...STROKE} />
    </Frame>
  );
}
