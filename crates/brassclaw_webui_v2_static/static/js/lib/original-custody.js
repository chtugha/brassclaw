// Transport originals only. No registry, fallback storage, auth grant or schema:
// each owning consumer validates its exact permitted nonsecret record fields.
const invalidOriginal = () => new Error("Original could not be verified.");

export function readOriginalJson(storage, key, invalid = invalidOriginal) {
  try {
    const bytes = storage.getItem(key);
    // Stored JSON null is different from an absent key.
    return bytes === null ? { present: false } : { present: true, value: JSON.parse(bytes) };
  } catch { throw invalid(); }
}

export function writeOriginalJson(storage, key, value, invalid = invalidOriginal) {
  try {
    const bytes = JSON.stringify(value);
    storage.setItem(key, bytes);
    if (storage.getItem(key) !== bytes) throw invalid();
    return value;
  } catch { throw invalid(); }
}
