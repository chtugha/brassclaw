"""Independent test-only extraction from Ornith's retained default template."""
import json
from pathlib import Path
import sys
from transformers import AutoConfig, AutoTokenizer

originals = Path(sys.argv[1])
data = json.load(sys.stdin)
model_config = AutoConfig.from_pretrained(
    originals, local_files_only=True, trust_remote_code=False
)
tokenizer = AutoTokenizer.from_pretrained(
    originals, config=model_config, local_files_only=True, trust_remote_code=False
)
text = tokenizer.apply_chat_template(
    [
        {"role": "system", "content": data["reference"]},
        {"role": "user", "content": "Independent prefix boundary probe."},
    ],
    chat_template=(originals / "chat_template.jinja").read_text(encoding="utf-8"),
    tokenize=False,
    add_generation_prompt=False,
    continue_final_message=False,
    enable_thinking=False,
)
# This is a model-specific TEST frame, checked against an actual legal render.
# Full reference inclusion prevents a frame literal inside reference data from
# silently truncating the prefix. No trimming or template injection is allowed.
prefix, frame, tail = text.partition("<|im_start|>user\n")
assert frame and data["reference"] in prefix and tail
sys.stdout.write(prefix)
