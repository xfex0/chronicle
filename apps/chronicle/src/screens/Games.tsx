import { useState } from "react";
import type { AppData } from "../App";
import { backend, errorCode } from "../api/backend";
import type { GameDef, InstallationRow, SteamInstall } from "../api/types";
import { errorText, useI18n, type Key } from "../i18n";

export function Games({ app }: { app: AppData }) {
  const { t } = useI18n();
  const { boot } = app;
  const [steam, setSteam] = useState<SteamInstall | null | undefined>(undefined);
  const [busy, setBusy] = useState(false);

  async function run<T>(fn: () => Promise<T>): Promise<T | undefined> {
    try {
      return await fn();
    } catch (e) {
      app.notify(errorText(t, errorCode(e)), "error");
      return undefined;
    }
  }

  async function scan() {
    setBusy(true);
    const r = await run(backend.scanGames);
    if (r) {
      setSteam(r.steam);
      app.setInstallations(r.installations);
    }
    setBusy(false);
  }

  const games = Object.values(boot.registry.games)
    .filter((g) => g.kind === "game")
    .sort((a, b) => order(boot.registry.chain, a.key) - order(boot.registry.chain, b.key));

  return (
    <section className="page page-wide">
      <h1>{t("games.title")}</h1>
      <p className="lede">{t("games.lede")}</p>
      <div className="actions">
        <button className="primary" onClick={scan} disabled={busy}>{busy ? t("games.scanning") : t("games.scan")}</button>
      </div>
      {steam !== undefined && (
        <p className={steam ? "notice" : "notice is-warning"}>
          {steam ? t("games.steamFound", { root: steam.root, n: steam.libraries.length }) : t("games.steamMissing")}
        </p>
      )}
      <div className="cards">
        {games.map((g) => (
          <GameCard key={g.key} game={g} row={boot.installations.find((r) => r.game_key === g.key)} app={app} run={run} />
        ))}
      </div>
    </section>
  );
}

function order(chain: string[], key: string): number {
  const i = chain.indexOf(key);
  return i < 0 ? 99 : i;
}

function GameCard({ game, row, app, run }: {
  game: GameDef;
  row: InstallationRow | undefined;
  app: AppData;
  run: <T>(fn: () => Promise<T>) => Promise<T | undefined>;
}) {
  const { t } = useI18n();
  const installed = !!row && row.source !== "missing" && !!row.install_path;

  async function chooseGame() {
    const p = await backend.pickFolder();
    if (!p) return;
    const rows = await run(() => backend.linkGameFolder(game.key, p));
    if (rows) app.setInstallations(rows);
  }
  async function chooseSaves() {
    const p = await backend.pickFolder();
    if (!p) return;
    const rows = await run(() => backend.linkSaveFolder(game.key, p));
    if (rows) app.setInstallations(rows);
  }
  async function reset() {
    await run(() => backend.resetDetection(game.key));
    const r = await run(backend.scanGames);
    if (r) app.setInstallations(r.installations);
  }

  return (
    <article className={installed ? "card" : "card is-missing"}>
      <header className="card-head">
        <h2>{game.display_name}</h2>
        <span className={installed ? "badge is-ok" : "badge is-off"}>{installed ? t("games.installed") : t("games.missing")}</span>
      </header>
      <dl className="facts">
        <dt>{t("games.gameFolder")}</dt>
        <dd>
          {row?.install_path ?? t("games.notSet")}
          {row && row.fingerprint !== "unknown" && <span className={`fp is-${row.fingerprint}`}>{t(`fp.${row.fingerprint}` as Key)}</span>}
        </dd>
        <dt>{t("games.saveFolder")}</dt>
        <dd>{row?.save_path ?? t("games.notSet")}</dd>
        <dt>{t("games.source")}</dt>
        <dd>{t(`source.${row?.source ?? "missing"}` as Key)}</dd>
        {row?.detected_version && (<><dt>{t("games.version")}</dt><dd>{row.detected_version}</dd></>)}
        <dt>{t("games.adapter")}</dt>
        <dd>{t(`adapter.${game.status}` as Key)}</dd>
      </dl>
      <div className="actions compact">
        {installed && game.steam_app_id && row?.source === "steam" && (
          <button className="primary" onClick={() => run(() => backend.launchGame(game.key))}>{t("games.play")}</button>
        )}
        {row?.install_path && <button onClick={() => run(() => backend.openFolder(row!.install_path!))}>{t("games.openGame")}</button>}
        {row?.save_path && <button onClick={() => run(() => backend.openFolder(row!.save_path!))}>{t("games.openSaves")}</button>}
        <button onClick={chooseGame}>{t("games.changeGame")}</button>
        <button onClick={chooseSaves}>{t("games.changeSaves")}</button>
        {(row?.install_manual || row?.save_manual) && <button onClick={reset}>{t("games.reset")}</button>}
      </div>
    </article>
  );
}
