#!/usr/bin/env python3
"""Build with reproducible public path names; never embed host usernames."""
import argparse
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]

def build(target=None):
    env = os.environ.copy()
    mappings = [(Path.home(), '/build/host'), (ROOT, '/build/openarchi')]
    flags = ['--remap-path-prefix=' + str(a) + '=' + b for a, b in mappings]
    # Encoded flags preserve spaces in a checkout path without shell parsing.
    env['CARGO_ENCODED_RUSTFLAGS'] = '\x1f'.join(flags)
    env.pop('RUSTFLAGS', None)
    env.setdefault('MACOSX_DEPLOYMENT_TARGET', '12.0')
    command = [str(ROOT / 'scripts/cargo.sh'), 'build', '--release', '--locked']
    if os.name == 'nt':
        command = ['cargo', 'build', '--release', '--locked']
    if target:
        command += ['--target', target]
    subprocess.run(command, cwd=ROOT, env=env, check=True)

if __name__ == '__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('--target')
    build(parser.parse_args().target)
