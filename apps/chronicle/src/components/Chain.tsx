import type { GameRegistry, InstallationRow } from "../api/types";
import { formatDate, useI18n, type Key } from "../i18n";

export type StepStatus = "finished" | "current" | "future";

/** The campaign chain: the app's signature element. */
export function Chain({ registry, installations, currentGame }: {
  registry: GameRegistry;
  installations: InstallationRow[];
  currentGame: string | null;
}) {
  const { t, lang } = useI18n();
  const currentIdx = currentGame ? registry.chain.indexOf(currentGame) : -1;

  return (
    <ol className="chain" aria-label="Campaign chain">
      {registry.chain.map((key, i) => {
        const g = registry.games[key];
        if (!g) return null;
        const status: StepStatus = currentIdx < 0 ? "future" : i < currentIdx ? "finished" : i === currentIdx ? "current" : "future";
        const inst = installations.find((r) => r.game_key === key);
        const missing = g.kind === "game" && (!inst || inst.source === "missing");
        return (
          <li key={key} className={`chain-step is-${status}${missing ? " is-missing" : ""}`}>
            <span className="chain-dot" aria-hidden="true" />
            <span className="chain-name">{g.display_name}</span>
            <span className="chain-era">
              {formatDate(lang, g.era.start)}
              {g.era.end ? ` – ${formatDate(lang, g.era.end)}` : "+"}
            </span>
            <span className="chain-status">
              {t(`status.${status}` as Key)}
              {g.kind === "virtual" ? `, ${t("status.virtual").toLowerCase()}` : missing ? `, ${t("status.missing").toLowerCase()}` : ""}
            </span>
          </li>
        );
      })}
    </ol>
  );
}
