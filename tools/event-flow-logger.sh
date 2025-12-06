#!/bin/bash
# Event Flow Logger - Phase 2 Implementation
# Parses JSON event logs and displays the subsystem choreography flow

clear
cat << 'EOF'
═══════════════════════════════════════════════════════════════════════
   🔗 QUANTUM-CHAIN EVENT FLOW LOGGER
═══════════════════════════════════════════════════════════════════════

Tracking subsystem choreography in real-time:

  🔨 BlockProduced (qc-17)
  ✅ BlockValidated (qc-08) → Triggers 3 subsystems
  🌳 MerkleRootComputed (qc-03) → qc-02 [1/3]
  💾 StateRootComputed (qc-04) → qc-02 [2/3]  
  📦 BlockStorageConfirmation (qc-02) [3/3]
  🔒 Finalization (qc-09)

Press Ctrl+C to stop
───────────────────────────────────────────────────────────────────────
EOF

# Track assembly state
declare -A ASSEMBLY_STATE

docker logs -f --tail 20 quantum-chain-node 2>&1 | \
  stdbuf -oL grep 'EVENT_FLOW_JSON' | \
  while IFS= read -r line; do
    # Extract JSON part (everything after EVENT_FLOW_JSON)
    json_part="${line#*EVENT_FLOW_JSON }"
    
    # Parse JSON with jq
    event_type=$(echo "$json_part" | jq -r '.event_type' 2>/dev/null)
    subsystem=$(echo "$json_part" | jq -r '.subsystem_id' 2>/dev/null)
    timestamp=$(echo "$json_part" | jq -r '.timestamp' 2>/dev/null)
    block_height=$(echo "$json_part" | jq -r '.block_height' 2>/dev/null)
    block_hash=$(echo "$json_part" | jq -r '.block_hash' 2>/dev/null)
    processing_ms=$(echo "$json_part" | jq -r '.processing_time_ms' 2>/dev/null)
    
    # Extract time (HH:MM:SS.microsec)
    time_display=$(echo "$timestamp" | sed -E 's/.*T([0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]{6}).*/\1/')
    
    # Color code by event type
    case "$event_type" in
        "BlockProduced")
            echo -e "\033[1;32m[$time_display] 🔨 [$subsystem] $event_type | block:#$block_height | hash:$block_hash\033[0m"
            echo -e "\033[0;90m   └─ Next: qc-08 (Consensus Validation)\033[0m"
            ;;
        "BlockValidated")
            echo -e "\033[1;92m[$time_display] ✅ [$subsystem] $event_type → EVENT BUS | block:#$block_height | ${processing_ms}ms\033[0m"
            echo -e "\033[0;90m   └─ Triggers: qc-03 (Merkle), qc-04 (State), qc-02 (Assembly)\033[0m"
            ;;
        "MerkleComputationStarted")
            echo -e "\033[0;36m[$time_display] 🌳 [$subsystem] Computing merkle tree | block:#$block_height\033[0m"
            ;;
        "MerkleRootComputed")
            echo -e "\033[1;36m[$time_display] ✓ [$subsystem] $event_type → qc-02 [1/3] | ${processing_ms}ms\033[0m"
            ASSEMBLY_STATE["$block_height"]="merkle"
            ;;
        "StateComputationStarted")
            echo -e "\033[0;35m[$time_display] 💾 [$subsystem] Computing state root | block:#$block_height\033[0m"
            ;;
        "StateRootComputed")
            echo -e "\033[1;35m[$time_display] ✓ [$subsystem] $event_type → qc-02 [2/3] | ${processing_ms}ms\033[0m"
            ASSEMBLY_STATE["$block_height"]="${ASSEMBLY_STATE[$block_height]},state"
            ;;
        "AssemblyStarted")
            echo -e "\033[0;34m[$time_display] 📦 [$subsystem] Assembly started | block:#$block_height\033[0m"
            ;;
        "BlockStorageConfirmation")
            echo -e "\033[1;34m[$time_display] ✅ [$subsystem] $event_type [3/3] COMPLETE | ${processing_ms}ms\033[0m"
            echo -e "\033[1;33m───────────────────────────────────────────────────────────────────────\033[0m"
            echo -e "\033[1;33mBlock #$block_height finalized | end-to-end flow complete\033[0m"
            echo -e "\033[1;33m───────────────────────────────────────────────────────────────────────\033[0m"
            unset ASSEMBLY_STATE["$block_height"]
            ;;
        *)
            # Other events in gray
            echo -e "\033[0;90m[$time_display] [$subsystem] $event_type\033[0m"
            ;;
    esac
  done
