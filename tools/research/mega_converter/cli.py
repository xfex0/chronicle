"""mega-converter CLI (ТЗ §21). The GUI calls the same functions through the sidecar."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import yaml

from mega_converter import __version__
from mega_converter.bridge import CoreUnavailable
from mega_converter.logging_setup import setup_logging

GAMES = ("imperator", "ck3", "eu4", "eu5", "victoria3", "hoi4", "stellaris")


def cmd_inspect(a: argparse.Namespace) -> int:
    from mega_converter import bridge

    info = bridge.detect_save(Path(a.save))
    print(json.dumps(info, indent=2, ensure_ascii=False))
    if info["container"] == "plain" and info["encoding"] != "binary":
        for row in bridge.inspect(Path(a.save))[: a.top]:
            print(f"{row['total_bytes']:>14}  {row['occurrences']:>6}  {row['key']}")
    else:
        print("For archives use the Rust tool: paradox-inspect <save> --entry <name>", file=sys.stderr)
    return 0


def cmd_rules_check(a: argparse.Namespace) -> int:
    from mega_converter.rules import load_rules

    rules = load_rules([Path(p) for p in a.files])
    for r in rules:
        print(f"ok  {r.priority:>5}  {r.phase:<15} {r.scope:<10} {r.name}")
    print(f"{len(rules)} rules valid")
    return 0


def cmd_formulas_check(a: argparse.Namespace) -> int:
    from mega_converter.conversion.formulas import load_formulas

    fs = load_formulas(Path(a.file))
    for t, formulas in fs.items():
        for name, f in formulas.items():
            print(f"ok  {t}.{name}  inputs={sorted(f.inputs)}")
    return 0


def cmd_preview(a: argparse.Namespace) -> int:
    from mega_converter.config import load_campaign
    from mega_converter.conversion.overrides import OverrideStore
    from mega_converter.pipeline.context import PipelineContext
    from mega_converter.pipeline.runner import run
    from mega_converter.rules import load_rules
    from mega_converter.rules.history_correction import compile_history_correction

    cfg = load_campaign(Path(a.campaign))
    base = Path(a.campaign).parent
    world = json.loads(Path(a.world).read_text(encoding="utf-8"))
    rules = load_rules([base / p for p in cfg.rule_files]) + compile_history_correction(cfg.history_correction)
    events = []
    if a.events:
        events = yaml.safe_load(Path(a.events).read_text(encoding="utf-8")) or []
    overrides = {}
    if a.overrides:
        store = OverrideStore(Path(a.overrides))
        for o in store.all():
            overrides.setdefault(o.uid, {})[o.field] = o.value
    ctx = run(PipelineContext(cfg, world, a.from_game, a.to_game, events=events, rules=rules, overrides=overrides))
    names = {uid: c.get("name", uid) for uid, c in (world.get("countries") or {}).items()}
    print("Rule effects:")
    for phase, outcome in ctx.rule_outcomes.items():
        for uid, fx in sorted(outcome.effects.items()):
            parts = [f"{k}{v:+.2f}" for k, v in sorted(fx.modifiers.items())] + sorted(fx.flags)
            parts += [f"recommend {r[0]}" for r in fx.recommendations]
            parts += [f"protect {k}={v}" for k, v in sorted(fx.protections.items())]
            if parts:
                print(f"  [{phase}] {names.get(uid, uid)}: {', '.join(parts)}")
    print("\nState transformation:")
    for uid, d in sorted(ctx.decisions.items()):
        print(f"{names.get(uid, uid):<28} → {d.convert_as:<17} score={d.score:.2f}  [{d.source}]")
        for reason in d.reasons:
            print(f"{'':<31}- {reason}")
    for w in ctx.warnings:
        print(f"warning: {w}", file=sys.stderr)
    if a.verbose:
        for n in ctx.notes:
            print(f"note: {n}", file=sys.stderr)
    return 0


def cmd_not_ready(a: argparse.Namespace) -> int:
    print(f"'{a.command}' is part of MVP step {a.mvp_step}; see docs/ARCHITECTURE.md §15", file=sys.stderr)
    return 3


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="mega-converter", description="Paradox Mega Campaign Converter")
    p.add_argument("--version", action="version", version=f"%(prog)s {__version__}")
    p.add_argument("--log-dir", type=Path, default=None)
    sub = p.add_subparsers(dest="command", required=True)

    s = sub.add_parser("inspect", help="show save packaging and top-level sections")
    s.add_argument("save")
    s.add_argument("--top", type=int, default=40)
    s.set_defaults(func=cmd_inspect)

    s = sub.add_parser("rules-check", help="validate rule files")
    s.add_argument("files", nargs="+")
    s.set_defaults(func=cmd_rules_check)

    s = sub.add_parser("formulas-check", help="validate a formulas file")
    s.add_argument("file")
    s.set_defaults(func=cmd_formulas_check)

    s = sub.add_parser("preview", help="history → rules → state transformation on a UWM JSON")
    s.add_argument("--from", dest="from_game", choices=GAMES, required=True)
    s.add_argument("--to", dest="to_game", choices=GAMES, required=True)
    s.add_argument("--world", required=True, help="UWM JSON (export-world output or fixture)")
    s.add_argument("--campaign", required=True, help="campaign.yaml")
    s.add_argument("--events", help="timeline events YAML (until campaign.db import exists)")
    s.add_argument("--overrides", help="campaign_overrides.yaml")
    s.add_argument("-v", "--verbose", action="store_true")
    s.set_defaults(func=cmd_preview)

    for name, step, helptext in (
        ("convert", "4-8", "full conversion to a target mod"),
        ("validate", "7", "validate a generated mod directory"),
        ("timeline", "4", "print campaign timeline from campaign.db"),
        ("export-world", "3", "parse a save and export UWM JSON"),
    ):
        s = sub.add_parser(name, help=f"{helptext} (not implemented yet)")
        s.add_argument("args", nargs="*")
        s.set_defaults(func=cmd_not_ready, mvp_step=step)
    return p


def main(argv: list[str] | None = None) -> int:
    a = build_parser().parse_args(argv)
    setup_logging(a.log_dir)
    try:
        return int(a.func(a))
    except CoreUnavailable as e:
        print(f"error: {e}", file=sys.stderr)
        return 2
    except (ValueError, OSError) as e:
        print(f"error: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
