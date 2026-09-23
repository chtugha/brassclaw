export const SETTINGS_SECTIONS = [
  {
    sectionKey: "settings.section.runtimeConfig",
    tabs: [
      { id: "inference",   labelKey: "settings.inference",   icon: "spark"    },
      { id: "agent",       labelKey: "settings.agent",       icon: "settings" },
      { id: "channels",    labelKey: "settings.channels",    icon: "send"     },
      { id: "networking",  labelKey: "settings.networking",  icon: "plug"     },
      { id: "monty-vm",    labelKey: "settings.montyVm",     icon: "pulse"    },
      { id: "tokens",      labelKey: "settings.tokens",      icon: "calendar" },
      { id: "language",    labelKey: "settings.language",    icon: "sun"      },
    ],
  },
  {
    // Class-coded catalog entries, in class-code order. `classCode` is the
    // single frontend source of the class→tab mapping; it mirrors
    // `class_code_to_table` in brassclaw_engine::memory::retrieval_source.
    sectionKey: "settings.section.componentCatalog",
    tabs: [
      { id: "tools",                labelKey: "settings.tools",                icon: "tool",   classCode: 0 },
      { id: "skills",               labelKey: "settings.skills",               icon: "file",   classCode: [1, 2, 3] },
      { id: "orchestrator",         labelKey: "settings.orchestrators",        icon: "layers", classCode: 10 },
      { id: "tool-skills",          labelKey: "settings.toolSkills",           icon: "tool",   classCode: 13 },
      { id: "actions",              labelKey: "settings.actions",              icon: "bolt",   classCode: 16 },
      { id: "docs",                 labelKey: "settings.docs",                 icon: "chat",   classCode: 17 },
      { id: "recipes",              labelKey: "settings.recipes",              icon: "copy",   classCode: 21 },
      { id: "python-code",          labelKey: "settings.pythonCode",           icon: "list",   classCode: 22 },
      { id: "extension-catalogues", labelKey: "settings.extensionCatalogues",  icon: "folder", classCode: 23 },
      { id: "scaffold",             labelKey: "settings.scaffolds",            icon: "layers", classCode: 50 },
    ],
  },
  {
    sectionKey: "settings.section.securityGovernance",
    tabs: [
      { id: "security",         labelKey: "settings.security",         icon: "shield" },
      { id: "safety",           labelKey: "settings.safety",           icon: "lock"   },
      { id: "validation-queue", labelKey: "settings.validationQueue",  icon: "check"  },
      { id: "interceptor",      labelKey: "settings.interceptor",      icon: "flag"   },
      { id: "prefix",           labelKey: "settings.prefix",           icon: "moon"   },
    ],
  },
  {
    sectionKey: "settings.section.accessOps",
    tabs: [
      { id: "tool-permissions",      labelKey: "settings.toolPermissions",      icon: "tool"     },
      { id: "users",                 labelKey: "settings.users",                icon: "logout"   },
      { id: "reliability",           labelKey: "settings.reliability",          icon: "clock"    },
    ],
  },
];

export const SETTINGS_TABS = SETTINGS_SECTIONS.flatMap((section) => section.tabs);

export const INFERENCE_FIELDS = [
  {
    groupKey: "settings.group.embeddings",
    fields: [
      { key: "embeddings.enabled", labelKey: "settings.field.embeddingsEnabled", descKey: "settings.field.embeddingsEnabledDesc", type: "boolean" },
      { key: "embeddings.provider", labelKey: "settings.field.embeddingsProvider", descKey: "settings.field.embeddingsProviderDesc", type: "select", options: ["openai", "nearai"] },
      { key: "embeddings.model", labelKey: "settings.field.embeddingsModel", descKey: "settings.field.embeddingsModelDesc", type: "text" },
    ],
  },
  {
    groupKey: "settings.group.sampling",
    fields: [
      { key: "temperature", labelKey: "settings.field.temperature", descKey: "settings.field.temperatureDesc", type: "float", min: 0, max: 2, step: 0.1 },
    ],
  },
];

export const AGENT_FIELDS = [
  {
    groupKey: "settings.group.core",
    fields: [
      { key: "agent.name", labelKey: "settings.field.agentName", descKey: "settings.field.agentNameDesc", type: "text" },
      { key: "agent.max_parallel_jobs", labelKey: "settings.field.maxParallelJobs", descKey: "settings.field.maxParallelJobsDesc", type: "number" },
      { key: "agent.job_timeout_secs", labelKey: "settings.field.jobTimeout", descKey: "settings.field.jobTimeoutDesc", type: "number" },
      { key: "agent.max_tool_iterations", labelKey: "settings.field.maxToolIterations", descKey: "settings.field.maxToolIterationsDesc", type: "number" },
      { key: "agent.use_planning", labelKey: "settings.field.planning", descKey: "settings.field.planningDesc", type: "boolean" },
      { key: "agent.auto_approve_tools", labelKey: "settings.field.autoApproveTools", descKey: "settings.field.autoApproveToolsDesc", type: "boolean" },
      { key: "agent.default_timezone", labelKey: "settings.field.timezone", descKey: "settings.field.timezoneDesc", type: "text" },
      { key: "agent.session_idle_timeout_secs", labelKey: "settings.field.sessionIdleTimeout", descKey: "settings.field.sessionIdleTimeoutDesc", type: "number" },
      { key: "agent.stuck_threshold_secs", labelKey: "settings.field.stuckThreshold", descKey: "settings.field.stuckThresholdDesc", type: "number" },
      { key: "agent.max_repair_attempts", labelKey: "settings.field.maxRepairAttempts", descKey: "settings.field.maxRepairAttemptsDesc", type: "number" },
      { key: "agent.max_cost_per_day_cents", labelKey: "settings.field.dailyCostLimit", descKey: "settings.field.dailyCostLimitDesc", type: "number", min: 0 },
      { key: "agent.max_actions_per_hour", labelKey: "settings.field.actionsPerHour", descKey: "settings.field.actionsPerHourDesc", type: "number", min: 0 },
      { key: "agent.allow_local_tools", labelKey: "settings.field.allowLocalTools", descKey: "settings.field.allowLocalToolsDesc", type: "boolean" },
    ],
  },
  {
    groupKey: "settings.group.heartbeat",
    fields: [
      { key: "heartbeat.enabled", labelKey: "settings.field.heartbeatEnabled", descKey: "settings.field.heartbeatEnabledDesc", type: "boolean" },
      { key: "heartbeat.interval_secs", labelKey: "settings.field.heartbeatInterval", descKey: "settings.field.heartbeatIntervalDesc", type: "number" },
      { key: "heartbeat.notify_channel", labelKey: "settings.field.heartbeatNotifyChannel", descKey: "settings.field.heartbeatNotifyChannelDesc", type: "text" },
      { key: "heartbeat.notify_user", labelKey: "settings.field.heartbeatNotifyUser", descKey: "settings.field.heartbeatNotifyUserDesc", type: "text" },
      { key: "heartbeat.quiet_hours_start", labelKey: "settings.field.quietHoursStart", descKey: "settings.field.quietHoursStartDesc", type: "number", min: 0, max: 23 },
      { key: "heartbeat.quiet_hours_end", labelKey: "settings.field.quietHoursEnd", descKey: "settings.field.quietHoursEndDesc", type: "number", min: 0, max: 23 },
      { key: "heartbeat.timezone", labelKey: "settings.field.heartbeatTimezone", descKey: "settings.field.heartbeatTimezoneDesc", type: "text" },
    ],
  },
  {
    groupKey: "settings.group.sandbox",
    fields: [
      { key: "sandbox.enabled", labelKey: "settings.field.sandboxEnabled", descKey: "settings.field.sandboxEnabledDesc", type: "boolean" },
      { key: "sandbox.policy", labelKey: "settings.field.sandboxPolicy", descKey: "settings.field.sandboxPolicyDesc", type: "select", options: ["readonly", "workspace_write", "full_access"] },
      { key: "sandbox.timeout_secs", labelKey: "settings.field.sandboxTimeout", descKey: "settings.field.sandboxTimeoutDesc", type: "number", min: 0 },
      { key: "sandbox.memory_limit_mb", labelKey: "settings.field.sandboxMemoryLimit", descKey: "settings.field.sandboxMemoryLimitDesc", type: "number", min: 0 },
      { key: "sandbox.image", labelKey: "settings.field.sandboxImage", descKey: "settings.field.sandboxImageDesc", type: "text" },
    ],
  },
  {
    groupKey: "settings.group.routines",
    fields: [
      { key: "routines.max_concurrent", labelKey: "settings.field.routinesMaxConcurrent", descKey: "settings.field.routinesMaxConcurrentDesc", type: "number", min: 0 },
      { key: "routines.default_cooldown_secs", labelKey: "settings.field.routinesDefaultCooldown", descKey: "settings.field.routinesDefaultCooldownDesc", type: "number", min: 0 },
    ],
  },
  {
    groupKey: "settings.group.safety",
    fields: [
      { key: "safety.max_output_length", labelKey: "settings.field.safetyMaxOutput", descKey: "settings.field.safetyMaxOutputDesc", type: "number", min: 0 },
      { key: "safety.injection_check_enabled", labelKey: "settings.field.safetyInjectionCheck", descKey: "settings.field.safetyInjectionCheckDesc", type: "boolean" },
    ],
  },
  {
    groupKey: "settings.group.skills",
    fields: [
      { key: "skills.max_active", labelKey: "settings.field.skillsMaxActive", descKey: "settings.field.skillsMaxActiveDesc", type: "number", min: 0 },
      { key: "skills.max_context_tokens", labelKey: "settings.field.skillsMaxContextTokens", descKey: "settings.field.skillsMaxContextTokensDesc", type: "number", min: 0 },
    ],
  },
  {
    groupKey: "settings.group.search",
    fields: [
      { key: "search.fusion_strategy", labelKey: "settings.field.fusionStrategy", descKey: "settings.field.fusionStrategyDesc", type: "select", options: ["rrf", "weighted"] },
    ],
  },
];

export const NETWORKING_FIELDS = [
  {
    groupKey: "settings.group.gateway",
    fields: [
      { key: "channels.gateway_host", labelKey: "settings.field.gatewayHost", descKey: "settings.field.gatewayHostDesc", type: "text" },
      { key: "channels.gateway_port", labelKey: "settings.field.gatewayPort", descKey: "settings.field.gatewayPortDesc", type: "number" },
    ],
  },
  {
    groupKey: "settings.group.tunnel",
    fields: [
      { key: "tunnel.provider", labelKey: "settings.field.tunnelProvider", descKey: "settings.field.tunnelProviderDesc", type: "select", options: ["ngrok", "cloudflare", "tailscale", "custom"] },
      { key: "tunnel.public_url", labelKey: "settings.field.tunnelPublicUrl", descKey: "settings.field.tunnelPublicUrlDesc", type: "text" },
    ],
  },
];

export const RESTART_REQUIRED_KEYS = new Set([
  "embeddings.enabled", "embeddings.provider", "embeddings.model",
  "agent.auto_approve_tools", "tunnel.provider", "tunnel.public_url",
  "gateway.rate_limit", "gateway.max_connections",
]);
