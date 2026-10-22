import assert from "node:assert/strict";
import test from "node:test";
import { modelTokenLimitsPayload } from "./model-token-limits.js";
const provider={id:"local",revision:7,adapter:"open_ai_completions",description:"Local",default_model:"ornith",context_window_tokens:131072,
  model_token_limits:{other:{max_prefix_tokens:2,max_prompt_tokens:3}}};
test("provider and prefix edits share exact per-model map and revision",()=>{
  const payload=modelTokenLimitsPayload(provider,"ornith","100000","14688",7);
  assert.equal(payload.expected_revision,7);
  assert.deepEqual(payload.model_token_limits,{other:provider.model_token_limits.other,ornith:{max_prefix_tokens:100000,max_prompt_tokens:14688}});
  assert.equal(payload.model_token_limits.ornith.max_prefix_tokens+payload.model_token_limits.ornith.max_prompt_tokens,114688);
  assert.equal(Object.hasOwn(payload,"context_window_tokens"),false);
  assert.equal(Object.hasOwn(payload,"set_active"),false);
  assert.equal(Object.hasOwn(payload,"api_key"),false);
  assert.deepEqual(provider.model_token_limits,{other:{max_prefix_tokens:2,max_prompt_tokens:3}});
});
test("stale unsupported and invalid edits cannot become another settings owner",()=>{
  for (const [prefix,prompt] of [["",1],[1,""],[true,1],[1,null],[-1,1],[1,0],[131072,1],[Number.MAX_SAFE_INTEGER,1],[1.5,1]])
    assert.throws(()=>modelTokenLimitsPayload(provider,"ornith",prefix,prompt,7));
  assert.throws(()=>modelTokenLimitsPayload(provider,"ornith",1,1,6));
  assert.throws(()=>modelTokenLimitsPayload({...provider,model_token_limits:undefined,revision:undefined},"ornith",1,1,7));
  assert.equal(modelTokenLimitsPayload(provider,"ornith",0,1,7).model_token_limits.ornith.max_prefix_tokens,0);
});
