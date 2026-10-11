import { useEffect, useState } from "react";
import type { AppData } from "../App";
import { backend, errorCode } from "../api/backend";
import type { Language, SaveFormatRow } from "../api/types";
import { Icon } from "../components/Icons";
import { errorText, useI18n, type Key } from "../i18n";

export function Settings({ app, lang, onLanguage }: { app: AppData; lang: Language; onLanguage: (l: Language) => void }) {
  const { t } = useI18n();
  const [log, setLog] = useState<string[]>([]);
  const [formats, setFormats] = useState<SaveFormatRow[]>([]);
  const [busy, setBusy] = useState<string | null>(null);

  useEffect(() => {
    backend.steamLog().then(setLog).catch(() => setLog([]));
    backend.saveFormatStatus().then(setFormats).catch(() => setFormats([]));
  }, []);

  async function switchFormat(game: string, text: boolean) {
    setBusy(game);
    try {
      const r = await backend.setSaveFormat(game, text);
      app.notify(r.changed ? t(text ? "sf.nowText" : "sf.nowBinary") : t("sf.already"));
      setFormats(await backend.saveFormatStatus());
    } catch (e) {
      app.notify(errorText(t, errorCode(e)), "error");
    } finally {
      setBusy(null);
    }
  }

  return (
    <section className="page">
      <h1>{t("set.title")}</h1>
      <fieldset className="group">
        <legend>{t("set.language")}</legend>
        <label className="radio"><input type="radio" name="lang" checked={lang === "en"} onChange={() => onLanguage("en")} /> English</label>
        <label className="radio"><input type="radio" name="lang" checked={lang === "uk"} onChange={() => onLanguage("uk")} /> Українська</label>
      </fieldset>
      <fieldset className="group">
        <legend>{t("set.devMode")}</legend>
        <label className="radio">
          <input type="checkbox" checked={app.boot.dev_mode} onChange={(e) => app.setDevMode(e.target.checked)} />
          <span>{t("set.devModeHint")}</span>
        </label>
      </fieldset>
      <h2>{t("sf.title")}</h2>
      <p className="muted">{t("sf.hint")}</p>
      {formats.map((f) => {
        const name = app.boot.registry.games[f.game]?.display_name ?? f.game;
        const mode = f.status?.mode ?? null;
        const isText = mode === "text";
        return (
          <div key={f.game} className="card save-format" data-game={f.game}>
            <div className="card-head">
              <h2><Icon name={f.game} size={18} /> {name}</h2>
              <span className={isText ? "badge is-ok" : "badge is-off"}>
                {mode ? t(`sf.mode.${mode}` as Key) : t("sf.noFolder")}
              </span>
            </div>
            {f.status && <p className="hint">{f.status.exists ? f.status.settings_file : t("sf.noFile", { file: f.status.settings_file })}</p>}
            <div className="actions compact">
              {f.status && !isText && (
                <button className="primary" disabled={busy === f.game} onClick={() => switchFormat(f.game, true)}>{t("sf.toText")}</button>
              )}
              {f.status && isText && (
                <button disabled={busy === f.game} onClick={() => switchFormat(f.game, false)}>{t("sf.toBinary")}</button>
              )}
              {!f.status && <button onClick={() => app.go("games")}>{t("sf.setFolder")}</button>}
            </div>
          </div>
        );
      })}
      <p className="hint">{t("sf.existing")}</p>

      <h2>{t("set.data")}</h2>
      <p className="muted">{t("set.dataBody")}</p>
      <h2>{t("set.steamLog")}</h2>
      {log.length === 0 ? <p className="empty">{t("set.steamLogEmpty")}</p> : <pre className="log">{log.join("\n")}</pre>}
      <p className="muted">{t("set.version")}: {app.boot.version}</p>
    </section>
  );
}
