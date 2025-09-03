#!/usr/bin/env python3

import subprocess
import re
import os

def get_compilation_errors():
    """Get all compilation errors from cargo check"""
    result = subprocess.run(['cargo', 'check'], capture_output=True, text=True)
    return result.stderr

def fix_remaining_imports():
    """Fix all remaining import issues"""
    errors = get_compilation_errors()
    
    # Find all files with spawn_tracked or TaskType errors
    files_to_fix = set()
    
    for line in errors.split('\n'):
        if 'cannot find function `spawn_tracked`' in line or 'cannot find type `TaskType`' in line:
            # Look for the file reference in the next few lines
            continue
        if '-->' in line and 'src/' in line:
            # Extract file path
            match = re.search(r'src/[^:]+', line)
            if match:
                files_to_fix.add(match.group(0))
    
    print(f"Found {len(files_to_fix)} files needing import fixes:")
    for filepath in files_to_fix:
        print(f"  {filepath}")
    
    # Fix each file
    target_import = "use crate::utils::task_tracker::{spawn_tracked, TaskType};"
    
    for filepath in files_to_fix:
        if not os.path.exists(filepath):
            continue
            
        with open(filepath, 'r') as f:
            content = f.read()
        
        lines = content.split('\n')
        
        # Check if import already exists at the top (first 50 lines)
        has_proper_import = False
        for i, line in enumerate(lines[:50]):
            if target_import in line:
                has_proper_import = True
                break
        
        if has_proper_import:
            print(f"  {filepath} already has proper import")
            continue
        
        # Remove any imports that are too far down
        cleaned_lines = []
        for i, line in enumerate(lines):
            if target_import in line and i > 50:
                print(f"  Removing misplaced import at line {i+1} in {filepath}")
                continue
            cleaned_lines.append(line)
        
        # Add import at the proper location
        insert_idx = None
        for i, line in enumerate(cleaned_lines):
            if line.strip().startswith('use crate::') and not line.strip().startswith('use crate::utils::task_tracker::'):
                insert_idx = i + 1
            elif line.strip().startswith('use ') and insert_idx is None:
                insert_idx = i + 1
        
        # If no use statements found, add after module docs
        if insert_idx is None:
            for i, line in enumerate(cleaned_lines):
                if not (line.strip().startswith('//') or line.strip() == '' or line.strip().startswith('pub mod')):
                    insert_idx = i
                    break
        
        if insert_idx is not None:
            cleaned_lines.insert(insert_idx, target_import)
            cleaned_lines.insert(insert_idx + 1, '')  # Add blank line
            print(f"  Added proper import to {filepath}")
            
            # Write back
            with open(filepath, 'w') as f:
                f.write('\n'.join(cleaned_lines))
        else:
            print(f"  Could not find insertion point in {filepath}")
    
    print("Final import cleanup complete!")

if __name__ == "__main__":
    fix_remaining_imports()