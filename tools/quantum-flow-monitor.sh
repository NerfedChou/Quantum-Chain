#!/bin/bash
# Quantum-Chain Flow Monitor v2
# Real-time subsystem activity logger - Shows complete block flow choreography

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
BRIGHT_BLUE='\033[1;34m'
GRAY='\033[0;90m'
NC='\033[0m'

clear
cat << 'EOF'
═══════════════════════════════════════════════════════════════════════
   ⛓️  QUANTUM-CHAIN FLOW MONITOR v2
═══════════════════════════════════════════════════════════════════════

Block Flow Choreography:
  ⛏️  qc-17 → ✅ Bridge → 📜 qc-11 (exec) → 🌳 qc-03 + 💾 qc-04 → 📦 qc-02 → 🔒 qc-09

Transaction Execution Flow:
  💰 qc-06 → 🔐 qc-10 → ✅ qc-08/qc-12 → 📜 qc-11 → 💾 qc-04

Subsystems:
  ⛏️  qc-17: Block Production    📦 qc-02: Block Storage
  ✅ qc-08: Consensus            🔒 qc-09: Finality  
  🌳 qc-03: Merkle Trees         💾 qc-04: State Management
  💰 qc-06: Mempool              🌐 qc-01: Peer Discovery
  🔐 qc-10: Signatures           📜 qc-11: Smart Contracts (EVM)
  🌉 qc-16: API Gateway          🔄 qc-12: Tx Ordering

Press Ctrl+C to stop
───────────────────────────────────────────────────────────────────────
EOF

# Track blocks in flight for summary stats
declare -A block_start_times

docker logs -f --tail 20 quantum-chain-node 2>&1 | \
  stdbuf -oL grep -E '\[qc-[0-9]+\]|Bridge|ERROR|WARN' | \
  while IFS= read -r line; do
    # Extract timestamp
    timestamp=$(echo "$line" | grep -oE '[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]+Z' | head -1)
    time_display=$(echo "$timestamp" | sed -E 's/.*T([0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]{3}).*/\1/' 2>/dev/null)
    [[ -z "$time_display" ]] && time_display="--:--:--.---"

    # Handle errors first
    if [[ "$line" == *"ERROR"* ]]; then
        echo -e "${RED}[$time_display] ❌ ERROR: ${line##*ERROR}${NC}"
        continue
    fi

    # Parse by subsystem/event
    case "$line" in
        # === QC-17: Block Production ===
        *"[qc-17]"*"Block #"*"mined"*)
            block=$(echo "$line" | grep -oE 'Block #[0-9]+')
            # Handle large nonces (u64 can be very big)
            nonce=$(echo "$line" | sed -n 's/.*nonce: \([0-9]*\).*/\1/p')
            echo -e "${BRIGHT_GREEN}[$time_display] ⛏️  [qc-17] $block mined! | nonce: $nonce${NC}"
            ;;
        *"[qc-17]"*"Mining block"*)
            block=$(echo "$line" | grep -oE 'block #[0-9]+')
            echo -e "${GREEN}[$time_display] ⛏️  [qc-17] Mining $block...${NC}"
            ;;
        *"[qc-17]"*"Difficulty"*)
            diff_info=$(echo "$line" | grep -oE '~[0-9]+ leading.*window.*blocks')
            echo -e "${GREEN}[$time_display] 📊 [qc-17] Difficulty: $diff_info${NC}"
            ;;
        *"[qc-17]"*"Resuming from height"*)
            height=$(echo "$line" | grep -oE 'height [0-9]+')
            diff=$(echo "$line" | grep -oE '~[0-9]+ leading zero bytes')
            echo -e "${BRIGHT_GREEN}[$time_display] 💾 [qc-17] Resuming: $height, difficulty: $diff${NC}"
            ;;
        
        # === Bridge: Choreography Trigger ===
        *"[Bridge]"*"Published BlockValidated"*)
            block=$(echo "$line" | grep -oE 'block #[0-9]+')
            echo -e "${BRIGHT_YELLOW}[$time_display] ✅ [Bridge] BlockValidated published for $block → EVENT BUS${NC}"
            echo -e "${GRAY}   └─ Triggers: qc-02, qc-03, qc-04${NC}"
            ;;
        *"[Bridge]"*"Storing block"*)
            block=$(echo "$line" | grep -oE 'block #[0-9]+')
            # Extract nonce and difficulty from new log format
            nonce=$(echo "$line" | sed -n 's/.*nonce: \([0-9]*\).*/\1/p')
            diff=$(echo "$line" | grep -oE '~[0-9]+ zero bytes')
            echo -e "${YELLOW}[$time_display] 🌉 [Bridge] Storing $block (nonce: $nonce, diff: $diff)${NC}"
            ;;
        *"[Bridge]"*"Loaded last block"*|*"[Bridge]"*"Resuming"*)
            height=$(echo "$line" | grep -oE 'height [0-9]+')
            diff=$(echo "$line" | grep -oE '~[0-9]+ zero bytes')
            echo -e "${BRIGHT_YELLOW}[$time_display] 📖 [Bridge] Loaded chain: $height, diff: $diff${NC}"
            ;;
        *"[Bridge]"*"Triggering"*)
            block=$(echo "$line" | grep -oE 'block #[0-9]+')
            echo -e "${YELLOW}[$time_display] 🌉 [Bridge] Triggering choreography for $block${NC}"
            ;;
        
        # === QC-03: Merkle Tree ===
        *"[qc-03]"*"Computing merkle"*)
            block=$(echo "$line" | grep -oE 'block #[0-9]+')
            echo -e "${CYAN}[$time_display] 🌳 [qc-03] Computing merkle tree for $block [1/3]${NC}"
            ;;
        *"[qc-03]"*"Merkle root computed"*)
            block=$(echo "$line" | grep -oE 'block #[0-9]+')
            echo -e "${BRIGHT_CYAN}[$time_display] 🌳 [qc-03] ✓ Merkle root computed for $block → qc-02${NC}"
            ;;
        
        # === QC-04: State Management ===
        *"[qc-04]"*"Computing state"*)
            block=$(echo "$line" | grep -oE 'block #[0-9]+')
            echo -e "${MAGENTA}[$time_display] 💾 [qc-04] Computing state root for $block [2/3]${NC}"
            ;;
        *"[qc-04]"*"State root computed"*)
            block=$(echo "$line" | grep -oE 'block #[0-9]+')
            echo -e "${BRIGHT_MAGENTA}[$time_display] 💾 [qc-04] ✓ State root computed for $block → qc-02${NC}"
            ;;
        
        # === QC-02: Block Storage (Assembler) ===
        *"[qc-02]"*"assembly"*|*"[qc-02]"*"Assembly"*)
            block=$(echo "$line" | grep -oE '[Bb]lock #[0-9]+')
            echo -e "${BLUE}[$time_display] 📦 [qc-02] Assembly initiated for $block [3/3]${NC}"
            ;;
        *"[qc-02]"*"stored"*|*"[qc-02]"*"STORED"*|*"[qc-02]"*"ATOMIC WRITE"*)
            block=$(echo "$line" | grep -oE 'Block #[0-9]+')
            echo -e "${BRIGHT_BLUE}[$time_display] 📦 [qc-02] ✓ $block STORED (atomic write complete)${NC}"
            echo -e "${GRAY}   └─ Next: qc-09 (Finality)${NC}"
            ;;
        
        # === QC-09: Finality ===
        *"[qc-09]"*"finalizing"*|*"[qc-09]"*"epoch"*"boundary"*)
            block=$(echo "$line" | grep -oE 'Block #[0-9]+')
            epoch=$(echo "$line" | grep -oE 'epoch [0-9]+')
            echo -e "${YELLOW}[$time_display] 🔒 [qc-09] $block at $epoch boundary, checking finality...${NC}"
            ;;
        *"[qc-09]"*"FINALIZED"*)
            block=$(echo "$line" | grep -oE 'Block #[0-9]+')
            echo -e "${BRIGHT_GREEN}[$time_display] 🔒 [qc-09] ✓ $block FINALIZED ✓${NC}"
            echo -e "${GREEN}═══════════════════════════════════════════════════════════════════════${NC}"
            ;;
        
        # === QC-08: Consensus ===
        *"[qc-08]"*)
            msg="${line##*\[qc-08\]}"
            echo -e "${GREEN}[$time_display] ✅ [qc-08]$msg${NC}"
            ;;
        
        # === QC-06: Mempool ===
        *"[qc-06]"*)
            msg="${line##*\[qc-06\]}"
            echo -e "${WHITE}[$time_display] 💰 [qc-06]$msg${NC}"
            ;;
        
        # === QC-01: Peer Discovery ===
        *"[qc-01]"*)
            msg="${line##*\[qc-01\]}"
            echo -e "${CYAN}[$time_display] 🌐 [qc-01]$msg${NC}"
            ;;
        
        # === QC-10: Signatures ===
        *"[qc-10]"*)
            msg="${line##*\[qc-10\]}"
            echo -e "${YELLOW}[$time_display] 🔐 [qc-10]$msg${NC}"
            ;;
        
        # === QC-11: Smart Contracts ===
        *"[qc-11]"*"Executing"*|*"[qc-11]"*"execution request"*)
            tx_hash=$(echo "$line" | grep -oE 'tx_hash=[a-fA-F0-9x]+' | head -1)
            block=$(echo "$line" | grep -oE 'block #[0-9]+' | head -1)
            echo -e "${BRIGHT_MAGENTA}[$time_display] 📜 [qc-11] Executing transaction ${tx_hash:-""} ${block:-""}${NC}"
            ;;
        *"[qc-11]"*"completed"*|*"[qc-11]"*"success"*)
            gas=$(echo "$line" | grep -oE 'gas_used=[0-9]+' | cut -d'=' -f2)
            echo -e "${BRIGHT_MAGENTA}[$time_display] 📜 [qc-11] ✓ Execution complete | gas: ${gas:-N/A}${NC}"
            ;;
        *"[qc-11]"*"deployed"*|*"[qc-11]"*"ContractCreate"*)
            addr=$(echo "$line" | grep -oE '0x[a-fA-F0-9]{40}' | head -1)
            echo -e "${BRIGHT_MAGENTA}[$time_display] 📜 [qc-11] ✓ Contract deployed: ${addr:-""}${NC}"
            echo -e "${GRAY}   └─ State changes → qc-04${NC}"
            ;;
        *"[qc-11]"*"HTLC"*)
            op=$(echo "$line" | grep -oiE 'claim|refund' | head -1)
            echo -e "${BRIGHT_MAGENTA}[$time_display] 📜 [qc-11] HTLC ${op:-operation} (via qc-15)${NC}"
            ;;
        *"[qc-11]"*"state_changes"*|*"[qc-11]"*"StateWrite"*)
            count=$(echo "$line" | grep -oE 'count=[0-9]+' | cut -d'=' -f2)
            echo -e "${MAGENTA}[$time_display] 📜 [qc-11] Publishing ${count:-"state"} changes → qc-04${NC}"
            ;;
        *"[qc-11]"*)
            msg="${line##*\[qc-11\]}"
            echo -e "${MAGENTA}[$time_display] 📜 [qc-11]$msg${NC}"
            ;;
        
        # === QC-16: API Gateway ===
        *"[qc-16]"*)
            msg="${line##*\[qc-16\]}"
            echo -e "${GRAY}[$time_display] 🌉 [qc-16]$msg${NC}"
            ;;
        
        # === Startup messages ===
        *"handler started"*|*"Starting"*|*"Initializing"*)
            if [[ "$line" == *"INFO"* ]]; then
                subsystem=$(echo "$line" | grep -oE '\[qc-[0-9]+\]')
                echo -e "${GRAY}[$time_display] 🚀 $subsystem Started${NC}"
            fi
            ;;
    esac
done
