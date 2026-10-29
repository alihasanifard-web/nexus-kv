# -*- coding: utf-8 -*-
"""
Generate 12 distinct, handcrafted SVG logo concepts across 4 NexusKV applications:
1. Nexus Server (Raft & LSM Core Engine) -> 3 concepts
2. Nexus CLI (Interactive Terminal REPL) -> 3 concepts
3. Nexus Benchmark (High-Load QPS Engine) -> 3 concepts
4. Nexus Dashboard / Cockpit (Telemetry & Radar) -> 3 concepts
Follows the logo-designer skill specifications (viewBox 0 0 512 512, pure SVG, small-size legibility).
"""

import os

logos_dir = "C:/Users/V I S I O N/nexus-kv/logos/concepts"
os.makedirs(logos_dir, exist_ok=True)

# -------------------------------------------------------------
# 1. NEXUS SERVER (Core Raft & LSM Engine)
# Accent: Royal Indigo & Deep Violet (#6366f1, #8b5cf6, #4f46e5)
# -------------------------------------------------------------

# Server Concept 1: The Raft Consensus Triad
server_c1 = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="100%" height="100%">
  <defs>
    <linearGradient id="srv1-grad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#818cf8"/>
      <stop offset="100%" stop-color="#4f46e5"/>
    </linearGradient>
    <linearGradient id="srv1-glow" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#c7d2fe"/>
      <stop offset="100%" stop-color="#6366f1"/>
    </linearGradient>
  </defs>
  <g id="icon">
    <!-- Connecting Interconnect Channels -->
    <path d="M256 128 L146 320 L366 320 Z" fill="none" stroke="#2e344d" stroke-width="28" stroke-linejoin="round" stroke-linecap="round"/>
    <path d="M256 128 L146 320 L366 320 Z" fill="none" stroke="url(#srv1-grad)" stroke-width="12" stroke-linejoin="round" stroke-linecap="round"/>
    
    <!-- Central Consensus Core -->
    <circle cx="256" cy="256" r="32" fill="#11131f" stroke="#6366f1" stroke-width="8"/>
    <circle cx="256" cy="256" r="14" fill="#a5b4fc"/>

    <!-- 3 Raft Nodes -->
    <!-- Top Leader Node -->
    <circle cx="256" cy="128" r="46" fill="#0c0e17" stroke="url(#srv1-grad)" stroke-width="14"/>
    <circle cx="256" cy="128" r="20" fill="url(#srv1-glow)"/>

    <!-- Bottom Left Node -->
    <circle cx="146" cy="320" r="42" fill="#0c0e17" stroke="#4f46e5" stroke-width="12"/>
    <circle cx="146" cy="320" r="16" fill="#818cf8"/>

    <!-- Bottom Right Node -->
    <circle cx="366" cy="320" r="42" fill="#0c0e17" stroke="#4f46e5" stroke-width="12"/>
    <circle cx="366" cy="320" r="16" fill="#818cf8"/>
  </g>
</svg>"""

# Server Concept 2: The Layered LSM-Tree Monogram (Letter N)
server_c2 = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="100%" height="100%">
  <defs>
    <linearGradient id="srv2-grad1" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#818cf8"/>
      <stop offset="100%" stop-color="#4338ca"/>
    </linearGradient>
    <linearGradient id="srv2-grad2" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#a5b4fc"/>
      <stop offset="100%" stop-color="#6366f1"/>
    </linearGradient>
  </defs>
  <g id="icon">
    <!-- Outer Shield / Container -->
    <rect x="76" y="76" width="360" height="360" rx="90" fill="#0d0f18" stroke="#25293d" stroke-width="14"/>

    <!-- Left Pillar (MemTable RAM) -->
    <rect x="140" y="140" width="56" height="232" rx="28" fill="url(#srv2-grad1)"/>

    <!-- Right Pillar (SSTable Disk) -->
    <rect x="316" y="140" width="56" height="232" rx="28" fill="url(#srv2-grad1)"/>

    <!-- Diagonal Pipeline (WAL & Raft Stream) -->
    <path d="M168 152 L344 360" stroke="url(#srv2-grad2)" stroke-width="52" stroke-linecap="round"/>

    <!-- Inner Core Dot -->
    <circle cx="256" cy="256" r="18" fill="#ffffff"/>
  </g>
</svg>"""

# Server Concept 3: The Persistent Hex-Vault (Indestructible Cell)
server_c3 = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="100%" height="100%">
  <defs>
    <linearGradient id="srv3-grad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#a5b4fc"/>
      <stop offset="50%" stop-color="#6366f1"/>
      <stop offset="100%" stop-color="#3730a3"/>
    </linearGradient>
  </defs>
  <g id="icon">
    <!-- Outer Hexagon -->
    <polygon points="256,64 416,160 416,352 256,448 96,352 96,160" fill="#0d0f18" stroke="url(#srv3-grad)" stroke-width="22" stroke-linejoin="round"/>

    <!-- Inner Floating Cube / Storage Core -->
    <polygon points="256,150 350,204 350,308 256,362 162,308 162,204" fill="#151827" stroke="#313752" stroke-width="12" stroke-linejoin="round"/>
    
    <!-- Isometric Top Face -->
    <polygon points="256,166 334,210 256,256 178,210" fill="#4f46e5" opacity="0.85"/>
    <!-- Isometric Left Face -->
    <polygon points="178,216 256,260 256,346 178,302" fill="#3730a3" opacity="0.9"/>
    <!-- Isometric Right Face -->
    <polygon points="334,216 256,260 256,346 334,302" fill="#6366f1" opacity="0.95"/>
  </g>
</svg>"""


# -------------------------------------------------------------
# 2. NEXUS CLI (Terminal REPL Client)
# Accent: Cyan & Electric Sky (#06b6d4, #0ea5e9, #38bdf8)
# -------------------------------------------------------------

# CLI Concept 1: The Dual Command Chevron (>> Terminal Flow)
cli_c1 = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="100%" height="100%">
  <defs>
    <linearGradient id="cli1-grad1" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#38bdf8"/>
      <stop offset="100%" stop-color="#0284c7"/>
    </linearGradient>
    <linearGradient id="cli1-grad2" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#67e8f9"/>
      <stop offset="100%" stop-color="#06b6d4"/>
    </linearGradient>
  </defs>
  <g id="icon">
    <!-- Rounded Terminal Frame -->
    <rect x="76" y="76" width="360" height="360" rx="90" fill="#081017" stroke="#162e3d" stroke-width="14"/>

    <!-- Primary Prompt Chevron > -->
    <path d="M164 168 L248 256 L164 344" fill="none" stroke="url(#cli1-grad1)" stroke-width="36" stroke-linecap="round" stroke-linejoin="round"/>

    <!-- Secondary Prompt Chevron > -->
    <path d="M248 168 L332 256 L248 344" fill="none" stroke="url(#cli1-grad2)" stroke-width="36" stroke-linecap="round" stroke-linejoin="round"/>
  </g>
</svg>"""

# CLI Concept 2: The Command Prompt & Cursor Monogram ( > _ )
cli_c2 = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="100%" height="100%">
  <defs>
    <linearGradient id="cli2-grad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#38bdf8"/>
      <stop offset="100%" stop-color="#0891b2"/>
    </linearGradient>
  </defs>
  <g id="icon">
    <!-- Dark Glass Shield -->
    <rect x="80" y="80" width="352" height="352" rx="84" fill="#0a1219" stroke="#1b3040" stroke-width="14"/>

    <!-- Command Prompt Arrow -->
    <path d="M160 176 L236 248 L160 320" fill="none" stroke="url(#cli2-grad)" stroke-width="34" stroke-linecap="round" stroke-linejoin="round"/>

    <!-- Terminal Cursor Block _ -->
    <rect x="272" y="300" width="92" height="24" rx="10" fill="#38bdf8"/>

    <!-- Terminal Window Top Dots -->
    <circle cx="160" cy="128" r="8" fill="#ef4444"/>
    <circle cx="188" cy="128" r="8" fill="#f59e0b"/>
    <circle cx="216" cy="128" r="8" fill="#10b981"/>
  </g>
</svg>"""

# CLI Concept 3: The Geometric Terminal N (Prompt Monogram)
cli_c3 = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="100%" height="100%">
  <defs>
    <linearGradient id="cli3-grad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#67e8f9"/>
      <stop offset="100%" stop-color="#0369a1"/>
    </linearGradient>
  </defs>
  <g id="icon">
    <!-- Isometric CLI Cube Frame -->
    <polygon points="256,80 406,170 406,342 256,432 106,342 106,170" fill="#081119" stroke="#162e3d" stroke-width="14" stroke-linejoin="round"/>

    <!-- Angled Command Slash & Chevrons forming 'N' -->
    <path d="M172 320 L172 192 L340 320 L340 192" fill="none" stroke="url(#cli3-grad)" stroke-width="34" stroke-linecap="round" stroke-linejoin="round"/>

    <!-- Center Prompt Glow Signal -->
    <circle cx="256" cy="256" r="16" fill="#38bdf8"/>
  </g>
</svg>"""


# -------------------------------------------------------------
# 3. NEXUS BENCHMARK (Stress Tester & High-Load Injector)
# Accent: Blaze Orange & Turbo Amber (#f59e0b, #f97316, #ef4444)
# -------------------------------------------------------------

# Bench Concept 1: The Tachometer Arc (100k+ QPS Speedometer)
bench_c1 = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="100%" height="100%">
  <defs>
    <linearGradient id="bnc1-grad" x1="0%" y1="100%" x2="100%" y2="0%">
      <stop offset="0%" stop-color="#f59e0b"/>
      <stop offset="65%" stop-color="#f97316"/>
      <stop offset="100%" stop-color="#ef4444"/>
    </linearGradient>
  </defs>
  <g id="icon">
    <!-- Dial Gauge Background -->
    <circle cx="256" cy="256" r="180" fill="#140f0c" stroke="#332117" stroke-width="14"/>

    <!-- High Velocity Speed Arc (240 degrees) -->
    <path d="M142 370 A 150 150 0 1 1 370 370" fill="none" stroke="url(#bnc1-grad)" stroke-width="32" stroke-linecap="round"/>

    <!-- Tachometer Needle Pin -->
    <circle cx="256" cy="256" r="28" fill="#1e140f" stroke="#f97316" stroke-width="8"/>
    <circle cx="256" cy="256" r="12" fill="#fff"/>

    <!-- Max Needle Pushing Redline -->
    <path d="M256 256 L342 170" stroke="#ff385c" stroke-width="14" stroke-linecap="round"/>
    
    <!-- Redline Tick Peak -->
    <circle cx="370" cy="370" r="10" fill="#ef4444"/>
  </g>
</svg>"""

# Bench Concept 2: The Kinetic Shockwave Bolts (Extreme Throughput)
bench_c2 = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="100%" height="100%">
  <defs>
    <linearGradient id="bnc2-grad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#fde047"/>
      <stop offset="50%" stop-color="#f97316"/>
      <stop offset="100%" stop-color="#dc2626"/>
    </linearGradient>
  </defs>
  <g id="icon">
    <!-- Rounded Shield -->
    <rect x="76" y="76" width="360" height="360" rx="90" fill="#130e0a" stroke="#382214" stroke-width="14"/>

    <!-- Dual Lightning Bolt Shockwaves -->
    <polygon points="276,128 176,268 246,268 216,384 336,244 266,244" fill="url(#bnc2-grad)" stroke="#fff" stroke-width="4" stroke-linejoin="round"/>
  </g>
</svg>"""

# Bench Concept 3: The Latency Pulse & Flame (Sub-Millisecond Engine)
bench_c3 = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="100%" height="100%">
  <defs>
    <linearGradient id="bnc3-grad" x1="0%" y1="100%" x2="100%" y2="0%">
      <stop offset="0%" stop-color="#f97316"/>
      <stop offset="100%" stop-color="#ef4444"/>
    </linearGradient>
  </defs>
  <g id="icon">
    <!-- Outer Octagon Frame -->
    <polygon points="180,76 332,76 436,180 436,332 332,436 180,436 76,332 76,180" fill="#120e0a" stroke="#382115" stroke-width="14" stroke-linejoin="round"/>

    <!-- High-Frequency Pulse Waveform -->
    <path d="M120 256 L180 256 L216 160 L256 352 L296 216 L332 256 L392 256" fill="none" stroke="url(#bnc3-grad)" stroke-width="26" stroke-linecap="round" stroke-linejoin="round"/>

    <!-- Metric Node Anchor -->
    <circle cx="256" cy="352" r="14" fill="#fbbf24"/>
  </g>
</svg>"""


# -------------------------------------------------------------
# 4. NEXUS DASHBOARD & COCKPIT (Telemetry & Topology Radar)
# Accent: Emerald & Mint Green (#10b981, #059669, #34d399)
# -------------------------------------------------------------

# Cockpit Concept 1: The Radar Topology Scope (3 Orbiting Nodes)
dash_c1 = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="100%" height="100%">
  <defs>
    <linearGradient id="dsh1-grad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#34d399"/>
      <stop offset="100%" stop-color="#059669"/>
    </linearGradient>
  </defs>
  <g id="icon">
    <!-- Radar Chassis -->
    <circle cx="256" cy="256" r="180" fill="#081410" stroke="#153628" stroke-width="14"/>

    <!-- Concentric Range Rings -->
    <circle cx="256" cy="256" r="124" fill="none" stroke="#1d4837" stroke-width="6" stroke-dasharray="8 8"/>
    <circle cx="256" cy="256" r="68" fill="none" stroke="#1d4837" stroke-width="6"/>

    <!-- Crosshair Reticle -->
    <line x1="256" y1="96" x2="256" y2="416" stroke="#1d4837" stroke-width="4"/>
    <line x1="96" y1="256" x2="416" y2="256" stroke="#1d4837" stroke-width="4"/>

    <!-- Radar Sweep Beam (45 deg) -->
    <path d="M256 256 L356 156 A 141 141 0 0 0 256 115 Z" fill="url(#dsh1-grad)" opacity="0.35"/>

    <!-- 3 Monitored Active Cluster Nodes -->
    <circle cx="340" cy="180" r="16" fill="#34d399"/>
    <circle cx="180" cy="310" r="12" fill="#10b981"/>
    <circle cx="320" cy="330" r="12" fill="#10b981"/>
    <circle cx="256" cy="256" r="8" fill="#ffffff"/>
  </g>
</svg>"""

# Cockpit Concept 2: The Telemetry Matrix & Wave (Cockpit Grid)
dash_c2 = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="100%" height="100%">
  <defs>
    <linearGradient id="dsh2-grad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#6ee7b7"/>
      <stop offset="100%" stop-color="#059669"/>
    </linearGradient>
  </defs>
  <g id="icon">
    <!-- Rounded Display Frame -->
    <rect x="76" y="76" width="360" height="360" rx="90" fill="#091410" stroke="#16382a" stroke-width="14"/>

    <!-- Dynamic 3-Bar Telemetry Levels -->
    <rect x="156" y="240" width="44" height="130" rx="16" fill="#10b981" opacity="0.6"/>
    <rect x="234" y="160" width="44" height="210" rx="16" fill="url(#dsh2-grad)"/>
    <rect x="312" y="200" width="44" height="170" rx="16" fill="#10b981" opacity="0.8"/>

    <!-- Horizontal Consensus Baseline -->
    <line x1="130" y1="390" x2="382" y2="390" stroke="#34d399" stroke-width="10" stroke-linecap="round"/>
  </g>
</svg>"""

# Cockpit Concept 3: The Interconnected Prism (Observability Mesh)
dash_c3 = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="100%" height="100%">
  <defs>
    <linearGradient id="dsh3-grad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#a7f3d0"/>
      <stop offset="100%" stop-color="#10b981"/>
    </linearGradient>
  </defs>
  <g id="icon">
    <!-- Outer Shield -->
    <polygon points="256,76 416,168 416,344 256,436 96,344 96,168" fill="#081410" stroke="#143628" stroke-width="14" stroke-linejoin="round"/>

    <!-- 3 Internal Observability Rays -->
    <line x1="256" y1="256" x2="256" y2="120" stroke="url(#dsh3-grad)" stroke-width="20" stroke-linecap="round"/>
    <line x1="256" y1="256" x2="140" y2="322" stroke="url(#dsh3-grad)" stroke-width="20" stroke-linecap="round"/>
    <line x1="256" y1="256" x2="372" y2="322" stroke="url(#dsh3-grad)" stroke-width="20" stroke-linecap="round"/>

    <!-- Central Signal Sensor -->
    <circle cx="256" cy="256" r="34" fill="#06120e" stroke="#34d399" stroke-width="10"/>
    <circle cx="256" cy="256" r="14" fill="#ffffff"/>

    <!-- Peripheral Sensor Nodes -->
    <circle cx="256" cy="120" r="16" fill="#34d399"/>
    <circle cx="140" cy="322" r="16" fill="#34d399"/>
    <circle cx="372" cy="322" r="16" fill="#34d399"/>
  </g>
</svg>"""

# Write all 12 SVG files
files = {
    "server-concept-1-raft-triad.svg": server_c1,
    "server-concept-2-lsm-monogram.svg": server_c2,
    "server-concept-3-hex-vault.svg": server_c3,
    "cli-concept-1-double-chevron.svg": cli_c1,
    "cli-concept-2-prompt-cursor.svg": cli_c2,
    "cli-concept-3-terminal-monogram.svg": cli_c3,
    "bench-concept-1-speed-arc.svg": bench_c1,
    "bench-concept-2-shockwave-bolt.svg": bench_c2,
    "bench-concept-3-latency-pulse.svg": bench_c3,
    "cockpit-concept-1-radar-scope.svg": dash_c1,
    "cockpit-concept-2-telemetry-levels.svg": dash_c2,
    "cockpit-concept-3-observability-mesh.svg": dash_c3,
}

for name, content in files.items():
    p = os.path.join(logos_dir, name)
    with open(p, "w", encoding="utf-8") as f:
        f.write(content)

print(f"Generated {len(files)} SVGs in {logos_dir}")
