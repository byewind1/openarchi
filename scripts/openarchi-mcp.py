#!/usr/bin/env python3
"""Local stdio MCP adapter. Never starts OpenArchi or changes client settings."""
import argparse
import json
import socket
import sys

PROTOCOL = '2025-11-25'
TARGET = {'type': 'object', 'properties': {'instance_id': {'type': 'string'}, 'project_id': {'type':'string','minLength':64,'maxLength':64}, 'document_session_id': {'type': 'integer', 'minimum': 1}}, 'required': ['instance_id', 'project_id', 'document_session_id'], 'additionalProperties': False}
TOOLS = [
    {'name':'list_workbench_commands', 'description':'Read the native workbench command catalog for the exact current document target. Returns stable command IDs, effects, parameter schemas and current disabled reasons. Discovery only; does not enable remote execution or change model/write permissions.', 'inputSchema': TARGET},
    {'name':'query_project_rules','description':'Read original revisions, cited requirements, lifecycle declarations or condition verification needs in revision-bound pages (1-10 entries, up to1 MiB). Declarations, missing topics, stale decisions and links remain explicit. The checks section separates stored numerical values from current cited results. Cited results require exact bound-original verification; missing, changed or rejected originals remain unknown. Never a compliance or approval verdict.', 'inputSchema':{'type':'object','additionalProperties':False,'required':['target','expected_revision','section','offset','limit'],'properties':{'target':TARGET,'expected_revision':{'type':'integer','minimum':0},'section':{'type':'string','enum':['sources','requirements','lifecycle','conditions','checks']},'offset':{'type':'integer','minimum':0},'limit':{'type':'integer','minimum':1,'maximum':10}}}},
    {'name':'query_source_evidence','description':'Read complete declared source notes and typed link statuses in revision-bound pages (1-10 notes, up to1 MiB). Use next_offset; source text is unverified and not approval.', 'inputSchema':{'type':'object','additionalProperties':False,'required':['target','expected_revision','offset','limit'],'properties':{'topics':{'type':'boolean','default':False,'description':'True reads persistent object discussion history, topic_sha256, frozen inputs, declared Agent replies and current link/input statuses. Replies are suggestions with declared identity, not authenticated execution or approval. Optional assignment carries an explicitly submitted pull-task request and lifecycle. Read its mode and get_context write_authorization scope before claiming or writing; fresh reads never resume permission. Saved task records do not start a model or retain session write authority.'},'target':TARGET,'expected_revision':{'type':'integer','minimum':0},'offset':{'type':'integer','minimum':0},'limit':{'type':'integer','minimum':1,'maximum':10}}}},
    {'name':'query_terrain_height','description':'Read-only revision-bound model XY terrain elevation in metres. Missing surface, holes and outside coverage remain unknown; declared numeric height is not survey approval.', 'inputSchema':{'type':'object','additionalProperties':False,'required':['target','expected_revision','xy_m'],'properties':{'target':TARGET,'expected_revision':{'type':'integer','minimum':0},'xy_m':{'type':'array','items':{'type':'number'},'minItems':2,'maxItems':2}}}},
    {'name':'analyze_spaces','description':'Read-only spatial analysis at an explicit revision. Returns authored area/space volume totals, gross wall footprint overlap, same-story overlap and near-boundary relations within 0.25m (not door connectivity). Optional explicit boundary and holes require story; uncovered area includes walls/voids unless excluded. At most 256 spaces per request, 512 returned relations with explicit total/truncation. Explicit design_scope all/main/option binds spaces and gross-wall inputs; omitted means all (mixed alternatives, not comparison). Unknown option or option-specific wall joins reject. Does not update boundaries or model.', 'inputSchema':{'type':'object','additionalProperties':False,'required':['target','expected_revision','options'],'properties':{'target':TARGET,'expected_revision':{'type':'integer','minimum':0},'options':{'type':'object','additionalProperties':False,'properties':{'design_scope':{'oneOf':[{'type':'object','properties':{'type':{'const':'all'}},'required':['type'],'additionalProperties':False},{'type':'object','properties':{'type':{'const':'main'}},'required':['type'],'additionalProperties':False},{'type':'object','properties':{'type':{'const':'option'},'id':{'type':'integer','minimum':1}},'required':['type','id'],'additionalProperties':False}]},'story':{'type':['integer','null'],'minimum':0},'boundary':{'type':['array','null'],'items':{'type':'object','properties':{'x':{'type':'number'},'y':{'type':'number'}},'required':['x','y'],'additionalProperties':False}},'holes':{'type':'array','items':{'type':'array','items':{'type':'object','properties':{'x':{'type':'number'},'y':{'type':'number'}},'required':['x','y'],'additionalProperties':False}}}}}}}},

    {'name': 'get_target', 'description': 'Discover the target on this explicitly configured local application endpoint. Does not start an application.', 'inputSchema': {'type': 'object', 'properties': {}, 'additionalProperties': False}},
    {'name': 'get_context', 'description': 'Read current revision, SI units, stories and selection on the bound document.', 'inputSchema': TARGET},
    {'name': 'query_model', 'description': 'Read a revision-bound page and full-filter count summary. Fields bounds/quantities/validation use shared derived sources (source-only validation). Nominal area semantics vary by type; material, void, concept-envelope and habitable-space volumes are separate. Lengths in shape are metres. For complete editable slab outline/holes/arcs/edge settings/elevation, request slab_contour with shape; null means the min/max rectangle at story elevation. wall_contour exposes optional authored wall contour parameters. opening_joinery returns effective single/double leaf, hinge/swing and frame/leaf dimensions. Native opening geometry accepts optional joinery; omit on update to preserve existing fittings. Bounds are derived display bounds, not editable contour data. Continue only using returned next_cursor and identical query.', 'inputSchema': {'type': 'object', 'properties': {'target': TARGET, 'expected_revision': {'type': 'integer', 'minimum': 0}, 'ids': {'type': 'array', 'items': {'type': 'integer', 'minimum': 1}}, 'kind': {'type': ['string', 'null'], 'enum': ['Wall', 'Slab', 'Column', 'Beam', 'Roof', 'Stair', 'Polyline', 'Zone', 'Concept mass', 'Door', 'Window', None]}, 'story': {'type': ['integer', 'null'], 'minimum': 0}, 'region': {'type':['object','null'],'description':'Inclusive project-space XYZ display-bounds overlap, metres; not exact solid intersection. min <= max. Source failures return diagnostics, never silent omissions.','properties':{'min':{'type':'array','items':{'type':'number'},'minItems':3,'maxItems':3},'max':{'type':'array','items':{'type':'number'},'minItems':3,'maxItems':3}},'required':['min','max'],'additionalProperties':False}, 'fields': {'type': 'array', 'minItems': 1, 'items': {'type': 'string', 'enum': ['id', 'kind', 'story', 'name', 'shape', 'slab_contour', 'wall_contour', 'opening_joinery', 'layer', 'material', 'bounds', 'quantities', 'validation']}}, 'limit': {'type': 'integer', 'minimum': 1, 'maximum': 100}, 'cursor': {'type': ['string', 'null']}}, 'required': ['target', 'expected_revision', 'fields', 'limit'], 'additionalProperties': False}}
]
# Optional local relation filter; the host shares this definition with native navigation.
next(t for t in TOOLS if t['name']=='query_source_evidence')['inputSchema']['properties']['object_targets'] = {
    'type':'array','maxItems':16,'uniqueItems':True,
    'description':'Only with topics=true. Empty/omitted retains all history. Filter by recorded object IDs, explicit mass-space links and task result mappings. Read-only relevance, never write authority or automatic task resume.',
    'items':{'oneOf':[
        {'type':'object','additionalProperties':False,'required':['type','id'],'properties':{'type':{'const':'element'},'id':{'type':'integer','minimum':1}}},
        {'type':'object','additionalProperties':False,'required':['type','sheet_id','key'],'properties':{'type':{'const':'program_row'},'sheet_id':{'type':'string','minLength':1,'maxLength':256},'key':{'type':'string','minLength':1,'maxLength':256}}}
    ]}
}

def object_schema(properties, required):
    return {'type': 'object', 'properties': properties, 'required': required, 'additionalProperties': False}


for name, description, schema in [
    ('query_object_task_operations','Read original journal phases/targets/request IDs for one persisted object task. This is recovery metadata, not current model effect or task reauthorization. Inspect unknown results by original identity; never replay a write. Task handoff is explicit connected-client pull, not an automatic outbound model call.',object_schema({'target':TARGET,'expected_revision':{'type':'integer','minimum':0},'request_id':{'type':'string','minLength':64,'maxLength':64}},['target','expected_revision','request_id'])),
    ('prepare_spatial_intent','Prepare two new editable Zone candidates from already-read imported unscaled spatial intent images and the requested outcome. All numerical geometry is declared design assumption, not survey/code fact. Preserves every existing element/task/choice; no complete brief/classification/2D tracing prerequisite. Requires existing explicit write authority and exact model/context/source revisions; reading fresh context never renews permission. Formal SDK discovery exposes the exact Rust schema (raw IPC get_native_schema); commit/cancel/inspect using existing operation tools and retain the original task/request identity. Source references and interpretation claims are declarations, not an automatic architectural verdict.',object_schema({'target':TARGET,'expected_revision':{'type':'integer','minimum':0},'expected_context_revision':{'type':'string'},'expected_sources_revision':{'type':'string'},'task_id':{'type':'string','minLength':64,'maxLength':64},'draft':{'type':'object','description':'Typed spatial intent draft; formal SDK exposes exact Rust-generated schema. Geometry coordinates/height in metres; two corresponding candidate sets; existing image page IDs/SHA.'}},['target','expected_revision','expected_context_revision','expected_sources_revision','task_id','draft'])),
    ('query_mass_spaces','Read explicit mass/Zone association status, assumptions, source and target guards and authored area in the shared project. Optional mass/zone filters return current object stamps before creating or binding; no write or renewed authorization. Missing links remain inspectable without filters. Same story/option only; not net floor area or compliance.',object_schema({'target':TARGET,'expected_revision':{'type':'integer','minimum':0},'mass':{'type':['integer','null'],'minimum':1},'zone':{'type':['integer','null'],'minimum':1},'offset':{'type':'integer','minimum':0},'limit':{'type':'integer','minimum':1,'maximum':4}},['target','expected_revision','offset','limit'])),
    ('query_model_figures','Read project-owned linked figure IDs and report_templates (ordered chapter titles and figure references, exact original guards), scope, source hashes, cached quantities and current/stale/missing status. Paginate with exact model revision; no write, approval or task reauthorization.',object_schema({'target':TARGET,'expected_revision':{'type':'integer','minimum':0},'offset':{'type':'integer','minimum':0},'limit':{'type':'integer','minimum':1,'maximum':4}},['target','expected_revision','offset','limit'])),
    ('query_task_updates','Read immutable project task updates and current scoped geometry/source status. Declarations are not approval or authorization. Paginate with exact model revision; no replay or write.',object_schema({'target':TARGET,'expected_revision':{'type':'integer','minimum':0},'offset':{'type':'integer','minimum':0},'limit':{'type':'integer','minimum':1,'maximum':4}},['target','expected_revision','offset','limit'])),
    ('prepare_task_update','Prepare a bounded same-type geometry update of 1–16 existing members of the current task candidate, with explicit task instruction/source versions/interpretation/assumptions and comparison fingerprint. No creation, deletion or unrelated edits. Exact Rust schema through formal SDK/get_native_schema. Shared preview/commit/cancel/original operation and user-edit authorization protection; fresh context never renews authority. Source bindings are declarations, not proof of reading/approval. Commit and inspect with the original update_id.',object_schema({'target':TARGET,'expected_revision':{'type':'integer','minimum':0},'expected_context_revision':{'type':'string'},'expected_sources_revision':{'type':'string'},'task_id':{'type':'string','minLength':64,'maxLength':64},'update_id':{'type':'string','minLength':64,'maxLength':64},'draft':{'type':'object','description':'Strict Rust-generated scoped update draft exposed by formal SDK.'}},['target','expected_revision','expected_context_revision','expected_sources_revision','task_id','update_id','draft'])),
    ('prepare_mass_intent','Prepare two new editable ConceptMass candidates from already-read imported unscaled intent images with an exterior/massing outcome and the requested outcome. All numerical geometry is declared design assumption, not survey/code fact. Preserves every existing element/task/choice; no complete brief/classification/2D tracing prerequisite. Requires existing explicit write authority and exact model/context/source revisions; reading fresh context never renews permission. Formal SDK discovery exposes the exact Rust schema (raw IPC get_native_schema); commit/cancel/inspect using existing operation tools and retain the original task/request identity. Source references and interpretation claims are declarations, not an automatic architectural verdict.',object_schema({'target':TARGET,'expected_revision':{'type':'integer','minimum':0},'expected_context_revision':{'type':'string'},'expected_sources_revision':{'type':'string'},'task_id':{'type':'string','minLength':64,'maxLength':64},'draft':{'type':'object','description':'Typed mass intent draft; formal SDK exposes exact Rust-generated schema. Geometry coordinates/height in metres; two corresponding concept-mass sets with visual surface choices; existing image page IDs/SHA.'}},['target','expected_revision','expected_context_revision','expected_sources_revision','task_id','draft'])),
    ('prepare_parametric_proposal','Prepare one bounded L-shaped two-wing concept pair or regenerate its unchanged mapped members through the shared parametric service. Persisted rule v1, integer-mm parameters, source bindings and assumptions; rejects changed sources, manual edits or locks. Concept envelope only, not compliance, statutory area or global optimum. Exact Rust-generated schema is exposed by formal SDK discovery. Successful preparation also returns declared_check_preview for affected stored definitions, using proposed geometry; saved reports are unchanged and cited-original assessment remains unknown. Requires existing target/revision/context/source/write authority; commit/cancel/inspect through original operation identity. This inbound tool does not dispatch an external Agent.',object_schema({'target':TARGET,'expected_revision':{'type':'integer','minimum':0},'expected_context_revision':{'type':'string'},'expected_sources_revision':{'type':'string'},'task_id':{'type':'string','minLength':64,'maxLength':64},'proposal':{'type':'object','description':'Rust ParametricAction create/update, see get_native_schema.parametric_proposal_schema; parameters in integer millimetres.'}},['target','expected_revision','expected_context_revision','expected_sources_revision','task_id','proposal'])),
    ('query_project_sources','Search existing attached repository and imported drawing metadata by name/ID/format. Metadata is not a content read. No classification or reupload. Keep distinct versions; use read_project_source and current model/task context before interpreting.',object_schema({'target':TARGET,'expected_revision':{'type':'integer','minimum':0},'expected_sources_revision':{'type':['string','null']},'query':{'type':'string','maxLength':512},'offset':{'type':'integer','minimum':0},'limit':{'type':'integer','minimum':1,'maximum':20}},['target','expected_revision','offset','limit'])),
    ('read_project_source','Actually read an existing source identified by query_project_sources, bound to current target/model/source-context revisions. Returns fully hash-verified original image content or explicit text extraction (UTF-8/DOCX body). Optional dxf_outline selects one strict closed model-space DXF record by handle, returning source-native vertices and separate original plane elevation, source/record hashes, units declarations and quality; explicit centerline interpretation is required for nonzero constant width. This allows a bounded derived record from a larger stored source without raising the2MiB original/representation limit. No OCR, measured dimensions, retain identity, interpretation or write grant inferred. Unsupported/changed content is explicit. Does not follow external source paths.',object_schema({'target':TARGET,'expected_revision':{'type':'integer','minimum':0},'sources_revision':{'type':'string'},'source_id':{'type':'string'},'representation':{'type':'string','enum':['original','text']},'dxf_outline':object_schema({'handle':{'type':'string','pattern':'^[0-9A-Fa-f]{1,16}$'},'allow_constant_width_centerline':{'type':'boolean','default':False}},['handle'])},['target','expected_revision','sources_revision','source_id','representation'])),
    ('validate_model','Validate the whole current project through the shared model service. Global validity/counts cover all diagnostics, even when this page is empty. Follow next_cursor with identical target/revision/limit.',object_schema({'target':TARGET,'expected_revision':{'type':'integer','minimum':0},'limit':{'type':'integer','minimum':1,'maximum':100},'cursor':{'type':['string','null']}},['target','expected_revision','limit'])),
    ('describe_command','Read the native BatchChange v1 contract, units, templates and edit examples for this active project. Read before prepare_change.',TARGET),
    ('save_project','Save the current project in the explicitly authorized output directory. Only a single filename; refuses overwriting unrelated existing files. Check revision.',object_schema({'target':TARGET,'expected_revision':{'type':'integer'},'filename':{'type':'string'}},['target','expected_revision','filename'])),
    ('reopen_project','Reopen the currently bound saved project, only when no unsaved changes exist. Rotates document_session_id; rediscover target afterwards.',object_schema({'target':TARGET,'expected_revision':{'type':'integer'},'filename':{'type':'string'}},['target','expected_revision','filename'])),
    ('prepare_change', 'Validate an atomic version-1 BatchChange using the shared model service. Creates use named templates with id=0; new opening host_wall=0 plus explicit temporary host. Does not change the model. Preview expires after 600s.', object_schema({'target': TARGET, 'expected_revision': {'type': 'integer'}, 'change': {'type': 'object', 'description': 'Native BatchChange {version:1,creates:[{name,template:{element,host}}],edits:[...]}. Full typed validation is performed by the application.'}}, ['target','expected_revision','change'])),
    ('commit_change', 'Commit exactly the immutable prepared preview. Persist request_id and preview_id before calling. On lost response call get_operation with the same request_id; never create a new operation identity to retry.', object_schema({'target': TARGET, 'preview_id': {'type':'string'}, 'request_id': {'type':'string','minLength':1,'maxLength':256}}, ['target','preview_id','request_id'])),
    ('recover_document','Open the original durable operation checkpoint in an idle unbound document. Refuses a loaded file or current edits/history. Preserves the original project identity, rotates session, requires Save As and cannot overwrite the original file. Use when recover_operation reports RECOVERY_DOCUMENT_REQUIRED.',object_schema({'target':TARGET,'expected_revision':{'type':'integer'},'original_target':TARGET,'request_id':{'type':'string'}},['target','expected_revision','original_target','request_id'])),
    ('recover_operation','Explicitly reconcile the original interrupted operation. Requires current target/revision plus saved original_target. Model changes require matching original file baseline and no intervening history; refuses user edits. Never accepts caller-supplied snapshots.',object_schema({'target':TARGET,'expected_revision':{'type':'integer'},'original_target':TARGET,'request_id':{'type':'string'}},['target','expected_revision','original_target','request_id'])),
    ('get_operation', 'Recover the original receipt after response loss, including after Undo. Does not replay any write.', object_schema({'target':TARGET,'request_id':{'type':'string'},'original_target':{**TARGET,'description':'Original saved target before interruption; current target must bind the same project.'}}, ['target','request_id'])),
    ('undo_operation', 'Safely undo this operation only if it is still the top unchanged model transaction. Returns UNDO_CONFLICT after intervening user changes.', object_schema({'target':TARGET,'request_id':{'type':'string'},'original_target':{**TARGET,'description':'Original saved target before interruption; current target must bind the same project.'}}, ['target','request_id'])),
    ('cancel_operation', 'Cancel an uncommitted preview. A committed operation returns already_committed and is not rolled back.', object_schema({'target':TARGET,'preview_id':{'type':'string'}}, ['target','preview_id'])),
]:
    TOOLS.append({'name':name,'description':description,'inputSchema':schema})

TOOLS.extend([
    {'name':'list_plugin_commands', 'description':'Read the same local plugin registrations as the native UI, including disabled declarations. Discovery only: does not enable/install a plugin or dispatch calculation. Paginate with returned catalog_sha256; declarations are not evidence of execution or approval.', 'inputSchema':object_schema({'target':TARGET,'offset':{'type':'integer','minimum':0},'limit':{'type':'integer','minimum':1,'maximum':4},'catalog_sha256':{'type':['string','null']}},['target','offset','limit'])},
    {'name':'list_plugin_jobs', 'description':'List original durable job request IDs in this exact current project. Read-only, no compute/replay, no renewed task authorization. Lexical ID order is not chronology. Paginate with returned index_sha256; read each original ID with read_plugin_job. Missing provider does not remove records.', 'inputSchema':object_schema({'target':TARGET,'expected_revision':{'type':'integer','minimum':0},'offset':{'type':'integer','minimum':0},'limit':{'type':'integer','minimum':1,'maximum':16},'index_sha256':{'type':['string','null']}},['target','expected_revision','offset','limit'])},
    {'name':'read_plugin_job', 'description':'Read and validate an original project job by request_id, even with missing/disabled provider. Returns phase/current/stale/unknown and a hex byte range of complete JSON intent/input/result/cancellation. Decode contiguous ranges, continue with content_sha256, and verify SHA256/total_bytes before using the complete artifact. Model/context versions are exact; no dispatch, cancel, file write or model effect. Use artifact=process for original exit/timeout/cancel status and bounded untrusted stdout/stderr. Missing diagnostics remain unknown. Unknown is not failure or permission to replay.', 'inputSchema':object_schema({'target':TARGET,'expected_revision':{'type':'integer','minimum':0},'expected_context_revision':{'type':'string'},'request_id':{'type':'string','pattern':'^[a-f0-9]{64}$'},'offset':{'type':'integer','minimum':0},'limit':{'type':'integer','minimum':1,'maximum':32768},'content_sha256':{'type':['string','null']},'artifact':{'type':'string','enum':['record','process'],'description':'record (default) is complete job input/result; process is the original bounded executable diagnostics receipt. Both are read-only, paginated and identity-bound.'}},['target','expected_revision','expected_context_revision','request_id','offset','limit'])},
    {'name':'invoke_workbench_command', 'description':'Invoke a supported native workbench UI command through the shared dispatcher. Requires enabled write permission, exact target/revision, empty parameters and a unique request_id saved before sending. Active user gestures and modal dialogs block execution. Does not commit geometry; Save/global Undo/Redo use their existing transaction adapters. On response loss query get_workbench_operation with the original identity; do not replay unknown outcomes. Receipts last for this application process.', 'inputSchema':object_schema({'target':TARGET,'expected_revision':{'type':'integer','minimum':0},'request_id':{'type':'string','minLength':1,'maxLength':256},'command_id':{'type':'string'},'parameters':{'type':'object','properties':{},'additionalProperties':False}},['target','expected_revision','request_id','command_id','parameters'])},
    {'name':'get_workbench_operation', 'description':'Read the original process-local workbench invocation result by original target/request_id. No re-execution. OPERATION_UNKNOWN after missing receipt or restart is not permission to replay.', 'inputSchema':object_schema({'target':TARGET,'request_id':{'type':'string','minLength':1,'maxLength':256}},['target','request_id'])},
])

ENVIRONMENT_TOOLS = {'inspect_work_environment','propose_work_environment','get_environment_proposal','cancel_environment_proposal'}
TOOLS.extend([
    {'name':'inspect_work_environment','description':'Read the actual shared personal environment configuration and immutable snapshot. No model/configuration write. Registered command IDs are discovered with list_workbench_commands. Configuration is user-local, not part of the BIM model.', 'inputSchema':object_schema({'target':TARGET,'expected_revision':{'type':'integer','minimum':0}},['target','expected_revision'])},
    {'name':'propose_work_environment','description':'Persist one bounded original configuration proposal, never apply it or execute code. Requires current default write-task permission, exact project/config snapshot and an original request_id saved before sending. Native user review/Save is required to apply. Canonical proposal <=32 KiB and existing64 KiB transport-frame cap both apply. On response loss query get_environment_proposal with the SAME ID; unknown is not failure or permission to replay. No automatic baseline renewal after user edits.', 'inputSchema':object_schema({'target':TARGET,'expected_revision':{'type':'integer','minimum':0},'proposal':{'type':'object','additionalProperties':False,'required':['schema_version','request_id','project_id','expected_snapshot','configuration'],'properties':{'schema_version':{'const':1},'request_id':{'type':'string','minLength':1,'maxLength':128,'pattern':'^[A-Za-z0-9._-]+$'},'project_id':{'type':'string','pattern':'^[a-f0-9]{64}$'},'expected_snapshot':{'type':'string','pattern':'^[a-f0-9]{64}$'},'configuration':{'type':'object'}}}},['target','expected_revision','proposal'])},
    {'name':'get_environment_proposal','description':'Read the original durable configuration proposal, exact intended/applied configurations, diff and observed outcome. Reading never applies or restarts a write; unknown remains unknown. Use the current target of the same project after restart, retaining original request_id.', 'inputSchema':object_schema({'target':TARGET,'request_id':{'type':'string','minLength':1,'maxLength':128}},['target','request_id'])},
    {'name':'cancel_environment_proposal','description':'Cancel only an original pending configuration proposal. Does not undo an applied configuration, modify the model or restore task authority. Already applying/unknown outcomes are returned for inspection, never replayed.', 'inputSchema':object_schema({'target':TARGET,'request_id':{'type':'string','minLength':1,'maxLength':128}},['target','request_id'])},
])

query_tool = next(t for t in TOOLS if t['name'] == 'query_model')
space_tool = next(t for t in TOOLS if t['name'] == 'analyze_spaces')
query_tool['inputSchema']['properties']['design_scope'] = space_tool['inputSchema']['properties']['options']['properties']['design_scope']
query_tool['inputSchema']['properties']['fields']['items']['enum'].append('design_option')
query_tool['description'] += ' Explicit design_scope all/main/option filters membership using the plan predicate; omitted is All, with mixed-alternative warning. Scope binds pagination cursors. Scoped geometry with alternative wall joins is unsupported.'

for tool in TOOLS:
    tool['annotations'] = {'readOnlyHint': tool['name'] not in {'commit_change','undo_operation','cancel_operation','save_project','reopen_project','recover_operation','recover_document','invoke_workbench_command','propose_work_environment','cancel_environment_proposal'}, 'destructiveHint': tool['name'] in {'commit_change','undo_operation'}, 'openWorldHint': False}


def call_host(path, name, arguments):
    if name in {'get_target','list_instances','get_native_schema'} and arguments != {}:
        return {'error': {'code': 'INVALID_REQUEST'}}
    frame = {'method': name}
    if name not in {'get_target','list_instances','get_native_schema'}:
        frame['params'] = arguments
    data = (json.dumps(frame, separators=(',', ':')) + '\n').encode()
    if len(data) > 65536:
        return {'error': {'code': 'INVALID_FRAME'}}
    family = getattr(socket, 'AF_UNIX', None)
    if family is None:
        return {'error': {'code': 'NO_INSTANCE',
            'message': 'Local Unix transport is unavailable on this platform.'}}
    try:
        with socket.socket(family, socket.SOCK_STREAM) as connection:
            connection.settimeout(6)
            connection.connect(path)
            connection.sendall(data)
            with connection.makefile('rb') as reader:
                line = reader.readline(4 * 1024 * 1024 + 1)
            if len(line) > 4 * 1024 * 1024 or not line.endswith(b'\n'):
                if name in {'propose_work_environment','cancel_environment_proposal'}:
                    raise ValueError('Invalid response after dispatch')
                return {'error': {'code': 'INVALID_HOST_RESPONSE'}}
            response = json.loads(line)
            if name in {'propose_work_environment','cancel_environment_proposal'} and (
                    not isinstance(response, dict) or ('result' in response) == ('error' in response)):
                raise ValueError('Invalid response after dispatch')
            return response
    except (FileNotFoundError, ConnectionRefusedError):
        return {'error': {'code': 'NO_INSTANCE'}}
    except (OSError, ValueError):
        if name in {'propose_work_environment','cancel_environment_proposal'}:
            original = arguments.get('proposal', {}) if name == 'propose_work_environment' else arguments
            return {'error': {'code': 'ENVIRONMENT_OUTCOME_UNKNOWN', 'retry_safe': False,
                'request_id': original.get('request_id'), 'target': arguments.get('target'),
                'read_tool': 'get_environment_proposal',
                'message': 'The response was lost or invalid. Query the original proposal on the same service; do not replay or change its identity.'}}
        return {'error': {'code': 'HOST_UNAVAILABLE'}}


def read_source_content(path, arguments, host_call=None):
    """Assemble bounded readonly ranges. Never expose partial bytes as an image/read success."""
    import hashlib
    import base64
    caller = host_call or call_host
    payload = bytearray()
    first = None
    while True:
        response = caller(path, 'read_project_source', {**arguments, 'offset':len(payload), 'limit':32768})
        if 'error' in response:
            return response, []
        part = response.get('result', {})
        try:
            if first is None:
                first = {k:v for k,v in part.items() if k not in {'data','offset','returned_bytes','next_offset'}}
                if not 0 <= first['total_bytes'] <= 2*1024*1024:
                    raise ValueError('Source representation limit')
            elif first != {k:v for k,v in part.items() if k not in {'data','offset','returned_bytes','next_offset'}}:
                raise ValueError('Source context/content changed during read')
            data = bytes.fromhex(part['data'])
            if part['encoding'] != 'hex' or part['offset'] != len(payload) or len(data) != part['returned_bytes'] or len(data)>32768:
                raise ValueError('Invalid source byte range')
            payload.extend(data)
            if len(payload)>first['total_bytes']:
                raise ValueError('Source byte range overflow')
            if part['next_offset'] is None:
                if len(payload)!=first['total_bytes'] or hashlib.sha256(payload).hexdigest()!=first['content_sha256']:
                    raise ValueError('Incomplete or changed source content')
                break
            if not data or part['next_offset']!=len(payload):
                raise ValueError('Noncontiguous source read')
        except (KeyError,ValueError,TypeError) as error:
            return {'error':{'code':'INVALID_SOURCE_CONTENT','message':str(error)}}, []
    receipt = {**first, 'assembled_bytes':len(payload),'complete_byte_read':True,'agent_understanding':'not_evaluated'}
    if first['mime_type'] in {'image/png','image/jpeg'}:
        content = [{'type':'image','mimeType':first['mime_type'],'data':base64.b64encode(payload).decode('ascii')}]
    elif first['mime_type'] in {'text/plain','application/json'}:
        try:
            content = [{'type':'text','text':payload.decode('utf-8')}]
        except UnicodeDecodeError:
            return {'error':{'code':'INVALID_SOURCE_CONTENT','message':'Invalid UTF-8 representation'}}, []
    else:
        content = [{'type':'text','text':'Verified original binary source; visual/text interpretation unsupported. Request an explicitly supported text representation when available.'}]
        receipt['original_base64'] = base64.b64encode(payload).decode('ascii')
    return {'result':receipt}, content


def dispatch(request, path):
    method = request.get('method')
    params = request.get('params', {})
    if method == 'initialize':
        return {'protocolVersion': PROTOCOL, 'capabilities': {'tools': {}}, 'serverInfo': {'name': 'openarchi-local', 'version': '0.1.0'}}
    if method == 'ping':
        return {}
    if method == 'tools/list':
        tools = [dict(t) for t in TOOLS]
        schema = call_host(path,'get_native_schema',{}).get('result',{})
        if schema.get('environment_proposal_version') != 1:
            tools = [t for t in tools if t['name'] not in ENVIRONMENT_TOOLS]
        elif 'environment_proposal_schema' in schema:
            native = dict(schema['environment_proposal_schema']);native.pop('$schema',None)
            next(t for t in tools if t['name']=='propose_work_environment')['inputSchema']=native
        return {'tools': tools}
    if method == 'tools/call':
        name = params.get('name')
        if name not in {tool['name'] for tool in TOOLS}:
            raise ValueError('Unknown tool')
        arguments = params.get('arguments', {})
        if not isinstance(arguments, dict):
            raise ValueError('Invalid arguments')
        if name == 'read_project_source':
            response, content = read_source_content(path, arguments)
        else:
            response, content = call_host(path, name, arguments), []
        value = response.get('result', response)
        return {'content': content or [{'type': 'text', 'text': json.dumps(value, ensure_ascii=False)}], 'structuredContent': value, 'isError': 'error' in response}
    raise ValueError('Unknown method')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--socket', required=True)
    args = parser.parse_args()
    initialized = False
    ready = False
    while True:
        line = sys.stdin.buffer.readline(65538)
        if not line:
            break
        request = None
        try:
            if len(line) > 65536:
                raise ValueError('Request too large')
            request = json.loads(line)
            if not isinstance(request, dict) or request.get('jsonrpc') != '2.0':
                raise ValueError('Invalid JSON-RPC request')
            if 'id' not in request:
                if request.get('method') == 'notifications/initialized' and initialized:
                    ready = True
                continue
            method = request.get('method')
            if method == 'initialize':
                if initialized:
                    raise ValueError('Already initialized')
                initialized = True
            elif method != 'ping' and not ready:
                raise ValueError('Not initialized')
            response = {'jsonrpc': '2.0', 'id': request['id'], 'result': dispatch(request, args.socket)}
        except (ValueError, TypeError, AttributeError):
            response = {'jsonrpc': '2.0', 'id': request.get('id') if isinstance(request, dict) else None, 'error': {'code': -32600, 'message': 'Invalid request or unsupported method'}}
        print(json.dumps(response, ensure_ascii=False), flush=True)


if __name__ == '__main__':
    main()
