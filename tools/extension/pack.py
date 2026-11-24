#!/usr/bin/env python3
"""Build version-matched browser packages without a PowerShell dependency."""
from pathlib import Path
import json
import re
import shutil
import tomllib
import zipfile

root = Path(__file__).resolve().parents[2]
source = root / 'extension'
out = root / 'target/extension'
version = tomllib.loads((root / 'Cargo.toml').read_text())['workspace']['package']['version']
if not re.fullmatch(r'\d+(\.\d+){0,3}', version):
    raise SystemExit('Browser manifests require a numeric version')
for kind in ('chromium', 'firefox', 'safari', 'chrome-store'):
    dest = out / kind
    if dest.exists():
        shutil.rmtree(dest)
    dest.mkdir(parents=True)
    for name in ('background.js', 'video.js', 'page.js', 'popup.html', 'popup.css', 'popup.js'):
        shutil.copy2(source / name, dest / name)
    for name in ('LICENSE', 'ATTRIBUTION.md'):
        shutil.copy2(root / name, dest / name)
    for name in ('icons', '_locales'):
        shutil.copytree(source / name, dest / name)
    manifest = source / ({'firefox':'manifest.firefox.json', 'safari':'manifest.safari.json'}.get(kind, 'manifest.json'))
    data = json.loads(manifest.read_text())
    data['version'] = version
    if kind == 'chrome-store':
        data.pop('key', None)
    if kind == 'firefox' and (dest/'_locales/no').exists():
        shutil.copytree(dest/'_locales/no', dest/'_locales/nb', dirs_exist_ok=True)
    (dest/'manifest.json').write_text(json.dumps(data, ensure_ascii=False, indent=2)+'\n')
    if kind != 'safari':
        filename = {'chromium':'ndm-chromium.zip', 'firefox':'ndm-firefox.xpi', 'chrome-store':'ndm-chrome-store.zip'}[kind]
        with zipfile.ZipFile(out/filename, 'w', zipfile.ZIP_DEFLATED) as archive:
            for path in sorted(dest.rglob('*')):
                if path.is_file():
                    archive.write(path, path.relative_to(dest).as_posix())
print(f'Built NDM browser packages {version} in {out}')
