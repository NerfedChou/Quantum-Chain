/**
 * Subsystem Topology Visualization
 * 
 * Renders a static radial layout of internal subsystem interactions
 * with QC-16 (API Gateway) at the center.
 * 
 * @module components/topology
 */

/**
 * Initialize the subsystem topology canvas.
 */
export function initTopology() {
    const canvas = document.getElementById('network-topology');
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    let animationId = null;

    // Set canvas size
    function resize() {
        const container = canvas.parentElement;
        canvas.width = container.clientWidth;
        canvas.height = Math.max(500, container.clientHeight);
    }

    resize();
    window.addEventListener('resize', resize);

    // Subsystem Definitions with Categories
    const categories = {
        gateway: { color: '#8b5cf6', size: 22, glow: true, label: 'Gateway' },
        core: { color: '#22c55e', size: 14, glow: false, label: 'Core' },
        data: { color: '#3b82f6', size: 14, glow: false, label: 'Data' },
        network: { color: '#f97316', size: 12, glow: false, label: 'Net' },
        security: { color: '#ec4899', size: 12, glow: false, label: 'Sec' },
    };

    // Define Nodes (Subsystems) - ordered for even distribution
    const subsystems = [
        // Core (4)
        { id: 'QC-08', name: 'Consensus', category: 'core' },
        { id: 'QC-09', name: 'Finality', category: 'core' },
        { id: 'QC-17', name: 'Block Prod', category: 'core' },
        { id: 'QC-12', name: 'Ordering', category: 'core' },
        // Data (4)
        { id: 'QC-02', name: 'Storage', category: 'data' },
        { id: 'QC-04', name: 'State', category: 'data' },
        { id: 'QC-03', name: 'Indexing', category: 'data' },
        { id: 'QC-14', name: 'Sharding', category: 'data' },
        // Network (4)
        { id: 'QC-01', name: 'Discovery', category: 'network' },
        { id: 'QC-05', name: 'Block Prop', category: 'network' },
        { id: 'QC-13', name: 'Light Client', category: 'network' },
        { id: 'QC-15', name: 'Cross-Chain', category: 'network' },
        // Security (4)
        { id: 'QC-10', name: 'Sig Verify', category: 'security' },
        { id: 'QC-07', name: 'Bloom Filters', category: 'security' },
        { id: 'QC-06', name: 'Mempool', category: 'security' },
        { id: 'QC-11', name: 'Contracts', category: 'security' },
    ];

    // Gateway node (center)
    const gatewayNode = { id: 'QC-16', name: 'API Gateway', category: 'gateway' };

    // Helper to find index by ID (in the satellite list)
    const getIdx = (id) => {
        if (id === 'QC-16') return -1; // Gateway is at index -1 (special)
        return subsystems.findIndex(n => n.id === id);
    };

    // IPC Matrix Connections
    const connections = [
        // Gateway to all satellites
        ...subsystems.map(s => ['QC-16', s.id]),
        // Core Flows
        ['QC-08', 'QC-09'], ['QC-08', 'QC-17'], ['QC-08', 'QC-02'],
        ['QC-17', 'QC-06'], ['QC-17', 'QC-02'], ['QC-12', 'QC-06'],
        // Data Flows
        ['QC-02', 'QC-04'], ['QC-04', 'QC-03'],
        // Network Flows
        ['QC-01', 'QC-05'], ['QC-05', 'QC-08'], ['QC-13', 'QC-01'],
        // Security Flows
        ['QC-06', 'QC-10'], ['QC-06', 'QC-07'], ['QC-11', 'QC-04'],
    ];

    // Calculate positions - called on each frame to handle resize
    function calculatePositions() {
        const cx = canvas.width / 2;
        const cy = canvas.height / 2;
        const radius = Math.min(cx, cy) * 0.75;
        const total = subsystems.length;

        // Satellites evenly distributed
        const satellites = subsystems.map((sub, i) => {
            const angle = (i / total) * Math.PI * 2 - (Math.PI / 2); // Start at top
            return {
                ...sub,
                x: cx + Math.cos(angle) * radius,
                y: cy + Math.sin(angle) * radius,
            };
        });

        // Gateway at center
        const gateway = {
            ...gatewayNode,
            x: cx,
            y: cy,
        };

        return { gateway, satellites };
    }

    // Build edges with proper node references
    function buildEdges(gateway, satellites) {
        return connections.map(([sourceId, targetId]) => {
            const getNode = (id) => {
                if (id === 'QC-16') return gateway;
                return satellites.find(s => s.id === id);
            };
            return {
                source: getNode(sourceId),
                target: getNode(targetId),
                active: false,
            };
        }).filter(e => e.source && e.target);
    }

    // Animation variables
    let edges = [];
    let gateway = null;
    let satellites = [];

    // Animation Loop
    function animate() {
        ctx.clearRect(0, 0, canvas.width, canvas.height);

        // Recalculate positions on each frame (handles resize)
        const positions = calculatePositions();
        gateway = positions.gateway;
        satellites = positions.satellites;
        edges = buildEdges(gateway, satellites);

        // Draw edges
        drawEdges(ctx, edges);

        // Draw satellite nodes
        satellites.forEach(node => drawNode(ctx, node, categories[node.category]));

        // Draw gateway node (on top)
        drawNode(ctx, gateway, categories.gateway);

        // Randomly activate edges to simulate traffic
        if (Math.random() > 0.97) {
            const edge = edges[Math.floor(Math.random() * edges.length)];
            if (edge) {
                edge.active = true;
                setTimeout(() => { edge.active = false; }, 400);
            }
        }

        animationId = requestAnimationFrame(animate);
    }

    animate();

    return () => {
        if (animationId) cancelAnimationFrame(animationId);
        window.removeEventListener('resize', resize);
    };
}

function drawEdges(ctx, edges) {
    edges.forEach(edge => {
        const { source, target, active } = edge;

        ctx.beginPath();
        ctx.moveTo(source.x, source.y);
        ctx.lineTo(target.x, target.y);

        if (active) {
            ctx.strokeStyle = '#8b5cf6';
            ctx.lineWidth = 2.5;
            ctx.globalAlpha = 0.9;
        } else {
            ctx.strokeStyle = '#3f3f46';
            ctx.lineWidth = 1;
            ctx.globalAlpha = 0.25;
        }

        ctx.stroke();
        ctx.globalAlpha = 1;
    });
}

function drawNode(ctx, node, cat) {
    const { x, y, id } = node;

    // Glow for gateway
    if (cat.glow) {
        const grad = ctx.createRadialGradient(x, y, 0, x, y, cat.size * 2.5);
        grad.addColorStop(0, `${cat.color}55`);
        grad.addColorStop(1, `${cat.color}00`);
        ctx.fillStyle = grad;
        ctx.beginPath();
        ctx.arc(x, y, cat.size * 2.5, 0, Math.PI * 2);
        ctx.fill();
    }

    // Node circle
    ctx.beginPath();
    ctx.arc(x, y, cat.size, 0, Math.PI * 2);
    ctx.fillStyle = cat.color;
    ctx.fill();

    // Border
    ctx.strokeStyle = '#18181b';
    ctx.lineWidth = 2;
    ctx.stroke();

    // Label
    ctx.fillStyle = '#d4d4d8';
    ctx.font = 'bold 11px Inter, sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText(id, x, y + cat.size + 14);
}
