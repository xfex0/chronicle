import { useEffect, useState } from "react";
import { backend, errorCode } from "../api/backend";
import type { ConfidencePolicy, GameRegistry, TargetStart, TransitionMode } from "../api/types";
import { formatDate, useI18n, type Key } from "../i18n";

export interface EditableSettings {
  start_game: string;
  transition_mode: TransitionMode;
  transition_dates: Record<string, string>;
  transition_target_start: Record<string, TargetStart>;
  confidence: ConfidencePolicy;
}

/** Transition mode, transition dates (+ where the next game starts) and confidence thresholds. */
export function SettingsEditor({ registry, value, onChange, onValidity }: {
  registry: GameRegistry;
  value: EditableSettings;
  onChange: (v: EditableSettings) => void;
  onValidity: (ok: boolean) => void;
}) {
  const { t, lang } = useI18n();
  const [dateErrors, setDateErrors] = useState<Record<string, string>>({});

  const startIdx = Math.max(registry.chain.indexOf(value.start_game), 0);
  const transitions = registry.chain.slice(startIdx, -1).map((from, i) => {
    const to = registry.chain[startIdx + i + 1]!;
    const def = registry.transitions.find((x) => x.from === from && x.to === to);
    return { from, to, key: `${from}->${to}`, def };
  });

  const confOk = value.confidence.auto_min > value.confidence.warn_min;
  useEffect(() => {
    onValidity(confOk && Object.keys(dateErrors).length === 0);
  }, [confOk, dateErrors, onValidity]);

  async function validate(key: string, from: string, to: string, date: string, start: TargetStart) {
    const next = { ...dateErrors };
    delete next[key];
    if (date.trim()) {
      try {
        await backend.checkTransitionDate(from, to, date.trim(), start);
      } catch (e) {
        next[key] = errorCode(e);
      }
    }
    setDateErrors(next);
  }

  function setTransition(key: string, from: string, to: string, date: string, start: TargetStart) {
    onChange({
      ...value,
      transition_dates: { ...value.transition_dates, [key]: date },
      transition_target_start: { ...value.transition_target_start, [key]: start },
    });
    void validate(key, from, to, date, start);
  }

  const pct = (v: number) => Math.round(v * 100);
  const setConf = (patch: Partial<ConfidencePolicy>) => onChange({ ...value, confidence: { ...value.confidence, ...patch } });

  return (
    <>
      <fieldset className="group">
        <legend>{t("camp.mode")}</legend>
        {(["manual", "suggested", "automatic"] as TransitionMode[]).map((m) => (
          <label key={m} className="radio">
            <input type="radio" name="mode" checked={value.transition_mode === m} onChange={() => onChange({ ...value, transition_mode: m })} />
            <span><strong>{t(`mode.${m}` as Key)}</strong> {t(`mode.${m}.desc` as Key)}</span>
          </label>
        ))}
      </fieldset>

      <fieldset className="group">
        <legend>{t("camp.dates")}</legend>
        <p className="hint">{t("camp.datesHint")}</p>
        {transitions.map(({ from, to, key, def }) => {
          const date = value.transition_dates[key] ?? "";
          const start: TargetStart = value.transition_target_start[key] ?? "native";
          const toName = registry.games[to]?.display_name ?? to;
          return (
            <div key={key} className="transition">
              <p className="transition-title">{registry.games[from]?.display_name} → {toName}</p>
              <div className="row wrap">
                <input
                  className="short"
                  aria-label={`${from} → ${to}`}
                  placeholder={formatDate(lang, def?.default_date ?? "")}
                  value={date}
                  onChange={(e) => setTransition(key, from, to, e.target.value, start)}
                />
                <span className="hint">{t("camp.dateDefault", { date: formatDate(lang, def?.default_date ?? "") })}</span>
              </div>
              {def && def.alternatives.length > 0 && (
                <div className="row wrap examples">
                  <span className="muted">{t("camp.alternatives")}:</span>
                  {def.alternatives.map((alt) => (
                    <button key={alt} className="chip" onClick={() => setTransition(key, from, to, alt, "custom")}>
                      {t("camp.useDate", { date: formatDate(lang, alt) })}
                    </button>
                  ))}
                </div>
              )}
              <div className="row wrap">
                <span className="muted">{t("camp.targetStart")}:</span>
                {(["native", "custom"] as TargetStart[]).map((s) => (
                  <label key={s} className="radio inline">
                    <input type="radio" name={`start-${key}`} checked={start === s} onChange={() => setTransition(key, from, to, date, s)} />
                    {t(s === "native" ? "camp.targetNative" : "camp.targetCustom")}
                  </label>
                ))}
              </div>
              {start === "custom" && def && !def.custom_start_supported && (
                <p className="notice is-warning">{t("camp.customUnsupported", { game: toName })}</p>
              )}
              {dateErrors[key] && <span className="field-error">{t("camp.dateInvalid", { reason: dateErrors[key]! })}</span>}
            </div>
          );
        })}
      </fieldset>

      <fieldset className="group">
        <legend>{t("camp.confidence")}</legend>
        <p className="hint">{t("camp.confidenceHint")}</p>
        <label className="slider">
          <span>{t("camp.autoMin")} <output>{pct(value.confidence.auto_min)}%</output></span>
          <input type="range" min={5} max={100} step={5} value={pct(value.confidence.auto_min)}
                 onChange={(e) => setConf({ auto_min: Number(e.target.value) / 100 })} />
        </label>
        <label className="slider">
          <span>{t("camp.warnMin")} <output>{pct(value.confidence.warn_min)}%</output></span>
          <input type="range" min={0} max={95} step={5} value={pct(value.confidence.warn_min)}
                 onChange={(e) => setConf({ warn_min: Number(e.target.value) / 100 })} />
        </label>
        <p className="hint">{t("camp.reviewBelow", { value: pct(value.confidence.warn_min) })}</p>
        {!confOk && <p className="field-error">{t("camp.confidenceOrder")}</p>}
      </fieldset>
    </>
  );
}
