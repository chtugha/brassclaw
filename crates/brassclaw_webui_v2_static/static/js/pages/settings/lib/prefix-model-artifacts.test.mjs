import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
const context = { Uint8Array, TextEncoder, btoa: text => Buffer.from(text,"latin1").toString("base64") };
vm.createContext(context);
vm.runInContext(readFileSync(new URL("./prefix-model-artifacts.js",import.meta.url),"utf8").replace("export async function", "async function")+"\nglobalThis.prepare=prepareModelArtifactImport;", context);
function file(name,text) { const bytes=Buffer.from(text);return {name,size:bytes.length,arrayBuffer:async()=>bytes}; }
test("model file import retains exact UTF-8/CRLF bytes and no claimed provenance",async()=>{
  const files=[file("config.json",'{"model":"ü"}\r\n'),file("tokenizer.json","{}\n"),file("tokenizer_config.json",'{}')];
  const encoded=JSON.parse(await context.prepare(files));
  assert.equal(encoded.format,"brassclaw-prefix-model-artifact-import/1");
  assert.deepEqual(Object.keys(encoded).sort(),["artifacts","format"]);
  for (const item of files) assert.deepEqual(Buffer.from(encoded.artifacts[item.name].data,"base64"),Buffer.from(await item.arrayBuffer()));
});
test("missing, duplicate, unsupported and oversized model files fail before reads",async()=>{
  let reads=0;
  const base=["config.json","tokenizer.json","tokenizer_config.json"].map(name=>({name,size:2,arrayBuffer:async()=>{reads++;return Buffer.from('{}');}}));
  for (const files of [base.slice(1),[...base,base[0]],[...base,{name:"../script.py",size:1}],base.map(file=>({...file,size:64*1024*1024}))]) await assert.rejects(context.prepare(files));
  assert.equal(reads,0);
});
