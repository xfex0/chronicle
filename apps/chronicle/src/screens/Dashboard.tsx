import { useState } from "react";
import type { AppData } from "../App";
import { backend, errorCode } from "../api/backend";
import { Chain } from "../components/Chain";
import { errorText, formatDate, useI18n } from "../i18n";

export function Dashboard({ app }: { app: AppData }) {
  const { t, lang } = useI18n();
  const { boot } = app;
  const c = boot.current_campaign;
  const [busy, setBusy] = useState(false);

  async function scan() {
    setBusy(true);
    try {
      const r = await backend.scanGames();
      app.setInstallations(r.installations);
      app.go("games");
    } catch (e) {
      app.notify(errorText(t, errorCode(e)), "error");
    } finally {
      setBusy(false);
    }
  }

  async function openCampaign() {
    const folder = await backend.pickFolder();
    if (!folder) return;
    try {
      app.setCampaign(await backend.openCampaign(folder));
    } catch (e) {
      app.notify(errorText(t, errorCode(e)), "error");
    }
  }

  if (!c) {
    return (
      <section className="page">
        <h1>{t("dash.noCampaign.title")}</h1>
        <p className="lede">{t("dash.noCampaign.body")}</p>
        <Chain registry={boot.registry} installations={boot.installations} currentGame={null} />
        <div className="actions">
          <button className="primary" onClick={scan} disabled={busy}>
            {busy ? t("games.scanning") : t("dash.scan")}
          </button>
          <button onClick={() => app.go("campaign")}>{t("dash.newCampaign")}</button>
          <button onClick={openCampaign}>{t("dash.openCampaign")}</button>
        </div>
      </section>
    );
  }

  const next = boot.registry.chain[boot.registry.chain.indexOf(c.current_game) + 1];
  const nextDate = next
    ? c.settings.transition_dates[`${c.current_game}->${next}`] ??
      boot.registry.transitions.find((x) => x.from === c.current_game && x.to === next)?.default_date ?? null
    : null;
  const currentName = boot.registry.games[c.current_game]?.display_name ?? c.current_game;

  async function backup() {
    try {
      app.notify(t("dash.checkpointDone", { path: await backend.backup() }));
    } catch (e) {
      app.notify(errorText(t, errorCode(e)), "error");
    }
  }

  async function play() {
    try {
      await backend.launchGame(c!.current_game);
    } catch (e) {
      app.notify(errorText(t, errorCode(e)), "error");
    }
  }

  return (
    <section className="page page-wide">
      <p className="eyebrow">{c.name}{c.is_demo ? `, ${t("dev.demoBadge").toLowerCase()}` : ""}</p>
      <h1>{currentName}</h1>
      <Chain registry={boot.registry} installations={boot.installations} currentGame={c.current_game} />
      <dl className="ledger">
        <div className="ledger-row"><dt>{t("dash.currentDate")}</dt><dd>{formatDate(lang, c.current_date)}</dd></div>
        <div className="ledger-row"><dt>{t("dash.currentGame")}</dt><dd>{currentName}</dd></div>
        {next && (
          <div className="ledger-row">
            <dt>{t("dash.nextTransition")}</dt>
            <dd>{boot.registry.games[next]?.display_name}, {formatDate(lang, nextDate)}</dd>
          </div>
        )}
      </dl>
      <p className="muted">{t("dash.events", { n: c.event_count })}</p>
      <div className="actions">
        <button className="primary" onClick={play}>{t("dash.continue")}</button>
        <button onClick={backup}>{t("dash.checkpoint")}</button>
        <button onClick={() => app.go("timeline")}>{t("dash.timeline")}</button>
      </div>
    </section>
  );
}
