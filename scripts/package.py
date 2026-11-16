#!/usr/bin/env python3
"""Assemble only allowlisted runtime files in an empty, owned staging tree."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import shutil
import subprocess
import tempfile
import tomllib
import zipfile

ROOT=Path(__file__).resolve().parents[1]
LICENSES = ['LICENSE', 'THIRD_PARTY_NOTICES.md', 'assets/fonts/OFL.txt',
            'vendor/accesskit_macos/LICENSE-MIT', 'vendor/accesskit_macos/LICENSE-APACHE']
PLUGINS=['example-quantities.json','example-zone-program.json','openarchi-quantities.json','openarchi-solar.json','README.md']

def copy(source, destination):
    source=ROOT/source
    if source.is_symlink() or not source.is_file():
        raise ValueError('Invalid release input: '+str(source.relative_to(ROOT)))
    destination.parent.mkdir(parents=True,exist_ok=True)
    shutil.copyfile(source,destination)

def package(target=None):
    version=tomllib.loads((ROOT/'Cargo.toml').read_text())['package']['version']
    windows = (target and 'windows' in target) or os.name=='nt'
    arch='x64' if windows else ('intel' if (target and target.startswith('x86_64')) or platform.machine()=='x86_64' else 'arm64')
    system='Windows' if windows else 'macOS'
    name=f'OpenArchi-{version}-{system}-{arch}'
    output=ROOT/'dist';output.mkdir(exist_ok=True)
    binary=ROOT/'target'
    if target:binary=binary/target
    binary=binary/'release'/('openarchi.exe' if windows else 'openarchi')
    if not binary.is_file():raise ValueError('Build the selected target first')
    with tempfile.TemporaryDirectory(prefix='openarchi-package-') as directory:
        stage=Path(directory)
        if windows:
            product=stage/name;product.mkdir();resources=product
            shutil.copyfile(binary,product/'OpenArchi.exe')
        else:
            product=stage/'OpenArchi.app';resources=product/'Contents/Resources'
            exe=product/'Contents/MacOS/openarchi';exe.parent.mkdir(parents=True)
            shutil.copyfile(binary,exe);exe.chmod(0o755)
            # Apple ld records N_OSO archive paths even with remapped Rust
            # sources. Remove debug/local symbols before signing, then scan.
            subprocess.run(['/usr/bin/strip','-S','-x',str(exe)],check=True)
            info=plistlib.loads((ROOT/'packaging/Info.plist').read_bytes())
            info['CFBundleShortVersionString']=version;info['CFBundleVersion']=version
            (product/'Contents/Info.plist').write_bytes(plistlib.dumps(info))
            iconset=stage/'OpenArchi.iconset';iconset.mkdir()
            for size in [16,32,128,256,512]:
                for scale in [1,2]:
                    suffix='@2x' if scale==2 else ''
                    subprocess.run(['sips','-s','format','png','-z',str(size*scale),str(size*scale),str(ROOT/'icon.png'),'--out',str(iconset/f'icon_{size}x{size}{suffix}.png')],check=True,stdout=subprocess.DEVNULL)
            resources.mkdir(parents=True)
            subprocess.run(['python3',str(ROOT/'scripts/png-to-icns.py'),str(iconset),str(resources/'OpenArchi.icns')],check=True)
        for license in LICENSES:
            copy(license,resources/'Licenses'/license)
        licenses=ROOT/'assets/licenses/dependencies'
        if not (licenses/'index.json').is_file():
            raise ValueError('Run scripts/collect-licenses.py before packaging')
        for file in sorted(licenses.rglob('*')):
            if file.is_file():
                copy(str(file.relative_to(ROOT)),resources/'Licenses/dependencies'/file.relative_to(licenses))
        standard=ROOT/'assets/licenses/rust-standard-library'
        for file in sorted(standard.rglob('*')):
            if file.is_file():
                copy(str(file.relative_to(ROOT)),resources/'Licenses/rust-standard-library'/file.relative_to(standard))
        (resources/'README.txt').write_text(f'OpenArchi {version}\n\nEarly software; not intended for production delivery. Keep independent project backups.\nDWG conversion requires your own compatible dwg2dxf executable; no converter is included.\nWindows packages are unsigned. macOS packages are ad-hoc signed and not notarized.\n\nUsage, limitations and feedback:\nhttps://github.com/byewind1/openarchi\n',encoding='utf-8')
        for filename in PLUGINS:
            copy('assets/plugins/'+filename,resources/'PluginExamples'/filename)
        if not windows:
            subprocess.run(['python3',str(ROOT/'scripts/package-mcp-adapter.py'),str(resources)],check=True)
        staged_binary=product/'OpenArchi.exe' if windows else product/'Contents/MacOS/openarchi'
        receipt={'version':version,'commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
                 'target':target or platform.machine(),'build_binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),
                 'binary_before_sign_sha256':hashlib.sha256(staged_binary.read_bytes()).hexdigest()}
        (resources/'build.json').write_text(json.dumps(receipt,indent=2)+'\n')
        if not windows:
            subprocess.run(['codesign','--force','--sign','-',str(product)],check=True)
            subprocess.run(['codesign','--verify','--deep','--strict',str(product)],check=True)
        archive=output/(name+'.zip')
        if windows:
            with zipfile.ZipFile(archive,'w',compression=zipfile.ZIP_DEFLATED) as zip:
                for path in sorted(product.rglob('*')):
                    if path.is_file():zip.write(path,path.relative_to(stage))
        else:
            subprocess.run(['ditto','-c','-k','--sequesterRsrc','--keepParent',str(product),str(archive)],check=True)
            # Install this new bundle atomically without touching mapped vnodes.
            saved=output/('OpenArchi.app' if arch=='arm64' else 'OpenArchi-Intel.app')
            if saved.exists():shutil.rmtree(saved)
            shutil.copytree(product,saved)
        digest=hashlib.sha256(archive.read_bytes()).hexdigest()
        (output/(archive.name+'.sha256')).write_text(digest+'  '+archive.name+'\n')
        print(archive.name,digest)
        return archive

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--target');package(parser.parse_args().target)
