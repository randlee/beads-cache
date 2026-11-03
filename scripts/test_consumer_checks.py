"""Product output remains UTF-8 even when the host subprocess locale is CP1252."""

from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

import consumer_checks  # noqa: E402


class ProductEncodingTest(unittest.TestCase):
    def test_utf8_diagnostic_survives_windows_locale(self):
        diagnostic = "CFG.PARSE invalid JSON — Fix the JSON.\n"
        command = [sys.executable, "-I", "-c",
                   f"import os; os.write(2, {diagnostic.encode('utf-8')!r}); raise SystemExit(1)"]
        # Simulate Python's Windows locale decoder without changing process-global
        # locale or relying on a particular Python UTF-8 mode setting.
        with patch("subprocess._text_encoding", return_value="cp1252"):
            locale_result = subprocess.run(command, capture_output=True, text=True, timeout=10)
            self.assertNotEqual(locale_result.stderr, diagnostic)
            result = consumer_checks.run_product(command, timeout=10)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, "")
        self.assertEqual(result.stderr, diagnostic)
        self.assertTrue(result.stderr.startswith("CFG.PARSE ") and " — " in result.stderr
                        and len(result.stderr.splitlines()) == 1)

    def test_invalid_utf8_is_not_silently_replaced(self):
        command = [sys.executable, "-I", "-c", "import os; os.write(2, b'\\xff')"]
        with self.assertRaises(UnicodeDecodeError):
            consumer_checks.run_product(command, timeout=10)


if __name__ == "__main__":
    unittest.main()
