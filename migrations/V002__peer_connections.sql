-- Peer connectivity tracking
-- Version: 002
-- Description: Track peer-to-peer connections and network topology

CREATE TABLE IF NOT EXISTS peer_connections (
    id TEXT PRIMARY KEY,
    peer_id TEXT NOT NULL,
    connection_type TEXT NOT NULL CHECK(connection_type IN ('bluetooth', 'tcp', 'websocket', 'udp')),
    transport_layer TEXT CHECK(transport_layer IN ('ble', 'classic_bt', 'wifi_direct', 'internet')),
    signal_strength INTEGER CHECK(signal_strength BETWEEN -100 AND 0),
    latency_ms INTEGER CHECK(latency_ms >= 0),
    connected_at INTEGER NOT NULL,
    disconnected_at INTEGER,
    data_sent_bytes INTEGER DEFAULT 0 CHECK(data_sent_bytes >= 0),
    data_received_bytes INTEGER DEFAULT 0 CHECK(data_received_bytes >= 0),
    connection_quality REAL DEFAULT 1.0 CHECK(connection_quality BETWEEN 0 AND 1),
    error_count INTEGER DEFAULT 0,
    last_ping_ms INTEGER,
    is_active INTEGER DEFAULT 1
);

-- Network topology for mesh routing
CREATE TABLE IF NOT EXISTS network_topology (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    from_peer_id TEXT NOT NULL,
    to_peer_id TEXT NOT NULL,
    hop_count INTEGER NOT NULL DEFAULT 1,
    route_quality REAL DEFAULT 1.0,
    discovered_at INTEGER NOT NULL,
    expires_at INTEGER,
    UNIQUE(from_peer_id, to_peer_id)
);

-- Peer discovery events
CREATE TABLE IF NOT EXISTS peer_discovery_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    peer_id TEXT NOT NULL,
    discovery_method TEXT NOT NULL CHECK(discovery_method IN ('bluetooth_scan', 'dhcp_discover', 'broadcast', 'referral')),
    advertised_services TEXT,
    device_info TEXT,
    discovered_at INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL
);

-- Indices for peer connections
CREATE INDEX IF NOT EXISTS idx_peer_connections_peer_id ON peer_connections(peer_id);
CREATE INDEX IF NOT EXISTS idx_peer_connections_type ON peer_connections(connection_type);
CREATE INDEX IF NOT EXISTS idx_peer_connections_connected_at ON peer_connections(connected_at);
CREATE INDEX IF NOT EXISTS idx_peer_connections_active ON peer_connections(is_active);
CREATE INDEX IF NOT EXISTS idx_peer_connections_quality ON peer_connections(connection_quality);

CREATE INDEX IF NOT EXISTS idx_network_topology_from ON network_topology(from_peer_id);
CREATE INDEX IF NOT EXISTS idx_network_topology_to ON network_topology(to_peer_id);
CREATE INDEX IF NOT EXISTS idx_network_topology_hops ON network_topology(hop_count);

CREATE INDEX IF NOT EXISTS idx_discovery_events_peer ON peer_discovery_events(peer_id);
CREATE INDEX IF NOT EXISTS idx_discovery_events_method ON peer_discovery_events(discovery_method);
CREATE INDEX IF NOT EXISTS idx_discovery_events_last_seen ON peer_discovery_events(last_seen_at);