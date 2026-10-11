import { useCallback, useEffect, useRef, useState } from "react";
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
import { Transition } from "./screens/Transition";
import { Hourglass, Icon } from "./components/Icons";
import { formatDate } from "./i18n";

type Theme = "dark" | "light";
function savedTheme(): Theme {
  try {
    return localStorage.getItem("chronicle.theme") === "light" ? "light" : "dark";
  } catch {
    return "dark";
  }
}

export type Screen = "dashboard" | "games" | "campaign" | "timeline" | "transition" | "bridge" | "settings" | "developer";
const NAV: Screen[] = ["dashboard", "games", "campaign", "timeline", "transition", "bridge", "settings"];

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
  const [theme, setTheme] = useState<Theme>(savedTheme());
  const notifyRef = useRef<(msg: string) => void>(() => {});

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    try {
      localStorage.setItem("chronicle.theme", theme);
    } catch {
      /* storage unavailable: theme lasts for this session */
    }
  }, [theme]);

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

  // "Recommended" and "automatic" transition modes: check the newest save once a minute.
  // Automatic mode prepares the transition up to the preview (never generates without you).
  const autoPrepared = useRef<string>("");
  useEffect(() => {
    const c = boot?.current_campaign;
    if (!boot || !c || c.settings.transition_mode === "manual") return;
    const tick = async () => {
      try {
        const r = await backend.transitionReadiness();
        if (!r.reached || !r.newest_save || !r.route) return;
        const id = `${c.id}:${r.newest_save.path}:${r.newest_save.modified}`;
        if (autoPrepared.current === id) return;
        const stableFor = Date.now() / 1000 - r.newest_save.modified;
        if (c.settings.transition_mode === "automatic" && stableFor > 60) {
          autoPrepared.current = id;
          await backend.transitionPrepare(r.newest_save.path, { seed: 45819283, start_year: null, empire_name: "", adjective: "Terran" }, null);
          notifyRef.current(translate(lang, "tr2.autoPrepared"));
        } else if (c.settings.transition_mode === "suggested") {
          autoPrepared.current = id;
          notifyRef.current(translate(lang, "tr2.suggest", { from: boot.registry.games[r.route.from]?.display_name ?? r.route.from, to: boot.registry.games[r.route.to]?.display_name ?? r.route.to }));
        }
      } catch {
        /* no campaign or no save yet: try again next minute */
      }
    };
    void tick();
    const timer = window.setInterval(tick, 60_000);
    return () => window.clearInterval(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [boot?.current_campaign?.id, boot?.current_campaign?.settings.transition_mode, boot?.current_campaign?.current_game]);

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
  notifyRef.current = data.notify;

  return (
    <I18nProvider lang={lang}>
      <div className="shell" data-game={boot.current_campaign?.current_game ?? "ck3"}>
        <aside className="rail">
          <div className="brand-block">
            <Hourglass />
            <div>
              <p className="brand">Chronicle</p>
              <p className="tagline">{translate(lang, "app.tagline")}</p>
              <p className="version" title={translate(lang, "set.version")}>v{boot.version}</p>
            </div>
          </div>
          <nav className="nav" aria-label="Chronicle">
            {(boot.dev_mode ? [...NAV, "developer" as Screen] : NAV).map((s) => (
              <button
                key={s}
                className={s === screen ? "nav-item is-active" : "nav-item"}
                aria-current={s === screen ? "page" : undefined}
                onClick={() => setScreen(s)}
              >
                <Icon name={s} size={22} />
                <span>{translate(lang, `nav.${s}` as Key)}</span>
              </button>
            ))}
          </nav>
          {boot.current_campaign && (
            <button className="campaign-card" onClick={() => setScreen("campaign")} data-game={boot.current_campaign.current_game}>
              <span className="campaign-card-name">{boot.current_campaign.name}</span>
              <span className="campaign-card-game">
                <Icon name={boot.current_campaign.current_game} size={16} />
                {boot.registry.games[boot.current_campaign.current_game]?.display_name ?? boot.current_campaign.current_game}
              </span>
              <span className="campaign-card-date">{formatDate(lang, boot.current_campaign.current_date)}</span>
            </button>
          )}
          <div className="rail-foot">
            <div className="lang-switch" role="group" aria-label={translate(lang, "set.language")}>
              {(["en", "uk"] as Language[]).map((l) => (
                <button key={l} className={l === lang ? "is-active" : ""} aria-pressed={l === lang} onClick={() => changeLanguage(l)}>
                  {l === "en" ? "EN" : "УКР"}
                </button>
              ))}
            </div>
            <button
              className="theme-toggle"
              onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
              aria-label={translate(lang, theme === "dark" ? "app.lightTheme" : "app.darkTheme")}
              title={translate(lang, theme === "dark" ? "app.lightTheme" : "app.darkTheme")}
            >
              <Icon name={theme === "dark" ? "sun" : "moon"} size={20} />
            </button>
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
          {screen === "transition" && <Transition app={data} />}
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
