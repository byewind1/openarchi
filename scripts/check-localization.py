#!/usr/bin/env python3
"""Check literal translation keys against both bundled locale packs.

This is deliberately a static check: it finds literal ``translate("...")`` and
``Self::t(language, "...")`` calls in Rust sources, then verifies that both
``locales/en.json`` and ``locales/zh-CN.json`` contain a string entry for every
key. Dynamic keys and text composed at runtime are outside this check and are
covered by focused UI tests where needed.
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LOCALES = ROOT / "locales"
KEYS = (
    re.compile(r'\.translate\s*\(\s*"((?:[^"\\]|\\.)*)"'),
    re.compile(r'Self::t\s*\(\s*[^,()]+,\s*"((?:[^"\\]|\\.)*)"'),
)
# Values that are intentionally identical in both languages: product names,
# scripts/symbols, and the test-only fallback key.
ALLOW_IDENTICAL = {
    "OpenArchi",
    "ΔX",
    "ΔY",
}
# Deliberately absent from the Chinese pack: exercises fallback behaviour.
IGNORED_KEYS = {"Untranslated key"}


def decode_rust_string(value: str) -> str:
    if "\\" not in value:
        return value
    try:
        return bytes(value, "utf-8").decode("unicode_escape")
    except UnicodeDecodeError:
        return value


def literal_keys() -> dict[str, set[str]]:
    found: dict[str, set[str]] = {}
    for path in sorted((ROOT / "src").rglob("*.rs")):
        text = path.read_text(encoding="utf-8")
        for pattern in KEYS:
            for match in pattern.finditer(text):
                key = decode_rust_string(match.group(1))
                found.setdefault(key, set()).add(str(path.relative_to(ROOT)))
    return found


def load_locale(path: Path) -> tuple[dict[str, str], list[str]]:
    duplicates: list[str] = []

    def pairs_hook(pairs: list[tuple[str, object]]) -> dict[str, object]:
        result: dict[str, object] = {}
        for key, value in pairs:
            if key in result:
                duplicates.append(key)
            result[key] = value
        return result

    data = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=pairs_hook)
    if not isinstance(data, dict) or any(not isinstance(value, str) for value in data.values()):
        raise ValueError(f"{path} must be a JSON object with string values")
    return data, duplicates


def has_cjk(text: str) -> bool:
    return bool(re.search(r"[\u3400-\u9fff]", text))


def main() -> int:
    found = literal_keys()
    keys = set(found) - IGNORED_KEYS
    english, en_duplicates = load_locale(LOCALES / "en.json")
    chinese, zh_duplicates = load_locale(LOCALES / "zh-CN.json")
    missing_en = sorted(keys - set(english))
    missing_zh = sorted(keys - set(chinese))
    identical = sorted(
        key
        for key in keys
        if key in chinese
        and chinese[key] == key
        and not has_cjk(key)
        and key not in ALLOW_IDENTICAL
    )
    problems = bool(missing_en or missing_zh or en_duplicates or zh_duplicates or identical)
    print(
        f"localization: {len(keys)} literal keys; "
        f"en missing {len(missing_en)}; zh missing {len(missing_zh)}; "
        f"duplicate en {len(en_duplicates)}; duplicate zh {len(zh_duplicates)}; "
        f"untranslated zh {len(identical)}"
    )
    for label, items in (
        ("missing in en.json", missing_en),
        ("missing in zh-CN.json", missing_zh),
        ("duplicate key in en.json", en_duplicates),
        ("duplicate key in zh-CN.json", zh_duplicates),
        ("zh-CN value still equals its English key", identical),
    ):
        for item in items:
            print(f"  {label}: {item}")
    if not problems:
        print("localization: OK")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
