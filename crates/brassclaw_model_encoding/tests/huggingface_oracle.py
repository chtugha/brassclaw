"""Independent offline HF/Jinja/tokenizers oracle; reads the exact worker request."""
import hashlib,json,struct,sys,os,copy
from transformers import AutoConfig, AutoTokenizer
from transformers.utils.chat_template_utils import _compile_jinja_template
request=json.load(sys.stdin)
assets=request['assets']
originals=os.environ['BRASSCLAW_ENCODING_MODEL_ORIGINALS']
# Require the actual supported model configuration. An old Transformers build
# must fail here rather than silently falling back to a generic configuration.
model_config=AutoConfig.from_pretrained(originals,local_files_only=True,trust_remote_code=False)
tokenizer=AutoTokenizer.from_pretrained(originals,config=model_config,local_files_only=True,trust_remote_code=False)
# Measure complete candidates, including ones deliberately too large for model
# inference. The physical native context guard checks that limit separately.
# Use HF's encoding backend, not the model-input preparation wrapper; retain
# original model limits and never truncate/pad an oversized measurement.
encoding_backend=tokenizer.backend_tokenizer
encoding_backend.no_truncation()
encoding_backend.no_padding()
config=json.loads(assets['tokenizer_config_json'])

def digest(data): return hashlib.sha256(data).hexdigest()
def ids_digest(ids): return digest(b''.join(struct.pack('<I',value) for value in ids))
defaults={name:(value.get('content') if isinstance(value,dict) else value) for name,value in config.items() if name in ['bos_token','eos_token','unk_token','sep_token','pad_token','cls_token','mask_token'] and value is not None}
template=_compile_jinja_template(assets['chat_template'])
def wire_context(probe):
    # Independent Python implementation of the explicit versioned projection.
    # This oracle supplies no approval or claim about deployed provider defaults.
    rules=json.loads(probe['mapping_json']); body=json.loads(probe['request_json'])
    assert rules['format']=='openai-text-chat-context/1'
    assert body['model']==probe['expected_model']
    context=json.loads(rules['render_defaults_json'])
    messages=copy.deepcopy(body['messages'])
    for message in messages:
        if isinstance(message.get('content'),list):
            assert all(set(p)=={'type','text'} and p['type']=='text' and isinstance(p['text'],str) for p in message['content'])
            if rules['text_parts']['mode']=='join': message['content']=rules['text_parts']['separator'].join(p['text'] for p in message['content'])
            else: assert rules['text_parts']['mode']=='preserve'
        if message.get('content') is None and rules['null_assistant_content']=='empty_string': message['content']=''
        for call in message.get('tool_calls') or []:
            if rules['tool_arguments']=='decode_json': call['function']['arguments']=json.loads(call['function']['arguments'])
    context['messages']=messages
    tools=body.get('tools')
    if tools is not None: context['tools']=tools
    elif rules['missing_tools']=='null': context['tools']=None
    if rules['forward_tool_choice']: context['tool_choice']=(body.get('tool_choice') or 'auto') if tools else None
    if rules['forward_response_format']: context['response_format']=body.get('response_format')
    context.update(body.get('chat_template_kwargs') or {})
    encoded=json.dumps(context,ensure_ascii=False,separators=(',',':'))
    output=body.get('max_completion_tokens') or body.get('max_tokens')
    assert isinstance(output,int) and not isinstance(output,bool) and output>0
    return encoded,dict(request_sha256=digest(probe['request_json'].encode()),mapping_sha256=digest(probe['mapping_json'].encode()),model=body['model'],reserved_output_tokens=output,render_context_sha256=digest(encoded.encode()))

probes=[];common=None
for probe in request['probes']:
    wire=None
    if probe['kind']=='wire_chat': context_json,wire=wire_context(probe)
    else: context_json=probe.get('context_json')
    if probe['kind'] in ('text', 'partitioned_text'): text=probe['text']
    else:
        context=json.loads(context_json)
        if 'messages' in context:
            messages=context.pop('messages')
            text=tokenizer.apply_chat_template(messages,chat_template=assets['chat_template'],tokenize=False,**context)
        else: text=template.render(**(defaults|context))
    if 'expected_reference' in request:
        # The model may trim separator whitespace after the reference.
        # Require every reference byte, not an unpromised trailing separator.
        expected = '<|im_start|>system\n' + request['expected_reference']
        if not text.startswith(expected):
            raise AssertionError('complete literal reference is not the stable leading rendered content')
    measured=encoding_backend.encode(text,add_special_tokens=False)
    ids=measured.ids
    result=dict(rendered_sha256=digest(text.encode()),rendered_bytes=len(text.encode()),token_count=len(ids),token_ids_sha256=ids_digest(ids))
    if probe['kind'] in ('partitioned_text','leading_prefix_chat','wire_chat'):
        if probe['kind'] in ('leading_prefix_chat','wire_chat'):
            assert text.startswith(probe['leading_prefix']), 'complete supplied prefix must lead the render'
            start,end=0,len(probe['leading_prefix'].encode())
        else:
            start,end=probe['prefix_start'],probe['prefix_end']
        # HF fast tokenizer offsets are Unicode character indices; convert to
        # original UTF-8 bytes before comparing to the worker's byte contract.
        byte_positions=[0]
        for character in text:
            byte_positions.append(byte_positions[-1]+len(character.encode()))
        assert start <= end and start in byte_positions and end in byte_positions
        assert start != end or start == 0
        prefix_ids=[];prompt_ids=[]
        assert len(measured.offsets)==len(ids)
        for value,(left,right) in zip(ids,measured.offsets):
            left,right=byte_positions[left],byte_positions[right]
            if start==end:
                prompt_ids.append(value)
            elif not 0 <= left < right <= len(text.encode()):
                raise AssertionError('unmappable physical offset')
            elif right <= start or left >= end:
                prompt_ids.append(value)
            elif start <= left and right <= end:
                prefix_ids.append(value)
            else:
                raise AssertionError('token spans partition boundary')
        result['partition']=dict(prefix_start=start,prefix_end=end,
            prefix_bytes_sha256=digest(text.encode()[start:end]),prefix_tokens=len(prefix_ids),
            prompt_tokens=len(prompt_ids),prefix_token_ids_sha256=ids_digest(prefix_ids),
            prompt_token_ids_sha256=ids_digest(prompt_ids),qualification='supplied-byte-span-only')
        if probe['kind'] in ('leading_prefix_chat','wire_chat'):
            result['partition']['qualification']='verified-leading-render-bytes-only'
            result['partition']['render_context_sha256']=digest(context_json.encode())
    if wire is not None: result['wire_input']=wire
    probes.append(result)
    if common is None: common=ids
    else:
        length=0
        for left,right in zip(common,ids):
            if left!=right: break
            length+=1
        common=common[:length]
print(json.dumps(dict(format='brassclaw-model-encoding/3' if any(p['kind']=='wire_chat' for p in request['probes']) else 'brassclaw-model-encoding/2' if any(p['kind'] in ('partitioned_text','leading_prefix_chat','wire_chat') for p in request['probes']) else 'brassclaw-model-encoding/1',tokenizer_sha256=digest(assets['tokenizer_json'].encode()),tokenizer_config_sha256=digest(assets['tokenizer_config_json'].encode()),chat_template_sha256=digest(assets['chat_template'].encode()),truncation_disabled=True,padding_disabled=True,probes=probes,common_prefix_tokens=len(common),common_prefix_sha256=ids_digest(common))))
