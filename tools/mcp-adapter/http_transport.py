"""Loopback Streamable HTTP transport for the existing official-SDK server."""
import contextlib
import hmac
import json
import os
from pathlib import Path
import socket
import time

import uvicorn
from mcp.server.streamable_http_manager import StreamableHTTPSessionManager
from mcp.server.transport_security import TransportSecuritySettings
from starlette.applications import Starlette
from starlette.responses import JSONResponse
from starlette.routing import Route


def load_token(path):
    if path is None or not path.is_absolute() or path.is_symlink():
        raise ValueError('Private token file required')
    stat = path.stat()
    if os.name == 'posix' and (stat.st_mode & 0o077 or stat.st_uid != os.getuid()):
        raise ValueError('Token file must be private to its owner')
    token = path.read_text().strip()
    if len(token) < 32:
        raise ValueError('Invalid token')
    return token


async def serve(server, port, token_path, status_path):
    if not 1024 <= port <= 65535 or status_path is None or not status_path.is_absolute():
        raise ValueError('Port 1024..65535 and absolute status file required')
    token = load_token(token_path)
    status = {'listening': False, 'requests': 0, 'last_request': None}
    def report():
        temporary = status_path.with_suffix('.tmp')
        temporary.write_text(json.dumps(status))
        temporary.replace(status_path)
    manager = StreamableHTTPSessionManager(
        server, json_response=True, stateless=True,
        security_settings=TransportSecuritySettings(
            enable_dns_rebinding_protection=True,
            allowed_hosts=[f'127.0.0.1:{port}', f'localhost:{port}'],
            allowed_origins=[f'http://127.0.0.1:{port}', f'http://localhost:{port}']),
        max_request_body_size=1024 * 1024)
    # Bind before reporting readiness; an occupied port never reports listening.
    listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    listener.bind(('127.0.0.1', port))
    listener.listen(64)
    listener.setblocking(False)

    class Authenticated:
        async def __call__(self, scope, receive, send):
            headers = dict(scope.get('headers', []))
            supplied = headers.get(b'authorization', b'').decode('latin-1')
            origin = headers.get(b'origin', b'').decode('latin-1')
            if origin and origin not in (f'http://127.0.0.1:{port}', f'http://localhost:{port}'):
                await JSONResponse({'error': 'Origin denied'}, status_code=403)(scope, receive, send)
                return
            if not hmac.compare_digest(supplied, 'Bearer ' + token):
                await JSONResponse({'error': 'Bearer token required'}, status_code=401)(scope, receive, send)
                return
            status.update(requests=status['requests'] + 1, last_request=int(time.time()))
            report()
            await manager.handle_request(scope, receive, send)

    @contextlib.asynccontextmanager
    async def lifespan(app):
        async with manager.run():
            status['listening'] = True
            report()
            try:
                yield
            finally:
                status['listening'] = False
                report()

    # ASGI route keeps the canonical /mcp URL without a trailing-slash redirect.
    application = Starlette(routes=[Route('/mcp', Authenticated(), methods=['GET', 'POST', 'DELETE'])], lifespan=lifespan)
    config = uvicorn.Config(application, host='127.0.0.1', port=port, access_log=False, log_level='warning', timeout_graceful_shutdown=2)
    try:
        await uvicorn.Server(config).serve(sockets=[listener])
    finally:
        listener.close()
