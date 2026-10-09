import { useEffect, useMemo, useState } from "react";
import type { AppData } from "../App";
import { backend, errorCode } from "../api/backend";
import type { BridgeResult, CivilizationState, Decision, Hoi4Summary, Ideology, Indicator, Reason, StellarisCompare, StellarisVocabulary } from "../api/types";
import { errorText, useI18n, type Key } from "../i18n";

const GROUPS: [Key, Indicator[]][] = [
  ["br.politics", ["authoritarianism", "social_equality", "xenophobia", "religious_influence", "militarism"]],
  ["br.world", ["international_cooperation", "global_wars", "nuclear_weapons", "planetary_unification"]],
  ["br.economy", ["industrialization", "technology", "space_program", "economic_planning", "environmental_policy"]],
];
const IDEOLOGIES: Ideology[] = ["democracy", "communism", "fascism", "monarchy", "non_aligned"];
const DEFAULT_SEED = 45819283;

export function Bridge({ app }: { app: AppData }) {
  const { t, lang } = useI18n();
  const [presets, setPresets] = useState<[string, CivilizationState][]>([]);
  const [vocab, setVocab] = useState<StellarisVocabulary | null>(null);
  const [civ, setCiv] = useState<CivilizationState | null>(null);
  const [seed, setSeed] = useState(String(DEFAULT_SEED));
  const [result, setResult] = useState<BridgeResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [actual, setActual] = useState<StellarisCompare | null>(null);
  const [hoi4, setHoi4] = useState<Hoi4Summary | null>(null);
  const fail = (e: unknown) => app.notify(errorText(t, errorCode(e)), "error");

  useEffect(() => {
    backend.bridgePresets().then((p) => {
      setPresets(p);
      if (p[0]) setCiv(structuredClone(p[0][1]));
    }).catch(fail);
    backend.bridgeVocabulary().then(setVocab).catch(fail);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const seedNum = /^\d{1,15}$/.test(seed.trim()) ? Number(seed.trim()) : null;

  async function simulate() {
    if (!civ || seedNum === null) return;
    setBusy(true);
    try {
      setResult(await backend.bridgeRun(civ, seedNum));
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  }

  function set(k: Indicator, v: number) {
    if (!civ) return;
    setCiv({ ...civ, values: { ...civ.values, [k]: v } });
    setResult(null);
  }

  async function commit() {
    if (!civ || seedNum === null) return;
    try {
      app.notify(t("br.committed", { n: await backend.bridgeCommit(civ, seedNum) }));
    } catch (e) {
      fail(e);
    }
  }

  async function exportCard() {
    if (!civ || seedNum === null) return;
    const folder = await backend.pickFolder();
    if (!folder) return;
    try {
      app.notify(t("br.exported", { path: await backend.bridgeExport(civ, seedNum, folder, lang) }));
    } catch (e) {
      fail(e);
    }
  }

  async function loadHoi4() {
    const path = await backend.pickFile();
    if (!path) return;
    try {
      const r = await backend.hoi4ReadWorld(path);
      setCiv(r.state);
      setHoi4(r.summary);
      setResult(null);
    } catch (e) {
      fail(e);
    }
  }

  async function compareSave() {
    const path = await backend.pickFile();
    if (!path) return;
    try {
      setActual(await backend.stellarisReadEmpire(path, result ? civ : null, result ? seedNum : null));
    } catch (e) {
      fail(e);
    }
  }

  if (!civ) return <section className="page"><p className="empty">{t("common.loading")}</p></section>;

  return (
    <section className="page page-wide">
      <h1>{t("br.title")}</h1>
      <p className="lede">{t("br.lede")}</p>
      <p className="hint">{hoi4 ? t("br.sourceHoi4") : t("br.source")}</p>
      <div className="actions compact">
        <button className="primary" onClick={loadHoi4}>{t("br.loadHoi4")}</button>
      </div>
      {hoi4 && (
        <div className="notice">
          <p>{t("br.hoi4Loaded", { version: hoi4.version ?? "?", date: hoi4.date ?? "?", n: civ.known_indicators })}</p>
          <p className="muted">
            {t("br.hoi4Powers", { list: hoi4.top_powers.slice(0, 5).map(([tag, w]) => `${tag} ${Math.round(w * 100)}%`).join(", ") })}
            {hoi4.factions.length > 0 && ` ${t("br.hoi4Factions", { list: hoi4.factions.map(([n, w]) => `${n} ${Math.round(w * 100)}%`).join(", ") })}`}
            {hoi4.countries_at_war.length > 0 && ` ${t("br.hoi4Wars", { list: hoi4.countries_at_war.join(" ") })}`}
          </p>
          {hoi4.missing.length > 0 && <p className="hint">{t("br.hoi4Missing", { list: hoi4.missing.map((k) => t(`ind.${k}` as Key)).join(", ") })}</p>}
        </div>
      )}

      <div className="row wrap examples">
        <span className="muted">{t("br.presets")}:</span>
        {presets.map(([name, p]) => (
          <button key={name} className="chip" onClick={() => { setCiv(structuredClone(p)); setResult(null); }}>
            {t(`preset.${name}` as Key)}
          </button>
        ))}
      </div>

      <div className="bridge-grid">
        {GROUPS.map(([title, keys]) => (
          <fieldset key={title} className="group">
            <legend>{t(title)}</legend>
            {keys.map((k) => (
              <label key={k} className="slider">
                <span>{t(`ind.${k}` as Key)} <output>{Math.round(civ.values[k] * 100)}%</output></span>
                <input type="range" min={0} max={100} step={5} value={Math.round(civ.values[k] * 100)}
                       onChange={(e) => set(k, Number(e.target.value) / 100)} />
              </label>
            ))}
          </fieldset>
        ))}
        <fieldset className="group">
          <legend>{t("br.world")}</legend>
          <label className="inline-field">{t("br.blocs")}
            <select value={civ.blocs} onChange={(e) => { setCiv({ ...civ, blocs: Number(e.target.value) }); setResult(null); }}>
              {[1, 2, 3, 4, 5, 6].map((n) => <option key={n} value={n}>{n}</option>)}
            </select>
          </label>
          <label className="inline-field">{t("br.ideology")}
            <select value={civ.ideology} onChange={(e) => { setCiv({ ...civ, ideology: e.target.value as Ideology }); setResult(null); }}>
              {IDEOLOGIES.map((i) => <option key={i} value={i}>{t(`ideo.${i}` as Key)}</option>)}
            </select>
          </label>
          <label className="inline-field">{t("br.seed")}
            <input className="short" value={seed} inputMode="numeric" onChange={(e) => { setSeed(e.target.value); setResult(null); }} />
          </label>
        </fieldset>
      </div>

      <div className="actions">
        <button className="primary" onClick={simulate} disabled={busy || seedNum === null}>
          {busy ? t("br.running") : t("br.run")}
        </button>
      </div>

      {result && vocab && <ResultView result={result} vocab={vocab} />}

      {result && (
        <div className="actions">
          <button onClick={commit} disabled={!app.boot.current_campaign}>{t("br.commit")}</button>
          <button onClick={exportCard}>{t("br.export")}</button>
          {!app.boot.current_campaign && <span className="hint">{t("br.noCampaign")}</span>}
        </div>
      )}

      <h2>{t("st.title")}</h2>
      <p className="hint">{t("st.hint")}</p>
      <div className="actions"><button onClick={compareSave}>{t("st.pick")}</button></div>
      {actual && vocab && <ActualEmpire data={actual} vocab={vocab} />}
    </section>
  );
}

function ResultView({ result, vocab }: { result: BridgeResult; vocab: StellarisVocabulary }) {
  const { t } = useI18n();
  const d = result.design;
  const ethicName = (e: string) => {
    const base = e.replace(/^fanatic_/, "");
    const name = vocab.ethics[base] ?? base;
    return e.startsWith("fanatic_") ? `${t("br.fanatic")} ${name}` : name;
  };
  const display: Record<Decision["part"], string> = {
    authority: vocab.authorities[d.authority] ?? d.authority,
    ethics: d.ethics.map(ethicName).join(", "),
    civics: d.civics.map((c) => vocab.civics[c] ?? c).join(", "),
    origin: vocab.origins[d.origin] ?? d.origin,
  };

  return (
    <>
      <h2>{t("br.empire")}</h2>
      {result.problems.length > 0 && <p className="notice is-warning">{t("br.invalid", { problems: result.problems.join("; ") })}</p>}
      <div className="empire">
        {d.decisions.map((dec) => (
          <article key={dec.part} className="empire-part">
            <p className="eyebrow">{t(`part.${dec.part}` as Key)}</p>
            <p className="empire-choice">{display[dec.part]}</p>
            <p className="muted">
              <span className={`pill review-${dec.review_state}`}>{t(`review.${dec.review_state}` as Key)}</span>{" "}
              {t("br.confidence", { value: Math.round(dec.confidence * 100) })}
            </p>
            <ul className="reasons">
              {dec.reasons.map((r, i) => <li key={i}><ReasonText r={r} vocab={vocab} /></li>)}
            </ul>
            {dec.alternative && <p className="hint">{t("br.closeCall", { alt: dec.part === "ethics" ? ethicName(dec.alternative) : vocab.civics[dec.alternative] ?? dec.alternative })}</p>}
          </article>
        ))}
      </div>
      <p className="hint">{t("br.namesNote")}</p>

      <h2>{t("br.history")}</h2>
      <ol className="journal">
        {result.run.events.map((e, i) => (
          <li key={i} className={`journal-row${e.kind === "nuclear_war" || e.kind === "ecological_collapse" ? " is-alarm" : ""}`}>
            <time className="journal-date">{t("br.decade", { year: e.year })}</time>
            <p className="journal-type">{t(`bev.${e.kind}` as Key)}</p>
          </li>
        ))}
      </ol>

      <h2>{t("br.change")}</h2>
      <Changes result={result} />
    </>
  );
}

function ReasonText({ r, vocab }: { r: Reason; vocab: StellarisVocabulary }) {
  const { t } = useI18n();
  const value = r.value === null ? "" : r.code.startsWith("ethic.") ? (r.value >= 0 ? "+" : "") + r.value.toFixed(2) : r.value.toFixed(2);
  const axis = r.detail && r.code.startsWith("ethic.") ? t(`axis.${r.detail}` as Key) : "";
  const civic = r.detail && r.code === "civic.score" ? vocab.civics[r.detail] ?? r.detail : "";
  const detail = r.code === "authority.adjusted_for_ethics" && r.detail ? vocab.authorities[r.detail] ?? r.detail : r.detail ?? "";
  const key = `reason.${r.code}` as Key;
  return <>{t(key, { value, axis, civic, detail })}</>;
}

function Changes({ result }: { result: BridgeResult }) {
  const { t } = useI18n();
  const rows = useMemo(
    () => (Object.keys(result.start.values) as Indicator[]).map((k) => [k, result.start.values[k], result.run.final_state.values[k]] as const),
    [result],
  );
  return (
    <div className="table-wrap">
      <table>
        <tbody>
          {rows.map(([k, a, b]) => (
            <tr key={k}>
              <td>{t(`ind.${k}` as Key)}</td>
              <td className="num">{Math.round(a * 100)}%</td>
              <td className="num">{Math.round(b * 100)}%</td>
              <td className={b > a + 0.05 ? "delta up" : b < a - 0.05 ? "delta down" : "delta"}>
                {b > a + 0.05 ? "▲" : b < a - 0.05 ? "▼" : "·"}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function ActualEmpire({ data, vocab }: { data: StellarisCompare; vocab: StellarisVocabulary }) {
  const { t } = useI18n();
  const e = data.empire;
  const cmp = data.comparison;
  const ethic = (x: string) => {
    const base = x.replace(/^fanatic_/, "");
    const name = vocab.ethics[base] ?? base;
    return x.startsWith("fanatic_") ? `${t("br.fanatic")} ${name}` : name;
  };
  const mark = (ok: boolean | undefined) => (cmp ? (ok ? " ✓" : " ✗") : "");
  return (
    <div className="empire-part actual">
      <p className="eyebrow">{e.name_is_literal ? e.name : `${e.name} (${t("st.locKey")})`}</p>
      <p className="muted">{t("st.meta", { version: e.game_version ?? "?", date: e.date ?? "?", ironman: e.ironman ? t("st.yes") : t("st.no") })}</p>
      <dl className="facts">
        <dt>{t("part.authority")}</dt>
        <dd>{(e.authority && vocab.authorities[e.authority]) ?? e.raw_authority}{mark(cmp?.authority_matches)}</dd>
        <dt>{t("part.ethics")}</dt>
        <dd>{e.ethics.map(ethic).join(", ")}</dd>
        <dt>{t("part.civics")}</dt>
        <dd>{e.civics.map((c) => vocab.civics[c] ?? c).join(", ")}</dd>
        <dt>{t("part.origin")}</dt>
        <dd>{(e.origin && vocab.origins[e.origin]) ?? e.raw_origin}{mark(cmp?.origin_matches)}</dd>
      </dl>
      {cmp && (
        <p className="notice">
          {t("st.match", { value: Math.round(cmp.score * 100) })}
          {cmp.shared_ethics.length > 0 && ` ${t("st.sharedEthics", { list: cmp.shared_ethics.map(ethic).join(", ") })}`}
          {cmp.shared_civics.length > 0 && ` ${t("st.sharedCivics", { list: cmp.shared_civics.map((c) => vocab.civics[c] ?? c).join(", ") })}`}
        </p>
      )}
      <p className="hint">{t("st.raw")}: <code>{[e.raw_authority, ...e.raw_ethics, ...e.raw_civics, e.raw_origin].filter(Boolean).join(" ")}</code></p>
    </div>
  );
}
