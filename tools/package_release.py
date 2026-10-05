"""Build the release zip (GitHub release asset / NexusMods main file).

usage: python tools/package_release.py      (after `cargo build --release`)
Creates dist/BL4SaveEditor-<version>.zip containing:
  BL4SaveEditor.exe, README.txt, CHANGELOG.md, LICENSE.txt
and prints its SHA-256.
"""
import hashlib
import re
import shutil
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
version = re.search(r'^version = "([^"]+)"', (ROOT / "Cargo.toml").read_text(encoding="utf-8"), re.M).group(1)
exe = ROOT / "target" / "release" / "BL4SaveEditor.exe"
assert exe.exists(), "build first: cargo build --release"

dist = ROOT / "dist"
dist.mkdir(exist_ok=True)
name = f"BL4SaveEditor-{version}"
out = dist / f"{name}.zip"

readme = (ROOT / "docs" / "README.txt").read_text(encoding="utf-8").replace("{version}", version)
files = {
    "BL4SaveEditor.exe": exe.read_bytes(),
    "README.txt": readme.replace("\n", "\r\n").encode("utf-8"),
    "CHANGELOG.md": (ROOT / "CHANGELOG.md").read_bytes(),
    "LICENSE.txt": (ROOT / "LICENSE").read_bytes(),
}
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
    for fname, data in files.items():
        z.writestr(f"{name}/{fname}", data)
shutil.copy2(exe, dist / "BL4SaveEditor.exe")
h = hashlib.sha256(out.read_bytes()).hexdigest()
(dist / f"{name}.zip.sha256").write_text(f"{h}  {out.name}\n", encoding="utf-8")
print(out, f"{out.stat().st_size / 1e6:.1f} MB", "sha256", h)
