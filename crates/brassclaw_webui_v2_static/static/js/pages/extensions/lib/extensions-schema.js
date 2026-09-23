// The Integrations surface: installable runtime packages with an
// install/activate/credential lifecycle (`reborn_extensions_unified`,
// classes 4–8). It is NOT the class-23 ExtensionCatalogue component, which
// lives in Settings › Component Catalog › Extension Catalogues.
export const EXTENSIONS_TABS = [
  { id: "installed", label: "Installed", icon: "bolt" },
  { id: "channels", label: "Channels", icon: "send" },
  { id: "registry", label: "Registry", icon: "plus" },
];

export const KIND_LABELS = {
  first_party: "First-party",
  system: "System",
};

export const STATE_TONES = {
  active: "success",
  ready: "success",
  pairing_required: "warning",
  pairing: "warning",
  auth_required: "warning",
  setup_required: "muted",
  failed: "danger",
  installed: "muted",
};

export const STATE_LABELS = {
  active: "active",
  ready: "ready",
  pairing_required: "pairing",
  pairing: "pairing",
  auth_required: "auth needed",
  setup_required: "setup needed",
  failed: "failed",
  installed: "installed",
};
