import { useEffect, useMemo, useState } from "react";
import type { AppData } from "../App";
import { backend, errorCode } from "../api/backend";
import type { Aspect, Check, EntityLabel, OwnershipPeriod, Period, QueryResult } from "../api/types";
import { errorText, formatDate, useI18n } from "../i18n";

const ASPECTS: Aspect[] = ["government", "ruler", "dynasty", "capital", "religion", "primary_culture", "overlord"];

const EXAMPLES: [string, string][] = [
  ["Ownership", "SELECT t.name, o.owner_id, o.from_year, o.to_year\nFROM territory_ownership o JOIN territories t ON t.entity_id = o.territory_id\nORDER BY t.name, o.from_key"],
  ["Periods", "SELECT entity_id, aspect, COALESCE(value, value_entity_id) AS value, from_year, to_year\nFROM entity_periods ORDER BY entity_id, aspect, from_key"],
  ["Why?", "SELECT s.entity_id, s.semantic_key, s.value, s.confidence, s.review_state, p.formula_id, p.contributions\nFROM semantic_values s JOIN provenance p ON p.id = s.provenance_id"],
  ["Events", "SELECT year, month, day, event_type, actor_entity_id, importance FROM events ORDER BY year, month, day"],
];

export function Developer({ app }: { app: AppData }) {
  const { t } = useI18n();
  const campaign = app.boot.current_campaign;
  const [labels, setLabels] = useState<EntityLabel[]>([]);
  const fail = (e: unknown) => app.notify(errorText(t, errorCode(e)), "error");

  const refreshLabels = () => {
    if (campaign) backend.devEntities(null).then(setLabels).catch(fail);
  };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(refreshLabels, [campaign?.id, campaign?.entity_count]);

  const name = useMemo(() => {
    const m = new Map(labels.map((l) => [l.id, l.name ?? l.id]));
    return (id: string | null) => (id ? m.get(id) ?? id : "—");
  }, [labels]);

  async function seed() {
    try {
      const r = await backend.devSeedDemo();
      app.setCampaign(r.campaign);
      app.notify(t("dev.seeded", { countries: r.demo.countries, territories: r.demo.territories, events: r.demo.events }));
    } catch (e) {
      fail(e);
    }
  }

  async function createDemo() {
    const folder = await backend.pickFolder();
    if (!folder) return;
    try {
      const r = await backend.devCreateDemoCampaign(folder);
      app.setCampaign(r.campaign);
      app.notify(t("dev.seeded", { countries: r.demo.countries, territories: r.demo.territories, events: r.demo.events }));
    } catch (e) {
      fail(e);
    }
  }

  return (
    <section className="page page-wide">
      <h1>{t("dev.title")}</h1>
      <p className="lede">{t("dev.lede")}</p>

      <h2>{t("dev.demo")}</h2>
      <p className="hint">{t("dev.demoHint")}</p>
      {campaign?.is_demo && <p className="notice">{t("dev.demoBadge")}: {campaign.name}</p>}
      {!campaign && <p className="notice is-warning">{t("dev.noCampaign")}</p>}
      <div className="actions">
        {campaign && campaign.entity_count === 0 && <button className="primary" onClick={seed}>{t("dev.seed")}</button>}
        <button onClick={createDemo}>{t("dev.createDemo")}</button>
      </div>

      {campaign && (
        <>
          <SelfTest onError={fail} />
          <TimeQuery labels={labels} name={name} onError={fail} />
          <Periods labels={labels} name={name} onError={fail} />
          <SqlConsole onError={fail} />
          <Tables onError={fail} />
        </>
      )}
    </section>
  );
}

type OnError = (e: unknown) => void;

function SelfTest({ onError }: { onError: OnError }) {
  const { t } = useI18n();
  const [checks, setChecks] = useState<Check[] | null>(null);
  const failed = checks?.filter((c) => !c.ok).length ?? 0;
  return (
    <>
      <h2>{t("dev.selfTest")}</h2>
      <p className="hint">{t("dev.selfTestHint")}</p>
      <div className="actions"><button onClick={() => backend.devSelfTest().then(setChecks).catch(onError)}>{t("dev.run")}</button></div>
      {checks && (
        <>
          <p className={failed ? "notice is-warning" : "notice"}>
            {failed ? t("dev.someFailed", { n: failed, total: checks.length }) : t("dev.allPassed", { n: checks.length })}
          </p>
          <div className="table-wrap">
            <table>
              <tbody>
                {checks.map((c) => (
                  <tr key={c.name}>
                    <td><span className={c.ok ? "pill is-ok" : "pill is-bad"}>{c.ok ? t("dev.pass") : t("dev.fail")}</span></td>
                    <td><code>{c.name}</code></td>
                    <td className="muted">{c.detail}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
    </>
  );
}

function TimeQuery({ labels, name, onError }: { labels: EntityLabel[]; name: (id: string | null) => string; onError: OnError }) {
  const { t, lang } = useI18n();
  const territories = labels.filter((l) => l.kind === "territory");
  const [territory, setTerritory] = useState("");
  const [date, setDate] = useState("1250");
  const [answer, setAnswer] = useState<OwnershipPeriod | null | undefined>(undefined);
  const [history, setHistory] = useState<OwnershipPeriod[]>([]);
  const selected = territory || territories[0]?.id || "";

  async function ask() {
    if (!selected) return;
    try {
      setAnswer(await backend.devOwnerAt(selected, date));
      setHistory(await backend.devOwnershipHistory(selected));
    } catch (e) {
      onError(e);
    }
  }

  return (
    <>
      <h2>{t("dev.time")}</h2>
      <div className="row wrap">
        <label className="inline-field">{t("dev.territory")}
          <select value={selected} onChange={(e) => setTerritory(e.target.value)}>
            {territories.map((l) => <option key={l.id} value={l.id}>{l.name ?? l.id}</option>)}
          </select>
        </label>
        <label className="inline-field">{t("dev.date")}
          <input className="short" value={date} onChange={(e) => setDate(e.target.value)} />
        </label>
        <div className="actions compact"><button onClick={ask} disabled={!selected}>{t("dev.ask")}</button></div>
      </div>
      {answer !== undefined && (
        <p className="answer">
          {answer ? `${formatDate(lang, date)}: ${name(answer.owner_id)}` : t("dev.noOwner")}
        </p>
      )}
      {history.length > 0 && (
        <div className="table-wrap">
          <table>
            <caption>{t("dev.history")}</caption>
            <thead><tr><th>{t("dev.from")}</th><th>{t("dev.to")}</th><th>{t("dev.owner")}</th></tr></thead>
            <tbody>
              {history.map((p) => (
                <tr key={p.from} className={answer && answer.from === p.from ? "is-hit" : ""}>
                  <td>{formatDate(lang, p.from)}</td><td>{p.to ? formatDate(lang, p.to) : t("dev.now")}</td><td>{name(p.owner_id)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}

function Periods({ labels, name, onError }: { labels: EntityLabel[]; name: (id: string | null) => string; onError: OnError }) {
  const { t, lang } = useI18n();
  const countries = labels.filter((l) => l.kind === "country");
  const [entity, setEntity] = useState("");
  const [aspect, setAspect] = useState<string>("");
  const [rows, setRows] = useState<Period[] | null>(null);
  const selected = entity || countries[0]?.id || "";

  return (
    <>
      <h2>{t("dev.periods")}</h2>
      <div className="row wrap">
        <label className="inline-field">{t("dev.entity")}
          <select value={selected} onChange={(e) => setEntity(e.target.value)}>
            {countries.map((l) => <option key={l.id} value={l.id}>{l.name ?? l.id}</option>)}
          </select>
        </label>
        <label className="inline-field">{t("dev.aspect")}
          <select value={aspect} onChange={(e) => setAspect(e.target.value)}>
            <option value="">{t("dev.allAspects")}</option>
            {ASPECTS.map((a) => <option key={a} value={a}>{a}</option>)}
          </select>
        </label>
        <div className="actions compact">
          <button disabled={!selected} onClick={() => backend.devPeriods(selected, aspect || null).then(setRows).catch(onError)}>{t("dev.ask")}</button>
        </div>
      </div>
      {rows && (
        <div className="table-wrap">
          <table>
            <thead><tr><th>{t("dev.aspect")}</th><th>{t("dev.value")}</th><th>{t("dev.from")}</th><th>{t("dev.to")}</th></tr></thead>
            <tbody>
              {rows.map((p) => (
                <tr key={`${p.aspect}-${p.from}`}>
                  <td><code>{p.aspect}</code></td>
                  <td>{p.value ?? name(p.value_entity_id)}</td>
                  <td>{formatDate(lang, p.from)}</td>
                  <td>{p.to ? formatDate(lang, p.to) : t("dev.now")}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}

function SqlConsole({ onError }: { onError: OnError }) {
  const { t } = useI18n();
  const [sql, setSql] = useState(EXAMPLES[0]![1]);
  const [result, setResult] = useState<QueryResult | null>(null);
  const cell = (v: unknown) => (v === null ? "NULL" : typeof v === "object" ? JSON.stringify(v) : String(v));

  return (
    <>
      <h2>{t("dev.sql")}</h2>
      <p className="hint">{t("dev.sqlHint")}</p>
      <div className="row wrap examples">
        <span className="muted">{t("dev.examples")}:</span>
        {EXAMPLES.map(([label, q]) => <button key={label} className="chip" onClick={() => setSql(q)}>{label}</button>)}
      </div>
      <textarea className="sql" rows={5} spellCheck={false} value={sql} onChange={(e) => setSql(e.target.value)} />
      <div className="actions compact">
        <button className="primary" onClick={() => backend.devQuery(sql).then(setResult).catch(onError)}>{t("dev.run")}</button>
      </div>
      {result && (
        <div className="table-wrap results">
          <table>
            <thead><tr>{result.columns.map((c) => <th key={c}>{c}</th>)}</tr></thead>
            <tbody>
              {result.rows.map((r, i) => <tr key={i}>{r.map((v, j) => <td key={j}>{cell(v)}</td>)}</tr>)}
            </tbody>
          </table>
          {result.truncated && <p className="hint">{t("dev.truncated")}</p>}
        </div>
      )}
    </>
  );
}

function Tables({ onError }: { onError: OnError }) {
  const { t } = useI18n();
  const [rows, setRows] = useState<[string, number][] | null>(null);
  return (
    <>
      <h2>{t("dev.tables")}</h2>
      <div className="actions compact"><button onClick={() => backend.devTables().then(setRows).catch(onError)}>{t("dev.run")}</button></div>
      {rows && (
        <div className="table-wrap">
          <table>
            <tbody>{rows.map(([n, c]) => <tr key={n}><td><code>{n}</code></td><td className="num">{c}</td></tr>)}</tbody>
          </table>
        </div>
      )}
    </>
  );
}
