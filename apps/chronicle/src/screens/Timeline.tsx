import { Fragment, useEffect, useMemo, useRef, useState } from "react";
import type { AppData } from "../App";
import { backend, errorCode } from "../api/backend";
import type { EventRow } from "../api/types";
import { GameScene } from "../components/GameScene";
import { Icon } from "../components/Icons";
import { errorText, formatDate, useI18n, type Key } from "../i18n";

type Category = "rulers" | "realms" | "wars" | "titles" | "diplomacy" | "culture" | "religion" | "economy" | "science" | "crises" | "other";
const CATEGORIES: Category[] = ["rulers", "realms", "wars", "titles", "diplomacy", "culture", "religion", "economy", "science", "crises", "other"];

const CATEGORY_OF: Record<string, Category> = {
  ruler_changed: "rulers",
  title_created: "titles",
  title_destroyed: "titles",
  war_started: "wars",
  war_ended: "wars",
  conquest: "wars",
  civil_war: "wars",
  nuclear_war: "crises",
  campaign_started: "realms",
  world_snapshot: "realms",
  earth_unified: "realms",
  capital_changed: "realms",
  border_change: "realms",
  blocs_merged: "diplomacy",
  climate_crisis: "crises",
  ecological_collapse: "crises",
  space_orbit: "science",
  space_moon: "science",
  space_mars: "science",
  space_interstellar: "science",
  stellaris_empire_designed: "science",
  era_transition: "realms",
  era_skipped: "realms",
  territory_changed_owner: "realms",
};
const categoryOf = (e: EventRow): Category => CATEGORY_OF[e.event_type] ?? "other";
const PAGE = 120;
type T = ReturnType<typeof useI18n>["t"];

function yearOf(d: string | null | undefined): number {
  const n = Number(String(d ?? "").split(".")[0]);
  return Number.isFinite(n) ? n : 0;
}

export function Timeline({ app }: { app: AppData }) {
  const { t, lang } = useI18n();
  const campaign = app.boot.current_campaign;
  const registry = app.boot.registry;
  const [events, setEvents] = useState<EventRow[] | null>(null);
  const [game, setGame] = useState<string>("");
  const [cat, setCat] = useState<Category | "all">("all");
  const [query, setQuery] = useState("");
  const [range, setRange] = useState<[number, number] | null>(null);
  const [selected, setSelected] = useState<number | null>(null);
  const [shown, setShown] = useState(PAGE);
  const [jumpOpen, setJumpOpen] = useState(false);
  const [jumpYear, setJumpYear] = useState("");
  const listRef = useRef<HTMLOListElement>(null);

  useEffect(() => {
    if (!campaign) return;
    backend.events(0, null).then(setEvents).catch((e) => app.notify(errorText(t, errorCode(e)), "error"));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [campaign?.id, campaign?.current_date]);

  // games present in the journal, in chain order
  const present = useMemo(() => new Set((events ?? []).map((e) => e.game)), [events]);
  // every era of the campaign chain gets a tab (dimmed while it has no events yet)
  const games = useMemo(
    () => [...registry.chain, ...Object.keys(registry.games)].filter((g, i, a) => a.indexOf(g) === i && (registry.chain.includes(g) || present.has(g))),
    [present, registry.chain, registry.games],
  );

  // open on the campaign's current game when it has events
  const [initialised, setInitialised] = useState(false);
  useEffect(() => {
    if (initialised || events === null) return;
    if (campaign && games.includes(campaign.current_game)) setGame(campaign.current_game);
    setInitialised(true);
  }, [events, games, campaign, initialised]);

  const byGame = useMemo(() => (events ?? []).filter((e) => !game || e.game === game), [events, game]);
  const bounds = useMemo<[number, number]>(() => {
    const def = game ? registry.games[game] : undefined;
    const ys = byGame.map((e) => yearOf(e.date));
    const lo = Math.min(def ? yearOf(def.era.start) : Infinity, ...(ys.length ? ys : [0]));
    const hi = Math.max(def?.era.end ? yearOf(def.era.end) : -Infinity, ...(ys.length ? ys : [1]));
    return [lo, hi > lo ? hi : lo + 1];
  }, [byGame, game, registry.games]);
  const [from, to] = range ?? bounds;

  useEffect(() => {
    setRange(null);
    setSelected(null);
    setShown(PAGE);
    setCat("all");
  }, [game]);

  const texts = useMemo(() => new Map(byGame.map((e) => [e.event_id, eventText(e, t)])), [byGame, t]);
  const q = query.trim().toLowerCase();
  const inRange = useMemo(
    () => byGame.filter((e) => {
      const y = yearOf(e.date);
      return y >= from && y <= to;
    }),
    [byGame, from, to],
  );
  const counts = useMemo(() => {
    const c: Partial<Record<Category, number>> = {};
    for (const e of inRange) c[categoryOf(e)] = (c[categoryOf(e)] ?? 0) + 1;
    return c;
  }, [inRange]);
  const visible = inRange.filter((e) => {
    if (cat !== "all" && categoryOf(e) !== cat) return false;
    if (!q) return true;
    const d = texts.get(e.event_id);
    return `${d?.title} ${d?.subject}`.toLowerCase().includes(q);
  });
  const current = visible.find((e) => e.event_id === selected) ?? visible[0] ?? null;
  const theme = game || campaign?.current_game || "ck3";
  const def = registry.games[theme];

  // density histogram under the era slider
  const buckets = useMemo(() => {
    const n = 90;
    const arr = new Array<number>(n).fill(0);
    const [lo, hi] = bounds;
    for (const e of byGame) {
      const i = Math.min(n - 1, Math.max(0, Math.floor(((yearOf(e.date) - lo) / (hi - lo)) * n)));
      arr[i] = (arr[i] ?? 0) + 1;
    }
    const max = Math.max(1, ...arr);
    return arr.map((v) => v / max);
  }, [byGame, bounds]);
  const ticks = useMemo(() => {
    const [lo, hi] = bounds;
    return Array.from({ length: 7 }, (_, i) => Math.round(lo + ((hi - lo) * i) / 6));
  }, [bounds]);
  const pct = (y: number) => ((y - bounds[0]) / (bounds[1] - bounds[0])) * 100;

  function jump() {
    const y = Number(jumpYear);
    if (!jumpYear.trim() || !Number.isFinite(y)) return;
    const clamped = Math.min(Math.max(y, bounds[0]), bounds[1]);
    setRange([clamped, Math.max(to, clamped)]);
    setJumpOpen(false);
    listRef.current?.scrollTo({ top: 0 });
  }

  if (!campaign) {
    return (
      <section className="page">
        <h1>{t("tl.title")}</h1>
        <p className="empty">{t("tl.noCampaign")}</p>
        <div className="actions"><button className="primary" onClick={() => app.go("campaign")}>{t("tl.openCampaign")}</button></div>
      </section>
    );
  }

  return (
    <section className="chronicle" data-game={theme}>
      <header className="era-banner">
        <GameScene game={theme} seed={7} variant="banner" />
        <div className="era-banner-content">
          <div className="era-emblem"><Icon name={theme} size={40} /></div>
          <div>
            <h1 className="era-title">{game ? def?.display_name ?? game : t("tl.allGames")}</h1>
            <p className="era-years">
              {game && def ? `${formatDate(lang, def.era.start)} – ${def.era.end ? formatDate(lang, def.era.end) : "…"}` : `${bounds[0]} – ${bounds[1]}`}
            </p>
          </div>
          <div className="era-actions">
            <button className="ghost" onClick={() => setJumpOpen((o) => !o)} aria-expanded={jumpOpen}>
              <Icon name="calendar" size={18} /> {t("tl.jump")}
            </button>
            {jumpOpen && (
              <div className="jump-pop">
                <input autoFocus inputMode="numeric" aria-label={t("tl.jump")} placeholder={String(bounds[0])} value={jumpYear}
                       onChange={(e) => setJumpYear(e.target.value)} onKeyDown={(e) => e.key === "Enter" && jump()} />
                <button onClick={jump}>{t("tl.go")}</button>
              </div>
            )}
          </div>
        </div>
        <nav className="game-tabs" aria-label={t("tl.game")}>
          <button className={!game ? "is-active" : ""} onClick={() => setGame("")}>{t("tl.allGames")}</button>
          {games.map((g) => (
            <button key={g} className={`${g === game ? "is-active" : ""} ${present.has(g) ? "" : "is-empty"}`} title={present.has(g) ? undefined : t("tl.eraEmpty")} onClick={() => setGame(g)}>
              <Icon name={g} size={16} /> {registry.games[g]?.display_name ?? g}
            </button>
          ))}
        </nav>
      </header>

      <div className="era-slider">
        <div className="density" aria-hidden="true">
          {buckets.map((v, i) => <span key={i} style={{ height: `${v ? Math.max(v * 100, 10) : 0}%` }} />)}
        </div>
        <div className="range-track">
          <span className="range-fill" style={{ left: `${pct(from)}%`, right: `${100 - pct(to)}%` }} />
          {current && <span className="range-marker" style={{ left: `${pct(yearOf(current.date))}%` }} />}
          <input type="range" min={bounds[0]} max={bounds[1]} value={from} aria-label={t("tl.from")}
                 onChange={(e) => setRange([Math.min(Number(e.target.value), to), to])} />
          <input type="range" min={bounds[0]} max={bounds[1]} value={to} aria-label={t("tl.to")}
                 onChange={(e) => setRange([from, Math.max(Number(e.target.value), from)])} />
        </div>
        <div className="ticks">{ticks.map((y, i) => <span key={`${y}-${i}`}>{y}</span>)}</div>
        {range && <button className="link" onClick={() => setRange(null)}>{t("tl.resetRange", { from, to })}</button>}
      </div>

      <div className="filters">
        <button className={`chip-cat ${cat === "all" ? "is-active" : ""}`} onClick={() => setCat("all")}>
          <Icon name="world" size={16} /> {t("cat.all")} <span className="count">{inRange.length}</span>
        </button>
        {CATEGORIES.filter((c) => counts[c]).map((c) => (
          <button key={c} className={`chip-cat cat-${c} ${cat === c ? "is-active" : ""}`} onClick={() => setCat(c)}>
            <Icon name={c} size={16} /> {t(`cat.${c}` as Key)} <span className="count">{counts[c]}</span>
          </button>
        ))}
        <label className="search">
          <Icon name="search" size={16} />
          <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t("tl.search")} aria-label={t("tl.search")} />
        </label>
      </div>

      {events === null ? (
        <p className="empty pad">{t("common.loading")}</p>
      ) : visible.length === 0 ? (
        <p className="empty pad">{events.length === 0 ? t("tl.empty") : t("tl.nothingHere")}</p>
      ) : (
        <div className="chronicle-body">
          <ol className="event-list" ref={listRef}>
            {visible.slice(0, shown).map((e) => {
              const c = categoryOf(e);
              const d = texts.get(e.event_id) ?? eventText(e, t);
              return (
                <li key={e.event_id} className={`event-row cat-${c}`}>
                  <span className="event-dot" />
                  <time className="event-date">
                    {formatDate(lang, e.date)}
                    {e.date_to ? <small>–{formatDate(lang, e.date_to)}</small> : null}
                  </time>
                  <button className={`event-card ${current?.event_id === e.event_id ? "is-selected" : ""}`} data-game={e.game}
                          onClick={() => setSelected(e.event_id)}>
                    <span className="event-icon"><Icon name={c} size={22} /></span>
                    <span className="event-text">
                      <span className="event-title">{d.title}</span>
                      {d.subject && <span className="event-subject">{d.subject}</span>}
                      <span className="event-meta">{sourceLabel(e, t, registry.games[e.game]?.display_name)}</span>
                    </span>
                    <GameScene game={e.game} seed={e.event_id} variant="thumb" />
                  </button>
                </li>
              );
            })}
            {visible.length > shown && (
              <li className="more"><button onClick={() => setShown((s) => s + PAGE)}>{t("tl.more", { n: visible.length - shown })}</button></li>
            )}
          </ol>
          {current && <EventDetail e={current} all={byGame} onPick={setSelected} app={app} />}
        </div>
      )}
    </section>
  );
}

interface EventText {
  title: string;
  subject: string;
  sentence: string;
}

function sourceLabel(e: EventRow, t: T, gameName?: string): string {
  if (e.origin === "game") return t("tl.scripted");
  if (e.evidence === "inferred") return t("tl.inferred");
  if (e.game === "modern_bridge") return t("tl.simulated");
  return gameName ?? e.game;
}

function eventText(e: EventRow, t: T): EventText {
  const p = (e.payload ?? {}) as Record<string, unknown>;
  const s = (v: unknown) => (typeof v === "string" ? v : "");
  const title = (k: string) => t(`evt.${k}` as Key);
  switch (e.event_type) {
    case "ruler_changed":
    case "title_created":
    case "title_destroyed": {
      const name = s(p.title_name) || s(p.title);
      return { title: title(e.event_type), subject: name, sentence: t(`ev.${e.event_type}` as Key, { title: name }) };
    }
    case "war_started": {
      const a = e.actor_name ?? s(p.attacker);
      const b = e.target_name ?? s(p.defender);
      return { title: title("war_started"), subject: `${a} – ${b}`, sentence: t("ev.war_started", { a, b }) };
    }
    case "world_snapshot": {
      const top = Array.isArray(p.top_powers)
        ? (p.top_powers as [string, number][]).slice(0, 3).map(([tag, w]) => `${tag} ${Math.round(w * 100)}%`).join(", ")
        : "";
      return { title: title("world_snapshot"), subject: top, sentence: t("ev.world_snapshot", { top }) };
    }
    case "campaign_started":
      return { title: title("campaign_started"), subject: s(p.name), sentence: t("ev.campaign_started", { name: s(p.name) }) };
    case "stellaris_empire_designed":
      return { title: title("stellaris_empire_designed"), subject: "", sentence: t("ev.stellaris_empire_designed") };
    case "era_transition":
    case "era_skipped":
      return { title: title(e.event_type), subject: `${s(p.from)} → ${s(p.to)}`, sentence: title(e.event_type) };
    default:
      if (e.game === "modern_bridge") {
        const label = t(`bev.${e.event_type}` as Key);
        return { title: label, subject: "", sentence: label };
      }
      return { title: e.event_type.replaceAll("_", " "), subject: e.actor_name ?? "", sentence: e.event_type.replaceAll("_", " ") };
  }
}

function EventDetail({ e, all, onPick, app }: { e: EventRow; all: EventRow[]; onPick: (id: number) => void; app: AppData }) {
  const { t, lang } = useI18n();
  const [tab, setTab] = useState<"details" | "related" | "provenance">("details");
  const d = eventText(e, t);
  const c = categoryOf(e);
  const p = (e.payload ?? {}) as Record<string, unknown>;
  const related = all.filter((x) => {
    if (x.event_id === e.event_id) return false;
    if (e.actor_entity_id && (x.actor_entity_id === e.actor_entity_id || x.target_entity_id === e.actor_entity_id)) return true;
    return typeof p.title === "string" && (x.payload as Record<string, unknown> | null)?.title === p.title;
  });
  const level = e.importance >= 4 ? "high" : e.importance >= 3 ? "medium" : "low";
  const details = Object.entries(p).filter(([k, v]) => k !== "key" && v !== null && typeof v !== "object");

  return (
    <aside className={`event-detail cat-${c}`} data-game={e.game} aria-live="polite">
      <header className="detail-head">
        <span className="event-icon big"><Icon name={c} size={28} /></span>
        <div className="detail-names">
          <h2>{d.title}</h2>
          {d.subject && <p className="detail-subject">{d.subject}</p>}
        </div>
        <div className="detail-when">
          <time>{formatDate(lang, e.date)}{e.date_to ? `–${formatDate(lang, e.date_to)}` : ""}</time>
          <span className={`importance imp-${level}`}>{t(`impl.${level}` as Key)}</span>
        </div>
      </header>
      <GameScene game={e.game} seed={e.event_id * 7 + 3} variant="detail" />
      <p className="detail-sentence">{d.sentence}. {sourceSentence(e, t)}</p>
      <dl className="detail-facts">
        <dt>{t("tl.type")}</dt><dd><Icon name={c} size={16} /> {t(`cat.${c}` as Key)}</dd>
        <dt>{t("tl.game")}</dt><dd><Icon name={e.game} size={16} /> {app.boot.registry.games[e.game]?.display_name ?? e.game}</dd>
        {e.actor_name && (<><dt>{t("tl.actor")}</dt><dd>{e.actor_name}<small>{e.actor_entity_id}</small></dd></>)}
        {e.target_name && (<><dt>{t("tl.target")}</dt><dd>{e.target_name}<small>{e.target_entity_id}</small></dd></>)}
        <dt>{t("tl.date")}</dt><dd>{formatDate(lang, e.date)} <small>{t(`prec.${e.date_precision}` as Key)}</small></dd>
        <dt>{t("tl.eventId")}</dt><dd>event_{String(e.event_id).padStart(6, "0")}</dd>
      </dl>
      <div className="tabs" role="tablist">
        {(["details", "related", "provenance"] as const).map((k) => (
          <button key={k} role="tab" aria-selected={tab === k} className={tab === k ? "is-active" : ""} onClick={() => setTab(k)}>
            {k === "related" ? t("tl.tab.related", { n: related.length }) : t(`tl.tab.${k}` as Key)}
          </button>
        ))}
      </div>
      <div className="tab-body" role="tabpanel">
        {tab === "details" &&
          (details.length ? (
            <dl className="detail-facts small">
              {details.map(([k, v]) => (
                <Fragment key={k}>
                  <dt>{k.replaceAll("_", " ")}</dt>
                  <dd>{String(v)}</dd>
                </Fragment>
              ))}
            </dl>
          ) : (
            <p className="muted">{t("tl.noDetails")}</p>
          ))}
        {tab === "related" &&
          (related.length ? (
            <ul className="related">
              {related.slice(0, 40).map((x) => {
                const xt = eventText(x, t);
                return (
                  <li key={x.event_id}>
                    <button onClick={() => onPick(x.event_id)}>
                      <time>{formatDate(lang, x.date)}</time> {xt.title} <span className="muted">{xt.subject}</span>
                    </button>
                  </li>
                );
              })}
            </ul>
          ) : (
            <p className="muted">{t("tl.noRelated")}</p>
          ))}
        {tab === "provenance" && (
          <dl className="detail-facts small">
            <dt>{t("tl.origin")}</dt><dd>{t(`origin.${e.origin}` as Key)}</dd>
            <dt>{t("tl.evidence")}</dt><dd>{t(`evidence.${e.evidence}` as Key)}</dd>
            <dt>{t("tl.precision")}</dt><dd>{t(`prec.${e.date_precision}` as Key)}</dd>
            {typeof p.key === "string" && (<><dt>{t("tl.sourceKey")}</dt><dd><code>{p.key}</code></dd></>)}
          </dl>
        )}
      </div>
    </aside>
  );
}

function sourceSentence(e: EventRow, t: T): string {
  if (e.origin === "game") return t("src.game");
  if (e.evidence === "inferred") return t("src.inferred");
  if (e.game === "modern_bridge" || e.origin === "converter") return t("src.converter");
  return t("src.save");
}
