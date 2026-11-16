#!/usr/bin/env python3
"""Inspect every archive member, including executable bytes; print no matches."""
import hashlib
import json
from pathlib import Path
import re
import sys
import zipfile

PATTERNS={
    'personal-build-path':rb'(?:/Users/[^\s\x00/]+/|/home/[^\s\x00/]+/|[A-Za-z]:[\\/]Users[\\/][^\s\x00\\/]+[\\/])',
    'private-notes':rb'iCloud~md~obsidian|obsidian://|Mobile Documents',
    'private-key':rb'-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----',
    'credential':rb'(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{50,}|sk-proj-[A-Za-z0-9_-]{40,})',
}
FORBIDDEN={'AGENTS.md','.DS_Store','Cargo.toml','Cargo.lock','.env'}

def inspect(archive):
    entries=[];failures=[]
    with zipfile.ZipFile(archive) as zip:
        for info in zip.infolist():
            if info.is_dir():continue
            name=info.filename
            if name.startswith('/') or '..' in Path(name).parts:failures.append((name,'unsafe-path'))
            if Path(name).name in FORBIDDEN and 'OpenArchiMCP' not in name and '/Licenses/' not in name:
                failures.append((name,'unexpected-file'))
            if any('/'+part+'/' in '/'+name for part in ['.git','skills','__pycache__','.venv','recovery','logs']):
                failures.append((name,'private-directory'))
            data=zip.read(info)
            for category,pattern in PATTERNS.items():
                if re.search(pattern,data):failures.append((name,category))
            entries.append({'path':name,'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()})
    result={'archive':archive.name,'sha256':hashlib.sha256(archive.read_bytes()).hexdigest(),'files':entries,'failures':failures}
    (archive.parent/(archive.name+'.audit.json')).write_text(json.dumps(result,indent=2)+'\n')
    if failures:raise ValueError('Package audit failed: '+str(failures))
    print(archive.name,'full-member byte scan passed; files:',len(entries))

if __name__=='__main__':
    source=Path(sys.argv[1]);archives=sorted(source.glob('OpenArchi-0.2.0-*.zip')) if source.is_dir() else [source]
    if not archives:raise SystemExit('No release archives found')
    for archive in archives:inspect(archive)
