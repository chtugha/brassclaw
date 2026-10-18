"""Independent offline HF/Jinja/tokenizers oracle; reads the exact worker request."""
import hashlib,json,struct,sys,os
from transformers import AutoTokenizer
from transformers.utils.chat_template_utils import _compile_jinja_template
request=json.load(sys.stdin)
assets=request['assets']
tokenizer=AutoTokenizer.from_pretrained(os.environ['BRASSCLAW_ENCODING_MODEL_ORIGINALS'],local_files_only=True,trust_remote_code=False)
config=json.loads(assets['tokenizer_config_json'])

def digest(data): return hashlib.sha256(data).hexdigest()
def ids_digest(ids): return digest(b''.join(struct.pack('<I',value) for value in ids))
defaults={name:(value.get('content') if isinstance(value,dict) else value) for name,value in config.items() if name in ['bos_token','eos_token','unk_token','sep_token','pad_token','cls_token','mask_token'] and value is not None}
template=_compile_jinja_template(assets['chat_template'])
probes=[];common=None
for probe in request['probes']:
    if probe['kind']=='text': text=probe['text']
    else:
        context=json.loads(probe['context_json'])
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
    ids=tokenizer(text,add_special_tokens=False,truncation=False,padding=False)['input_ids']
    probes.append(dict(rendered_sha256=digest(text.encode()),rendered_bytes=len(text.encode()),token_count=len(ids),token_ids_sha256=ids_digest(ids)))
    if common is None: common=ids
    else:
        length=0
        for left,right in zip(common,ids):
            if left!=right: break
            length+=1
        common=common[:length]
print(json.dumps(dict(format='brassclaw-model-encoding/1',tokenizer_sha256=digest(assets['tokenizer_json'].encode()),tokenizer_config_sha256=digest(assets['tokenizer_config_json'].encode()),chat_template_sha256=digest(assets['chat_template'].encode()),truncation_disabled=True,padding_disabled=True,probes=probes,common_prefix_tokens=len(common),common_prefix_sha256=ids_digest(common))))
