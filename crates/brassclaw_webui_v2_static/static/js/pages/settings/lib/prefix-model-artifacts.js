// Model files are data. No path, program, source-origin or qualification fields
// are accepted from the browser. The server derives canonical hashes/provenance.
const names = new Set(["config.json", "tokenizer.json", "tokenizer_config.json", "special_tokens_map.json", "added_tokens.json", "vocab.json", "merges.txt", "chat_template.jinja"]);
export async function prepareModelArtifactImport(selected) {
  const files = Array.from(selected || []);
  if (!files.length || files.some(file => !names.has(file.name)) || new Set(files.map(file => file.name)).size !== files.length || !["config.json", "tokenizer.json", "tokenizer_config.json"].every(name => files.some(file => file.name === name))) throw new Error("prefix.modelArtifactFilesInvalid");
  const total = files.reduce((size, file) => size + file.size, 0);
  if (!Number.isSafeInteger(total) || total <= 0 || total > 45 * 1024 * 1024) throw new Error("prefix.modelArtifactFilesTooLarge");
  const artifacts = {};
  for (const file of files) {
    const bytes = new Uint8Array(await file.arrayBuffer());
    if (!bytes.length || bytes.length !== file.size) throw new Error("prefix.modelArtifactFilesInvalid");
    let text = "";
    for (let offset = 0; offset < bytes.length; offset += 8192) text += String.fromCharCode(...bytes.subarray(offset, offset + 8192));
    artifacts[file.name] = { encoding: "base64", data: btoa(text) };
  }
  const result = JSON.stringify({ format: "brassclaw-prefix-model-artifact-import/1", artifacts });
  if (new TextEncoder().encode(result).length > 63 * 1024 * 1024) throw new Error("prefix.modelArtifactFilesTooLarge");
  return result;
}
