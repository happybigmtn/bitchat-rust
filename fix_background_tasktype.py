#!/usr/bin/env python3

import os
import subprocess

def fix_tasktype_references():
    """Fix all TaskType::Background references to TaskType::Maintenance"""
    
    # Get all files with TaskType::Background
    result = subprocess.run([
        'find', 'src', '-name', '*.rs', '-exec', 'grep', '-l', 'TaskType::Background', '{}', ';'
    ], capture_output=True, text=True)
    
    if result.returncode != 0:
        print("Error finding files")
        return
    
    files = result.stdout.strip().split('\n')
    if files == ['']:
        print("No files found with TaskType::Background")
        return
    
    print(f"Fixing TaskType::Background in {len(files)} files:")
    
    for filepath in files:
        if not os.path.exists(filepath):
            continue
            
        print(f"  {filepath}")
        
        # Read file
        with open(filepath, 'r') as f:
            content = f.read()
        
        # Replace TaskType::Background with TaskType::Maintenance
        new_content = content.replace('TaskType::Background', 'TaskType::Maintenance')
        
        # Write back if changed
        if content != new_content:
            with open(filepath, 'w') as f:
                f.write(new_content)
    
    print("TaskType::Background fixing complete!")

if __name__ == "__main__":
    fix_tasktype_references()