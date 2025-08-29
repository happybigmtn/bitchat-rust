-- Audit logging and security tracking
-- Version: 005
-- Description: Comprehensive audit trail for security and compliance

CREATE TABLE IF NOT EXISTS audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL CHECK(event_type IN (
        'user_login', 'user_logout', 'user_create', 'user_update', 'user_delete',
        'game_create', 'game_join', 'game_leave', 'game_complete',
        'bet_place', 'bet_cancel', 'bet_resolve',
        'transaction_create', 'transaction_confirm', 'transaction_fail',
        'consensus_propose', 'consensus_vote', 'consensus_finalize',
        'security_violation', 'admin_action', 'system_event'
    )),
    entity_type TEXT CHECK(entity_type IN ('user', 'game', 'bet', 'transaction', 'consensus', 'system')),
    entity_id TEXT,
    user_id TEXT REFERENCES users(id),
    old_value TEXT,
    new_value TEXT,
    metadata TEXT,
    ip_address TEXT,
    user_agent TEXT,
    session_id TEXT,
    device_fingerprint TEXT,
    geo_location TEXT,
    risk_score REAL DEFAULT 0.0 CHECK(risk_score BETWEEN 0 AND 10),
    created_at INTEGER NOT NULL
);

-- Security events for threat detection
CREATE TABLE IF NOT EXISTS security_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_category TEXT NOT NULL CHECK(event_category IN (
        'authentication', 'authorization', 'data_access', 'network', 'consensus', 'gaming'
    )),
    event_severity TEXT NOT NULL CHECK(event_severity IN ('low', 'medium', 'high', 'critical')),
    event_title TEXT NOT NULL,
    event_description TEXT,
    source_ip TEXT,
    target_resource TEXT,
    user_id TEXT REFERENCES users(id),
    attack_type TEXT,
    mitigation_action TEXT,
    false_positive INTEGER DEFAULT 0,
    resolved INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL,
    resolved_at INTEGER
);

-- Data integrity checks
CREATE TABLE IF NOT EXISTS integrity_checks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    check_type TEXT NOT NULL CHECK(check_type IN ('checksum', 'signature', 'consensus', 'balance')),
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    expected_hash TEXT,
    actual_hash TEXT,
    is_valid INTEGER NOT NULL,
    error_details TEXT,
    performed_by TEXT,
    performed_at INTEGER NOT NULL
);

-- Compliance tracking
CREATE TABLE IF NOT EXISTS compliance_records (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    regulation_type TEXT NOT NULL CHECK(regulation_type IN ('gdpr', 'ccpa', 'pci_dss', 'aml', 'kyc')),
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    compliance_status TEXT NOT NULL CHECK(compliance_status IN ('compliant', 'non_compliant', 'pending', 'exempt')),
    compliance_data TEXT,
    expiry_date INTEGER,
    verified_by TEXT,
    notes TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Access control log
CREATE TABLE IF NOT EXISTS access_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id TEXT REFERENCES users(id),
    resource_type TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    access_type TEXT NOT NULL CHECK(access_type IN ('read', 'write', 'delete', 'execute')),
    permission_granted INTEGER NOT NULL,
    denial_reason TEXT,
    access_context TEXT,
    created_at INTEGER NOT NULL
);

-- Indices for audit logging
CREATE INDEX IF NOT EXISTS idx_audit_log_event_type ON audit_log(event_type);
CREATE INDEX IF NOT EXISTS idx_audit_log_entity ON audit_log(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_user_id ON audit_log(user_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_created_at ON audit_log(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_log_risk_score ON audit_log(risk_score DESC);

CREATE INDEX IF NOT EXISTS idx_security_events_category ON security_events(event_category);
CREATE INDEX IF NOT EXISTS idx_security_events_severity ON security_events(event_severity);
CREATE INDEX IF NOT EXISTS idx_security_events_resolved ON security_events(resolved);
CREATE INDEX IF NOT EXISTS idx_security_events_created_at ON security_events(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_security_events_user ON security_events(user_id);

CREATE INDEX IF NOT EXISTS idx_integrity_checks_type ON integrity_checks(check_type);
CREATE INDEX IF NOT EXISTS idx_integrity_checks_entity ON integrity_checks(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_integrity_checks_valid ON integrity_checks(is_valid);
CREATE INDEX IF NOT EXISTS idx_integrity_checks_performed_at ON integrity_checks(performed_at);

CREATE INDEX IF NOT EXISTS idx_compliance_records_regulation ON compliance_records(regulation_type);
CREATE INDEX IF NOT EXISTS idx_compliance_records_entity ON compliance_records(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_compliance_records_status ON compliance_records(compliance_status);
CREATE INDEX IF NOT EXISTS idx_compliance_records_expiry ON compliance_records(expiry_date);

CREATE INDEX IF NOT EXISTS idx_access_log_user ON access_log(user_id);
CREATE INDEX IF NOT EXISTS idx_access_log_resource ON access_log(resource_type, resource_id);
CREATE INDEX IF NOT EXISTS idx_access_log_access_type ON access_log(access_type);
CREATE INDEX IF NOT EXISTS idx_access_log_granted ON access_log(permission_granted);
CREATE INDEX IF NOT EXISTS idx_access_log_created_at ON access_log(created_at DESC);