#!/usr/bin/env python3
"""Independent public-source checks; generated evidence stays under target/."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

root=Path(__file__).resolve().parents[1]
cargo=['cargo'] if os.name=='nt' else [str(root/'scripts/cargo.sh')]
steps=[cargo+['fmt','--all','--','--check'],cargo+['check','--locked','--all-targets'],
       [sys.executable,'scripts/check-localization.py'],cargo+['test','--locked','--all-targets','--no-fail-fast'],
       [sys.executable,'-m','unittest','discover','-s','tests/tooling','-v']]
for command in steps:subprocess.run(command,cwd=root,check=True)
print('Public-source verification passed')
