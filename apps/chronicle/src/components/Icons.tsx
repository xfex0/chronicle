// Original line icons (24×24, stroke = currentColor). No game logos: each game gets its own
// emblem drawn for Chronicle (crown, laurel, compass, cog, helmet, bridge, ringed planet).

import type { ReactElement } from "react";

const P = (d: string) => <path d={d} />;

const ICONS: Record<string, ReactElement> = {
  // navigation
  dashboard: <>{P("M3 11.5 12 4l9 7.5")}{P("M5.5 10v9.5h13V10")}{P("M10 19.5v-5h4v5")}</>,
  games: <>{P("M4 21V4")}{P("M4 5h11l-2 3.5L15 12H4")}</>,
  campaign: <>{P("M5 5l14 14")}{P("M19 5 5 19")}{P("M3.5 8.5 8.5 3.5")}{P("M15.5 20.5l5-5")}</>,
  timeline: <><circle cx="12" cy="12" r="8.5" />{P("M12 7.5V12l3 2")}</>,
  transition: <>{P("M12 3v13")}{P("M7 11l5 5 5-5")}{P("M5 20.5h14")}</>,
  bridge: <>{P("M3 18h18")}{P("M5 18c0-5 3-8 7-8s7 3 7 8")}{P("M12 10V5")}{P("M9 6.5h6")}</>,
  settings: <><circle cx="12" cy="12" r="3" />{P("M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M5.6 18.4l2.1-2.1M16.3 7.7l2.1-2.1")}</>,
  developer: <>{P("M8 8l-4 4 4 4")}{P("M16 8l4 4-4 4")}{P("M13.5 5l-3 14")}</>,
  // event categories
  rulers: <><circle cx="12" cy="8" r="3.5" />{P("M5 20c.8-4 3.6-6 7-6s6.2 2 7 6")}</>,
  realms: <>{P("M6 21V3.5")}{P("M6 4.5c3-1.5 5 1.5 8 0s4 0 4 0v8s-1-1.5-4 0-5-1.5-8 0")}</>,
  wars: <>{P("M4 4l9 9")}{P("M20 4l-9 9")}{P("M6.5 14.5 4 17l3 3 2.5-2.5")}{P("M17.5 14.5 20 17l-3 3-2.5-2.5")}</>,
  titles: <>{P("M4 17h16l1-9-5 4-4-6-4 6-5-4z")}{P("M4 20.5h16")}</>,
  diplomacy: <>{P("M3 12l4-4 5 3 5-3 4 4")}{P("M7 8v6l5 4 5-4V8")}</>,
  culture: <>{P("M12 3c-4 4-4 14 0 18")}{P("M12 3c4 4 4 14 0 18")}<circle cx="12" cy="12" r="9" /></>,
  religion: <>{P("M12 3v18")}{P("M7 8h10")}</>,
  economy: <><circle cx="12" cy="12" r="8.5" />{P("M14.5 9.5c-.5-1-1.5-1.5-2.5-1.5-1.4 0-2.5.8-2.5 2s1.1 1.6 2.5 2 2.5.9 2.5 2-1.1 2-2.5 2c-1.1 0-2-.5-2.5-1.5")}{P("M12 6.5v11")}</>,
  science: <><circle cx="12" cy="12" r="2" /><ellipse cx="12" cy="12" rx="9" ry="3.5" /><ellipse cx="12" cy="12" rx="9" ry="3.5" transform="rotate(60 12 12)" /><ellipse cx="12" cy="12" rx="9" ry="3.5" transform="rotate(120 12 12)" /></>,
  crises: <>{P("M12 3 2.5 20h19z")}{P("M12 9.5v5")}{P("M12 17.2v.1")}</>,
  world: <><circle cx="12" cy="12" r="8.5" />{P("M3.5 12h17")}{P("M12 3.5c3 3 3 14 0 17")}{P("M12 3.5c-3 3-3 14 0 17")}</>,
  other: <><circle cx="12" cy="12" r="1.2" /><circle cx="6" cy="12" r="1.2" /><circle cx="18" cy="12" r="1.2" /></>,
  // ui
  search: <><circle cx="10.5" cy="10.5" r="6" />{P("M15 15l5.5 5.5")}</>,
  calendar: <><rect x="3.5" y="5" width="17" height="15.5" rx="1.5" />{P("M3.5 9.5h17M8 3v4M16 3v4")}</>,
  moon: P("M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z"),
  sun: <><circle cx="12" cy="12" r="4" />{P("M12 2.5v2.5M12 19v2.5M2.5 12H5M19 12h2.5M5.3 5.3l1.8 1.8M16.9 16.9l1.8 1.8M5.3 18.7l1.8-1.8M16.9 7.1l1.8-1.8")}</>,
  // game emblems
  imperator: <>{P("M12 20c-5-2-7.5-6-7-12")}{P("M12 20c5-2 7.5-6 7-12")}{P("M6.5 11l-2-1M7 14.5l-2.2-.3M8.8 17.3l-1.8.8M17.5 11l2-1M17 14.5l2.2-.3M15.2 17.3l1.8.8")}</>,
  ck3: <>{P("M4 17h16l1-9-5 4-4-6-4 6-5-4z")}{P("M4 20.5h16")}</>,
  eu5: <><circle cx="12" cy="12" r="8.5" />{P("M12 5.5 14 12l-2 6.5-2-6.5z")}{P("M12 3.5v1.5M12 19v1.5M3.5 12H5M19 12h1.5")}</>,
  victoria3: <><circle cx="12" cy="12" r="3" />{P("M12 2.5l1.3 3 3.2-1 .3 3.3 3.2 1-1.6 2.9 1.6 2.9-3.2 1-.3 3.3-3.2-1-1.3 3-1.3-3-3.2 1-.3-3.3-3.2-1 1.6-2.9-1.6-2.9 3.2-1 .3-3.3 3.2 1z")}</>,
  hoi4: <>{P("M4 15c0-5 3.6-9 8-9s8 4 8 9")}{P("M2.5 15h19")}{P("M7 15v3h10v-3")}</>,
  modern_bridge: <>{P("M3 18h18")}{P("M5 18c0-5 3-8 7-8s7 3 7 8")}{P("M12 10V5")}{P("M9 6.5h6")}</>,
  stellaris: <><circle cx="12" cy="12" r="4.5" /><ellipse cx="12" cy="12" rx="10" ry="3" transform="rotate(-20 12 12)" /></>,
  eu4: <><circle cx="12" cy="12" r="8.5" />{P("M12 5.5 14 12l-2 6.5-2-6.5z")}</>,
};

export function Icon({ name, size = 20, className }: { name: string; size?: number; className?: string }) {
  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.6}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {ICONS[name] ?? ICONS.other}
    </svg>
  );
}

/** Chronicle's own mark: an hourglass. */
export function Hourglass({ size = 44 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 48 48" aria-hidden="true">
      <defs>
        <linearGradient id="hg-metal" x1="0" x2="1">
          <stop offset="0" stopColor="#8a6a2e" />
          <stop offset=".5" stopColor="#f0cf7c" />
          <stop offset="1" stopColor="#8a6a2e" />
        </linearGradient>
      </defs>
      <rect x="9" y="4" width="30" height="4" rx="1.5" fill="url(#hg-metal)" />
      <rect x="9" y="40" width="30" height="4" rx="1.5" fill="url(#hg-metal)" />
      <path d="M13 8c0 9 9 11 9 16s-9 7-9 16h22c0-9-9-11-9-16s9-7 9-16z" fill="rgba(240,207,124,.12)" stroke="#d9b45c" strokeWidth="1.5" />
      <path d="M17 12c2 4 7 6 7 9 0-3 5-5 7-9z" fill="#e3b85a" opacity=".85" />
      <path d="M16 39c1.5-4 5-6 8-6s6.5 2 8 6z" fill="#e3b85a" />
      <path d="M24 24v9" stroke="#e3b85a" strokeWidth="1" />
    </svg>
  );
}
