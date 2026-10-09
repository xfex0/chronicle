import { useEffect, useState } from "react";
import type { AppData } from "../App";
import { backend } from "../api/backend";
import type { Language } from "../api/types";
import { useI18n } from "../i18n";

export function Settings({ app, lang, onLanguage }: { app: AppData; lang: Language; onLanguage: (l: Language) => void }) {
  const { t } = useI18n();
  const [log, setLog] = useState<string[]>([]);

  useEffect(() => {
    backend.steamLog().then(setLog).catch(() => setLog([]));
  }, []);

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
      <h2>{t("set.data")}</h2>
      <p className="muted">{t("set.dataBody")}</p>
      <h2>{t("set.steamLog")}</h2>
      {log.length === 0 ? <p className="empty">{t("set.steamLogEmpty")}</p> : <pre className="log">{log.join("\n")}</pre>}
      <p className="muted">{t("set.version")}: {app.boot.version}</p>
    </section>
  );
}
