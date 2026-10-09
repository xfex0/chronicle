import { useCallback, useEffect, useState } from "react";
import { backend, errorCode } from "./api/backend";
import type { Bootstrap, CampaignSummary, InstallationRow, Language } from "./api/types";
import { I18nProvider, detectLanguage, errorText, translate, type Key } from "./i18n";
import { Dashboard } from "./screens/Dashboard";
import { Games } from "./screens/Games";
import { Campaign } from "./screens/Campaign";
import { Timeline } from "./screens/Timeline";
import { Settings } from "./screens/Settings";
import { Developer } from "./screens/Developer";
import { Bridge } from "./screens/Bridge";

export type Screen = "dashboard" | "games" | "campaign" | "timeline" | "bridge" | "settings" | "developer";
const NAV: Screen[] = ["dashboard", "games", "campaign", "timeline", "bridge", "settings"];

export interface AppData {
  boot: Bootstrap;
  setInstallations: (rows: InstallationRow[]) => void;
  setCampaign: (c: CampaignSummary) => void;
  setDevMode: (on: boolean) => void;
  go: (s: Screen) => void;
  notify: (msg: string, kind?: "info" | "error") => void;
}

export function App() {
  const [boot, setBoot] = useState<Bootstrap | null>(null);
  const [lang, setLang] = useState<Language>(detectLanguage());
  const [screen, setScreen] = useState<Screen>("dashboard");
  const [toast, setToast] = useState<{ msg: string; kind: "info" | "error" } | null>(null);

  useEffect(() => {
    backend
      .bootstrap()
      .then((b) => {
        setBoot(b);
        if (b.language) setLang(b.language);
      })
      .catch((e) => setToast({ msg: errorText((k, v) => translate(lang, k, v), errorCode(e)), kind: "error" }));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    document.documentElement.lang = lang;
  }, [lang]);

  const changeLanguage = useCallback((l: Language) => {
    setLang(l);
    void backend.setLanguage(l);
  }, []);

  if (!boot) {
    return <div className="boot">{translate(lang, "common.loading")}</div>;
  }

  const data: AppData = {
    boot,
    setInstallations: (rows) => setBoot({ ...boot, installations: rows }),
    setCampaign: (c) => setBoot({ ...boot, current_campaign: c }),
    setDevMode: (on) => {
      setBoot({ ...boot, dev_mode: on });
      if (!on && screen === "developer") setScreen("settings");
      void backend.setDevMode(on);
    },
    go: setScreen,
    notify: (msg, kind = "info") => {
      setToast({ msg, kind });
      window.setTimeout(() => setToast(null), 6000);
    },
  };

  return (
    <I18nProvider lang={lang}>
      <div className="shell">
        <aside className="rail">
          <p className="brand">Chronicle</p>
          <p className="tagline">{translate(lang, "app.tagline")}</p>
          <p className="version" title={translate(lang, "set.version")}>v{boot.version}</p>
          <nav className="nav" aria-label="Chronicle">
            {(boot.dev_mode ? [...NAV, "developer" as Screen] : NAV).map((s) => (
              <button
                key={s}
                className={s === screen ? "nav-item is-active" : "nav-item"}
                aria-current={s === screen ? "page" : undefined}
                onClick={() => setScreen(s)}
              >
                {translate(lang, `nav.${s}` as Key)}
              </button>
            ))}
          </nav>
          <div className="lang-switch" role="group" aria-label={translate(lang, "set.language")}>
            {(["en", "uk"] as Language[]).map((l) => (
              <button key={l} className={l === lang ? "is-active" : ""} aria-pressed={l === lang} onClick={() => changeLanguage(l)}>
                {l === "en" ? "EN" : "УКР"}
              </button>
            ))}
          </div>
        </aside>
        <main className="stage">
          {screen === "dashboard" && <Dashboard app={data} />}
          {screen === "games" && <Games app={data} />}
          {screen === "campaign" && <Campaign app={data} />}
          {screen === "timeline" && <Timeline app={data} />}
          {screen === "settings" && <Settings app={data} lang={lang} onLanguage={changeLanguage} />}
          {screen === "developer" && boot.dev_mode && <Developer app={data} />}
          {screen === "bridge" && <Bridge app={data} />}
        </main>
        {toast && (
          <div className={`toast is-${toast.kind}`} role={toast.kind === "error" ? "alert" : "status"}>
            {toast.msg}
          </div>
        )}
      </div>
    </I18nProvider>
  );
}
