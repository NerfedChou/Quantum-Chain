#!/bin/bash
# Event Flow Logger - Phase 3 Implementation
# Captures ALL subsystem events and displays the full choreography flow

clear
cat << 'EOF'
═══════════════════════════════════════════════════════════════════════
   🔗 QUANTUM-CHAIN EVENT FLOW LOGGER v3
═══════════════════════════════════════════════════════════════════════

Tracking ALL subsystems in real-time:

  🌐 qc-01: Peer Discovery
  📦 qc-02: Block Storage (Stateful Assembler)
  🌳 qc-03: Transaction Indexing (Merkle Trees)
  💾 qc-04: State Management
  📡 qc-05: Block Propagation
  💰 qc-06: Mempool
  🔐 qc-10: Signature Verification
  📜 qc-11: Smart Contracts (EVM)
  ✅ qc-08: Consensus
  🔒 qc-09: Finality
  🌉 qc-16: API Gateway
  ⛏️  qc-17: Block Production

Press Ctrl+C to stop
───────────────────────────────────────────────────────────────────────
EOF

# Color definitions
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
WHITE='\033[0;37m'
BRIGHT_GREEN='\033[1;32m'
BRIGHT_CYAN='\033[1;36m'
BRIGHT_MAGENTA='\033[1;35m'
BRIGHT_YELLOW='\033[1;33m'
GRAY='\033[0;90m'
NC='\033[0m' # No Color

docker logs -f --tail 10 quantum-chain-node 2>&1 | \
  stdbuf -oL grep -E '\[qc-[0-9]+\]|Bridge' | \
  while IFS= read -r line; do
    # Extract timestamp
    timestamp=$(echo "$line" | grep -oE '[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]+Z' | head -1)
    time_display=$(echo "$timestamp" | sed -E 's/.*T([0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]{3}).*/\1/')
    
    # Determine subsystem and format output
    case "$line" in
        *"[qc-17]"*"Block #"*"mined"*)
            block=$(echo "$line" | grep -oE 'Block #[0-9]+')
            nonce=$(echo "$line" | grep -oE 'Nonce: [0-9]+' | cut -d' ' -f2)
            echo -e "${BRIGHT_GREEN}[$time_display] ⛏️  [qc-17] $block mined! | nonce: $nonce${NC}"
            ;;
        *"[qc-17]"*"Mining block"*)
            block=$(echo "$line" | grep -oE 'block #[0-9]+')
            echo -e "${GREEN}[$time_display] ⛏️  [qc-17] Mining $block...${NC}"
            ;;
        *"[Bridge]"*"Triggering"*)
            block=$(echo "$line" | grep -oE 'block #[0-9]+')
            echo -e "${BRIGHT_YELLOW}[$time_display] 🌉 [Bridge] → Triggering choreography for $block${NC}"
            ;;
        *"[Bridge]"*"Published BlockValidated"*)
            block=$(echo "$line" | grep -oE 'block #[0-9]+')
            echo -e "${BRIGHT_YELLOW}[$time_display] ✅ [Bridge] BlockValidated published for $block → EVENT BUS${NC}"
            echo -e "${GRAY}   └─ Triggers: qc-02, qc-03, qc-04${NC}"
            ;;
        *"[qc-03]"*"Computing merkle"*)
            block=$(echo "$line" | grep -oE 'block #[0-9]+')
            echo -e "${CYAN}[$time_display] 🌳 [qc-03] Computing merkle tree for $block [1/3]${NC}"
            ;;
        *"[qc-03]"*"Merkle root computed"*)
            block=$(echo "$line" | grep -oE 'block #[0-9]+')
            echo -e "${BRIGHT_CYAN}[$time_display] 🌳 [qc-03] ✓ Merkle root computed for $block → qc-02${NC}"
            ;;
        *"[qc-04]"*"Computing state"*)
            block=$(echo "$line" | grep -oE 'block #[0-9]+')
            echo -e "${MAGENTA}[$time_display] 💾 [qc-04] Computing state root for $block [2/3]${NC}"
            ;;
        *"[qc-04]"*"State root computed"*)
            block=$(echo "$line" | grep -oE 'block #[0-9]+')
            echo -e "${BRIGHT_MAGENTA}[$time_display] 💾 [qc-04] ✓ State root computed for $block → qc-02${NC}"
            ;;
        *"[qc-02]"*"Starting assembly"*|*"[qc-02]"*"assembly initiated"*)
            block=$(echo "$line" | grep -oE 'block #[0-9]+|Block #[0-9]+')
            echo -e "${BLUE}[$time_display] 📦 [qc-02] Assembly initiated for $block [3/3]${NC}"
            ;;
        *"[qc-02]"*"stored"*|*"[qc-02]"*"ATOMIC WRITE"*)
            block=$(echo "$line" | grep -oE 'Block #[0-9]+')
            echo -e "${BRIGHT_GREEN}[$time_display] 📦 [qc-02] ✓ $block STORED (atomic write complete)${NC}"
            echo -e "${GRAY}   └─ Next: qc-09 (Finality)${NC}"
            ;;
        *"[qc-09]"*"finalizing"*)
            block=$(echo "$line" | grep -oE 'Block #[0-9]+')
            epoch=$(echo "$line" | grep -oE 'epoch [0-9]+')
            echo -e "${YELLOW}[$time_display] 🔒 [qc-09] $block at $epoch boundary, checking finality...${NC}"
            ;;
        *"[qc-09]"*"FINALIZED"*)
            block=$(echo "$line" | grep -oE 'Block #[0-9]+')
            echo -e "${BRIGHT_GREEN}[$time_display] 🔒 [qc-09] ✓ $block FINALIZED ✓${NC}"
            echo -e "${GREEN}═══════════════════════════════════════════════════════════════════════${NC}"
            ;;
        *"[qc-01]"*"Peer"*|*"[qc-01]"*"peer"*)
            echo -e "${WHITE}[$time_display] 🌐 [qc-01] ${line##*\[qc-01\]}${NC}"
            ;;
        *"[qc-06]"*"Mempool"*|*"[qc-06]"*"transaction"*)
            echo -e "${WHITE}[$time_display] 💰 [qc-06] ${line##*\[qc-06\]}${NC}"
            ;;
        *"[qc-10]"*"Signature"*|*"[qc-10]"*"verified"*)
            echo -e "${WHITE}[$time_display] 🔐 [qc-10] ${line##*\[qc-10\]}${NC}"
            ;;
        *"[qc-11]"*"execution"*|*"[qc-11]"*"Executing"*)
            tx_hash=$(echo "$line" | grep -oE 'tx: 0x[a-fA-F0-9]+' | head -1)
            echo -e "${BRIGHT_MAGENTA}[$time_display] 📜 [qc-11] Executing contract ${tx_hash:-""}${NC}"
            ;;
        *"[qc-11]"*"completed"*|*"[qc-11]"*"result"*)
            gas=$(echo "$line" | grep -oE 'gas_used: [0-9]+' | cut -d' ' -f2)
            success=$(echo "$line" | grep -oE 'success: (true|false)' | cut -d' ' -f2)
            echo -e "${BRIGHT_MAGENTA}[$time_display] 📜 [qc-11] ✓ Execution complete | gas: ${gas:-N/A} | success: ${success:-true}${NC}"
            ;;
        *"[qc-11]"*"Contract deployed"*|*"[qc-11]"*"CREATE"*)
            addr=$(echo "$line" | grep -oE '0x[a-fA-F0-9]{40}' | head -1)
            echo -e "${BRIGHT_MAGENTA}[$time_display] 📜 [qc-11] ✓ Contract deployed at ${addr:-""}${NC}"
            ;;
        *"[qc-11]"*"HTLC"*)
            op=$(echo "$line" | grep -oE 'Claim|Refund' | head -1)
            echo -e "${BRIGHT_MAGENTA}[$time_display] 📜 [qc-11] HTLC ${op:-operation}${NC}"
            ;;
        *"[qc-11]"*)
            msg="${line##*\[qc-11\]}"
            echo -e "${MAGENTA}[$time_display] 📜 [qc-11]$msg${NC}"
            ;;
        *"[qc-16]"*"API"*|*"[qc-16]"*"Gateway"*)
            echo -e "${WHITE}[$time_display] 🌉 [qc-16] ${line##*\[qc-16\]}${NC}"
            ;;
        *"handler started"*)
            subsystem=$(echo "$line" | grep -oE '\[qc-[0-9]+\]')
            echo -e "${GRAY}[$time_display] $subsystem Handler started${NC}"
            ;;
    esac
done
