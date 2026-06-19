import { fetchTools, updateToolPermission } from "./settings-api.js";

/**
 * ToolsManager - Business logic for managing tool capabilities and permissions
 */
export class ToolsManager {
  constructor() {
    this.tools = [];
    this.filteredTools = [];
    this.groupedTools = new Map();
    this.isLoading = false;
    this.error = null;
  }

  /**
   * Initialize the manager by fetching tools from the API
   * @returns {Promise<void>}
   */
  async init() {
    this.isLoading = true;
    this.error = null;
    
    try {
      const response = await fetchTools();
      this.tools = response.capabilities || [];
      this.filteredTools = [...this.tools];
      this.groupByProvider();
      this.isLoading = false;
      return this.tools;
    } catch (err) {
      this.error = err;
      this.isLoading = false;
      throw err;
    }
  }

  /**
   * Get all tools
   * @returns {Array} List of tools
   */
  getTools() {
    return this.tools;
  }

  /**
   * Filter tools by search query
   * @param {string} query - Search query
   * @returns {Array} Filtered tools
   */
  filterTools(query) {
    if (!query || query.trim() === "") {
      this.filteredTools = [...this.tools];
    } else {
      const lowerQuery = query.toLowerCase();
      this.filteredTools = this.tools.filter((tool) => {
        const searchableFields = [
          tool.name,
          tool.description,
          tool.provider,
          ...(tool.effect_kinds || []),
        ];
        return searchableFields.some(
          (field) => field && field.toLowerCase().includes(lowerQuery)
        );
      });
    }
    this.groupByProvider();
    return this.filteredTools;
  }

  /**
   * Group tools by provider
   * @returns {Map} Map of provider -> tools
   */
  groupByProvider() {
    this.groupedTools = new Map();
    
    for (const tool of this.filteredTools) {
      const provider = tool.provider || "unknown";
      if (!this.groupedTools.has(provider)) {
        this.groupedTools.set(provider, []);
      }
      this.groupedTools.get(provider).push(tool);
    }
    
    return this.groupedTools;
  }

  /**
   * Update tool permission mode
   * @param {string} id - Tool ID
   * @param {string} mode - Permission mode (allow, ask, deny)
   * @returns {Promise<Object>} API response
   */
  async updatePermission(id, mode) {
    try {
      const response = await updateToolPermission(id, mode);
      
      // Update local state if successful
      if (response.success !== false) {
        const tool = this.tools.find((t) => t.id === id);
        if (tool) {
          tool.permission_mode = mode;
        }
      }
      
      return response;
    } catch (err) {
      throw err;
    }
  }

  /**
   * Get effect kind badge color
   * @param {string} effectKind - Effect kind
   * @returns {string} CSS color class
   */
  getEffectKindColor(effectKind) {
    const colorMap = {
      read: "bg-blue-500/20 text-blue-200 border-blue-400/30",
      write: "bg-amber-500/20 text-amber-200 border-amber-400/30",
      execute: "bg-red-500/20 text-red-200 border-red-400/30",
      network: "bg-purple-500/20 text-purple-200 border-purple-400/30",
      system: "bg-orange-500/20 text-orange-200 border-orange-400/30",
    };
    return colorMap[effectKind] || "bg-gray-500/20 text-gray-200 border-gray-400/30";
  }

  /**
   * Get permission mode display info
   * @param {string} mode - Permission mode
   * @returns {Object} Display info with label and color
   */
  getPermissionModeInfo(mode) {
    const infoMap = {
      allow: { labelKey: "settings.tools.permission.allow", color: "text-emerald-400" },
      ask: { labelKey: "settings.tools.permission.ask", color: "text-amber-400" },
      deny: { labelKey: "settings.tools.permission.deny", color: "text-red-400" },
    };
    return infoMap[mode] || infoMap.ask;
  }
}

// Made with Bob
