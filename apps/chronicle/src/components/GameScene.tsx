// Procedural scenes drawn by Chronicle — no game artwork. Each game has its own skyline
// (castles, temples, harbour, factories, ruined city, modern skyline, ringed planet) over a
// sky whose colours come from the game theme (CSS variables set by [data-game]).
// Deterministic: the same seed always draws the same picture.

import { useId, useMemo } from "react";

function rng(seed: number) {
  let a = seed >>> 0 || 1;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

type Rand = () => number;
const W = 1200;
const H = 240;

function castles(r: Rand, base: number, scale: number): string {
  let d = "";
  let x = -20;
  while (x < W + 40) {
    const w = (18 + r() * 26) * scale;
    const h = (40 + r() * 90) * scale;
    const top = base - h;
    d += `M${x} ${base}V${top}`;
    if (r() < 0.5) {
      d += `L${x + w / 2} ${top - (26 + r() * 30) * scale}L${x + w} ${top}`;
    } else {
      const m = w / 5;
      for (let i = 0; i < 5; i++) d += `H${x + m * i + m / 2}V${top - 7 * scale}H${x + m * (i + 1)}V${top}`;
    }
    d += `V${base}Z`;
    const wall = (30 + r() * 70) * scale;
    const wh = (16 + r() * 22) * scale;
    d += `M${x + w} ${base}V${base - wh}H${x + w + wall}V${base}Z`;
    x += w + wall;
  }
  return d;
}

function temples(r: Rand, base: number, scale: number): string {
  let d = "";
  let x = -10;
  while (x < W + 40) {
    if (r() < 0.55) {
      const cols = 4 + Math.floor(r() * 5);
      const cw = 9 * scale;
      const gap = 12 * scale;
      const width = cols * (cw + gap) + gap;
      const h = (60 + r() * 40) * scale;
      d += `M${x} ${base}V${base - 8 * scale}H${x + width}V${base}Z`;
      for (let i = 0; i < cols; i++) {
        const cx = x + gap + i * (cw + gap);
        d += `M${cx} ${base - 8 * scale}V${base - h}H${cx + cw}V${base - 8 * scale}Z`;
      }
      d += `M${x - 6} ${base - h}H${x + width + 6}V${base - h - 10 * scale}H${x - 6}Z`;
      d += `M${x - 6} ${base - h - 10 * scale}L${x + width / 2} ${base - h - 40 * scale}L${x + width + 6} ${base - h - 10 * scale}Z`;
      x += width + 30 * scale;
    } else {
      const h = (50 + r() * 70) * scale;
      d += `M${x} ${base}C${x - 4 * scale} ${base - h * 0.6} ${x + 4 * scale} ${base - h} ${x + 7 * scale} ${base - h}C${x + 10 * scale} ${base - h} ${x + 18 * scale} ${base - h * 0.6} ${x + 14 * scale} ${base}Z`;
      x += 22 * scale;
    }
  }
  return d;
}

function harbour(r: Rand, base: number, scale: number): string {
  let d = `M0 ${base}V${base - 14 * scale}`;
  for (let x = 0; x <= W; x += 40) d += `L${x} ${base - (14 + r() * 26) * scale}`;
  d += `L${W} ${base}Z`;
  for (let i = 0; i < 4; i++) {
    const x = 80 + r() * (W - 160);
    const s = (0.7 + r() * 0.6) * scale;
    const y = base - 4;
    d += `M${x - 46 * s} ${y - 14 * s}L${x + 46 * s} ${y - 14 * s}L${x + 34 * s} ${y}L${x - 34 * s} ${y}Z`;
    for (const [mx, mh] of [[-20, 70], [4, 92], [26, 64]] as const) {
      const px = x + mx * s;
      d += `M${px} ${y - 14 * s}V${y - mh * s}`;
      d += `M${px + 1} ${y - (mh - 6) * s}Q${px + 22 * s} ${y - (mh - 26) * s} ${px + 1} ${y - 22 * s}Z`;
    }
  }
  return d;
}

function factories(r: Rand, base: number, scale: number): string {
  let d = "";
  let x = -10;
  while (x < W + 40) {
    const w = (60 + r() * 90) * scale;
    const h = (30 + r() * 40) * scale;
    d += `M${x} ${base}V${base - h}`;
    const teeth = Math.max(2, Math.round(w / (22 * scale)));
    const tw = w / teeth;
    for (let i = 0; i < teeth; i++) d += `L${x + tw * i + tw * 0.75} ${base - h - 14 * scale}V${base - h}H${x + tw * (i + 1)}`;
    d += `V${base}Z`;
    if (r() < 0.8) {
      const cx = x + w * (0.2 + r() * 0.6);
      const ch = (60 + r() * 80) * scale;
      d += `M${cx} ${base - h}V${base - h - ch}H${cx + 9 * scale}V${base - h}Z`;
    }
    x += w + (8 + r() * 20) * scale;
  }
  return d;
}

function ruins(r: Rand, base: number, scale: number): string {
  let d = "";
  let x = -10;
  while (x < W + 40) {
    const w = (26 + r() * 50) * scale;
    const h = (30 + r() * 100) * scale;
    d += `M${x} ${base}V${base - h * (0.6 + r() * 0.3)}L${x + w * 0.3} ${base - h}L${x + w * 0.55} ${base - h * (0.75 + r() * 0.2)}L${x + w} ${base - h * (0.5 + r() * 0.4)}V${base}Z`;
    x += w + r() * 10 * scale;
  }
  return d;
}

function skyline(r: Rand, base: number, scale: number): string {
  let d = "";
  let x = -10;
  while (x < W + 40) {
    const w = (20 + r() * 40) * scale;
    const h = (40 + r() * 140) * scale;
    d += `M${x} ${base}V${base - h}H${x + w * 0.5}V${base - h - (r() < 0.3 ? 30 * scale : 0)}H${x + w * 0.55}V${base - h}H${x + w}V${base}Z`;
    x += w + r() * 6 * scale;
  }
  return d;
}

function hills(r: Rand, base: number, amp: number): string {
  let d = `M0 ${base}`;
  for (let x = 0; x <= W; x += 60) d += `L${x} ${base - amp * (0.3 + r() * 0.7)}`;
  return `${d}L${W} ${H}L0 ${H}Z`;
}

const SILHOUETTE: Record<string, (r: Rand, base: number, scale: number) => string> = {
  imperator: temples,
  ck3: castles,
  eu5: harbour,
  eu4: harbour,
  victoria3: factories,
  hoi4: ruins,
  modern_bridge: skyline,
};

const NIGHT = new Set(["hoi4", "modern_bridge", "stellaris"]);

export type SceneVariant = "banner" | "thumb" | "detail";

export function GameScene({ game, seed = 1, variant = "banner", className }: { game: string; seed?: number; variant?: SceneVariant; className?: string }) {
  const id = useId().replace(/:/g, "");
  const shapes = useMemo(() => {
    const r = rng(seed * 2654435761 + game.length * 97);
    const sun = { x: 120 + r() * (W - 240), y: 40 + r() * 70, r: 26 + r() * 30 };
    const stars = Array.from({ length: NIGHT.has(game) ? 70 : 0 }, () => [r() * W, r() * H * 0.7, r() * 1.4 + 0.3] as const);
    if (game === "stellaris") {
      return {
        sun,
        stars: Array.from({ length: 140 }, () => [r() * W, r() * H, r() * 1.6 + 0.2] as const),
        far: "",
        near: "",
        planet: { x: 200 + r() * 800, y: 150 + r() * 60, r: 110 + r() * 60 },
      };
    }
    const fn = SILHOUETTE[game] ?? castles;
    return {
      sun,
      stars,
      far: `${hills(r, 190, 40)} ${fn(r, 200, 0.7)}`,
      near: `${fn(r, 240, 1)}`,
      planet: null,
    };
  }, [game, seed]);

  const sky = `sky-${id}`;
  const glow = `glow-${id}`;
  const fog = `fog-${id}`;
  return (
    <svg className={`scene scene-${variant} ${className ?? ""}`} viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="xMidYMid slice" aria-hidden="true">
      <defs>
        <linearGradient id={sky} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" style={{ stopColor: "var(--sky-top)" }} />
          <stop offset=".62" style={{ stopColor: "var(--sky-mid)" }} />
          <stop offset="1" style={{ stopColor: "var(--sky-low)" }} />
        </linearGradient>
        <radialGradient id={glow}>
          <stop offset="0" style={{ stopColor: "var(--sun)", stopOpacity: 0.95 }} />
          <stop offset=".35" style={{ stopColor: "var(--sun)", stopOpacity: 0.35 }} />
          <stop offset="1" style={{ stopColor: "var(--sun)", stopOpacity: 0 }} />
        </radialGradient>
        <linearGradient id={fog} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" style={{ stopColor: "var(--sky-low)", stopOpacity: 0 }} />
          <stop offset="1" style={{ stopColor: "var(--sky-low)", stopOpacity: 0.55 }} />
        </linearGradient>
      </defs>
      <rect width={W} height={H} fill={`url(#${sky})`} />
      {shapes.stars.map(([x, y, s], i) => (
        <circle key={i} cx={x} cy={y} r={s} style={{ fill: "var(--star)" }} opacity={0.4 + (i % 5) * 0.12} />
      ))}
      <circle cx={shapes.sun.x} cy={shapes.sun.y} r={shapes.sun.r * 4} fill={`url(#${glow})`} />
      <circle cx={shapes.sun.x} cy={shapes.sun.y} r={shapes.sun.r * 0.45} style={{ fill: "var(--sun)" }} opacity=".9" />
      {shapes.planet && (
        <g>
          <circle cx={shapes.planet.x} cy={shapes.planet.y} r={shapes.planet.r} style={{ fill: "var(--far)" }} />
          <circle cx={shapes.planet.x - shapes.planet.r * 0.25} cy={shapes.planet.y - shapes.planet.r * 0.3} r={shapes.planet.r * 0.9} style={{ fill: "var(--sky-mid)" }} opacity=".35" />
          <ellipse cx={shapes.planet.x} cy={shapes.planet.y} rx={shapes.planet.r * 1.8} ry={shapes.planet.r * 0.32} fill="none" style={{ stroke: "var(--sun)" }} strokeWidth="3" opacity=".55" transform={`rotate(-12 ${shapes.planet.x} ${shapes.planet.y})`} />
        </g>
      )}
      {shapes.far && <path d={shapes.far} style={{ fill: "var(--far)" }} />}
      <rect y={H * 0.55} width={W} height={H * 0.45} fill={`url(#${fog})`} />
      {shapes.near && <path d={shapes.near} style={{ fill: "var(--near)" }} />}
    </svg>
  );
}
