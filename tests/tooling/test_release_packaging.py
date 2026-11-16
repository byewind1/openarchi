import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

SPEC=importlib.util.spec_from_file_location('release_package',Path(__file__).resolve().parents[2]/'scripts/package.py')
PACKAGE=importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGE)

class PackageIsolationTests(unittest.TestCase):
    def test_windows_package_excludes_unlisted_and_stale_private_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            files={'Cargo.toml':'[package]\nversion="0.2.0"\n',
                   'target/x86_64-pc-windows-msvc/release/openarchi.exe':'synthetic executable',
                   'AGENTS.md':'PRIVATE_SENTINEL',
                   'assets/plugins/unreviewed.json':'PRIVATE_SENTINEL',
                   'dist/OpenArchi-0.2.0-Windows-x64/old-recovery.json':'PRIVATE_SENTINEL',
                   'assets/licenses/dependencies/index.json':'[]'}
            files.update({name:'synthetic license' for name in PACKAGE.LICENSES})
            files.update({'assets/plugins/'+name:'{}' for name in PACKAGE.PLUGINS})
            for name,text in files.items():
                path=root/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text)
            with patch.object(PACKAGE,'ROOT',root),patch.object(PACKAGE.subprocess,'check_output',return_value='a'*40):
                archive=PACKAGE.package('x86_64-pc-windows-msvc')
            with zipfile.ZipFile(archive) as result:
                self.assertFalse(any('AGENTS' in name or 'recovery' in name or 'unreviewed' in name for name in result.namelist()))
                self.assertFalse(any(b'PRIVATE_SENTINEL' in result.read(name) for name in result.namelist()))
                receipt=json.loads(result.read('OpenArchi-0.2.0-Windows-x64/build.json'))
                self.assertEqual(receipt['version'],'0.2.0')
            self.assertEqual((root/'AGENTS.md').read_text(),'PRIVATE_SENTINEL')

if __name__=='__main__':unittest.main()
