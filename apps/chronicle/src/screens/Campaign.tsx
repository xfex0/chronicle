import { useCallback, useEffect, useState } from "react";
import type { AppData } from "../App";
import { backend, errorCode } from "../api/backend";
import type { CampaignSummary, Ck3ImportReport, SaveInfo } from "../api/types";
import { SettingsEditor, type EditableSettings } from "../components/SettingsEditor";
import { errorText, formatDate, useI18n, type Key } from "../i18n";

export function Campaign({ app }: { app: AppData }) {
  const c = app.boot.current_campaign;
  const [creating, setCreating] = useState(!c);
  if (!c || creating) return <NewCampaign app={app} onDone={() => setCreating(false)} />;
  return <Overview app={app} campaign={c} onNew={() => setCreating(true)} />;
}

function sanitize(name: string): string {
  return name.replace(/[<>:"/\\|?*]/g, "").trim() || "Chronicle campaign";
}

function NewCampaign({ app, onDone }: { app: AppData; onDone: () => void }) {
  const { t } = useI18n();
  const reg = app.boot.registry;
  const startable = reg.chain.filter((k) => reg.games[k]?.kind === "game");
  const [name, setName] = useState("");
  const [parent, setParent] = useState("");
  const [seed, setSeed] = useState("");
  const [busy, setBusy] = useState(false);
  const [valid, setValid] = useState(true);
  const [settings, setSettings] = useState<EditableSettings>({
    start_game: startable.includes("ck3") ? "ck3" : startable[0] ?? "ck3",
    transition_mode: "suggested",
    transition_dates: {},
    transition_target_start: {},
    confidence: { auto_min: 0.8, warn_min: 0.5 },
  });
  const onValidity = useCallback((ok: boolean) => setValid(ok), []);

  const sep = parent.includes("\\") ? "\\" : "/";
  const folder = parent ? `${parent.replace(/[\\/]+$/, "")}${sep}${sanitize(name)}` : "";

  async function browse() {
    const p = await backend.pickFolder();
    if (p) setParent(p);
  }

  async function create() {
    if (!name.trim()) return app.notify(t("err.name_required"), "error");
    setBusy(true);
    try {
      const summary = await backend.createCampaign({
        name: name.trim(),
        folder,
        start_game: settings.start_game,
        transition_mode: settings.transition_mode,
        transition_dates: settings.transition_dates,
        transition_target_start: settings.transition_target_start,
        confidence: settings.confidence,
        seed: seed.trim() ? Number(seed) : null,
      });
      app.setCampaign(summary);
      onDone();
      app.go("dashboard");
    } catch (e) {
      app.notify(errorText(t, errorCode(e)), "error");
    } finally {
      setBusy(false);
    }
  }

  const seedOk = !seed.trim() || /^\d{1,18}$/.test(seed.trim());

  return (
    <section className="page">
      <h1>{t("camp.new.title")}</h1>
      <p className="lede">{t("camp.new.lede")}</p>

      <label className="field">
        <span>{t("camp.name")}</span>
        <input value={name} onChange={(e) => setName(e.target.value)} placeholder="My World" />
      </label>

      <div className="field">
        <span>{t("camp.folder")}</span>
        <div className="row">
          <input value={folder} readOnly placeholder="…" aria-label={t("camp.folder")} />
          <button onClick={browse}>{t("camp.browse")}</button>
        </div>
        <span className="hint">{t("camp.folderHint")}</span>
      </div>

      <label className="field">
        <span>{t("camp.startGame")}</span>
        <select value={settings.start_game} onChange={(e) => setSettings({ ...settings, start_game: e.target.value, transition_dates: {}, transition_target_start: {} })}>
          {startable.map((k) => <option key={k} value={k}>{reg.games[k]?.display_name}</option>)}
        </select>
      </label>

      <SettingsEditor registry={reg} value={settings} onChange={setSettings} onValidity={onValidity} />

      <label className="field">
        <span>{t("camp.seed")}</span>
        <input value={seed} inputMode="numeric" onChange={(e) => setSeed(e.target.value)} placeholder="45819283" />
        <span className="hint">{t("camp.seedHint")}</span>
      </label>

      <div className="actions">
        <button className="primary" onClick={create} disabled={busy || !valid || !seedOk || !name.trim() || !parent}>
          {busy ? t("camp.creating") : t("camp.create")}
        </button>
      </div>
    </section>
  );
}

function Overview({ app, campaign, onNew }: { app: AppData; campaign: CampaignSummary; onNew: () => void }) {
  const { t, lang } = useI18n();
  const reg = app.boot.registry;
  const [settings, setSettings] = useState<EditableSettings>({
    ...campaign.settings,
    transition_target_start: campaign.settings.transition_target_start ?? {},
  });
  const [valid, setValid] = useState(true);
  const [inspect, setInspect] = useState<SaveInfo | null>(null);
  const [report, setReport] = useState<Ck3ImportReport | null>(null);
  const [importing, setImporting] = useState(false);
  const [seconds, setSeconds] = useState(0);
  useEffect(() => {
    if (!importing) return;
    setSeconds(0);
    const timer = window.setInterval(() => setSeconds((s) => s + 1), 1000);
    return () => window.clearInterval(timer);
  }, [importing]);

  async function importCk3() {
    const p = await backend.pickFile();
    if (!p) return;
    setImporting(true);
    try {
      const r = await backend.importCk3(p);
      setReport(r.report);
      app.setCampaign(r.campaign);
    } catch (e) {
      app.notify(errorText(t, errorCode(e)), "error");
    } finally {
      setImporting(false);
    }
  }
  const onValidity = useCallback((ok: boolean) => setValid(ok), []);

  async function save() {
    try {
      const clean = Object.fromEntries(Object.entries(settings.transition_dates).filter(([, v]) => v.trim()));
      // A custom start without a date means nothing: keep only meaningful choices.
      const starts = Object.fromEntries(Object.entries(settings.transition_target_start).filter(([k]) => k in clean));
      app.setCampaign(await backend.updateSettings({ ...campaign.settings, ...settings, transition_dates: clean, transition_target_start: starts }));
      app.notify(t("camp.saved"));
    } catch (e) {
      app.notify(errorText(t, errorCode(e)), "error");
    }
  }

  async function openOther() {
    const p = await backend.pickFolder();
    if (!p) return;
    try {
      app.setCampaign(await backend.openCampaign(p));
    } catch (e) {
      app.notify(errorText(t, errorCode(e)), "error");
    }
  }

  async function inspectSave() {
    const p = await backend.pickFile();
    if (!p) return;
    try {
      setInspect(await backend.inspectSave(p, campaign.current_game));
    } catch (e) {
      app.notify(errorText(t, errorCode(e)), "error");
    }
  }

  return (
    <section className="page">
      <p className="eyebrow">{t("camp.overview")}</p>
      <h1>{campaign.name}</h1>
      <dl className="facts">
        <dt>{t("camp.folderLabel")}</dt><dd>{campaign.root}</dd>
        <dt>{t("dash.currentGame")}</dt><dd>{reg.games[campaign.current_game]?.display_name}</dd>
        <dt>{t("dash.currentDate")}</dt><dd>{formatDate(lang, campaign.current_date)}</dd>
        <dt>{t("camp.seed")}</dt><dd>{campaign.settings.seed}</dd>
      </dl>
      <div className="actions">
        <button onClick={() => void backend.openFolder(campaign.root)}>{t("camp.openFolder")}</button>
        <button onClick={openOther}>{t("camp.open")}</button>
        <button onClick={onNew}>{t("dash.newCampaign")}</button>
      </div>

      <SettingsEditor registry={reg} value={settings} onChange={setSettings} onValidity={onValidity} />
      <div className="actions">
        <button className="primary" onClick={save} disabled={!valid}>{t("camp.save")}</button>
      </div>

      <h2>{t("imp.title")}</h2>
      <p className="hint">{t("imp.hint")}</p>
      <div className="actions">
        <button className="primary" onClick={importCk3} disabled={importing}>{importing ? t("imp.running") : t("imp.pick")}</button>
        {importing && <span className="hint">{t("imp.elapsed", { s: seconds })}</span>}
      </div>
      {report && (
        <div className="notice">
          <p>{t("imp.done", { realms: report.realms, counties: report.counties, rulers: report.rulers, events: report.events_added, date: formatDate(lang, report.date) })}</p>
          {report.player_realm && <p>{t("imp.player", { name: report.player_realm })}</p>}
          <p className="muted">{t("imp.details", { cultures: report.cultures, faiths: report.faiths, dynasties: report.dynasties, values: report.semantic_values, version: report.game_version ?? "?" })}</p>
          {!report.version_verified && <p className="field-error">{t("imp.unverified")}</p>}
          {report.warnings.length > 0 && <ul className="reasons">{report.warnings.map((w) => <li key={w}>{w}</li>)}</ul>}
        </div>
      )}

      <h2>{t("camp.inspect")}</h2>
      <p className="hint">{t("camp.inspectHint")}</p>
      <div className="actions"><button onClick={inspectSave}>{t("camp.inspectPick")}</button></div>
      {inspect && (
        <div className="inspect">
          <p className="muted">{inspect.path}, {(inspect.size / 1e6).toFixed(1)} MB</p>
          <dl className="facts">
            <dt>{t("inspect.format")}</dt><dd>{t(`format.${inspect.format}` as Key)}</dd>
            {inspect.version_hint && (<><dt>{t("inspect.version")}</dt><dd>{inspect.version_hint}</dd></>)}
          </dl>
          {inspect.header === "HOI4bin" && <p className="notice is-warning">{t("inspect.hoi4Binary")}</p>}
          {inspect.support && (
            <p className={inspect.support === "supported" ? "notice" : "notice is-warning"}>{t(`support.${inspect.support}` as Key)}</p>
          )}
          {inspect.sections.length > 0 && (
            <div className="table-wrap">
              <table>
                <tbody>
                  {inspect.sections.map(([k, n]) => (
                    <tr key={k}><td>{k}</td><td className="num">{(n / 1e6).toFixed(2)} MB</td></tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      )}
    </section>
  );
}
