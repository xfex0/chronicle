"""Rules DSL (docs/ARCHITECTURE.md §11)."""

from mega_converter.rules.dsl import Rule, RuleSyntaxError, load_rules, parse_rules
from mega_converter.rules.engine import EntityContext, EntityEffects, RuleEngine, RuleOutcome

__all__ = [
    "EntityContext",
    "EntityEffects",
    "Rule",
    "RuleEngine",
    "RuleOutcome",
    "RuleSyntaxError",
    "load_rules",
    "parse_rules",
]
