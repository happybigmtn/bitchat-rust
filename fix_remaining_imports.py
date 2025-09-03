#!/usr/bin/env python3

import re
import os

# Files that need import fixes
files_to_fix = [
    "src/mesh/mod.rs",
    "src/mesh/service.rs", 
    "src/mobile/mod.rs",
    "src/monitoring/alerting.rs",
    "src/monitoring/dashboard.rs",
    "src/monitoring/logging.rs",
    "src/monitoring/metrics.rs",
    "src/monitoring/prometheus_server.rs",
    "src/performance/benchmarking.rs",
    "src/performance/optimizer.rs",
    "src/protocol/ble_dispatch.rs",
    "src/protocol/ble_optimization.rs",
    "src/protocol/consensus_coordinator.rs",
    "src/protocol/network_consensus_bridge.rs",
    "src/protocol/runtime/game_lifecycle.rs",
    "src/resilience/mod.rs",
    "src/transport/crypto.rs",
    "src/transport/intelligent_coordinator.rs",
    "src/transport/kademlia.rs",
    "src/transport/keystore.rs",
    "src/transport/linux_ble.rs",
    "src/transport/mod.rs",
    "src/transport/nat_traversal.rs",
    "src/transport/security.rs",
    "src/transport/tcp_transport.rs"
]

import_line = "use crate::utils::task_tracker::{spawn_tracked, TaskType};"

def add_import_to_file(filepath):
    """Add the spawn_tracked import to a file if it doesn't exist"""
    if not os.path.exists(filepath):
        print(f"File not found: {filepath}")
        return
    
    with open(filepath, 'r') as f:
        content = f.read()
    
    # Check if import already exists
    if import_line in content:
        print(f"Import already exists in {filepath}")
        return
    
    lines = content.split('\n')
    
    # Find the best place to insert the import
    # Look for the first use statement and add after it
    insert_index = -1
    for i, line in enumerate(lines):
        if line.strip().startswith('use ') and 'crate::' in line:
            insert_index = i + 1
        elif line.strip().startswith('use ') and insert_index == -1:
            insert_index = i + 1
    
    # If no use statements found, add after module doc comments
    if insert_index == -1:
        for i, line in enumerate(lines):
            if not line.strip().startswith('//') and line.strip() != '':
                insert_index = i
                break
    
    # Insert the import
    if insert_index != -1:
        lines.insert(insert_index, import_line)
        lines.insert(insert_index + 1, '')  # Add blank line after
        
        with open(filepath, 'w') as f:
            f.write('\n'.join(lines))
        print(f"Added import to {filepath}")
    else:
        print(f"Could not find insertion point in {filepath}")

# Fix all files
for filepath in files_to_fix:
    add_import_to_file(filepath)

print("Import fixing complete!")