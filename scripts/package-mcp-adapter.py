#!/usr/bin/env python3
"""Copy the finite official MCP adapter distribution before bundle signing."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil

FILES = ('tools/mcp-adapter/server.py', 'tools/mcp-adapter/http_transport.py', 'tools/mcp-adapter/task_guard.py', 'tools/mcp-adapter/pyproject.toml',
         'tools/mcp-adapter/uv.lock', 'scripts/openarchi-mcp.py')

def package(repo: Path, resources: Path):
    destination = resources / 'OpenArchiMCP'
    # This generated directory is owned solely by this packager. Never traverse
    # an unexpected symlink or include a developer environment/credentials.
    if destination.is_symlink():
        raise ValueError('MCP resource destination is a symlink')
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    hashes = {}
    for relative in FILES:
        source = repo / relative
        if source.is_symlink() or not source.is_file():
            raise ValueError(f'Invalid adapter source: {relative}')
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        hashes[relative] = hashlib.sha256(target.read_bytes()).hexdigest()
    (destination / 'manifest.json').write_text(json.dumps({
        'schema': 1, 'capability_version': 8, 'files': hashes,
        'environment': 'External Python environment required; set UV_PROJECT_ENVIRONMENT outside signed bundle',
        'transport': 'Official MCP SDK stdio and authenticated loopback Streamable HTTP; shared authorized native Unix socket',
    }, indent=2) + '\n')
    return destination

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('resources', type=Path)
    args = parser.parse_args()
    print(package(Path(__file__).resolve().parents[1], args.resources))
