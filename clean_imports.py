#!/usr/bin/env python3

import re
import os
import subprocess

def get_files_with_spawn_tracked_errors():
    """Get files that have spawn_tracked compilation errors"""
    try:
        result = subprocess.run(['cargo', 'check', '--quiet'], 
                              capture_output=True, text=True, cwd='.')
        
        files = set()
        lines = result.stderr.split('\n')
        for line in lines:
            if 'cannot find function `spawn_tracked`' in line:
                # Look for the next line with file path
                idx = lines.index(line)
                if idx + 1 < len(lines):
                    next_line = lines[idx + 1]
                    if 'src/' in next_line:
                        # Extract file path
                        match = re.search(r'src/[^:]+', next_line)
                        if match:
                            files.add(match.group(0))
        return list(files)
    except Exception as e:
        print(f"Error getting files: {e}")
        return []

def clean_and_fix_imports(filepath):
    """Clean up malformed imports and add proper ones"""
    if not os.path.exists(filepath):
        print(f"File not found: {filepath}")
        return False
    
    with open(filepath, 'r') as f:
        content = f.read()
    
    lines = content.split('\n')
    cleaned_lines = []
    import_added = False
    target_import = "use crate::utils::task_tracker::{spawn_tracked, TaskType};"
    
    # Track if we've seen the import already
    has_proper_import = False
    
    i = 0
    while i < len(lines):
        line = lines[i].rstrip()
        
        # Skip malformed imports that are inside functions or in wrong places
        if (target_import in line and 
            (i > 50 or any(keyword in line for keyword in ['fn ', 'impl ', 'mod ', '{']))):
            print(f"Removing misplaced import at line {i+1} in {filepath}")
            i += 1
            continue
            
        # Check if we already have the import in proper location
        if target_import in line and i < 50:
            has_proper_import = True
            
        cleaned_lines.append(line)
        i += 1
    
    # Add import if not present
    if not has_proper_import:
        # Find insertion point - after other crate:: imports
        insert_idx = -1
        for idx, line in enumerate(cleaned_lines):
            if line.strip().startswith('use crate::') and not line.strip().startswith('use crate::utils::task_tracker::'):
                insert_idx = idx + 1
        
        # If no crate imports, add after regular use statements
        if insert_idx == -1:
            for idx, line in enumerate(cleaned_lines):
                if line.strip().startswith('use ') and not line.strip().startswith('//'):
                    insert_idx = idx + 1
        
        # If still not found, add after module declarations
        if insert_idx == -1:
            for idx, line in enumerate(cleaned_lines):
                if not (line.strip().startswith('//') or line.strip().startswith('pub mod') or line.strip() == ''):
                    insert_idx = idx
                    break
        
        if insert_idx != -1:
            cleaned_lines.insert(insert_idx, target_import)
            cleaned_lines.insert(insert_idx + 1, '')  # Add blank line
            print(f"Added proper import to {filepath}")
        else:
            print(f"Could not find insertion point for {filepath}")
    
    # Write cleaned content
    with open(filepath, 'w') as f:
        f.write('\n'.join(cleaned_lines))
    
    return True

# Get files with errors and clean them
error_files = get_files_with_spawn_tracked_errors()
print(f"Found {len(error_files)} files with spawn_tracked errors:")

for filepath in error_files:
    print(f"Cleaning {filepath}")
    clean_and_fix_imports(filepath)

print("Import cleanup complete!")