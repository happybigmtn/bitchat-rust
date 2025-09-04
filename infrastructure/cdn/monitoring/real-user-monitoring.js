/**
 * Real User Monitoring (RUM) for BitCraps CDN
 * Collects performance metrics from actual users
 */

(function() {
    'use strict';
    
    // Configuration
    const RUM_CONFIG = {
        endpoint: '/api/analytics/rum',
        sampleRate: 0.1, // Sample 10% of users
        batchSize: 10,
        flushInterval: 30000, // 30 seconds
        enableDeviceInfo: true,
        enableNetworkInfo: true,
        enableWasmMonitoring: true
    };
    
    // Metrics storage
    let metricsQueue = [];
    let sessionId = generateSessionId();
    let userId = getUserId();
    let deviceInfo = collectDeviceInfo();
    
    // Performance observer for collecting metrics
    let performanceObserver;
    
    // Initialize RUM monitoring
    function initRUM() {
        if (Math.random() > RUM_CONFIG.sampleRate) {
            return; // Skip monitoring for this user (sampling)
        }
        
        console.log('BitCraps RUM: Initializing monitoring');
        
        // Collect initial page load metrics
        collectPageLoadMetrics();
        
        // Set up performance observers
        setupPerformanceObservers();
        
        // Set up WASM monitoring
        if (RUM_CONFIG.enableWasmMonitoring) {
            setupWasmMonitoring();
        }
        
        // Set up network monitoring
        if (RUM_CONFIG.enableNetworkInfo && navigator.connection) {
            setupNetworkMonitoring();
        }
        
        // Set up batch flushing
        setInterval(flushMetrics, RUM_CONFIG.flushInterval);
        
        // Flush on page unload
        window.addEventListener('beforeunload', flushMetrics);
        window.addEventListener('pagehide', flushMetrics);
    }
    
    // Generate unique session ID
    function generateSessionId() {
        return 'rum_' + Date.now() + '_' + Math.random().toString(36).substr(2, 9);
    }
    
    // Get or generate user ID
    function getUserId() {
        let userId = localStorage.getItem('bitcraps_user_id');
        if (!userId) {
            userId = 'user_' + Date.now() + '_' + Math.random().toString(36).substr(2, 9);
            localStorage.setItem('bitcraps_user_id', userId);
        }
        return userId;
    }
    
    // Collect device and browser information
    function collectDeviceInfo() {
        if (!RUM_CONFIG.enableDeviceInfo) return {};
        
        return {
            userAgent: navigator.userAgent,
            language: navigator.language,
            platform: navigator.platform,
            cookieEnabled: navigator.cookieEnabled,
            onLine: navigator.onLine,
            screenWidth: screen.width,
            screenHeight: screen.height,
            screenColorDepth: screen.colorDepth,
            windowWidth: window.innerWidth,
            windowHeight: window.innerHeight,
            devicePixelRatio: window.devicePixelRatio || 1,
            timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
            hardwareConcurrency: navigator.hardwareConcurrency || 'unknown'
        };
    }
    
    // Collect initial page load metrics
    function collectPageLoadMetrics() {
        if (!window.performance || !window.performance.timing) {
            return;
        }
        
        window.addEventListener('load', function() {
            setTimeout(function() {
                const timing = window.performance.timing;
                const navigation = window.performance.navigation;
                
                const metrics = {
                    type: 'page_load',
                    timestamp: Date.now(),
                    sessionId: sessionId,
                    userId: userId,
                    url: window.location.href,
                    referrer: document.referrer,
                    deviceInfo: deviceInfo,
                    
                    // Navigation timing metrics
                    navigationStart: timing.navigationStart,
                    domainLookupStart: timing.domainLookupStart,
                    domainLookupEnd: timing.domainLookupEnd,
                    connectStart: timing.connectStart,
                    connectEnd: timing.connectEnd,
                    secureConnectionStart: timing.secureConnectionStart,
                    requestStart: timing.requestStart,
                    responseStart: timing.responseStart,
                    responseEnd: timing.responseEnd,
                    domLoading: timing.domLoading,
                    domInteractive: timing.domInteractive,
                    domContentLoadedEventStart: timing.domContentLoadedEventStart,
                    domContentLoadedEventEnd: timing.domContentLoadedEventEnd,
                    domComplete: timing.domComplete,
                    loadEventStart: timing.loadEventStart,
                    loadEventEnd: timing.loadEventEnd,
                    
                    // Calculated metrics
                    dnsTime: timing.domainLookupEnd - timing.domainLookupStart,
                    tcpTime: timing.connectEnd - timing.connectStart,
                    sslTime: timing.secureConnectionStart > 0 ? timing.connectEnd - timing.secureConnectionStart : 0,
                    ttfb: timing.responseStart - timing.navigationStart,
                    responseTime: timing.responseEnd - timing.responseStart,
                    domProcessingTime: timing.domComplete - timing.domLoading,
                    pageLoadTime: timing.loadEventEnd - timing.navigationStart,
                    
                    // Navigation type
                    navigationType: navigation.type,
                    redirectCount: navigation.redirectCount
                };
                
                // Add paint metrics if available
                if (window.performance.getEntriesByType) {
                    const paintMetrics = window.performance.getEntriesByType('paint');
                    paintMetrics.forEach(function(entry) {
                        if (entry.name === 'first-paint') {
                            metrics.firstPaint = entry.startTime;
                        } else if (entry.name === 'first-contentful-paint') {
                            metrics.firstContentfulPaint = entry.startTime;
                        }
                    });
                }
                
                addMetric(metrics);
            }, 100);
        });
    }
    
    // Set up performance observers for modern metrics
    function setupPerformanceObservers() {
        if (!window.PerformanceObserver) {
            return;
        }
        
        try {
            // Largest Contentful Paint
            const lcpObserver = new PerformanceObserver(function(entryList) {
                const entries = entryList.getEntries();
                const lastEntry = entries[entries.length - 1];
                
                addMetric({
                    type: 'lcp',
                    timestamp: Date.now(),
                    sessionId: sessionId,
                    userId: userId,
                    url: window.location.href,
                    value: lastEntry.startTime,
                    element: lastEntry.element ? lastEntry.element.tagName : 'unknown'
                });
            });
            lcpObserver.observe({ entryTypes: ['largest-contentful-paint'] });
            
            // First Input Delay
            const fidObserver = new PerformanceObserver(function(entryList) {
                const entries = entryList.getEntries();
                entries.forEach(function(entry) {
                    addMetric({
                        type: 'fid',
                        timestamp: Date.now(),
                        sessionId: sessionId,
                        userId: userId,
                        url: window.location.href,
                        value: entry.processingStart - entry.startTime,
                        eventType: entry.name
                    });
                });
            });
            fidObserver.observe({ entryTypes: ['first-input'] });
            
            // Cumulative Layout Shift
            let clsScore = 0;
            const clsObserver = new PerformanceObserver(function(entryList) {
                const entries = entryList.getEntries();
                entries.forEach(function(entry) {
                    if (!entry.hadRecentInput) {
                        clsScore += entry.value;
                    }
                });
                
                addMetric({
                    type: 'cls',
                    timestamp: Date.now(),
                    sessionId: sessionId,
                    userId: userId,
                    url: window.location.href,
                    value: clsScore
                });
            });
            clsObserver.observe({ entryTypes: ['layout-shift'] });
            
            // Resource timing
            const resourceObserver = new PerformanceObserver(function(entryList) {
                const entries = entryList.getEntries();
                entries.forEach(function(entry) {
                    // Only monitor critical resources
                    if (entry.name.includes('.wasm') || 
                        entry.name.includes('.js') || 
                        entry.name.includes('.css') ||
                        entry.name.includes('/api/')) {
                        
                        addMetric({
                            type: 'resource',
                            timestamp: Date.now(),
                            sessionId: sessionId,
                            userId: userId,
                            url: window.location.href,
                            resourceUrl: entry.name,
                            resourceType: entry.initiatorType,
                            duration: entry.duration,
                            transferSize: entry.transferSize || 0,
                            encodedBodySize: entry.encodedBodySize || 0,
                            decodedBodySize: entry.decodedBodySize || 0,
                            responseStart: entry.responseStart,
                            responseEnd: entry.responseEnd,
                            cached: entry.transferSize === 0 && entry.decodedBodySize > 0
                        });
                    }
                });
            });
            resourceObserver.observe({ entryTypes: ['resource'] });
            
        } catch (error) {
            console.warn('BitCraps RUM: Performance observers not fully supported', error);
        }
    }
    
    // Set up WASM-specific monitoring
    function setupWasmMonitoring() {
        // Monitor WebAssembly instantiation
        const originalInstantiate = WebAssembly.instantiate;
        WebAssembly.instantiate = function() {
            const startTime = performance.now();
            return originalInstantiate.apply(this, arguments).then(function(result) {
                const endTime = performance.now();
                
                addMetric({
                    type: 'wasm_instantiate',
                    timestamp: Date.now(),
                    sessionId: sessionId,
                    userId: userId,
                    url: window.location.href,
                    duration: endTime - startTime,
                    wasmSize: arguments[0] ? arguments[0].byteLength : 'unknown'
                });
                
                return result;
            });
        };
        
        // Monitor fetch requests for WASM files
        const originalFetch = window.fetch;
        window.fetch = function(url) {
            if (typeof url === 'string' && url.endsWith('.wasm')) {
                const startTime = performance.now();
                return originalFetch.apply(this, arguments).then(function(response) {
                    const endTime = performance.now();
                    
                    addMetric({
                        type: 'wasm_fetch',
                        timestamp: Date.now(),
                        sessionId: sessionId,
                        userId: userId,
                        url: window.location.href,
                        wasmUrl: url,
                        duration: endTime - startTime,
                        status: response.status,
                        size: response.headers.get('content-length') || 'unknown',
                        cached: response.headers.get('cf-cache-status') === 'HIT' ||
                                response.headers.get('x-cache') === 'HIT'
                    });
                    
                    return response;
                });
            }
            return originalFetch.apply(this, arguments);
        };
    }
    
    // Set up network monitoring
    function setupNetworkMonitoring() {
        const connection = navigator.connection || navigator.mozConnection || navigator.webkitConnection;
        if (!connection) return;
        
        function recordConnectionInfo() {
            addMetric({
                type: 'network_info',
                timestamp: Date.now(),
                sessionId: sessionId,
                userId: userId,
                url: window.location.href,
                effectiveType: connection.effectiveType,
                downlink: connection.downlink,
                rtt: connection.rtt,
                saveData: connection.saveData
            });
        }
        
        recordConnectionInfo();
        connection.addEventListener('change', recordConnectionInfo);
    }
    
    // Add metric to queue
    function addMetric(metric) {
        metricsQueue.push(metric);
        
        if (metricsQueue.length >= RUM_CONFIG.batchSize) {
            flushMetrics();
        }
    }
    
    // Flush metrics to server
    function flushMetrics() {
        if (metricsQueue.length === 0) {
            return;
        }
        
        const metrics = metricsQueue.splice(0);
        
        // Use sendBeacon if available for reliability
        if (navigator.sendBeacon) {
            const data = JSON.stringify({
                metrics: metrics,
                timestamp: Date.now(),
                userAgent: navigator.userAgent,
                url: window.location.href
            });
            
            navigator.sendBeacon(RUM_CONFIG.endpoint, data);
        } else {
            // Fallback to fetch with keep-alive
            fetch(RUM_CONFIG.endpoint, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({
                    metrics: metrics,
                    timestamp: Date.now(),
                    userAgent: navigator.userAgent,
                    url: window.location.href
                }),
                keepalive: true
            }).catch(function(error) {
                console.warn('BitCraps RUM: Failed to send metrics', error);
                // Re-queue metrics for retry
                metricsQueue.unshift(...metrics);
            });
        }
    }
    
    // Error tracking
    window.addEventListener('error', function(event) {
        addMetric({
            type: 'javascript_error',
            timestamp: Date.now(),
            sessionId: sessionId,
            userId: userId,
            url: window.location.href,
            message: event.message,
            filename: event.filename,
            lineNumber: event.lineno,
            columnNumber: event.colno,
            stack: event.error ? event.error.stack : null
        });
    });
    
    // Unhandled promise rejections
    window.addEventListener('unhandledrejection', function(event) {
        addMetric({
            type: 'promise_rejection',
            timestamp: Date.now(),
            sessionId: sessionId,
            userId: userId,
            url: window.location.href,
            reason: event.reason ? event.reason.toString() : 'Unknown',
            stack: event.reason && event.reason.stack ? event.reason.stack : null
        });
    });
    
    // Visibility change tracking
    document.addEventListener('visibilitychange', function() {
        addMetric({
            type: 'visibility_change',
            timestamp: Date.now(),
            sessionId: sessionId,
            userId: userId,
            url: window.location.href,
            visibilityState: document.visibilityState,
            hidden: document.hidden
        });
    });
    
    // Custom BitCraps game metrics
    window.bitcrapsRUM = {
        // Track game events
        trackGameEvent: function(eventType, data) {
            addMetric({
                type: 'game_event',
                timestamp: Date.now(),
                sessionId: sessionId,
                userId: userId,
                url: window.location.href,
                eventType: eventType,
                data: data
            });
        },
        
        // Track custom performance marks
        mark: function(markName) {
            if (window.performance && window.performance.mark) {
                window.performance.mark(markName);
                
                addMetric({
                    type: 'performance_mark',
                    timestamp: Date.now(),
                    sessionId: sessionId,
                    userId: userId,
                    url: window.location.href,
                    markName: markName,
                    markTime: performance.now()
                });
            }
        },
        
        // Track custom timing
        measure: function(measureName, startMark, endMark) {
            if (window.performance && window.performance.measure) {
                window.performance.measure(measureName, startMark, endMark);
                const measure = window.performance.getEntriesByName(measureName)[0];
                
                addMetric({
                    type: 'performance_measure',
                    timestamp: Date.now(),
                    sessionId: sessionId,
                    userId: userId,
                    url: window.location.href,
                    measureName: measureName,
                    duration: measure.duration,
                    startMark: startMark,
                    endMark: endMark
                });
            }
        }
    };
    
    // Initialize when DOM is ready
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', initRUM);
    } else {
        initRUM();
    }
})();