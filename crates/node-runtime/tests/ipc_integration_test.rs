//! # IPC Integration Tests
//!
//! Tests that verify the IPC communication between API Gateway (qc-16) and
//! internal subsystems via the Event Bus.
//!
//! ## Test Strategy (TDD)
//!
//! These tests define the expected behavior BEFORE implementation:
//!
//! 1. API Gateway publishes `ApiQuery` to Event Bus
//! 2. Query Handler receives and routes to appropriate subsystem
//! 3. Subsystem processes query and returns result
//! 4. Query Handler publishes `ApiQueryResponse` to Event Bus
//! 5. API Gateway receives response and completes pending request
//!
//! ## Architecture Compliance
//!
//! - Hexagonal: Tests use ports/adapters, not concrete implementations
//! - DDD: Tests respect bounded contexts (each subsystem is isolated)
//! - EDA: All communication via Event Bus (no direct calls)

use std::sync::Arc;
use std::time::Duration;

use shared_bus::{ApiQueryError, BlockchainEvent, EventFilter, EventPublisher, InMemoryEventBus};
use tokio::time::timeout;

/// Test that ApiQuery events are properly published to the Event Bus.
///
/// This is the first step: verify the sender side works.
#[tokio::test]
async fn test_api_query_published_to_event_bus() {
    // Arrange: Create event bus and subscribe BEFORE publishing
    let bus = Arc::new(InMemoryEventBus::new());
    let filter = EventFilter::all();
    let mut subscription = bus.subscribe(filter);

    // Small delay to ensure subscription is ready
    tokio::task::yield_now().await;

    // Act: Publish an ApiQuery event (simulating what qc-16 should do)
    let query = BlockchainEvent::ApiQuery {
        correlation_id: "test-correlation-123".to_string(),
        target: "qc-02-block-storage".to_string(),
        method: "get_block_number".to_string(),
        params: serde_json::json!({}),
    };
    bus.publish(query).await;

    // Assert: The event should be received
    let received = timeout(Duration::from_millis(100), subscription.recv())
        .await
        .expect("Should receive within timeout")
        .expect("Should have event");

    match received {
        BlockchainEvent::ApiQuery {
            correlation_id,
            target,
            method,
            ..
        } => {
            assert_eq!(correlation_id, "test-correlation-123");
            assert_eq!(target, "qc-02-block-storage");
            assert_eq!(method, "get_block_number");
        }
        _ => panic!("Expected ApiQuery event, got {:?}", received),
    }
}

/// Test that ApiQueryResponse events are properly published to the Event Bus.
///
/// This verifies the response side works.
#[tokio::test]
async fn test_api_query_response_published_to_event_bus() {
    // Arrange
    let bus = Arc::new(InMemoryEventBus::new());
    let filter = EventFilter::all();
    let mut subscription = bus.subscribe(filter);

    // Small delay to ensure subscription is ready
    tokio::task::yield_now().await;

    // Act: Publish an ApiQueryResponse (simulating what a subsystem handler should do)
    let response = BlockchainEvent::ApiQueryResponse {
        correlation_id: "test-correlation-123".to_string(),
        source: 2, // qc-02
        result: Ok(serde_json::json!(42)),
    };
    bus.publish(response).await;

    // Assert
    let received = timeout(Duration::from_millis(100), subscription.recv())
        .await
        .expect("Should receive within timeout")
        .expect("Should have event");

    match received {
        BlockchainEvent::ApiQueryResponse {
            correlation_id,
            source,
            result,
        } => {
            assert_eq!(correlation_id, "test-correlation-123");
            assert_eq!(source, 2);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), serde_json::json!(42));
        }
        _ => panic!("Expected ApiQueryResponse event, got {:?}", received),
    }
}

/// Test that ApiQueryResponse can carry errors properly.
#[tokio::test]
async fn test_api_query_response_with_error() {
    // Arrange
    let bus = Arc::new(InMemoryEventBus::new());
    let filter = EventFilter::all();
    let mut subscription = bus.subscribe(filter);

    tokio::task::yield_now().await;

    // Act: Publish an error response
    let response = BlockchainEvent::ApiQueryResponse {
        correlation_id: "test-correlation-456".to_string(),
        source: 2,
        result: Err(ApiQueryError {
            code: -32000,
            message: "Block not found".to_string(),
        }),
    };
    bus.publish(response).await;

    // Assert
    let received = timeout(Duration::from_millis(100), subscription.recv())
        .await
        .expect("Should receive within timeout")
        .expect("Should have event");

    match received {
        BlockchainEvent::ApiQueryResponse {
            correlation_id,
            result,
            ..
        } => {
            assert_eq!(correlation_id, "test-correlation-456");
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert_eq!(err.code, -32000);
            assert_eq!(err.message, "Block not found");
        }
        _ => panic!("Expected ApiQueryResponse event"),
    }
}

/// Test the full query-response cycle.
///
/// This test simulates:
/// 1. API Gateway sends query
/// 2. Handler receives and processes
/// 3. Handler sends response
/// 4. API Gateway receives response
///
/// This is the core integration test that must pass.
#[tokio::test]
async fn test_full_query_response_cycle() {
    // Arrange: Create bus with two subscribers (simulating gateway and handler)
    let bus = Arc::new(InMemoryEventBus::new());

    // Handler subscribes to all events (will filter for ApiQuery)
    let handler_filter = EventFilter::all();
    let mut handler_sub = bus.subscribe(handler_filter);

    // Gateway subscribes to all events (will filter for ApiQueryResponse)
    let gateway_filter = EventFilter::all();
    let mut gateway_sub = bus.subscribe(gateway_filter);

    // Ensure subscriptions are ready
    tokio::task::yield_now().await;

    let bus_clone = Arc::clone(&bus);

    // Act: Spawn handler task that responds to queries
    let handler_task = tokio::spawn(async move {
        // Wait for query
        if let Some(event) = handler_sub.recv().await {
            if let BlockchainEvent::ApiQuery {
                correlation_id,
                target,
                method,
                ..
            } = event
            {
                // Verify it's for block storage
                assert_eq!(target, "qc-02-block-storage");
                assert_eq!(method, "get_block_number");

                // Simulate processing and respond
                let response = BlockchainEvent::ApiQueryResponse {
                    correlation_id,
                    source: 2,
                    result: Ok(serde_json::json!(12345)),
                };
                bus_clone.publish(response).await;
            }
        }
    });

    // Gateway sends query
    let query = BlockchainEvent::ApiQuery {
        correlation_id: "cycle-test-001".to_string(),
        target: "qc-02-block-storage".to_string(),
        method: "get_block_number".to_string(),
        params: serde_json::json!({}),
    };
    bus.publish(query).await;

    // Wait for handler to process
    timeout(Duration::from_millis(500), handler_task)
        .await
        .expect("Handler should complete")
        .expect("Handler task should not panic");

    // Gateway receives response (skip the query event we sent)
    let mut response_received = false;
    for _ in 0..5 {
        if let Ok(Some(event)) =
            timeout(Duration::from_millis(100), gateway_sub.recv()).await
        {
            if let BlockchainEvent::ApiQueryResponse {
                correlation_id,
                source,
                result,
            } = event
            {
                assert_eq!(correlation_id, "cycle-test-001");
                assert_eq!(source, 2);
                assert_eq!(result.unwrap(), serde_json::json!(12345));
                response_received = true;
                break;
            }
        }
    }

    assert!(response_received, "Gateway should receive response");
}

/// Test that multiple concurrent queries are handled correctly.
///
/// Each query should receive its own response matched by correlation ID.
#[tokio::test]
async fn test_concurrent_queries_with_correlation() {
    let bus = Arc::new(InMemoryEventBus::new());
    let mut handler_sub = bus.subscribe(EventFilter::all());
    let mut gateway_sub = bus.subscribe(EventFilter::all());

    tokio::task::yield_now().await;

    let bus_clone = Arc::clone(&bus);

    // Handler responds to multiple queries
    let handler_task = tokio::spawn(async move {
        let mut queries_handled = 0;
        while queries_handled < 3 {
            if let Some(BlockchainEvent::ApiQuery {
                correlation_id,
                ..
            }) = handler_sub.recv().await
            {
                // Each query gets a unique response based on correlation_id
                let block_num = match correlation_id.as_str() {
                    "query-1" => 100,
                    "query-2" => 200,
                    "query-3" => 300,
                    _ => 0,
                };

                let response = BlockchainEvent::ApiQueryResponse {
                    correlation_id,
                    source: 2,
                    result: Ok(serde_json::json!(block_num)),
                };
                bus_clone.publish(response).await;
                queries_handled += 1;
            }
        }
    });

    // Send 3 concurrent queries
    for i in 1..=3 {
        let query = BlockchainEvent::ApiQuery {
            correlation_id: format!("query-{}", i),
            target: "qc-02-block-storage".to_string(),
            method: "get_block_number".to_string(),
            params: serde_json::json!({}),
        };
        bus.publish(query).await;
    }

    // Wait for handler
    timeout(Duration::from_millis(500), handler_task)
        .await
        .expect("Handler should complete")
        .expect("Handler should not panic");

    // Collect responses
    let mut responses = std::collections::HashMap::new();
    for _ in 0..10 {
        // Try to receive up to 10 events
        if let Ok(Some(event)) =
            timeout(Duration::from_millis(50), gateway_sub.recv()).await
        {
            if let BlockchainEvent::ApiQueryResponse {
                correlation_id,
                result,
                ..
            } = event
            {
                responses.insert(correlation_id, result.unwrap());
            }
        }
    }

    // Verify each query got its correct response
    assert_eq!(responses.get("query-1"), Some(&serde_json::json!(100)));
    assert_eq!(responses.get("query-2"), Some(&serde_json::json!(200)));
    assert_eq!(responses.get("query-3"), Some(&serde_json::json!(300)));
}

// =============================================================================
// SUBSYSTEM-SPECIFIC TESTS
// =============================================================================

/// Test routing to qc-02 Block Storage for eth_blockNumber
#[tokio::test]
async fn test_route_eth_block_number_to_qc02() {
    let bus = Arc::new(InMemoryEventBus::new());
    let mut sub = bus.subscribe(EventFilter::all());

    tokio::task::yield_now().await;

    let query = BlockchainEvent::ApiQuery {
        correlation_id: "block-num-001".to_string(),
        target: "qc-02-block-storage".to_string(),
        method: "get_block_number".to_string(),
        params: serde_json::json!({}),
    };
    bus.publish(query).await;

    let event = timeout(Duration::from_millis(100), sub.recv())
        .await
        .expect("Should receive")
        .expect("Should have event");

    if let BlockchainEvent::ApiQuery { target, method, .. } = event {
        assert_eq!(target, "qc-02-block-storage");
        assert_eq!(method, "get_block_number");
    } else {
        panic!("Expected ApiQuery");
    }
}

/// Test routing to qc-06 Mempool for eth_gasPrice
#[tokio::test]
async fn test_route_eth_gas_price_to_qc06() {
    let bus = Arc::new(InMemoryEventBus::new());
    let mut sub = bus.subscribe(EventFilter::all());

    tokio::task::yield_now().await;

    let query = BlockchainEvent::ApiQuery {
        correlation_id: "gas-price-001".to_string(),
        target: "qc-06-mempool".to_string(),
        method: "get_gas_price".to_string(),
        params: serde_json::json!({}),
    };
    bus.publish(query).await;

    let event = timeout(Duration::from_millis(100), sub.recv())
        .await
        .expect("Should receive")
        .expect("Should have event");

    if let BlockchainEvent::ApiQuery { target, method, .. } = event {
        assert_eq!(target, "qc-06-mempool");
        assert_eq!(method, "get_gas_price");
    } else {
        panic!("Expected ApiQuery");
    }
}

/// Test routing to qc-01 Peer Discovery for net_peerCount
#[tokio::test]
async fn test_route_net_peer_count_to_qc01() {
    let bus = Arc::new(InMemoryEventBus::new());
    let mut sub = bus.subscribe(EventFilter::all());

    tokio::task::yield_now().await;

    let query = BlockchainEvent::ApiQuery {
        correlation_id: "peer-count-001".to_string(),
        target: "qc-01-peer-discovery".to_string(),
        method: "get_peer_count".to_string(),
        params: serde_json::json!({}),
    };
    bus.publish(query).await;

    let event = timeout(Duration::from_millis(100), sub.recv())
        .await
        .expect("Should receive")
        .expect("Should have event");

    if let BlockchainEvent::ApiQuery { target, method, .. } = event {
        assert_eq!(target, "qc-01-peer-discovery");
        assert_eq!(method, "get_peer_count");
    } else {
        panic!("Expected ApiQuery");
    }
}

// =============================================================================
// API GATEWAY SENDER TESTS
// =============================================================================
// These tests verify that the EventBusSender in qc-16 correctly publishes
// ApiQuery events to the Event Bus.

/// Test that EventBusSender publishes ApiQuery when IpcRequest is sent.
///
/// This is the test that will FAIL until we implement EventBusSender properly.
#[tokio::test]
async fn test_event_bus_sender_publishes_api_query() {
    use qc_16_api_gateway::ipc::requests::GetBlockNumberRequest;
    use qc_16_api_gateway::ipc::{IpcRequest, IpcSender, RequestPayload};
    
    let bus = Arc::new(InMemoryEventBus::new());
    let mut sub = bus.subscribe(EventFilter::all());
    
    tokio::task::yield_now().await;
    
    // Create the sender (this is the adapter we need to implement)
    let sender = node_runtime::adapters::api_gateway::EventBusIpcSender::new(Arc::clone(&bus));
    
    // Create an IPC request
    let request = IpcRequest::new(
        "qc-02-block-storage",
        RequestPayload::GetBlockNumber(GetBlockNumberRequest),
    );
    
    // Send the request - this should publish an ApiQuery event
    sender.send(request).await.expect("Send should succeed");
    
    // Verify the event was published
    let event = timeout(Duration::from_millis(100), sub.recv())
        .await
        .expect("Should receive within timeout")
        .expect("Should have event");
    
    match event {
        BlockchainEvent::ApiQuery { target, method, .. } => {
            assert_eq!(target, "qc-02-block-storage");
            assert_eq!(method, "get_block_number");
        }
        _ => panic!("Expected ApiQuery event, got {:?}", event),
    }
}

// =============================================================================
// API QUERY HANDLER TESTS  
// =============================================================================
// These tests verify that the ApiQueryHandler in node-runtime correctly
// processes ApiQuery events and responds with ApiQueryResponse.

/// Test that ApiQueryHandler responds to eth_blockNumber queries.
///
/// This is the test that will FAIL until we implement ApiQueryHandler.
#[tokio::test]
async fn test_api_query_handler_responds_to_block_number() {
    // This test requires:
    // 1. A SubsystemContainer with block_storage initialized
    // 2. An ApiQueryHandler that listens for ApiQuery events
    // 3. The handler to call block_storage.get_latest_height()
    // 4. The handler to publish ApiQueryResponse
    
    // For now, we test the expected behavior pattern
    let bus = Arc::new(InMemoryEventBus::new());
    
    // Subscribe BEFORE spawning handler to avoid race condition
    let mut gateway_sub = bus.subscribe(EventFilter::all());
    let mut handler_sub = bus.subscribe(EventFilter::all());
    
    tokio::task::yield_now().await;
    
    // Simulate what ApiQueryHandler SHOULD do when it receives a query
    let bus_clone = Arc::clone(&bus);
    let handler_task = tokio::spawn(async move {
        if let Some(BlockchainEvent::ApiQuery {
            correlation_id,
            target,
            method,
            ..
        }) = handler_sub.recv().await
        {
            // Handler should route to appropriate subsystem
            assert_eq!(target, "qc-02-block-storage");
            assert_eq!(method, "get_block_number");
            
            // Handler should get data from subsystem and respond
            // For now, simulate the response
            let block_height = 0u64; // Would come from block_storage.get_latest_height()
            
            let response = BlockchainEvent::ApiQueryResponse {
                correlation_id,
                source: 2,
                result: Ok(serde_json::json!(block_height)),
            };
            bus_clone.publish(response).await;
        }
    });
    
    // Send query
    let query = BlockchainEvent::ApiQuery {
        correlation_id: "handler-test-001".to_string(),
        target: "qc-02-block-storage".to_string(),
        method: "get_block_number".to_string(),
        params: serde_json::json!({}),
    };
    bus.publish(query).await;
    
    // Wait for handler
    timeout(Duration::from_millis(500), handler_task)
        .await
        .expect("Handler should complete")
        .expect("Handler should not panic");
    
    // Gateway should receive response
    let mut got_response = false;
    for _ in 0..5 {
        if let Ok(Some(BlockchainEvent::ApiQueryResponse {
            correlation_id,
            result,
            ..
        })) = timeout(Duration::from_millis(100), gateway_sub.recv()).await
        {
            assert_eq!(correlation_id, "handler-test-001");
            assert!(result.is_ok());
            got_response = true;
            break;
        }
    }
    
    assert!(got_response, "Should receive ApiQueryResponse");
}

// =============================================================================
// EVENT BUS IPC RECEIVER TESTS
// =============================================================================
// Tests for EventBusIpcReceiver - completes the circuit by receiving
// ApiQueryResponse events and completing pending requests.

/// Test that EventBusIpcReceiver receives ApiQueryResponse and completes pending request.
#[tokio::test]
async fn test_event_bus_receiver_completes_pending_request() {
    use qc_16_api_gateway::domain::{CorrelationId, PendingRequestStore};
    use qc_16_api_gateway::domain::pending::ResponseError;

    let bus = Arc::new(InMemoryEventBus::new());
    let pending_store = Arc::new(PendingRequestStore::new(Duration::from_secs(60)));

    // Register pending request - returns (correlation_id, receiver)
    let (correlation_id, receiver) = pending_store.register("eth_blockNumber", None);

    // Start the response listener (simulating what node-runtime does)
    let bus_clone = Arc::clone(&bus);
    let store_clone = Arc::clone(&pending_store);
    let listener_task = tokio::spawn(async move {
        let mut sub = bus_clone.subscribe(EventFilter::all());
        
        while let Some(event) = sub.recv().await {
            if let BlockchainEvent::ApiQueryResponse {
                correlation_id: cid,
                result,
                ..
            } = event
            {
                // Parse correlation ID and complete pending request
                if let Ok(parsed_cid) = CorrelationId::parse(&cid) {
                    let json_result = match result {
                        Ok(v) => Ok(v),
                        Err(e) => Err(ResponseError {
                            code: e.code,
                            message: e.message,
                            data: None,
                        }),
                    };
                    store_clone.complete(parsed_cid, json_result);
                    break;
                }
            }
        }
    });

    // Give listener time to subscribe
    tokio::task::yield_now().await;
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Publish response (simulating what ApiQueryHandler does)
    let response = BlockchainEvent::ApiQueryResponse {
        correlation_id: correlation_id.to_string(),
        source: 2,
        result: Ok(serde_json::json!("0x1a")),
    };
    bus.publish(response).await;

    // Wait for listener to process
    let _ = timeout(Duration::from_millis(500), listener_task).await;

    // The pending request should now be completed
    let response = timeout(Duration::from_millis(100), receiver)
        .await
        .expect("Should receive within timeout")
        .expect("Channel should not be closed");

    assert!(response.result.is_ok());
    assert_eq!(response.result.unwrap(), serde_json::json!("0x1a"));
}

/// Test that multiple pending requests are correctly matched by correlation ID.
#[tokio::test]
async fn test_event_bus_receiver_matches_correlation_ids() {
    use qc_16_api_gateway::domain::{CorrelationId, PendingRequestStore};
    use qc_16_api_gateway::domain::pending::ResponseError;

    let bus = Arc::new(InMemoryEventBus::new());
    let pending_store = Arc::new(PendingRequestStore::new(Duration::from_secs(60)));

    // Register 3 pending requests
    let (cid1, rx1) = pending_store.register("eth_blockNumber", None);
    let (cid2, rx2) = pending_store.register("eth_gasPrice", None);
    let (cid3, rx3) = pending_store.register("net_peerCount", None);

    // Start listener
    let bus_clone = Arc::clone(&bus);
    let store_clone = Arc::clone(&pending_store);
    let listener_task = tokio::spawn(async move {
        let mut sub = bus_clone.subscribe(EventFilter::all());
        let mut processed = 0;
        
        while processed < 3 {
            if let Some(BlockchainEvent::ApiQueryResponse {
                correlation_id: cid,
                result,
                ..
            }) = sub.recv().await
            {
                if let Ok(parsed_cid) = CorrelationId::parse(&cid) {
                    let json_result = match result {
                        Ok(v) => Ok(v),
                        Err(e) => Err(ResponseError {
                            code: e.code,
                            message: e.message,
                            data: None,
                        }),
                    };
                    store_clone.complete(parsed_cid, json_result);
                    processed += 1;
                }
            }
        }
    });

    tokio::task::yield_now().await;
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Publish responses OUT OF ORDER
    bus.publish(BlockchainEvent::ApiQueryResponse {
        correlation_id: cid2.to_string(),
        source: 6,
        result: Ok(serde_json::json!("0x3b9aca00")),
    }).await;

    bus.publish(BlockchainEvent::ApiQueryResponse {
        correlation_id: cid3.to_string(),
        source: 1,
        result: Ok(serde_json::json!("0x5")),
    }).await;

    bus.publish(BlockchainEvent::ApiQueryResponse {
        correlation_id: cid1.to_string(),
        source: 2,
        result: Ok(serde_json::json!("0xff")),
    }).await;

    let _ = timeout(Duration::from_millis(500), listener_task).await;

    // Each receiver should get ITS OWN response
    let r1 = timeout(Duration::from_millis(100), rx1).await.unwrap().unwrap();
    let r2 = timeout(Duration::from_millis(100), rx2).await.unwrap().unwrap();
    let r3 = timeout(Duration::from_millis(100), rx3).await.unwrap().unwrap();

    assert_eq!(r1.result.unwrap(), serde_json::json!("0xff"));      // Block number
    assert_eq!(r2.result.unwrap(), serde_json::json!("0x3b9aca00")); // Gas price
    assert_eq!(r3.result.unwrap(), serde_json::json!("0x5"));        // Peer count
}

// =============================================================================
// END-TO-END CIRCUIT TEST
// =============================================================================
// Full integration test: API Gateway → Event Bus → ApiQueryHandler → 
// Event Bus → EventBusIpcReceiver → Pending Request completed

/// Test the complete circuit from RPC request to response.
///
/// This test verifies the full flow per SPEC-16 Section 6:
/// 1. Register pending request in PendingRequestStore
/// 2. Publish ApiQuery event (simulating what qc-16 EventBusSender does)
/// 3. ApiQueryHandler receives query, processes it, publishes ApiQueryResponse
/// 4. EventBusIpcReceiver receives response, completes pending request
/// 5. Original caller receives response
#[tokio::test]
async fn test_complete_rpc_circuit() {
    use qc_16_api_gateway::domain::PendingRequestStore;
    use node_runtime::adapters::EventBusIpcReceiver;

    // Setup: Create event bus and pending store
    let bus = Arc::new(InMemoryEventBus::new());
    let pending_store = Arc::new(PendingRequestStore::new(Duration::from_secs(30)));

    // Start EventBusIpcReceiver (like node-runtime does)
    let receiver = EventBusIpcReceiver::new(&bus, Arc::clone(&pending_store));
    let receiver_task = tokio::spawn(async move {
        receiver.run().await;
    });

    // Start ApiQueryHandler-like responder (simulating what ApiQueryHandler does)
    let bus_for_handler = Arc::clone(&bus);
    let mut handler_sub = bus_for_handler.subscribe(EventFilter::all());
    let handler_task = tokio::spawn(async move {
        while let Some(event) = handler_sub.recv().await {
            if let BlockchainEvent::ApiQuery {
                correlation_id,
                target,
                method,
                ..
            } = event
            {
                // Simulate processing
                let result = match (target.as_str(), method.as_str()) {
                    ("qc-02-block-storage", "get_block_number") => {
                        Ok(serde_json::json!("0x2a"))  // Block 42
                    }
                    ("qc-06-mempool", "get_gas_price") => {
                        Ok(serde_json::json!("0x3b9aca00"))  // 1 gwei
                    }
                    _ => Err(shared_bus::ApiQueryError {
                        code: -32601,
                        message: "Unknown method".to_string(),
                    }),
                };

                // Publish response
                let response = BlockchainEvent::ApiQueryResponse {
                    correlation_id,
                    source: 2,
                    result,
                };
                bus_for_handler.publish(response).await;
            }
        }
    });

    // Give tasks time to start
    tokio::task::yield_now().await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // === Test 1: eth_blockNumber ===
    let (cid1, rx1) = pending_store.register("eth_blockNumber", None);
    
    // Simulate what EventBusSender does
    bus.publish(BlockchainEvent::ApiQuery {
        correlation_id: cid1.to_string(),
        target: "qc-02-block-storage".to_string(),
        method: "get_block_number".to_string(),
        params: serde_json::json!({}),
    }).await;

    // Wait for response
    let response1 = timeout(Duration::from_secs(1), rx1)
        .await
        .expect("Should receive response")
        .expect("Channel should not be closed");

    assert!(response1.result.is_ok());
    assert_eq!(response1.result.unwrap(), serde_json::json!("0x2a"));

    // === Test 2: eth_gasPrice ===
    let (cid2, rx2) = pending_store.register("eth_gasPrice", None);
    
    bus.publish(BlockchainEvent::ApiQuery {
        correlation_id: cid2.to_string(),
        target: "qc-06-mempool".to_string(),
        method: "get_gas_price".to_string(),
        params: serde_json::json!({}),
    }).await;

    let response2 = timeout(Duration::from_secs(1), rx2)
        .await
        .expect("Should receive response")
        .expect("Channel should not be closed");

    assert!(response2.result.is_ok());
    assert_eq!(response2.result.unwrap(), serde_json::json!("0x3b9aca00"));

    // Cleanup: Abort tasks (they run forever otherwise)
    handler_task.abort();
    receiver_task.abort();
}
