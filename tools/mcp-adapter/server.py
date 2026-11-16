#!/usr/bin/env python3
"""Official SDK transport; native business schemas are discovered from Rust."""
import argparse
import asyncio
import copy
import json
import importlib.util
from pathlib import Path
import anyio
from mcp.server.lowlevel import Server
from mcp.server.stdio import stdio_server
import mcp.types as types
from task_guard import TaskGuard

MODULE = Path(__file__).resolve().parents[2] / 'scripts/openarchi-mcp.py'
SPEC = importlib.util.spec_from_file_location('openarchi_transport', MODULE)
TRANSPORT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(TRANSPORT)


def create_server(socket_path, catalog_mode="progressive", fault_once=None, fault_request_id=None, task_guard=None):
    guard = TaskGuard(task_guard) if task_guard else None
    catalog = {}
    async def list_tools(context, params):
        tools = copy.deepcopy(TRANSPORT.TOOLS)
        # Raw native Element JSON was an M1 compatibility probe. Keep it out of
        # the formal SDK catalog; registered wire types are authoritative.
        tools = [t for t in tools if t['name'] != 'prepare_change']
        description = next(t for t in tools if t['name'] == 'describe_command')
        description['inputSchema']=copy.deepcopy(description['inputSchema'])
        description['description'] = 'Describe an exact registered native type from search_commands, with input/output schemas, units, prerequisites, errors and supported variants. Required command is one registered name.'
        description['inputSchema']['required'].append('command')
        description['inputSchema']['properties']['command'] = {'type':'string','description':'Registered name from search_commands. Required; legacy batch examples are outside the formal catalog.'}
        tools.append({'name':'list_instances','description':'List instances on this explicitly bound local endpoint, including build and document identity. Does not start an application.','inputSchema':{'type':'object','additionalProperties':False},'annotations':{'readOnlyHint':True,'openWorldHint':False}})
        tools.append({'name':'search_commands','description':'Search registered native types. Stable order; retrieve exact schema with describe_command.','inputSchema':TRANSPORT.object_schema({'target':TRANSPORT.TARGET,'query':{'type':'string'},'limit':{'type':'integer','minimum':1,'maximum':100},'cursor':{'type':['string','null']}},['target','limit']),'annotations':{'readOnlyHint':True,'openWorldHint':False}})
        response = await anyio.to_thread.run_sync(TRANSPORT.call_host,socket_path,'get_native_schema',{})
        if response.get('result',{}).get('mass_space_read_version') != 1:
            tools = [t for t in tools if t['name'] != 'query_mass_spaces']
        if response.get('result',{}).get('plugin_job_read_version') != 1:
            tools = [t for t in tools if t['name'] not in {'list_plugin_commands','list_plugin_jobs','read_plugin_job'}]
        if response.get('result',{}).get('environment_proposal_version') != 1:
            tools = [t for t in tools if t['name'] not in TRANSPORT.ENVIRONMENT_TOOLS]
        if 'result' in response:
            if response['result'].get('environment_proposal_version') == 1 and 'environment_proposal_schema' in response['result']:
                native=copy.deepcopy(response['result']['environment_proposal_schema']);native.pop('$schema',None)
                next(t for t in tools if t['name']=='propose_work_environment')['inputSchema']=native
            for key, tool in [('spatial_intent_schema','prepare_spatial_intent'),('mass_intent_schema','prepare_mass_intent'),('parametric_proposal_schema','prepare_parametric_proposal'),('task_update_schema','prepare_task_update')]:
                if key in response['result']:
                    intent_schema = copy.deepcopy(response['result'][key])
                    intent_schema.pop('$schema',None)
                    next(t for t in tools if t['name']==tool)['inputSchema']=intent_schema
            schema = copy.deepcopy(response['result']['discovered_schema' if catalog_mode=='progressive' else 'schema'])
            definitions = schema.pop('$defs',{})
            description['inputSchema']['properties']['command']['enum']=response['result']['commands']
            schema.pop('$schema',None)
            input_schema = TRANSPORT.object_schema({'target':TRANSPORT.TARGET,'expected_revision':{'type':'integer','minimum':0},'expected_context_revision':{'type':['string','null'],'description':'Fresh get_context.context_revision required for selection/current-story dependent input; checked at prepare and commit.'},'change':schema},['target','expected_revision','change'])
            input_schema['$defs'] = definitions
            tools.append({'name':'prepare_discovered_change' if catalog_mode=='progressive' else 'prepare_native_change','outputSchema':response['result']['prepare_output_schema'],'description':('Prepare a version-1 registered batch. Each geometry is {command,parameters}; first retrieve that command parameters_schema with describe_command. Do not include type in parameters. All business parameters are validated strictly by the same native service; no executable code. ' if catalog_mode=='progressive' else 'Prepare a version-1 native modeling batch. ')+ ' Pass expected_context_revision from get_context for selected/current-story dependent inputs. Reading context never renews write authorization after a manual model edit; WRITE_AUTHORIZATION_SUSPENDED requires explicit native reauthorization. View-relative coordinates unsupported. Lengths metres, angles degrees. Native wall/slab/column/beam/roof/stair/opening/polyline/zone/concept_mass; named forward hosts, move/update/remove. Validates with the shared model service. Does not modify the model; commit the returned immutable preview. Inspect relevant describe_command first.','inputSchema':input_schema,'annotations':{'readOnlyHint':True,'openWorldHint':False}})
        catalog.clear()
        catalog.update({t['name']:t for t in tools})
        return types.ListToolsResult(tools=[types.Tool.model_validate(t) for t in tools])

    async def call_tool(context, params):
        if params.name not in catalog:
            raise ValueError('Unknown or unavailable tool; refresh tools/list')
        source_content = []
        if params.name == 'read_project_source':
            response, source_content = await anyio.to_thread.run_sync(TRANSPORT.read_source_content,socket_path,params.arguments or {})
        elif guard:
            response = await anyio.to_thread.run_sync(guard.dispatch,TRANSPORT.call_host,socket_path,params.name,params.arguments or {})
        else:
            response = await anyio.to_thread.run_sync(TRANSPORT.call_host,socket_path,params.name,params.arguments or {})
        if fault_once and params.name=='commit_change' and (params.arguments or {}).get('request_id')==fault_request_id and response.get('result',{}).get('status')=='committed':
            try:
                with open(fault_once,'x',encoding='utf-8') as marker:
                    json.dump({'kind':'simulated_response_loss_after_dispatch','arguments':params.arguments,'discarded_host_result':response},marker,ensure_ascii=False)
                response={'error':{'code':'OUTCOME_UNKNOWN_AFTER_DISPATCH','message':'The commit result was lost after dispatch. Query get_operation using the original target/request_id. Never change request identity to replay.'}}
            except FileExistsError:
                pass
        value = response.get('result',response)
        # Short textual fallback; structured output is not duplicated verbatim.
        content = [types.ImageContent(**item) if item['type']=='image' else types.TextContent(**item) for item in source_content]
        return types.CallToolResult(content=content or [types.TextContent(type='text',text='OpenArchi error' if 'error' in response else 'OpenArchi result')],structuredContent=value,isError='error' in response)

    return Server('openarchi-local',version='0.4.0',on_list_tools=list_tools,on_call_tool=call_tool,get_tool_input_schema=lambda name:catalog.get(name,{}).get('inputSchema'))


async def main():
    parser=argparse.ArgumentParser();parser.add_argument('--socket',required=True);parser.add_argument('--catalog-mode',choices=['full','progressive'],default='progressive');parser.add_argument('--fault-once');parser.add_argument('--fault-request-id');parser.add_argument('--task-guard')
    parser.add_argument('--http-port', type=int)
    parser.add_argument('--token-file', type=Path)
    parser.add_argument('--status-file', type=Path)
    args=parser.parse_args()
    if bool(args.fault_once)!=bool(args.fault_request_id):parser.error('both evaluation fault flags required')
    if args.fault_once and (not Path(args.fault_once).is_absolute() or not Path(args.fault_once).parent.is_dir()):parser.error('absolute existing private fault directory required')
    server=create_server(args.socket,args.catalog_mode,args.fault_once,args.fault_request_id,args.task_guard)
    if args.http_port is not None:
        from http_transport import serve
        await serve(server, args.http_port, args.token_file, args.status_file)
        return
    async with stdio_server() as (read,write):
        await server.run(read,write,server.create_initialization_options())


if __name__=='__main__':
    asyncio.run(main())
