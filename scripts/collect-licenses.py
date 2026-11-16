#!/usr/bin/env python3
"""Collect distributable dependency licenses without leaking registry paths."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import os
import re
import urllib.request

ROOT=Path(__file__).resolve().parents[1]
command=['cargo','metadata','--locked','--format-version','1'] if os.name=='nt' else [str(ROOT/'scripts/cargo.sh'),'metadata','--locked','--format-version','1']
data=json.loads(subprocess.check_output(command,cwd=ROOT))
destination=ROOT/'assets/licenses/dependencies';destination.mkdir(parents=True,exist_ok=True)
entries=[]
for package in data['packages']:
    if package['name']=='atelier-bim':continue
    source=Path(package['manifest_path']).parent
    files=[p for p in source.rglob('*') if p.is_file() and p.name.upper().startswith(('LICENSE','LICENCE','COPYING','NOTICE','UNLICENSE'))]
    if package.get('license_file'):
        files.append(source/package['license_file'])
    record={'name':package['name'],'version':package['version'],'license':package.get('license'),
            'authors':package.get('authors',[]),'repository':package.get('repository'),
            'source':f"https://crates.io/api/v1/crates/{package['name']}/{package['version']}/download",'texts':[]}
    for file in sorted(set(files)):
        if file.is_symlink():raise ValueError('Unexpected license symlink')
        relative=Path(package['name']+'-'+package['version'])/file.relative_to(source)
        target=destination/relative;target.parent.mkdir(parents=True,exist_ok=True)
        shutil.copyfile(file,target)
        record['texts'].append(str(relative))
    existing=destination/(package['name']+'-'+package['version'])
    if not record['texts'] and existing.is_dir():
        record['texts']=[str(p.relative_to(destination)) for p in existing.rglob('*') if p.is_file()]
    license=package.get('license') or ''
    def save_text(name, content):
        relative=Path(package['name']+'-'+package['version'])/name
        target=destination/relative;target.parent.mkdir(parents=True,exist_ok=True)
        target.write_bytes(content);record['texts'].append(str(relative))
    def upstream(names):
        repo=package.get('repository') or ''
        match=re.search(r'github.com/([^/]+/[^/]+)',repo)
        vcs=source/'.cargo_vcs_info.json'
        if not match or not vcs.exists():return False
        commit=json.loads(vcs.read_text())['git']['sha1']
        for name in names:
            url=f'https://raw.githubusercontent.com/{match[1].removesuffix(".git")}/{commit}/{name}'
            try:
                with urllib.request.urlopen(url,timeout=15) as response:content=response.read()
                save_text(Path(name).name,content);return True
            except Exception:continue
        return False
    # Apache-2.0 OR alternatives permit choosing Apache-2.0. Its standard text
    # is unchanged; package/source attribution remains in the manifest.
    if not record['texts'] and 'Apache-2.0' in license and 'AND' not in license:
        save_text('LICENSE-APACHE', (ROOT/'LICENSE').read_bytes())
        record['distribution_license']='Apache-2.0'
    if not record['texts']:
        upstream(['LICENSE-MIT.txt','LICENSE-MIT','LICENSE','LICENSE.txt','LICENSE.md','COPYING'])
    if package['name']=='epaint_default_fonts':
        save_text('LICENSE-APACHE',(ROOT/'LICENSE').read_bytes())
        for name in ['OFL.txt','UFL.txt','Hack-Regular.txt','emoji-icon-font-mit-license.txt']:
            file=source/'fonts'/name
            if not file.is_file():raise ValueError('Missing bundled-font license '+name)
            save_text(name,file.read_bytes())
    if not record['texts'] and license in ['MPL-2.0','MIT','BSL-1.0']:
        # Some published crate archives omit a standalone license. Preserve
        # their declared license and manifest attribution with the SPDX text.
        with urllib.request.urlopen(f'https://raw.githubusercontent.com/spdx/license-list-data/main/text/{license}.txt',timeout=15) as response:
            save_text('LICENSE-'+license,response.read())
        save_text('ATTRIBUTION.txt',('\n'.join([package['name']+' '+package['version'],*package.get('authors',[]),package.get('repository') or '',record['source']])+'\n').encode())
    if not record['texts']:raise ValueError('License text unavailable for '+package['name'])
    entries.append(record)
(destination/'index.json').write_text(json.dumps(entries,indent=2)+'\n')
print('Collected licenses for',len(entries),'dependencies')
