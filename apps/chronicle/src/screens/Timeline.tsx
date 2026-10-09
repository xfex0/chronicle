import { useEffect, useState } from "react";
import type { AppData } from "../App";
import { backend, errorCode } from "../api/backend";
import type { EventRow } from "../api/types";
import { errorText, formatDate, useI18n } from "../i18n";

export function Timeline({ app }: { app: AppData }) {
  const { t, lang } = useI18n();
  const [minImportance, setMin] = useState(0);
  const [events, setEvents] = useState<EventRow[] | null>(null);
  const campaign = app.boot.current_campaign;

  useEffect(() => {
    if (!campaign) return;
    backend.events(minImportance).then(setEvents).catch((e) => app.notify(errorText(t, errorCode(e)), "error"));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [minImportance, campaign?.id]);

  if (!campaign) {
    return <section className="page"><h1>{t("tl.title")}</h1><p className="empty">{t("tl.noCampaign")}</p></section>;
  }

  return (
    <section className="page">
      <h1>{t("tl.title")}</h1>
      <p className="lede">{t("tl.lede")}</p>
      <label className="inline-field">
        {t("tl.filter")}
        <select value={minImportance} onChange={(e) => setMin(Number(e.target.value))}>
          <option value={0}>{t("tl.all")}</option>
          <option value={4}>{t("tl.important")}</option>
        </select>
      </label>
      {events === null ? (
        <p className="empty">{t("common.loading")}</p>
      ) : events.length === 0 ? (
        <p className="empty">{t("tl.empty")}</p>
      ) : (
        <ol className="journal">
          {events.map((e) => (
            <li key={e.event_id} className={`journal-row imp-${e.importance}`}>
              <time className="journal-date">
                {formatDate(lang, e.date)}
                {e.date_to ? `–${formatDate(lang, e.date_to)}` : ""}
              </time>
              <div>
                <p className="journal-type">
                  {e.event_type.replaceAll("_", " ")}
                  {e.evidence === "inferred" && (
                    <span className="pill is-inferred" title={t("tl.inferredHint")}>{t("tl.inferred")}</span>
                  )}
                </p>
                <p className="journal-meta">
                  {app.boot.registry.games[e.game]?.display_name ?? e.game}
                  {e.actor_entity_id ? `, ${e.actor_entity_id}` : ""}
                </p>
              </div>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}
