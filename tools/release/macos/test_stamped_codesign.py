#!/usr/bin/env python3
"""`stamped_codesign` in build_app.sh: a timestamp server that does not answer is tried again, anything else is not.

Release 0.5.1 failed because Apple's timestamp server did not answer one `codesign` call ("A timestamp was expected but was
not found"): nothing about the bundle was wrong, and the whole release had to be run again. A stub `codesign` on PATH stands
for the tool, and the function is cut out of the script, so the test reads the code that ships.

    python3 tools/release/macos/test_stamped_codesign.py
"""
import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("build_app.sh")


def function_text() -> str:
    text = SCRIPT.read_text()
    start = text.index("SIGN_ATTEMPTS=")
    end = text.index("\n}\n", text.index("stamped_codesign()")) + 3
    return text[start:end]


def run_with(stub_body: str) -> tuple[int, int, str]:
    """(exit status, how many times codesign ran, what the function printed)."""
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        (tmp / "bin").mkdir()
        stub = tmp / "bin" / "codesign"
        stub.write_text(f'#!/bin/bash\nn=$(cat "{tmp}/count" 2>/dev/null || echo 0); n=$((n+1)); echo $n > "{tmp}/count"\n{stub_body}\n')
        stub.chmod(0o755)
        driver = tmp / "driver.sh"
        driver.write_text("set -e\n" + function_text() + '\nstamped_codesign --force --sign x thing\n')
        env = {**os.environ, "PATH": f"{tmp / 'bin'}:{os.environ['PATH']}", "CAULDRON_SIGN_WAIT_S": "0"}
        done = subprocess.run(["bash", str(driver)], env=env, capture_output=True, text=True)
        count = int((tmp / "count").read_text()) if (tmp / "count").exists() else 0
        return done.returncode, count, done.stderr


class StampedCodesign(unittest.TestCase):
    def test_a_timestamp_server_that_does_not_answer_is_tried_again(self):
        body = 'if [ "$n" -lt 3 ]; then echo "thing: A timestamp was expected but was not found." >&2; exit 1; fi'
        status, count, said = run_with(body)
        self.assertEqual((status, count), (0, 3), said)
        self.assertEqual(len(re.findall("did not answer", said)), 2, said)

    def test_it_gives_up_after_the_attempts_and_keeps_codesigns_status(self):
        status, count, _ = run_with('echo "The timestamp service is not available." >&2; exit 7')
        self.assertEqual((status, count), (7, 4))

    def test_any_other_failure_is_a_failure_at_once(self):
        status, count, said = run_with('echo "thing: no identity found" >&2; exit 3')
        self.assertEqual((status, count), (3, 1), said)


if __name__ == "__main__":
    unittest.main()
