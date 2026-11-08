#!/usr/bin/env python3
"""Check the public film source and shipped assets before packaging."""
from pathlib import Path
import re
import subprocess
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]


def check():
    errors = []
    film = ROOT / 'crates/rawmakase-engine/src/develop/film'
    expected = {'grain.rs', 'compression.rs'}
    actual = {p.name for p in film.iterdir() if p.is_file()}
    if actual != expected:
        errors.append(f'Unexpected film module or resource: {sorted(actual ^ expected)}')
    if (ROOT / 'crates/rawmakase-engine/build.rs').exists():
        errors.append('The film engine must build without a resource-generation hook')
    paths = subprocess.check_output(
        ['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard'],
        cwd=ROOT,
    ).decode().split('\0')
    for name in filter(None, paths):
        path = ROOT / name
        if not path.is_file():
            continue
        data = path.read_bytes()
        if re.search(rb'-----BEGIN (?:RSA |EC |OPENSSH |ENCRYPTED )?PRIVATE KEY-----', data):
            errors.append(f'Private signing material: {name}')
        if name.startswith(('crates/rawmakase-engine/src/develop/film', 'crates/rawmakase-model/src/model/film')):
            text = data.decode('utf-8')
            if re.search(r'(?i)damage|include_bytes!|FILM_TEXTURES', text):
                errors.append(f'Excluded effect or external resource dependency: {name}')
    presets = sorted((ROOT / 'assets/presets/FilmEffects').glob('*'))
    if len(presets) != 11 or any(p.suffix != '.xmp' for p in presets):
        errors.append('Expected exactly eleven source-only film presets')
    for path in presets:
        ET.parse(path)
        if 'damage' in path.read_text().lower():
            errors.append(f'Excluded preset setting: {path.name}')
    for name in ['LICENSE', 'THIRD_PARTY_NOTICES.txt', 'docs/provenance.md']:
        if not (ROOT / name).is_file():
            errors.append(f'Missing distribution notice: {name}')
    if errors:
        raise SystemExit('\n'.join(errors))
    print('Public film source, preset and signing-material checks passed')


if __name__ == '__main__':
    check()
