import { SETTINGS_SECTIONS } from "../pages/settings/lib/settings-schema.js";

export const defaultRoute = "/chat";

// `hidden: true` keeps the route registered (direct URL access and
// breadcrumb/title resolution still work) but suppresses it from
// sidebar navigation. Routes whose page-level API libs are entirely
// Known tech-debt: stubs against missing v2 endpoints are hidden here until the
// matching `/api/webchat/v2/*` contracts land. Remove the flag once
// the page's `lib/*-api.js` calls real endpoints.
export const primaryRoutes = [
  { id: "chat", path: "/chat", labelKey: "nav.chat" },
  { id: "workspace", path: "/workspace", labelKey: "nav.workspace", hidden: true },
  { id: "projects", path: "/projects", labelKey: "nav.projects", hidden: true },
  { id: "jobs", path: "/jobs", labelKey: "nav.jobs", hidden: true },
  { id: "routines", path: "/routines", labelKey: "nav.routines", hidden: true },
  { id: "automations", path: "/automations", labelKey: "nav.automations" },
  { id: "missions", path: "/missions", labelKey: "nav.missions", hidden: true },
  { id: "extensions", path: "/extensions", labelKey: "nav.integrations" },
  { id: "settings", path: "/settings", labelKey: "nav.settings", hidden: false },
  { id: "admin", path: "/admin", labelKey: "nav.admin", hidden: true },
];

export const routeSectionDefs = [
  {
    labelKey: "nav.sectionWork",
    ids: ["chat", "workspace", "projects", "jobs", "routines", "automations", "missions"],
  },
  {
    labelKey: "nav.sectionSystem",
    ids: ["extensions", "settings", "admin"],
  },
];

// Settings tabs kept out of the sidebar. The route stays registered, so
// direct URL access still renders the tab; it just isn't advertised. Per
// the `hidden` rule above, drop an id from this set once its page-level
// api lib calls real endpoints.
const HIDDEN_SETTINGS_TABS = new Set([
  "agent",
  "channels",
  "networking",
  // Superseded by per-provider token settings in the provider dialog.
  "tokens",
]);

// Derived from `SETTINGS_SECTIONS`, the single source of truth for the
// settings tab set (`pages/settings/lib/settings-schema.js`). Keeping a
// hand-maintained copy here is what made the v3 component-catalog tabs
// reachable only by direct URL: the schema gained them, this list did not.
// `sectionKey` rides along so the sidebar can group entries the same way
// the schema does.
export const SETTINGS_SUB_ROUTES = SETTINGS_SECTIONS.flatMap((section) =>
  section.tabs
    .filter((tab) => !HIDDEN_SETTINGS_TABS.has(tab.id))
    .map((tab) => ({
      id: tab.id,
      labelKey: tab.labelKey,
      icon: tab.icon,
      sectionKey: section.sectionKey,
    }))
);

export const EXTENSIONS_SUB_ROUTES = [
  { id: "installed", labelKey: "extensions.installed", icon: "bolt" },
  { id: "channels", labelKey: "extensions.channels", icon: "send" },
  { id: "registry", labelKey: "extensions.registry", icon: "plus" },
];

export const ADMIN_SUB_ROUTES = [
  { id: "dashboard", labelKey: "admin.tab.dashboard", icon: "pulse" },
  { id: "users", labelKey: "admin.tab.users", icon: "lock" },
  { id: "usage", labelKey: "admin.tab.usage", icon: "spark" },
];

export const EXPANDABLE_SUB_ROUTES = {
  settings: SETTINGS_SUB_ROUTES,
  extensions: EXTENSIONS_SUB_ROUTES,
  admin: ADMIN_SUB_ROUTES,
};

export function routeForId(id) {
  return primaryRoutes.find((route) => route.id === id) || primaryRoutes[0];
}
