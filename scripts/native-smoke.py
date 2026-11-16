#!/usr/bin/env python3
"""Launch the final packaged executable with isolated runtime state."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import zipfile

root=Path(__file__).resolve().parents[1]
archive=Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix='openarchi-smoke-') as name:
    temp=Path(name)
    if os.name=='nt':
        with zipfile.ZipFile(archive) as zip:zip.extractall(temp/'package')
        executable=next((temp/'package').rglob('OpenArchi.exe'))
    else:
        subprocess.run(['ditto','-x','-k',str(archive),str(temp/'package')],check=True)
        executable=temp/'package/OpenArchi.app/Contents/MacOS/openarchi'
    report=temp/'startup.json'
    env=os.environ.copy();env['OPENARCHI_CONFIG_DIR']=str(temp/'profile')
    env['OPENARCHI_NATIVE_SMOKE_REPORT']=str(report)
    result=subprocess.run([str(executable)],env=env,timeout=90,capture_output=True)
    if result.returncode or not report.exists():
        # Keep runner diagnostics local to its job, never in the application.
        sys.stderr.write(result.stderr.decode(errors='replace'))
        raise SystemExit('Native startup failed')
    data=json.loads(report.read_text())
    if not data.get('adapter') or data['frames']!=30:raise SystemExit('No verified GPU adapter')
    output=root/'dist'/('native-'+data['os']+'-'+data['arch']+'.json')
    output.write_text(json.dumps(data,indent=2)+'\n')
    print(output.name, data['adapter'])
