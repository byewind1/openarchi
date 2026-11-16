import importlib.util
from pathlib import Path
import unittest
import subprocess
import sys
import json

SPEC = importlib.util.spec_from_file_location('openarchi_mcp', Path(__file__).resolve().parents[2] / 'scripts/openarchi-mcp.py')
MCP = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MCP)


class AdapterTests(unittest.TestCase):
    def test_initialize_and_read_only_catalog(self):
        result = MCP.dispatch({'method': 'initialize'}, '/unused')
        self.assertEqual(result['protocolVersion'], '2025-11-25')
        tools = MCP.dispatch({'method': 'tools/list'}, '/unused')['tools']
        self.assertEqual(len(tools), 34)
        task_ops = next(t for t in tools if t['name'] == 'query_object_task_operations')
        self.assertTrue(task_ops['annotations']['readOnlyHint'])
        self.assertFalse(task_ops['annotations']['destructiveHint'])
        self.assertEqual(set(task_ops['inputSchema']['required']), {'target','expected_revision','request_id'})
        rule = next(t for t in tools if t['name'] == 'prepare_parametric_proposal')
        self.assertTrue(rule['annotations']['readOnlyHint'])  # immutable preview; commit is the model write
        self.assertIn('expected_sources_revision', rule['inputSchema']['required'])
        self.assertIn('proposal', rule['inputSchema']['required'])
        spaces = next(t for t in tools if t['name'] == 'query_mass_spaces')
        self.assertTrue(spaces['annotations']['readOnlyHint'])
        self.assertEqual(spaces['inputSchema']['properties']['limit']['maximum'], 4)
        self.assertEqual(spaces['inputSchema']['properties']['mass']['minimum'], 1)
        self.assertIn('expected_revision', spaces['inputSchema']['required'])
        rules = next(t for t in tools if t['name'] == 'query_project_rules')
        self.assertTrue(rules['annotations']['readOnlyHint'])
        self.assertEqual(rules['inputSchema']['properties']['limit']['maximum'], 10)
        self.assertEqual(rules['inputSchema']['properties']['section']['enum'], ['sources','requirements','lifecycle','conditions','checks'])
        self.assertIn('expected_revision', rules['inputSchema']['required'])
        evidence = next(t for t in tools if t['name'] == 'query_source_evidence')
        self.assertTrue(evidence['annotations']['readOnlyHint'])
        self.assertEqual(evidence['inputSchema']['properties']['limit']['maximum'], 10)
        terrain = next(t for t in tools if t['name'] == 'query_terrain_height')
        self.assertTrue(terrain['annotations']['readOnlyHint'])
        self.assertEqual(terrain['inputSchema']['properties']['xy_m']['maxItems'], 2)
        analysis = next(t for t in tools if t['name'] == 'analyze_spaces')
        self.assertTrue(analysis['annotations']['readOnlyHint'])
        scopes = analysis['inputSchema']['properties']['options']['properties']['design_scope']['oneOf']
        self.assertEqual([s['properties']['type']['const'] for s in scopes], ['all','main','option'])
        self.assertEqual(scopes[2]['properties']['id']['minimum'], 1)
        self.assertTrue(all(s['additionalProperties'] is False for s in scopes))
        catalog = next(t for t in tools if t['name'] == 'list_workbench_commands')
        self.assertTrue(catalog['annotations']['readOnlyHint'])
        self.assertEqual(catalog['inputSchema'], MCP.TARGET)
        self.assertFalse(next(t for t in tools if t['name']=='invoke_workbench_command')['annotations']['readOnlyHint'])
        self.assertTrue(next(t for t in tools if t['name']=='get_workbench_operation')['annotations']['readOnlyHint'])
        self.assertFalse(next(t for t in tools if t['name']=='commit_change')['annotations']['readOnlyHint'])

    def test_numerical_checks_query_keeps_revision_and_original_host_response(self):
        from unittest.mock import patch
        arguments = {
            'target': {'instance_id': 'owned', 'project_id': 'a' * 64, 'document_session_id': 1},
            'expected_revision': 7, 'section': 'checks', 'offset': 0, 'limit': 10,
        }
        # The adapter carries the shared host's unknown/stale result unchanged;
        # it must not turn a successful read into a numerical or compliance pass.
        report = {'compliance': 'not_evaluated', 'entries': [
            {'current_state': 'unknown', 'freshness': 'stale'},
        ]}
        with patch.object(MCP, 'call_host', return_value={'result': report}) as call:
            response = MCP.dispatch({'method': 'tools/call', 'params': {
                'name': 'query_project_rules', 'arguments': arguments,
            }}, '/owned')
        call.assert_called_once_with('/owned', 'query_project_rules', arguments)
        self.assertFalse(response['isError'])
        self.assertEqual(response['structuredContent'], report)
        self.assertEqual(json.loads(response['content'][0]['text']), report)

    def test_original_object_task_operation_query_preserves_scope_and_unknown_phase(self):
        from unittest.mock import patch
        args={'target':{'instance_id':'current','project_id':'a'*64,'document_session_id':2},'expected_revision':9,'request_id':'b'*64}
        report={'read_only':True,'task_request_id':'b'*64,'operations':[{'request_id':'c'*64,'original_target':{'instance_id':'original','project_id':'a'*64,'document_session_id':1},'phase':'unknown','native_confirmation':False}]}
        with patch.object(MCP,'call_host',return_value={'result':report}) as call:
            result=MCP.dispatch({'method':'tools/call','params':{'name':'query_object_task_operations','arguments':args}},'/owned')
        call.assert_called_once_with('/owned','query_object_task_operations',args)
        self.assertEqual(result['structuredContent'],report)
        self.assertEqual(json.loads(result['content'][0]['text']),report)
        self.assertFalse(result['isError'])  # successful read, original write still unknown

    def test_configuration_tools_are_native_version_gated_and_mutations_are_explicit(self):
        from unittest.mock import patch
        native={'type':'object','properties':{'target':MCP.TARGET},'additionalProperties':False}
        with patch.object(MCP,'call_host',return_value={'result':{'environment_proposal_version':1,'environment_proposal_schema':native}}):
            tools=MCP.dispatch({'method':'tools/list'},'/owned')['tools']
        self.assertEqual(len(tools),38)
        self.assertEqual(next(t for t in tools if t['name']=='propose_work_environment')['inputSchema'],native)
        self.assertFalse(next(t for t in tools if t['name']=='propose_work_environment')['annotations']['readOnlyHint'])
        self.assertFalse(next(t for t in tools if t['name']=='cancel_environment_proposal')['annotations']['readOnlyHint'])
        self.assertTrue(next(t for t in tools if t['name']=='get_environment_proposal')['annotations']['readOnlyHint'])

    def test_configuration_lost_response_preserves_original_identity_without_retry(self):
        from unittest.mock import patch
        target={'instance_id':'owned','project_id':'a'*64,'document_session_id':1}
        for method in ['propose_work_environment','cancel_environment_proposal']:
            args={'target':target,'request_id':'original'}
            if method=='propose_work_environment':args={'target':target,'expected_revision':0,'proposal':{'request_id':'original'}}
            with patch.object(MCP.socket,'socket') as factory:
                connection=factory.return_value.__enter__.return_value
                connection.makefile.side_effect=TimeoutError('response lost after send')
                result=MCP.call_host('/owned',method,args)
                connection.sendall.assert_called_once()
            self.assertEqual(result['error']['code'],'ENVIRONMENT_OUTCOME_UNKNOWN')
            self.assertFalse(result['error']['retry_safe'])
            self.assertEqual(result['error']['request_id'],'original')
            self.assertEqual(result['error']['target'],target)
            self.assertEqual(result['error']['read_tool'],'get_environment_proposal')

    def test_configuration_invalid_response_is_unknown_after_one_send(self):
        from unittest.mock import patch
        for line in [b'',b'incomplete',b'[]\n',b'{"unexpected":true}\n']:
            with patch.object(MCP.socket,'socket') as factory:
                connection=factory.return_value.__enter__.return_value
                connection.makefile.return_value.__enter__.return_value.readline.return_value=line
                result=MCP.call_host('/owned','cancel_environment_proposal',{'target':{},'request_id':'original'})
                connection.sendall.assert_called_once()
            self.assertEqual(result['error']['code'],'ENVIRONMENT_OUTCOME_UNKNOWN')
            self.assertEqual(result['error']['request_id'],'original')
            self.assertFalse(result['error']['retry_safe'])

    def test_missing_instance_is_tool_error_not_success(self):
        result = MCP.dispatch({'method': 'tools/call', 'params': {'name': 'get_target'}}, '/tmp/oa-definitely-absent-mcp.sock')
        self.assertTrue(result['isError'])
        self.assertEqual(result['structuredContent']['error']['code'], 'NO_INSTANCE')

    def test_unknown_tool_never_reaches_host(self):
        with self.assertRaises(ValueError):
            MCP.dispatch({'method': 'tools/call', 'params': {'name': 'delete_all'}}, '/unused')

    def test_stdio_requires_initialization_and_preserves_rpc_ids(self):
        messages = [
            {'jsonrpc':'2.0','id':1,'method':'tools/list'},
            {'jsonrpc':'2.0','id':2,'method':'initialize','params':{'protocolVersion':'2025-11-25','capabilities':{},'clientInfo':{'name':'test','version':'1'}}},
            {'jsonrpc':'2.0','method':'notifications/initialized'},
            {'jsonrpc':'2.0','id':3,'method':'tools/list'},
            {'jsonrpc':'2.0','id':4,'method':'tools/call','params':{'name':'get_target','arguments':{}}}
        ]
        process = subprocess.run([sys.executable,str(Path(MCP.__file__)),'--socket','/tmp/oa-definitely-absent-mcp.sock'],input=''.join(json.dumps(m)+'\n' for m in messages),capture_output=True,text=True,check=True)
        replies=[json.loads(line) for line in process.stdout.splitlines()]
        self.assertEqual([r['id'] for r in replies],[1,2,3,4])
        self.assertIn('error',replies[0])
        self.assertEqual(replies[1]['result']['protocolVersion'],'2025-11-25')
        self.assertEqual(len(replies[2]['result']['tools']),34)
        self.assertTrue(replies[3]['result']['isError'])


class SourceReadTests(unittest.TestCase):
    def test_selected_dxf_record_keeps_selector_and_hashes_across_ranges(self):
        selector={'handle':'36CE','allow_constant_width_centerline':False}
        schema=next(t for t in MCP.TOOLS if t['name']=='read_project_source')['inputSchema']
        self.assertIn('dxf_outline',schema['properties'])
        self.assertNotIn('dxf_outline',schema['required'])
        payload=json.dumps({'points':[[1,2],[3,2],[3,4]],'source_sha256':'a'*64,'record_sha256':'b'*64,'note':'source-native, not survey '+('x'*70000)}).encode()
        call,_=self.stream(payload,'application/json')
        received=[]
        def selected(path,name,args):
            received.append(args)
            result=call(path,name,args)
            result['result']['dxf_outline']=selector
            result['result']['original_sha256']='a'*64
            return result
        response,content=MCP.read_source_content('/unused',{'dxf_outline':selector},selected)
        self.assertGreater(len(received),1)
        self.assertTrue(all(r['dxf_outline']==selector for r in received))
        self.assertEqual(json.loads(content[0]['text'])['record_sha256'],'b'*64)
        self.assertEqual(response['result']['original_sha256'],'a'*64)
        self.assertEqual(response['result']['dxf_outline'],selector)
        self.assertEqual(response['result']['agent_understanding'],'not_evaluated')

    def stream(self, payload, mime='text/plain', corrupt=None):
        import hashlib
        calls=[]
        def call(path, name, args):
            calls.append(name)
            offset=args['offset']; end=min(offset+args['limit'],len(payload))
            value={'target':{'project_id':'fixture'},'revision':4,'source_id':'drawing:1','sources_revision':'same','representation':'original','original_sha256':hashlib.sha256(payload).hexdigest(),'content_sha256':hashlib.sha256(payload).hexdigest(),'total_bytes':len(payload),'offset':offset,'returned_bytes':end-offset,'next_offset':end if end<len(payload) else None,'encoding':'hex','data':payload[offset:end].hex(),'mime_type':mime}
            if corrupt: corrupt(value,offset)
            return {'result':value}
        return call,calls

    def test_complete_utf8_across_chunk_boundaries_and_image_content(self):
        import base64
        payload=('项目已有条件，保留未知。'*8000).encode()
        call,calls=self.stream(payload)
        response,content=MCP.read_source_content('/unused',{},call)
        self.assertEqual(content,[{'type':'text','text':payload.decode()}])
        self.assertEqual(response['result']['assembled_bytes'],len(payload))
        self.assertNotIn('data',response['result'])
        self.assertEqual(set(calls),{'read_project_source'})
        image=b'opaque original PNG bytes; Rust verifies decoding'
        call,_=self.stream(image,'image/png')
        response,content=MCP.read_source_content('/unused',{},call)
        self.assertEqual(content[0]['type'],'image')
        self.assertEqual(base64.b64decode(content[0]['data']),image)
        self.assertEqual(response['result']['agent_understanding'],'not_evaluated')

    def test_changed_context_or_missing_range_never_returns_partial_image(self):
        for corrupt in [lambda v,o:v.update(sources_revision='changed') if o else None,
                        lambda v,o:v.update(offset=o+1),
                        lambda v,o:v.update(next_offset=1)]:
            call,_=self.stream(b'x'*80000,'image/png',corrupt)
            response,content=MCP.read_source_content('/unused',{},call)
            self.assertEqual(response['error']['code'],'INVALID_SOURCE_CONTENT')
            self.assertEqual(content,[])

    def test_oversize_refuses_before_assembly(self):
        call,_=self.stream(b'x','image/png',lambda v,o:v.update(total_bytes=2*1024*1024+1))
        response,content=MCP.read_source_content('/unused',{},call)
        self.assertEqual(response['error']['code'],'INVALID_SOURCE_CONTENT')
        self.assertEqual(content,[])


if __name__ == '__main__':
    unittest.main()
