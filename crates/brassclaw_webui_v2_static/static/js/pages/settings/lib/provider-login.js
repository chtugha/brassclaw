import { readProviderMutation, reconcileProviderMutation } from "./provider-mutations.js";

// Read and reconcile only this original login. Provider-name polling can report
// success for an older login and must never confirm the current request.
export async function waitForProviderLogin(id, providerId, deadlineMs, intervalMs, isCurrent = () => true) {
  if (typeof id !== "string" || !id) throw new Error("Original login identity is unavailable.");
  const deadline = Date.now() + deadlineMs;
  while (isCurrent() && Date.now() < deadline) {
    let originalKnown = false;
    try {
      const receipt = await readProviderMutation(id);
      if (!isCurrent()) return false;
      if (receipt?.mutation_id !== id || receipt.operation !== "upsert" || receipt.provider_id !== providerId) throw new Error("Original login acknowledgement differs.");
      originalKnown = true;
      const snapshot = await reconcileProviderMutation(id);
      if (!isCurrent()) return false;
      if (snapshot?.mutation_receipt?.mutation_id !== id) throw new Error("Original login reconciliation differs.");
      return true;
    } catch (error) {
      // An absent original before authorization is pending. Transport and
      // reconciliation failures remain errors, never success or another write.
      if (originalKnown || error?.status !== 404) throw error;
    }
    await new Promise(resolve => setTimeout(resolve, intervalMs));
  }
  return false;
}
