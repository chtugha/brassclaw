-- Safety configuration table
-- Stores user-specific safety rules for sensitive paths, workspace rules, and blocked paths
CREATE TABLE IF NOT EXISTS safety_config (
    id BIGSERIAL PRIMARY KEY,
    user_id TEXT NOT NULL,
    category TEXT NOT NULL, -- 'sensitive_paths', 'workspace_rules', 'blocked_paths'
    pattern TEXT NOT NULL,
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    is_default BOOLEAN NOT NULL DEFAULT FALSE, -- System defaults cannot be deleted
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(user_id, category, pattern)
);

-- Index for efficient lookups by user and category
CREATE INDEX idx_safety_config_user_category ON safety_config(user_id, category);

-- Index for filtering enabled rules
CREATE INDEX idx_safety_config_enabled ON safety_config(is_enabled);

-- Made with Bob
