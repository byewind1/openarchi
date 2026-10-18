import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/check-localization.py"
spec = importlib.util.spec_from_file_location("check_localization", SCRIPT)
check = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(check)


class LocalizationCheckTests(unittest.TestCase):
    def test_literal_key_patterns_find_translate_and_self_t(self):
        text = '''
            self.language.translate("Wall");
            Self::t(self.language, "Slab");
            self.language.translate(dynamic_key);
        '''
        found = set()
        for pattern in check.KEYS:
            found.update(match.group(1) for match in pattern.finditer(text))
        self.assertEqual(found, {"Wall", "Slab"})

    def test_load_locale_reports_duplicate_keys(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "locale.json"
            path.write_text('{"Wall": "Wall", "Wall": "Duplicate"}', encoding="utf-8")
            values, duplicates = check.load_locale(path)
            self.assertEqual(values["Wall"], "Duplicate")
            self.assertEqual(duplicates, ["Wall"])

    def test_repository_locale_packs_pass(self):
        result = subprocess.run(
            [sys.executable, str(SCRIPT)],
            cwd=ROOT,
            text=True,
            capture_output=True,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("localization: OK", result.stdout)

    def test_repository_packs_have_no_duplicate_members(self):
        for name in ("en.json", "zh-CN.json"):
            _, duplicates = check.load_locale(ROOT / "locales" / name)
            self.assertEqual(duplicates, [], f"duplicate keys in {name}")


if __name__ == "__main__":
    unittest.main()
