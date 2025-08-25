//! Maintenance Scheduling and Automation

use std::time::SystemTime;
use serde::{Serialize, Deserialize};

/// Maintenance scheduler
pub struct MaintenanceScheduler {
    scheduled_tasks: Vec<MaintenanceTask>,
    windows: Vec<MaintenanceWindow>,
}

impl MaintenanceScheduler {
    pub fn new() -> Self {
        Self {
            scheduled_tasks: Vec::new(),
            windows: Vec::new(),
        }
    }

    pub fn schedule_task(&mut self, task: MaintenanceTask) {
        self.scheduled_tasks.push(task);
    }

    pub fn add_window(&mut self, window: MaintenanceWindow) {
        self.windows.push(window);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceTask {
    pub id: String,
    pub name: String,
    pub description: String,
    pub task_type: TaskType,
    pub scheduled_time: SystemTime,
    pub duration_minutes: u64,
    pub auto_execute: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskType {
    DatabaseMaintenance,
    SystemUpdate,
    BackupCleanup,
    LogRotation,
    SecurityScan,
    PerformanceOptimization,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceWindow {
    pub id: String,
    pub name: String,
    pub start_time: SystemTime,
    pub end_time: SystemTime,
    pub description: Option<String>,
    pub notification_enabled: bool,
}