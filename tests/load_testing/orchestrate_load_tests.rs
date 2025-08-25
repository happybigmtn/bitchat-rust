//! Load Testing Orchestration for Production Validation
//! Executes comprehensive load testing scenarios for BitCraps

use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use serde_json;
use tracing::{info, warn, error};

use crate::mesh::service::MeshService;
use crate::transport::connection_pool::ConnectionPool;
use super::load_test_framework::{
    LoadTestOrchestrator, LoadTestConfig, LoadTestResults, LoadTestError, ResourceLimits
};

/// Production load testing scenarios
pub struct LoadTestSuite {
    mesh_service: Arc<MeshService>,
    connection_pool: Arc<ConnectionPool>,
}

impl LoadTestSuite {
    pub fn new(mesh_service: Arc<MeshService>, connection_pool: Arc<ConnectionPool>) -> Self {
        Self {
            mesh_service,
            connection_pool,
        }
    }

    /// Execute full production load test suite
    pub async fn execute_production_suite(&self) -> Result<Vec<LoadTestResults>, LoadTestError> {
        info!("Starting production load test suite");
        
        let scenarios = vec![
            ("Baseline Load Test", self.baseline_load_config()),
            ("Peak Load Test", self.peak_load_config()),
            ("Stress Test", self.stress_test_config()),
            ("Endurance Test", self.endurance_test_config()),
            ("Spike Test", self.spike_test_config()),
        ];

        let mut results = Vec::new();

        for (name, config) in scenarios {
            info!("Executing scenario: {}", name);
            
            let orchestrator = LoadTestOrchestrator::new(
                config,
                Arc::clone(&self.mesh_service),
                Arc::clone(&self.connection_pool),
            );

            // Set timeout for each test scenario
            let test_timeout = Duration::from_secs(config.duration_seconds + 120); // Extra 2 minutes
            
            match timeout(test_timeout, orchestrator.execute_load_test()).await {
                Ok(Ok(result)) => {
                    info!("Scenario '{}' completed successfully", name);
                    results.push(result);
                },
                Ok(Err(e)) => {
                    error!("Scenario '{}' failed: {:?}", name, e);
                    let mut failed_result = LoadTestResults::new();
                    failed_result.success = false;
                    failed_result.failure_reason = Some(format!("{:?}", e));
                    results.push(failed_result);
                },
                Err(_) => {
                    error!("Scenario '{}' timed out", name);
                    let mut failed_result = LoadTestResults::new();
                    failed_result.success = false;
                    failed_result.failure_reason = Some("Test timeout".to_string());
                    results.push(failed_result);
                }
            }

            // Cool-down period between tests
            info!("Cool-down period (60 seconds)...");
            tokio::time::sleep(Duration::from_secs(60)).await;
        }

        self.generate_test_report(&results).await;
        Ok(results)
    }

    /// Baseline load test - normal operating conditions
    fn baseline_load_config(&self) -> LoadTestConfig {
        LoadTestConfig {
            concurrent_users: 500,
            duration_seconds: 300,  // 5 minutes
            ramp_up_seconds: 60,
            target_ops_per_second: 2000,
            max_latency_ms: 200,
            max_error_rate: 0.5,
            resource_limits: ResourceLimits {
                max_memory_mb: 1024,
                max_cpu_percent: 70.0,
                max_connections: 2000,
            },
        }
    }

    /// Peak load test - expected peak traffic
    fn peak_load_config(&self) -> LoadTestConfig {
        LoadTestConfig {
            concurrent_users: 1000,
            duration_seconds: 600,  // 10 minutes
            ramp_up_seconds: 120,
            target_ops_per_second: 5000,
            max_latency_ms: 500,
            max_error_rate: 1.0,
            resource_limits: ResourceLimits {
                max_memory_mb: 2048,
                max_cpu_percent: 80.0,
                max_connections: 4000,
            },
        }
    }

    /// Stress test - beyond normal capacity
    fn stress_test_config(&self) -> LoadTestConfig {
        LoadTestConfig {
            concurrent_users: 2000,
            duration_seconds: 300,  // 5 minutes
            ramp_up_seconds: 180,
            target_ops_per_second: 10000,
            max_latency_ms: 1000,
            max_error_rate: 5.0,
            resource_limits: ResourceLimits {
                max_memory_mb: 4096,
                max_cpu_percent: 95.0,
                max_connections: 8000,
            },
        }
    }

    /// Endurance test - sustained load over time
    fn endurance_test_config(&self) -> LoadTestConfig {
        LoadTestConfig {
            concurrent_users: 750,
            duration_seconds: 3600,  // 1 hour
            ramp_up_seconds: 300,    // 5 minute ramp-up
            target_ops_per_second: 3000,
            max_latency_ms: 300,
            max_error_rate: 1.0,
            resource_limits: ResourceLimits {
                max_memory_mb: 1536,
                max_cpu_percent: 75.0,
                max_connections: 3000,
            },
        }
    }

    /// Spike test - sudden load increases
    fn spike_test_config(&self) -> LoadTestConfig {
        LoadTestConfig {
            concurrent_users: 1500,
            duration_seconds: 180,   // 3 minutes
            ramp_up_seconds: 30,     // Very fast ramp-up
            target_ops_per_second: 8000,
            max_latency_ms: 800,
            max_error_rate: 3.0,
            resource_limits: ResourceLimits {
                max_memory_mb: 3072,
                max_cpu_percent: 90.0,
                max_connections: 6000,
            },
        }
    }

    /// Generate comprehensive test report
    async fn generate_test_report(&self, results: &[LoadTestResults]) {
        let report = LoadTestReport {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            total_scenarios: results.len(),
            passed_scenarios: results.iter().filter(|r| r.success).count(),
            failed_scenarios: results.iter().filter(|r| !r.success).count(),
            scenario_results: results.iter().enumerate().map(|(i, result)| {
                ScenarioSummary {
                    scenario_name: match i {
                        0 => "Baseline Load Test".to_string(),
                        1 => "Peak Load Test".to_string(),
                        2 => "Stress Test".to_string(),
                        3 => "Endurance Test".to_string(),
                        4 => "Spike Test".to_string(),
                        _ => format!("Scenario {}", i + 1),
                    },
                    success: result.success,
                    duration_seconds: result.test_duration_seconds,
                    total_operations: result.total_operations,
                    ops_per_second: result.final_ops_per_second,
                    error_rate: result.final_error_rate,
                    average_latency_ms: result.average_latency_ms,
                    p95_latency_ms: result.latency_p95_ms,
                    p99_latency_ms: result.latency_p99_ms,
                    max_memory_mb: result.max_memory_usage_mb,
                    max_cpu_percent: result.max_cpu_usage_percent,
                    failure_reason: result.failure_reason.clone(),
                }
            }).collect(),
            overall_assessment: self.assess_overall_performance(results),
        };

        // Write report to file
        match serde_json::to_string_pretty(&report) {
            Ok(json) => {
                if let Err(e) = tokio::fs::write("load_test_report.json", json).await {
                    error!("Failed to write load test report: {:?}", e);
                } else {
                    info!("Load test report written to load_test_report.json");
                }
            },
            Err(e) => error!("Failed to serialize load test report: {:?}", e),
        }

        // Print summary to console
        self.print_test_summary(&report);
    }

    /// Assess overall system performance
    fn assess_overall_performance(&self, results: &[LoadTestResults]) -> PerformanceAssessment {
        let passed_count = results.iter().filter(|r| r.success).count();
        let total_count = results.len();
        
        let avg_latency = if !results.is_empty() {
            results.iter().map(|r| r.average_latency_ms).sum::<f64>() / results.len() as f64
        } else {
            0.0
        };

        let max_ops_per_sec = results.iter()
            .map(|r| r.final_ops_per_second)
            .fold(0.0, f64::max);

        let assessment_level = match passed_count as f64 / total_count as f64 {
            ratio if ratio >= 1.0 => AssessmentLevel::Excellent,
            ratio if ratio >= 0.8 => AssessmentLevel::Good,
            ratio if ratio >= 0.6 => AssessmentLevel::Acceptable,
            ratio if ratio >= 0.4 => AssessmentLevel::Poor,
            _ => AssessmentLevel::Critical,
        };

        PerformanceAssessment {
            overall_level: assessment_level,
            passed_scenarios: passed_count,
            total_scenarios: total_count,
            average_latency_ms: avg_latency,
            peak_throughput_ops_per_sec: max_ops_per_sec,
            recommendations: self.generate_recommendations(results),
        }
    }

    /// Generate performance recommendations
    fn generate_recommendations(&self, results: &[LoadTestResults]) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Check for high latency
        if results.iter().any(|r| r.average_latency_ms > 500.0) {
            recommendations.push("High latency detected. Consider optimizing database queries and network protocols.".to_string());
        }

        // Check for high error rates
        if results.iter().any(|r| r.final_error_rate > 2.0) {
            recommendations.push("Elevated error rates detected. Review error handling and circuit breaker configurations.".to_string());
        }

        // Check for memory usage
        if results.iter().any(|r| r.max_memory_usage_mb > 2048) {
            recommendations.push("High memory usage detected. Consider implementing memory optimizations and garbage collection tuning.".to_string());
        }

        // Check for CPU usage
        if results.iter().any(|r| r.max_cpu_usage_percent > 85.0) {
            recommendations.push("High CPU usage detected. Consider horizontal scaling or CPU optimization.".to_string());
        }

        // Check for failed tests
        let failed_count = results.iter().filter(|r| !r.success).count();
        if failed_count > 0 {
            recommendations.push(format!("{} test scenarios failed. Review failure reasons and implement fixes before production deployment.", failed_count));
        }

        if recommendations.is_empty() {
            recommendations.push("All performance metrics within acceptable ranges. System ready for production deployment.".to_string());
        }

        recommendations
    }

    /// Print test summary to console
    fn print_test_summary(&self, report: &LoadTestReport) {
        println!("\n" + "=".repeat(80).as_str());
        println!("LOAD TEST SUITE SUMMARY");
        println!("=".repeat(80));
        println!("Total Scenarios: {}", report.total_scenarios);
        println!("Passed: {} | Failed: {}", report.passed_scenarios, report.failed_scenarios);
        println!("Overall Assessment: {:?}", report.overall_assessment.overall_level);
        println!("Peak Throughput: {:.2} ops/sec", report.overall_assessment.peak_throughput_ops_per_sec);
        println!("Average Latency: {:.2}ms", report.overall_assessment.average_latency_ms);
        
        println!("\nScenario Details:");
        println!("{}", "-".repeat(80));
        for scenario in &report.scenario_results {
            let status = if scenario.success { "PASS" } else { "FAIL" };
            println!("{:<20} | {} | {:.2} ops/sec | {:.2}ms avg | {:.2}% errors", 
                scenario.scenario_name, status, scenario.ops_per_second, 
                scenario.average_latency_ms, scenario.error_rate);
            
            if let Some(reason) = &scenario.failure_reason {
                println!("    Failure: {}", reason);
            }
        }

        println!("\nRecommendations:");
        println!("{}", "-".repeat(80));
        for (i, rec) in report.overall_assessment.recommendations.iter().enumerate() {
            println!("{}. {}", i + 1, rec);
        }
        println!("=".repeat(80));
    }
}

/// Load test report structure
#[derive(Debug, serde::Serialize)]
struct LoadTestReport {
    timestamp: u64,
    total_scenarios: usize,
    passed_scenarios: usize,
    failed_scenarios: usize,
    scenario_results: Vec<ScenarioSummary>,
    overall_assessment: PerformanceAssessment,
}

/// Individual scenario summary
#[derive(Debug, serde::Serialize)]
struct ScenarioSummary {
    scenario_name: String,
    success: bool,
    duration_seconds: u64,
    total_operations: u64,
    ops_per_second: f64,
    error_rate: f64,
    average_latency_ms: f64,
    p95_latency_ms: f64,
    p99_latency_ms: f64,
    max_memory_mb: u64,
    max_cpu_percent: f64,
    failure_reason: Option<String>,
}

/// Overall performance assessment
#[derive(Debug, serde::Serialize)]
struct PerformanceAssessment {
    overall_level: AssessmentLevel,
    passed_scenarios: usize,
    total_scenarios: usize,
    average_latency_ms: f64,
    peak_throughput_ops_per_sec: f64,
    recommendations: Vec<String>,
}

/// Performance assessment levels
#[derive(Debug, serde::Serialize)]
enum AssessmentLevel {
    Excellent,  // All tests pass with great performance
    Good,       // Most tests pass with good performance
    Acceptable, // Some tests pass with acceptable performance
    Poor,       // Few tests pass, performance issues
    Critical,   // Most tests fail, critical issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;

    #[tokio::test]
    async fn test_load_test_suite_creation() {
        let mesh_service = Arc::new(MeshService::new().await.unwrap());
        let connection_pool = Arc::new(ConnectionPool::new(100));
        
        let suite = LoadTestSuite::new(mesh_service, connection_pool);
        
        // Test configuration generation
        let baseline = suite.baseline_load_config();
        assert_eq!(baseline.concurrent_users, 500);
        
        let peak = suite.peak_load_config();
        assert_eq!(peak.concurrent_users, 1000);
        
        let stress = suite.stress_test_config();
        assert_eq!(stress.concurrent_users, 2000);
    }

    #[test]
    fn test_performance_assessment() {
        let mesh_service = Arc::new(MeshService::new().await.unwrap());
        let connection_pool = Arc::new(ConnectionPool::new(100));
        let suite = LoadTestSuite::new(mesh_service, connection_pool);

        let mut results = vec![LoadTestResults::new(); 5];
        results[0].success = true;
        results[1].success = true; 
        results[2].success = false;
        results[3].success = true;
        results[4].success = true;

        let assessment = suite.assess_overall_performance(&results);
        assert_eq!(assessment.passed_scenarios, 4);
        assert_eq!(assessment.total_scenarios, 5);
    }
}