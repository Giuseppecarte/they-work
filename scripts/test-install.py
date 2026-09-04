#!/usr/bin/env python3
"""Exercise the documented bootstrap and installer failure boundaries offline."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
BOOTSTRAP = (ROOT / "INSTALL.md").read_text().split("~~~bash\n", 1)[1].split("~~~", 1)[0]
RELEASE_SCRIPT = (ROOT / "scripts/fixtures/install-v0.1.0.sh").read_bytes()


class InstallFailures(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="they-work-install-test-")
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        (self.base / "release.sh").write_bytes(RELEASE_SCRIPT)
        self.env = dict(os.environ, PATH=f"{self.base}:{os.environ['PATH']}",
                        INSTALL_TEST_DIR=str(self.base), INSTALL_TEST_MODE="valid",
                        INSTALL_TEST_ERROR="denied", THEYWORK_IMAGE="example.invalid/test:missing")
        self.mock("curl", '''#!/usr/bin/env python3
import os, pathlib, sys
base = pathlib.Path(os.environ["INSTALL_TEST_DIR"])
mode = os.environ["INSTALL_TEST_MODE"]
if mode == "download_failure":
    print("curl: (22) HTTP 404 (deliberate test)", file=sys.stderr)
    sys.exit(22)
payload = (base / "release.sh").read_bytes()
if mode == "truncated":
    payload = b"#!/usr/bin/env sh\\n# truncated at a valid shell boundary\\nset -eu\\n"
pathlib.Path(sys.argv[sys.argv.index("-o") + 1]).write_bytes(payload)
''')
        self.mock("docker", '''#!/usr/bin/env python3
import os, pathlib, sys
base = pathlib.Path(os.environ["INSTALL_TEST_DIR"])
(base / "docker-called").write_text(" ".join(sys.argv[1:]))
print(os.environ["INSTALL_TEST_ERROR"], file=sys.stderr)
sys.exit(17)
''')

    def mock(self, name, source):
        path = self.base / name
        path.write_text(source)
        path.chmod(0o755)

    def run_script(self, command):
        result = subprocess.run(command, cwd=self.base, env=self.env, text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        print(f"\n{self.id()}: exit={result.returncode}\n{result.stdout}", end="")
        return result

    def test_download_failure_never_executes(self):
        self.env["INSTALL_TEST_MODE"] = "download_failure"
        result = self.run_script(["sh", "-c", BOOTSTRAP])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Installer download failed", result.stdout)
        self.assertFalse((self.base / "docker-called").exists())

    def test_valid_shell_truncation_never_executes(self):
        self.env["INSTALL_TEST_MODE"] = "truncated"
        result = self.run_script(["sh", "-c", BOOTSTRAP])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("truncated or modified download", result.stdout)
        self.assertFalse((self.base / "docker-called").exists())

    def test_complete_release_reaches_pull_and_propagates_failure(self):
        result = self.run_script(["sh", "-c", BOOTSTRAP])
        self.assertEqual(result.returncode, 17)
        self.assertIn("denied", result.stdout)
        self.assertTrue((self.base / "docker-called").exists())

    def test_local_installer_names_missing_image(self):
        self.env["INSTALL_TEST_ERROR"] = "manifest unknown"
        result = self.run_script(["sh", str(ROOT / "docs/install.sh")])
        self.assertEqual(result.returncode, 17)
        self.assertIn("image or tag not found", result.stdout)

    def test_local_installer_does_not_guess_from_denied(self):
        result = self.run_script(["sh", str(ROOT / "docs/install.sh")])
        self.assertEqual(result.returncode, 17)
        self.assertIn("registry access denied", result.stdout)
        self.assertIn("cannot distinguish", result.stdout)

    def test_local_installer_names_other_pull_error(self):
        self.env["INSTALL_TEST_ERROR"] = "network unreachable"
        result = self.run_script(["sh", str(ROOT / "docs/install.sh")])
        self.assertEqual(result.returncode, 17)
        self.assertIn("Docker exit 17", result.stdout)


if __name__ == "__main__":
    unittest.main(verbosity=2)
