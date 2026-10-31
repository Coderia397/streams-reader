"""Load brand/tokens.toml and resolve references into concrete values."""

from __future__ import annotations

import re
import tomllib
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from .colors import Rgba, contrast

_REF = re.compile(r"^\{([a-z0-9_.\-]+?)(?:/([0-9.]+))?\}$")

# Scalar tables emitted as CSS custom properties: table -> prefix.
SCALAR_TABLES = {
    "radius": "radius-",
    "space": "space-",
    "font": "font-",
    "text": "text-",
    "leading": "leading-",
    "weight": "weight-",
    "tracking": "tracking-",
    "motion": "",
    "component": "",
    "z": "z-",
}


@dataclass
class Theme:
    name: str
    colors: dict[str, Rgba]
    shadows: dict[str, str]


@dataclass
class Tokens:
    raw: dict[str, Any]
    palette: dict[str, Rgba]
    themes: dict[str, Theme]
    scalars: dict[str, dict[str, str]]
    breakpoints: dict[str, str]
    art_colors: dict[str, Rgba]
    art: dict[str, Any]
    meta: dict[str, Any] = field(default_factory=dict)

    @property
    def default_theme(self) -> str:
        return self.meta["default_theme"]

    @property
    def light_theme(self) -> str:
        return self.meta["light_theme"]


class TokenError(ValueError):
    pass


def load(path: Path) -> Tokens:
    raw = tomllib.loads(path.read_text(encoding="utf-8"))
    return resolve(raw)


def _lookup(raw: dict[str, Any], dotted: str) -> Any:
    node: Any = raw
    for part in dotted.split("."):
        if not isinstance(node, dict) or part not in node:
            raise TokenError(f"unknown token reference {{{dotted}}}")
        node = node[part]
    return node


def _resolve_str(raw: dict[str, Any], value: str, depth: int = 0) -> tuple[str, float | None]:
    if depth > 8:
        raise TokenError(f"reference cycle at {value!r}")
    m = _REF.match(value)
    if not m:
        return value, None
    target = _lookup(raw, m.group(1))
    if not isinstance(target, str):
        raise TokenError(f"{{{m.group(1)}}} is not a string token")
    inner, inner_alpha = _resolve_str(raw, target, depth + 1)
    alpha = float(m.group(2)) if m.group(2) else inner_alpha
    return inner, alpha


def resolve_color(raw: dict[str, Any], value: str) -> Rgba:
    text, alpha = _resolve_str(raw, value)
    color = Rgba.parse(text)
    return color.with_alpha(alpha) if alpha is not None else color


def _scalar(raw: dict[str, Any], value: str) -> str:
    """A reference to another emitted scalar stays live as var(--…); others resolve."""
    m = _REF.match(value)
    if m and not m.group(2):
        table, _, key = m.group(1).partition(".")
        if table in SCALAR_TABLES and "." not in key:
            _lookup(raw, m.group(1))
            return f"var(--{SCALAR_TABLES[table]}{key})"
    return _resolve_str(raw, value)[0]


def resolve(raw: dict[str, Any]) -> Tokens:
    palette = {k: Rgba.parse(v) for k, v in raw["palette"].items()}
    themes: dict[str, Theme] = {}
    for name in raw["meta"]["themes"]:
        spec = raw["theme"][name]
        colors = {k: resolve_color(raw, v) for k, v in spec["color"].items()}
        shadows = {str(k): v for k, v in spec.get("shadow", {}).items()}
        themes[name] = Theme(name, colors, shadows)
    _check_same_keys(themes)
    scalars = {
        table: {str(k): _scalar(raw, str(v)) for k, v in raw.get(table, {}).items()}
        for table in SCALAR_TABLES
    }
    art_colors = {k: resolve_color(raw, v) for k, v in raw["art"]["color"].items()}
    art = {k: v for k, v in raw["art"].items() if k != "color"}
    return Tokens(
        raw=raw,
        palette=palette,
        themes=themes,
        scalars=scalars,
        breakpoints=dict(raw.get("breakpoint", {})),
        art_colors=art_colors,
        art=art,
        meta=raw["meta"],
    )


def _check_same_keys(themes: dict[str, Theme]) -> None:
    names = list(themes)
    first = themes[names[0]]
    for other in names[1:]:
        t = themes[other]
        if set(t.colors) != set(first.colors) or set(t.shadows) != set(first.shadows):
            missing = (set(first.colors) ^ set(t.colors)) | (set(first.shadows) ^ set(t.shadows))
            raise TokenError(f"theme {other} differs from {names[0]} in: {sorted(missing)}")


def css_vars(tokens: Tokens) -> dict[str, str]:
    """Theme-independent custom properties, name (without --) -> value."""
    out: dict[str, str] = {}
    for table, prefix in SCALAR_TABLES.items():
        for key, value in tokens.scalars[table].items():
            out[f"{prefix}{key}"] = value
    return out


def theme_vars(theme: Theme) -> dict[str, str]:
    out = {f"color-{k}": c.css for k, c in theme.colors.items()}
    out.update({f"shadow-{k}": v for k, v in theme.shadows.items()})
    return out


@dataclass(frozen=True)
class ContrastResult:
    theme: str
    fg: str
    bg: str
    ratio: float
    minimum: float

    @property
    def ok(self) -> bool:
        return self.ratio + 1e-9 >= self.minimum


def contrast_report(tokens: Tokens) -> list[ContrastResult]:
    results: list[ContrastResult] = []
    for rule in tokens.raw.get("contrast", []):
        for theme in tokens.themes.values():
            base = theme.colors["bg"]
            for bg_name in rule["bg"]:
                bg = theme.colors[bg_name]
                bg = bg.over(base) if bg.a < 1 else bg
                for fg_name in rule["fg"]:
                    ratio = contrast(theme.colors[fg_name], bg)
                    results.append(ContrastResult(theme.name, fg_name, bg_name, ratio, rule["min"]))
    return results
