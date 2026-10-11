import { useEffect, useState } from "react";
import type { AppData } from "../App";
import { backend, errorCode } from "../api/backend";
import type { BridgeParams, Readiness, StepStatus, TransitionMode, TransitionPlan, TransitionStep } from "../api/types";
import { Icon } from "../components/Icons";
import { errorText, formatDate, useI18n, type Key } from "../i18n";

const STATUS_MARK: Record<StepStatus, string> = { pending: "○", done: "✓", skipped: "–", blocked: "⊘", warning: "!", failed: "✗" };
const STEP_ORDER = ["find_save", "check_save", "backup", "import_final", "final_snapshot", "diff_history", "semantics", "geo_core",
  "coefficients", "build_world", "preview", "generate", "validate", "close_era", "next_era"] as const;

export function Transition({ app }: { app: AppData }) {
  const { t, lang } = useI18n();
  const campaign = app.boot.current_campaign;
  const registry = app.boot.registry;
  const [ready, setReady] = useState<Readiness | null>(null);
  const [plan, setPlan] = useState<TransitionPlan | null>(null);
  const [live, setLive] = useState<Record<string, TransitionStep>>({});
  const [phase, setPhase] = useState<"idle" | "preparing" | "preview" | "generating" | "done">("idle");
  const [savePath, setSavePath] = useState<string | null>(null);
  const [bridge, setBridge] = useState<BridgeParams>({ seed: 45819283, start_year: null, empire_name: "", adjective: "Terran" });
  const fail = (e: unknown) => app.notify(errorText(t, errorCode(e)), "error");

  useEffect(() => {
    if (!campaign) return;
    backend.transitionReadiness().then(setReady).catch(fail);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [campaign?.id, campaign?.current_game]);

  useEffect(() => {
    let off: (() => void) | undefined;
    void backend.onTransitionStep((s) => setLive((l) => ({ ...l, [s.id]: s }))).then((u) => (off = u));
    return () => off?.();
  }, []);

  if (!campaign) {
    return (
      <section className="page">
        <h1>{t("tr2.title")}</h1>
        <p className="empty">{t("tl.noCampaign")}</p>
        <div className="actions"><button className="primary" onClick={() => app.go("campaign")}>{t("tl.openCampaign")}</button></div>
      </section>
    );
  }

  const route = plan?.route ?? ready?.route ?? null;
  const name = (g: string) => registry.games[g]?.display_name ?? g;
  const mode = campaign.settings.transition_mode;

  async function setMode(m: TransitionMode) {
    try {
      app.setCampaign(await backend.updateSettings({ ...campaign!.settings, transition_mode: m }));
    } catch (e) {
      fail(e);
    }
  }

  async function start() {
    setLive({});
    setPlan(null);
    setPhase("preparing");
    try {
      const p = await backend.transitionPrepare(savePath, bridge);
      setPlan(p);
      setPhase("preview");
      if (p.preview?.design && !bridge.empire_name) setBridge((b) => ({ ...b, empire_name: suggested(p.preview!.design!.authority, p.preview!.design!.origin) }));
    } catch (e) {
      setPhase("idle");
      fail(e);
    }
  }

  async function confirm() {
    setPhase("generating");
    try {
      const r = await backend.transitionGenerate();
      setPlan(r.plan);
      app.setCampaign(r.campaign);
      setPhase("done");
    } catch (e) {
      setPhase("preview");
      fail(e);
    }
  }

  async function chooseSave() {
    const p = await backend.pickFile();
    if (p) setSavePath(p);
  }

  const steps: TransitionStep[] = STEP_ORDER.map(
    (id) => plan?.steps.find((s) => s.id === id) ?? live[id] ?? { id, status: "pending", counts: {}, note: null },
  );
  const running = phase === "preparing" || phase === "generating";
  const firstPending = running ? steps.find((s) => s.status === "pending")?.id : undefined;
  const pv = plan?.preview ?? null;

  return (
    <section className="page page-wide transition-screen" data-game={route?.from ?? campaign.current_game}>
      <h1>{t("tr2.title")}</h1>
      <p className="lede">{t("tr2.lede")}</p>

      {route ? (
        <div className="route">
          <div className="route-game" data-game={route.from}>
            <Icon name={route.from} size={30} />
            <div><strong>{name(route.from)}</strong><span>{t("tr2.current")}</span></div>
          </div>
          <div className="route-arrow" aria-hidden="true">↓{route.via && <span className="route-via">{name(route.via)}</span>}</div>
          <div className="route-game" data-game={route.to}>
            <Icon name={route.to} size={30} />
            <div><strong>{name(route.to)}</strong><span>{t(`tr2.support.${ready?.support ?? "none"}` as Key)}</span></div>
          </div>
        </div>
      ) : (
        <p className="notice">{t("tr2.noNext")}</p>
      )}

      <dl className="facts">
        <dt>{t("tr2.campaignDate")}</dt><dd>{formatDate(lang, campaign.current_date)}</dd>
        <dt>{t("tr2.save")}</dt>
        <dd>
          {savePath ?? ready?.newest_save?.name ?? t("tr2.noSave")}
          {(savePath || ready?.newest_save) && <span className="ok-mark"> ✓ {t("tr2.found")}</span>}
        </dd>
        {ready?.save_date && (<><dt>{t("tr2.saveDate")}</dt><dd>{formatDate(lang, ready.save_date)}</dd></>)}
        {ready?.transition_date && (<><dt>{t("tr2.transitionDate")}</dt><dd>{formatDate(lang, ready.transition_date)}</dd></>)}
      </dl>

      {ready?.route && (
        <p className={ready.reached ? "notice" : "notice is-warning"}>
          {ready.reached
            ? t("tr2.reached", { from: name(ready.route.from), to: name(ready.route.to) })
            : t("tr2.notReached", { date: formatDate(lang, ready.transition_date) })}
        </p>
      )}

      <fieldset className="group">
        <legend>{t("tr2.mode")}</legend>
        {(["manual", "suggested", "automatic"] as TransitionMode[]).map((m) => (
          <label key={m} className="radio">
            <input type="radio" name="tmode" checked={mode === m} onChange={() => setMode(m)} />
            <span><strong>{t(`tr2.mode.${m}` as Key)}</strong> {t(`tr2.mode.${m}.desc` as Key)}</span>
          </label>
        ))}
      </fieldset>

      {route?.kind === "hoi4_to_stellaris" && phase !== "done" && (
        <fieldset className="group">
          <legend>{t("tr2.bridge")}</legend>
          <div className="row wrap">
            <label className="inline-field">{t("br.startYear")}
              <input className="short" inputMode="numeric" placeholder={ready?.save_date?.split(".")[0] ?? "1948"} value={bridge.start_year ?? ""}
                     onChange={(e) => setBridge({ ...bridge, start_year: e.target.value ? Number(e.target.value) || null : null })} />
            </label>
            <label className="inline-field">{t("br.seed")}
              <input className="short" inputMode="numeric" value={bridge.seed} onChange={(e) => setBridge({ ...bridge, seed: Number(e.target.value) || 0 })} />
            </label>
            <label className="inline-field">{t("tr.name")}
              <input value={bridge.empire_name} placeholder="Terran Federation" onChange={(e) => setBridge({ ...bridge, empire_name: e.target.value })} />
            </label>
            <label className="inline-field">{t("tr.adjective")}
              <input className="short" value={bridge.adjective} onChange={(e) => setBridge({ ...bridge, adjective: e.target.value })} />
            </label>
          </div>
        </fieldset>
      )}

      <div className="actions">
        <button className="primary" onClick={start} disabled={running || !route}>
          {phase === "preparing" ? t("tr2.preparing") : plan ? t("tr2.restart") : t("tr2.start")}
        </button>
        <button onClick={chooseSave} disabled={running}>{t("tr2.otherSave")}</button>
      </div>

      {(phase !== "idle" || plan) && (
        <ol className="steps-list">
          {steps.map((s) => {
            const status = firstPending === s.id ? "running" : s.status;
            return (
              <li key={s.id} className={`step is-${status}`}>
                <span className="step-mark" aria-hidden="true">{status === "running" ? "●" : STATUS_MARK[s.status]}</span>
                <span className="step-name">{t(`step.${s.id}` as Key)}</span>
                <span className="step-info">
                  {Object.entries(s.counts).map(([k, v]) => `${t(`cnt.${k}` as Key) || k}: ${v.toLocaleString(lang === "uk" ? "uk-UA" : "en-GB")}`).join(", ")}
                  {s.note && <span className="step-note">{registry.games[s.note]?.display_name ?? noteText(s.note, t)}</span>}
                </span>
              </li>
            );
          })}
        </ol>
      )}

      {pv && plan && (
        <section className="preview-card">
          <h2>{t("tr2.result")}</h2>
          <div className="preview-grid">
            <div><span>{name(plan.route.from)}</span><strong>{pv.source_countries}</strong><small>{t("tr2.states")}</small></div>
            <div><span>{name(plan.route.to)}</span><strong>{pv.target_countries ?? "—"}</strong><small>{t("tr2.states")}</small></div>
            <div><span>{t("tr2.merged")}</span><strong>{pv.merged}</strong></div>
            <div><span>{t("tr2.split")}</span><strong>{pv.split}</strong></div>
            <div><span>{t("tr2.vanished")}</span><strong>{pv.vanished}</strong></div>
            <div><span>{t("tr2.created")}</span><strong>{pv.created}</strong></div>
            <div><span>{t("tr2.lowConfidence")}</span><strong>{pv.low_confidence}</strong></div>
            <div><span>{t("tr2.geocore")}</span><strong>{pv.geocore_warnings}</strong></div>
            <div className={pv.critical_errors ? "is-bad" : ""}><span>{t("tr2.critical")}</span><strong>{pv.critical_errors}</strong></div>
          </div>
          {pv.design && (
            <p className="muted">
              {t("tr2.empire", { start: pv.start_year ?? "", events: pv.bridge_events })}{" "}
              <strong>{pv.design.authority}</strong> · {pv.design.ethics.join(", ")} · {pv.design.civics.join(", ")} · {pv.design.origin}
            </p>
          )}
          {pv.notes.length > 0 && <ul className="reasons">{pv.notes.map((n) => <li key={n}>{noteText(n, t)}</li>)}</ul>}
          {plan.blocked_reason && !plan.can_generate && <p className="notice is-warning">{noteText(plan.blocked_reason, t)}</p>}
          <div className="actions">
            <button onClick={() => app.go("timeline")}>{t("tr2.viewChanges")}</button>
            {phase !== "done" && (
              <button className="primary" onClick={confirm} disabled={!plan.can_generate || phase === "generating"}>
                {phase === "generating" ? t("tr2.generating") : t("tr2.create", { game: name(plan.route.to) })}
              </button>
            )}
          </div>
        </section>
      )}

      {phase === "done" && plan?.generated && (
        <div className="notice">
          <p><strong>{t("tr2.done", { game: name(plan.route.to) })}</strong></p>
          <p className="muted">{t("tr.where", { path: plan.generated.file })}</p>
          {plan.generated.warnings.includes("old_chronicle_mod_installed") && <p className="notice is-warning">{t("tr.oldMod")}</p>}
          <ol className="steps">
            <li>{t("tr.step1")}</li>
            <li>{t("tr.step2")}</li>
            <li>{t("tr.step3", { name: plan.generated.empire_name })}</li>
          </ol>
          <p className="muted">{t("tr2.oldEraKept", { game: name(plan.route.from) })}</p>
        </div>
      )}
    </section>
  );
}

function suggested(authority: string, origin: string): string {
  if (origin === "post_apocalyptic") return "Terran Remnant";
  return ({ democratic: "Terran Federation", oligarchic: "Terran Commonwealth", dictatorial: "Terran Directorate", imperial: "Terran Imperium" } as Record<string, string>)[authority] ?? "Terran Union";
}

/** Notes are codes when Chronicle knows them, otherwise text from the backend. */
function noteText(note: string, t: ReturnType<typeof useI18n>["t"]): string {
  const translated: string | undefined = t(`note.${note}` as Key);
  return translated || note;
}
