PART 1: ARCHITECTURAL CONTEXT & PRINCIPLES

      1.1 Core Architecture Mandates (Non-Negotiable)

      
       Mandate                          Source                  Implication                                                                   
      
       Choreography over Orchestration  Architecture.md v2.3    No subsystem "commands" another. All communication via Event Bus              
      
       Hexagonal Architecture           Architecture.md §2.2    Subsystems expose Ports (traits). Runtime provides Adapters (implementations) 
      
       Envelope-Only Identity           Architecture.md §3.2.1  AuthenticatedMessage<T>.sender_id is the sole identity source                 
      
       Zero-Trust Verification          IPC-MATRIX.md           Consensus (8) and Finality (9) MUST re-verify signatures independently        
      
       Two-Phase Commit                 System.md §6            Mempool transactions deleted ONLY after Block Storage confirmation            
      
       Time-Bounded Nonces              Architecture.md §3.2.2  All authenticated messages use nonce + timestamp for replay prevention        
      

      1.2 Dependency Graph (Critical for Initialization Order)

        LEVEL 0 (No Dependencies):
         [10] Signature Verification
        
        LEVEL 1 (Depends on Level 0):
         [1] Peer Discovery → [10]
         [6] Mempool → [10]
        
        LEVEL 2 (Depends on Level 0-1):
         [3] Transaction Indexing → [10]
         [4] State Management
         [5] Block Propagation → [1]
        
        LEVEL 3 (Depends on Level 0-2):
         [8] Consensus → [5, 6, 10]
        
        LEVEL 4 (Depends on Level 0-3):
         [2] Block Storage ← subscribes to [3, 4, 8] events
         [9] Finality → [8, 10]

      1.3 Choreography Data Flow (The Block Lifecycle)

        PHASE 1: Block Validation
          [Network] → [5] Block Propagation → [8] Consensus
                                                   
                                                   ↓ validate_block()
                                             BlockValidated event
                                                   
                                                   ↓ (Event Bus broadcast)
                                             
                                             ↓           ↓
                                       [3] Tx Index   [4] State Mgmt
                                                        
                                             ↓           ↓
                                     MerkleRootComputed  StateRootComputed
                                                        
                                             
                                                   ↓
        PHASE 2: Assembly               [2] Block Storage (Assembler)
                                                   
                                                   ↓ (when all 3 arrive)
                                             ATOMIC WRITE
                                                   
                                                   ↓
                                             BlockStored event
                                                   
                                             
                                             ↓           ↓
                                       [6] Mempool  [9] Finality
                                       (confirm tx)  (check finalization)

      
---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

      PART 2: IMPLEMENTATION PHASES

      Phase 1: Subsystem Container Infrastructure

      Objective: Create the container that holds all subsystem instances with proper lifetime management.

      What to create:

        node-runtime/src/
         container/
            mod.rs           # Module definition
            subsystems.rs    # SubsystemContainer struct
            config.rs        # NodeConfig with all subsystem configs

      SubsystemContainer Design:

        pub struct SubsystemContainer {
            // Level 0: No dependencies
            pub sig_verification: Arc<SignatureVerificationService>,
            
            // Level 1: Depends on Level 0
            pub peer_discovery: Arc<RwLock<PeerDiscoveryService>>,
            pub mempool: Arc<RwLock<MempoolService>>,
            
            // Level 2: Depends on Level 0-1
            pub tx_indexing: Arc<TransactionIndexingService>,
            pub state_management: Arc<StateManagementService>,
            pub block_propagation: Arc<BlockPropagationService>,
            
            // Level 3: Depends on Level 0-2
            pub consensus: Arc<ConsensusService>,
            
            // Level 4: Depends on Level 0-3
            pub block_storage: Arc<RwLock<BlockStorageService>>,
            pub finality: Arc<FinalityService>,
            
            // Shared infrastructure
            pub event_bus: Arc<InMemoryEventBus>,
            pub nonce_cache: Arc<RwLock<TimeBoundedNonceCache>>,
        }

      Key Decisions:

        - Arc for thread-safe sharing across async tasks
        - RwLock only for subsystems that mutate state (Mempool, BlockStorage, PeerDiscovery)
        - InMemoryEventBus as the single event router
        - Centralized TimeBoundedNonceCache shared by all IPC handlers

      
---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

      Phase 2: Port Adapter Implementations

      Objective: Create adapters that implement each subsystem's outbound ports.

      For each subsystem, I will create adapters in node-runtime/src/adapters/:

      2.1 Signature Verification Adapters

        - Outbound Port: MempoolGateway → forwards verified transactions to Mempool
        - Adapter: MempoolGatewayAdapter wraps Arc<RwLock<MempoolService>>

      2.2 Consensus Adapters

        - Outbound Ports:
          - EventBus → publishes BlockValidated events
          - MempoolGateway → gets transactions for block building
          - SignatureVerifier → re-verifies signatures (Zero-Trust)
          - ValidatorSetProvider → gets validator set from State Management
        - Adapters: Each wraps the appropriate subsystem with async boundary

      2.3 Block Storage Adapters

        - No outbound ports that call other subsystems
        - Receives: Events via subscription (not direct calls)
        - Publishes: BlockStored, BlockStorageConfirmation

      2.4 Mempool Adapters

        - Outbound Ports: None (passive - receives from Sig Verification, responds to Consensus)

      
---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

      Phase 3: Event Handler Registration

      Objective: Wire event subscriptions to actual subsystem methods.

      Handler Architecture:

        pub struct ChoreographyHandler {
            container: Arc<SubsystemContainer>,
            shutdown: watch::Receiver<bool>,
        }
        
        impl ChoreographyHandler {
            pub async fn run_block_validated_handler(&self) {
                let mut subscription = self.container.event_bus
                    .subscribe(EventFilter::topics(vec![EventTopic::Consensus]));
                
                while let Some(event) = subscription.next().await {
                    if let BlockchainEvent::BlockValidated(block) = event {
                        // Trigger parallel computation
                        let tx_handle = self.handle_tx_indexing(block.clone());
                        let state_handle = self.handle_state_management(block.clone());
                        
                        // These run concurrently (choreography, not orchestration)
                        tokio::join!(tx_handle, state_handle);
                    }
                }
            }
            
            async fn handle_tx_indexing(&self, block: ValidatedBlock) {
                // Call actual qc-03 domain logic
                let merkle_root = self.container.tx_indexing
                    .compute_merkle_root(&block);
                
                // Publish result
                self.container.event_bus.publish(
                    BlockchainEvent::MerkleRootComputed {
                        block_hash: block.header.hash(),
                        merkle_root,
                    }
                ).await;
            }
        }

      
---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

      Phase 4: Genesis and Initialization

      Objective: Create the genesis block and initialize chain state.

      Genesis Requirements:

        - Create genesis block with:
          - height: 0
          - parent_hash: [0u8; 32]
          - merkle_root: EMPTY_MERKLE_ROOT
          - state_root: EMPTY_STATE_ROOT
          - timestamp: GENESIS_TIMESTAMP
        - Store genesis in Block Storage
        - Initialize State Management with genesis state root
        - Set finalized height to 0

      
---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

      Phase 5: Runtime Lifecycle

      Objective: Implement proper startup and shutdown sequences.

      Startup Sequence:

        1. Load configuration (from file/env)
        2. Validate HMAC secret is not default
        3. Initialize subsystems in dependency order (Level 0 → Level 4)
        4. Create genesis block (if not exists)
        5. Start event handlers (spawn async tasks)
        6. Start P2P listener (future phase)
        7. Start RPC server (future phase)
        8. Signal ready

      Shutdown Sequence:

        1. Signal shutdown to all handlers
        2. Drain pending events (with timeout)
        3. Persist subsystem state
        4. Close database connections
        5. Exit

      
---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

      PART 3: DETAILED FILE CHANGES

      3.1 New Files to Create

      
       File                                  Purpose                                        
      
       container/mod.rs                      Module exports                                 
      
       container/subsystems.rs               SubsystemContainer and initialization          
      
       container/config.rs                   Unified node configuration                     
      
       adapters/ports/mod.rs                 Port adapter module                            
      
       adapters/ports/mempool_gateway.rs     Implements MempoolGateway for Sig Verification 
      
       adapters/ports/event_bus_adapter.rs   Implements EventBus for Consensus              
      
       adapters/ports/signature_verifier.rs  Implements SignatureVerifier for Consensus     
      
       adapters/ports/validator_provider.rs  Implements ValidatorSetProvider for Consensus  
      
       genesis/mod.rs                        Genesis block creation                         
      
       genesis/builder.rs                    Genesis block builder                          
      

      3.2 Files to Modify

      
       File                       Changes                                            
      
       main.rs                    Use SubsystemContainer instead of stubs            
      
       wiring/core_subsystems.rs  Implement actual initialization, not logging stubs 
      
       handlers/choreography.rs   Call actual subsystem methods, not placeholders    
      
       adapters/block_storage.rs  Wire to actual qc-02-block-storage service         


      
---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

      PART 4: CRITICAL INVARIANTS TO MAINTAIN

      4.1 Security Invariants

      
       Invariant                Enforcement                                  
      
       HMAC Secret not default  Panic on startup if hmac_secret == [0u8; 32] 
      
       Nonce uniqueness         TimeBoundedNonceCache rejects duplicates     
      
       Timestamp validity       Reject messages outside [now-60s, now+10s]   
      
       Sender authorization     Check sender_id against IPC-MATRIX rules     
      
       Reply-to validation      reply_to.subsystem_id == sender_id           
      

      4.2 Data Integrity Invariants

      
       Invariant                  Enforcement                                      
      
       Atomic block writes        Block Storage assembler batches all components   
      
       Assembly timeout           GC incomplete assemblies after 30s               
      
       Two-phase commit           Mempool deletes only on BlockStorageConfirmation 
      
       Sequential blocks          Block Storage rejects if parent not found        
      
       Finalization monotonicity  Cannot finalize height < current finalized       
      

      4.3 Concurrency Invariants

      
       Invariant       Enforcement                                            
      
       No deadlocks    Single lock acquisition order: Bus → Storage → Mempool 
      
       No starvation   Bounded channels with backpressure                     
      
       Event ordering  Correlation IDs track causality                        


      
---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

      PART 5: TESTING STRATEGY

      5.1 Unit Tests (Per Adapter)

      Each adapter gets tests verifying:

        - Correct port trait implementation
        - Error propagation
        - Authorization checks

      5.2 Integration Tests (Choreography Flow)

        #[tokio::test]
        async fn test_full_choreography_flow() {
            let container = SubsystemContainer::new_for_testing();
            
            // Simulate block validation
            let block = create_test_block();
            container.consensus.validate_block(block).await.unwrap();
            
            // Wait for choreography to complete
            tokio::time::sleep(Duration::from_millis(100)).await;
            
            // Verify block was stored
            let stored = container.block_storage.read()
                .read_block_by_height(1).unwrap();
            assert_eq!(stored.block.header.height, 1);
        }

      5.3 Security Tests (Already Exist)

      The existing integration-tests/src/exploits/ tests cover:

        - Memory bomb attacks
        - Eclipse attacks
        - IPC authentication bypass attempts
        - Crash recovery scenarios

      
---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

      PART 6: IMPLEMENTATION ORDER

      
       Step  Task                                                 Dependencies  Estimated Files 
      
       1     Create container/config.rs with unified config       None          1               
      
       2     Create container/subsystems.rs with empty container  Step 1        1               
      
       3     Implement Level 0: Sig Verification initialization   Step 2        1               
      
       4     Implement Level 1: Peer Discovery + Mempool init     Step 3        2               
      
       5     Create MempoolGatewayAdapter for Sig Verification    Step 4        1               
      
       6     Implement Level 2: Tx Indexing + State Mgmt init     Step 4        2               
      
       7     Implement Level 3: Consensus init with all ports     Step 5, 6     3               
      
       8     Implement Level 4: Block Storage + Finality init     Step 7        2               
      
       9     Create genesis builder                               Step 8        1               
      
       10    Wire event handlers to actual subsystem calls        Step 8        1               
      
       11    Update main.rs to use new container                  Step 10       1               
      
       12    Integration testing                                  Step 11       1               
      

      Total: 17 file modifications/creations

      
---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

      PART 7: RISKS AND MITIGATIONS

      
       Risk                     Mitigation                                                                 
      
       Circular dependencies    Strict initialization order; adapters take Arc<> not owned values          
      
       Deadlocks                Single lock ordering: Event Bus → Block Storage → Mempool → Peer Discovery 
      
       Memory leaks             Bounded channels; assembly GC; nonce cache TTL                             
      
       Performance bottlenecks  Parallel event handling; async I/O; batch verification                     
      
       State inconsistency      Atomic batch writes; two-phase commit; checksums                           


      
---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------

      CONFIRMATION

      This plan:

        - ✅ Follows Architecture.md v2.3 Choreography Pattern precisely
        - ✅ Respects all port/adapter boundaries from hexagonal architecture
        - ✅ Maintains all security invariants from IPC-MATRIX.md
        - ✅ Initializes subsystems in correct dependency order
        - ✅ Keeps the remaining 6 subsystems (7, 11-15) as future work
        - ✅ Is maintainable through clear separation of concerns
        - ✅ Does not conflict with existing domain logic in qc-* crates