#!/bin/bash
# Quantum-Chain Flow Monitor
# Real-time subsystem activity logger - No UI, pure terminal logs

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
WHITE='\033[0;37m'
GRAY='\033[0;90m'
BOLD='\033[1m'
RESET='\033[0m'

clear
cat << 'EOF'
═══════════════════════════════════════════════════════════════════════
   ⛓️  QUANTUM-CHAIN LIVE SUBSYSTEM FLOW MONITOR
═══════════════════════════════════════════════════════════════════════

Watching all subsystems in real-time:

  ⛏️  QC-17: Block Production (Mining)
  ✅ QC-08: Consensus (Validation)
  🌳 QC-03: Transaction Indexing (Merkle Trees)
  💾 QC-04: State Management
  📦 QC-02: Block Storage (Choreography)
  🔒 QC-09: Finality
  💰 QC-06: Mempool
  🌐 QC-01: Peer Discovery
  📡 QC-05: Block Propagation
  🔐 QC-10: Signature Verification
  🌉 QC-16: API Gateway

Press Ctrl+C to stop
───────────────────────────────────────────────────────────────────────
EOF

# Follow docker logs and format output
docker logs -f --tail 0 quantum-chain-node 2>&1 | while IFS= read -r line; do
    # Extract timestamp
    if [[ $line =~ ([0-9]{2}:[0-9]{2}:[0-9]{2}) ]]; then
        TIME="${BASH_REMATCH[1]}"
    else
        TIME="--:--:--"
    fi
    
    # Parse and colorize by subsystem
    case "$line" in
        *"[qc-17]"*)
            if [[ $line =~ "mined!" ]]; then
                echo -e "${GREEN}${BOLD}[$TIME] ✓ QC-17${RESET}${GREEN} $(echo "$line" | sed 's/.*\[qc-17\]//')${RESET}"
            elif [[ $line =~ "Mining block" ]]; then
                echo -e "${YELLOW}[$TIME] ⛏  QC-17$(echo "$line" | sed 's/.*\[qc-17\]//' | head -c 80)...${RESET}"
            else
                echo -e "${YELLOW}[$TIME] ⛏  QC-17$(echo "$line" | sed 's/.*\[qc-17\]//')${RESET}"
            fi
            ;;
        *"[qc-08]"*)
            echo -e "${GREEN}[$TIME] ✅ QC-08$(echo "$line" | sed 's/.*\[qc-08\]//')${RESET}"
            ;;
        *"[qc-03]"*)
            echo -e "${CYAN}[$TIME] 🌳 QC-03$(echo "$line" | sed 's/.*\[qc-03\]//')${RESET}"
            ;;
        *"[qc-04]"*)
            echo -e "${BLUE}[$TIME] 💾 QC-04$(echo "$line" | sed 's/.*\[qc-04\]//')${RESET}"
            ;;
        *"[qc-02]"*)
            echo -e "${MAGENTA}[$TIME] 📦 QC-02$(echo "$line" | sed 's/.*\[qc-02\]//')${RESET}"
            ;;
        *"[qc-09]"*)
            echo -e "${MAGENTA}[$TIME] 🔒 QC-09$(echo "$line" | sed 's/.*\[qc-09\]//')${RESET}"
            ;;
        *"[qc-06]"*)
            echo -e "${WHITE}[$TIME] 💰 QC-06$(echo "$line" | sed 's/.*\[qc-06\]//')${RESET}"
            ;;
        *"[qc-01]"*)
            echo -e "${CYAN}[$TIME] 🌐 QC-01$(echo "$line" | sed 's/.*\[qc-01\]//')${RESET}"
            ;;
        *"[qc-05]"*)
            echo -e "${BLUE}[$TIME] 📡 QC-05$(echo "$line" | sed 's/.*\[qc-05\]//')${RESET}"
            ;;
        *"[qc-10]"*)
            echo -e "${YELLOW}[$TIME] 🔐 QC-10$(echo "$line" | sed 's/.*\[qc-10\]//')${RESET}"
            ;;
        *"[qc-16]"*|*"API Gateway"*)
            if [[ $line =~ "INFO" ]]; then
                echo -e "${GRAY}[$TIME] 🌉 QC-16$(echo "$line" | sed 's/.*qc_16//' | sed 's/.*INFO//')${RESET}"
            fi
            ;;
        *"started"*|*"Starting"*)
            if [[ $line =~ "INFO" ]]; then
                echo -e "${GRAY}[$TIME] 🚀 $(echo "$line" | sed 's/.*INFO//' | sed 's/node_runtime:://')${RESET}"
            fi
            ;;
        *"ERROR"*)
            echo -e "${RED}${BOLD}[$TIME] ❌ $(echo "$line" | sed 's/.*ERROR//')${RESET}"
            ;;
        *"WARN"*)
            # Skip debug warnings
            if [[ ! $line =~ "correlation_id" ]] && [[ ! $line =~ "pending request" ]]; then
                echo -e "${RED}[$TIME] ⚠️  $(echo "$line" | sed 's/.*WARN//')${RESET}"
            fi
            ;;
    esac
done
