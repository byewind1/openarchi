"""Protocol interoperability with pinned official client + server SDK."""
import asyncio
import sys
from pathlib import Path
from mcp.client.stdio import stdio_client,StdioServerParameters
from mcp.client.session import ClientSession

async def main():
    server=StdioServerParameters(command=sys.executable,args=[str(Path(__file__).with_name('server.py')),'--socket','/tmp/oa-no-instance-sdk-test.sock'])
    async with stdio_client(server) as (read,write):
        async with ClientSession(read,write) as client:
            initialized=await client.initialize()
            tools=await client.list_tools()
            assert any(t.name=='list_instances' for t in tools.tools)
            assert not any(t.name=='prepare_native_change' for t in tools.tools)
            result=await client.call_tool('get_target',{})
            assert result.is_error
            assert result.structured_content['error']['code']=='NO_INSTANCE'
            print('official SDK initialize/list/call/no-instance passed; protocol',initialized.protocol_version)

if __name__=='__main__':asyncio.run(main())
