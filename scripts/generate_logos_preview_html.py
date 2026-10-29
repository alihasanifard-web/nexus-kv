# -*- coding: utf-8 -*-
"""
Generate the Interactive Logo Showcase & Favicon Test Matrix HTML page.
Placed in:
1. nexus-kv/logos/preview.html
2. Desktop/nexus_logos_preview.html
"""

import os
import base64

logos_dir = "C:/Users/V I S I O N/nexus-kv/logos/concepts"

apps_meta = [
    {
        "title": "1. Nexus Server (موتور اصلی دیتابیس و هسته اجماع Raft)",
        "desc": "سرور دیتابیس توزیع‌شده با الگوریتم اجماع Raft و موتور ذخیره‌سازی LSM-Tree",
        "tag": "SERVER",
        "color": "#6366f1",
        "concepts": [
            {
                "file": "server-concept-1-raft-triad.svg",
                "name": "کانسپت ۱: نکسوس سه‌گانه Raft (The Consensus Triad)",
                "desc": "سه نود متقارن با کانال‌های پایدار ارتباطی و هسته اجماع مرکزی. نماد حد نصاب اکثریت و پایداری کلاستر."
            },
            {
                "file": "server-concept-2-lsm-monogram.svg",
                "name": "کانسپت ۲: مونوگرام لایه‌ای N (LSM-Tree Monogram)",
                "desc": "حرف N شکل‌گرفته از ۳ لایه ذخیره‌سازی: بافر رم (MemTable)، جریان لاگ WAL و دیسک پایدار SSTable."
            },
            {
                "file": "server-concept-3-hex-vault.svg",
                "name": "کانسپت ۳: سلول شش‌ضلعی پایدار (Indestructible Hex-Vault)",
                "desc": "محفظه شش‌ضلعی هندسی با هسته ایزومتریک مکعبی، نماد ذخیره‌سازی امن و ماندگاری بدون از دست رفتن داده."
            }
        ]
    },
    {
        "title": "2. Nexus CLI (کنسول تعاملی و ترمینال کلاینت)",
        "desc": "کلاینت خط فرمان و مفسر تعاملی REPL برای ارسال آنی فرامین RESP",
        "tag": "CLI",
        "color": "#06b6d4",
        "concepts": [
            {
                "file": "cli-concept-1-double-chevron.svg",
                "name": "کانسپت ۱: فلش پرامپت دوگانه (>> Double Chevron)",
                "desc": "دو علامت بزرگتر پشت‌سرهم با پالت آسمانی، نماد اجرای فوق‌سریع دستورات خط فرمان در ترمینال."
            },
            {
                "file": "cli-concept-2-prompt-cursor.svg",
                "name": "کانسپت ۲: نماد پرامپت و کرسر ترمینال ( > _ )",
                "desc": "شیلد تیره‌رنگ با نشانگر ورودی `>` و نشانگر مکان‌نمای چشمک‌زن `_` همراه با دکمه‌های کنترل پنجره."
            },
            {
                "file": "cli-concept-3-terminal-monogram.svg",
                "name": "کانسپت ۳: مونوگرام ترمینال N (Prompt Monogram)",
                "desc": "مکعب خط فرمان زاویه‌دار که بدنه آن مونوگرام N و سیگنال مرکزی اتصال زنده را نشان می‌دهد."
            }
        ]
    },
    {
        "title": "3. Nexus Benchmark (تستر فشار و بارگذاری سنگین)",
        "desc": "ابزار بنچمارک چندترده برای سنجش ۱۰۲k QPS و تحلیل تاخیر صدک‌ها",
        "tag": "BENCHMARK",
        "color": "#f97316",
        "concepts": [
            {
                "file": "bench-concept-1-speed-arc.svg",
                "name": "کانسپت ۱: سرعت‌سنج تاکومتر (The Velocity Arc)",
                "desc": "گیج سرعت با سوزن متمایل به حداکثر توان و ردلاین قرمز، نماد شکستن رکورد ۱۰۰ هزار درخواست در ثانیه."
            },
            {
                "file": "bench-concept-2-shockwave-bolt.svg",
                "name": "کانسپت ۲: صاعقه ضربتی دوگانه (Kinetic Shockwave)",
                "desc": "صاعقه پرانرژی زرد-نارنجی، نماد توان تزریق بارهای همزمان ۵۰ ورکر با تاخیر ساب-میلی‌ثانیه‌ای."
            },
            {
                "file": "bench-concept-3-latency-pulse.svg",
                "name": "کانسپت ۳: پالس تاخیر و فرکانس بالا (Latency Pulse)",
                "desc": "سیگنال ضربانی فشرده درون فریم هشت‌ضلعی، نماد کنترل تاخیر P99 در محدوده میکروثانیه."
            }
        ]
    },
    {
        "title": "4. Nexus Cockpit & Dashboard (کنسول وب و رادار پایش)",
        "desc": "داشبورد زنده تلمتری، مانیتورینگ سلامت نودها و تست آشوب کلاستر",
        "tag": "COCKPIT",
        "color": "#10b981",
        "concepts": [
            {
                "file": "cockpit-concept-1-radar-scope.svg",
                "name": "کانسپت ۱: رادار پایش توپولوژی (Topology Radar Scope)",
                "desc": "صفحه رادار دوار با ۳ مدار نودهای فعال و خطوط کراس‌هیر متقاطع، نماد پایش زنده و دائمی وضعیت کلاستر."
            },
            {
                "file": "cockpit-concept-2-telemetry-levels.svg",
                "name": "کانسپت ۲: ماتریس تلمتری و شاخص‌ها (Telemetry Levels)",
                "desc": "سه ستون نوری موازی با سطح پایه پایدار، نماد مانیتورینگ لحظه‌ای متریک‌های عملکردی و توان دیتابیس."
            },
            {
                "file": "cockpit-concept-3-observability-mesh.svg",
                "name": "کانسپت ۳: شبکه مش دیده‌پذیری (Observability Mesh)",
                "desc": "سنسور مرکزی متصل به ۳ نود پیرامونی، نماد دیده‌پذیری کامل و دریافت لاگ‌های سلامت سرتاسر کلاستر."
            }
        ]
    }
]

# Read inline SVGs so the HTML is 100% self-contained and portable
svg_data = {}
for root, _, files in os.walk(logos_dir):
    for f in files:
        if f.endswith(".svg"):
            with open(os.path.join(root, f), "r", encoding="utf-8") as sfile:
                svg_data[f] = sfile.read()

html = """<!DOCTYPE html>
<html lang="fa" dir="rtl">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>NexusKV — مجموعه هویت بصری و لوگوهای اختصاصی</title>
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Vazirmatn:wght@400;500;600;700;800&family=JetBrains+Mono:wght@500;700&display=swap" rel="stylesheet">
  <style>
    :root {
      --bg: #090a0f;
      --surface: #10121a;
      --surface-elevated: #161925;
      --border: rgba(255, 255, 255, 0.08);
      --border-hover: rgba(255, 255, 255, 0.2);
      --text: #f8fafc;
      --text-muted: #94a3b8;
      --text-dim: #64748b;
      --accent: #6366f1;
    }

    body.light {
      --bg: #f8fafc;
      --surface: #ffffff;
      --surface-elevated: #f1f5f9;
      --border: rgba(0, 0, 0, 0.09);
      --border-hover: rgba(0, 0, 0, 0.22);
      --text: #0f172a;
      --text-muted: #475569;
      --text-dim: #94a3b8;
      --accent: #4f46e5;
    }

    * { box-sizing: border-box; margin: 0; padding: 0; }

    body {
      font-family: 'Vazirmatn', sans-serif;
      background: var(--bg);
      color: var(--text);
      min-height: 100vh;
      line-height: 1.7;
      transition: background 0.25s, color 0.25s;
      padding-bottom: 60px;
    }

    header {
      padding: 24px 40px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      border-bottom: 1px solid var(--border);
      background: var(--surface);
      position: sticky;
      top: 0;
      z-index: 100;
    }

    .brand-title {
      font-size: 1.25rem;
      font-weight: 800;
      display: flex;
      align-items: center;
      gap: 12px;
    }

    .brand-sub {
      font-size: 0.85rem;
      color: var(--text-dim);
      font-weight: 400;
    }

    .theme-toggle-btn {
      background: var(--surface-elevated);
      border: 1px solid var(--border);
      color: var(--text);
      font-family: inherit;
      font-size: 0.85rem;
      font-weight: 600;
      padding: 8px 18px;
      border-radius: 8px;
      cursor: pointer;
      display: flex;
      align-items: center;
      gap: 8px;
      transition: all 0.2s;
    }

    .theme-toggle-btn:hover {
      border-color: var(--border-hover);
    }

    .hero-banner {
      max-width: 1200px;
      margin: 36px auto 20px;
      padding: 0 24px;
      text-align: center;
    }

    .hero-banner h1 {
      font-size: 1.85rem;
      font-weight: 800;
      margin-bottom: 8px;
    }

    .hero-banner p {
      color: var(--text-muted);
      font-size: 0.95rem;
      max-width: 720px;
      margin: 0 auto;
    }

    .main-wrap {
      max-width: 1200px;
      margin: 0 auto;
      padding: 0 24px;
    }

    .app-section {
      margin-top: 48px;
      background: var(--surface);
      border: 1px solid var(--border);
      border-radius: 14px;
      padding: 28px;
    }

    .section-head {
      display: flex;
      align-items: center;
      justify-content: space-between;
      margin-bottom: 24px;
      padding-bottom: 16px;
      border-bottom: 1px solid var(--border);
    }

    .section-badge {
      font-family: 'JetBrains Mono', monospace;
      font-size: 0.75rem;
      font-weight: 700;
      padding: 4px 10px;
      border-radius: 6px;
      letter-spacing: 0.05em;
    }

    .concepts-grid {
      display: grid;
      grid-template-columns: repeat(3, 1fr);
      gap: 20px;
    }

    @media (max-width: 900px) {
      .concepts-grid { grid-template-columns: 1fr; }
    }

    .concept-card {
      background: var(--bg);
      border: 1px solid var(--border);
      border-radius: 12px;
      overflow: hidden;
      display: flex;
      flex-direction: column;
      transition: transform 0.2s, border-color 0.2s;
    }

    .concept-card:hover {
      border-color: var(--border-hover);
      transform: translateY(-2px);
    }

    .card-preview {
      height: 220px;
      display: flex;
      align-items: center;
      justify-content: center;
      padding: 24px;
      background: radial-gradient(circle at center, rgba(255,255,255,0.03) 0%, transparent 70%);
    }

    body.light .card-preview {
      background: radial-gradient(circle at center, rgba(0,0,0,0.03) 0%, transparent 70%);
    }

    .card-preview svg {
      max-width: 150px;
      max-height: 150px;
      filter: drop-shadow(0 8px 24px rgba(0,0,0,0.35));
    }

    .card-details {
      padding: 18px 20px;
      border-top: 1px solid var(--border);
      flex: 1;
      display: flex;
      flex-direction: column;
      justify-content: space-between;
    }

    .concept-title {
      font-size: 0.98rem;
      font-weight: 700;
      margin-bottom: 6px;
    }

    .concept-desc {
      font-size: 0.82rem;
      color: var(--text-muted);
      line-height: 1.6;
      margin-bottom: 16px;
    }

    /* Favicon Strip Check */
    .favicon-strip {
      background: var(--surface);
      border: 1px solid var(--border);
      border-radius: 8px;
      padding: 10px 14px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      direction: ltr;
    }

    .strip-label {
      font-size: 0.72rem;
      font-family: 'JetBrains Mono', monospace;
      color: var(--text-dim);
    }

    .strip-sizes {
      display: flex;
      align-items: center;
      gap: 12px;
    }

    .strip-item {
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 3px;
    }

    .strip-item svg {
      border-radius: 3px;
    }

    .strip-px {
      font-size: 0.62rem;
      font-family: 'JetBrains Mono', monospace;
      color: var(--text-dim);
    }

    footer {
      max-width: 1200px;
      margin: 50px auto 0;
      padding: 24px;
      border-top: 1px solid var(--border);
      text-align: center;
      font-size: 0.85rem;
      color: var(--text-dim);
    }
  </style>
</head>
<body>

  <!-- Top Header -->
  <header>
    <div class="brand-title">
      <span>NexusKV Brand Identity</span>
      <span class="brand-sub">۱۲ کانسپت لوگوی اختصاصی و مجزا برای ۴ برنامه اکوسیستم نکسوس</span>
    </div>
    <button class="theme-toggle-btn" onclick="toggleTheme()" id="btnTheme">
      <span>☀️ حالت روشن (Light)</span>
    </button>
  </header>

  <!-- Hero Banner -->
  <div class="hero-banner">
    <h1>هویت بصری و کانسپت‌های لوگوی NexusKV</h1>
    <p>هر ۴ برنامه موجود در پروژه دارای نماد، پالت رنگی و استعاره اختصاصی هستند تا کاربر در اولین نگاه عملکرد هر ابزار را فوراً از یکدیگر متمایز کند.</p>
  </div>

  <div class="main-wrap">
"""

for app in apps_meta:
    html += f"""
    <!-- App Group: {app['title']} -->
    <div class="app-section">
      <div class="section-head">
        <div>
          <h2 style="font-size:1.15rem; font-weight:700;">{app['title']}</h2>
          <div style="font-size:0.84rem; color:var(--text-muted); margin-top:2px;">{app['desc']}</div>
        </div>
        <span class="section-badge" style="background:{app['color']}22; color:{app['color']}; border:1px solid {app['color']}44;">{app['tag']}</span>
      </div>

      <div class="concepts-grid">
"""
    for c in app["concepts"]:
        svg_content = svg_data.get(c["file"], "")
        html += f"""
        <!-- Concept Card -->
        <div class="concept-card">
          <div class="card-preview">
            {svg_content}
          </div>
          <div class="card-details">
            <div>
              <div class="concept-title">{c['name']}</div>
              <div class="concept-desc">{c['desc']}</div>
            </div>

            <!-- Favicon Legibility Check Strip -->
            <div class="favicon-strip">
              <span class="strip-label">FAVICON SIZES</span>
              <div class="strip-sizes">
                <div class="strip-item">
                  <div style="width:36px; height:36px;">{svg_content}</div>
                  <span class="strip-px">36px</span>
                </div>
                <div class="strip-item">
                  <div style="width:24px; height:24px;">{svg_content}</div>
                  <span class="strip-px">24px</span>
                </div>
                <div class="strip-item">
                  <div style="width:16px; height:16px;">{svg_content}</div>
                  <span class="strip-px">16px</span>
                </div>
              </div>
            </div>
          </div>
        </div>
"""
    html += """
      </div>
    </div>
"""

html += """
  </div>

  <footer>
    طراح و معمار سیستم: علی حسنی‌فرد (Ali Hasanifard) &bull; تمامی لوگوها با استاندارد وکتور مقیاس‌پذیر SVG (ViewBox 512x512) و بدون افت کیفیت رندر می‌شوند.
  </footer>

  <script>
    function toggleTheme() {
      const b = document.body;
      const btn = document.getElementById('btnTheme');
      if (b.classList.contains('light')) {
        b.classList.remove('light');
        btn.innerHTML = '<span>☀️ حالت روشن (Light)</span>';
      } else {
        b.classList.add('light');
        btn.innerHTML = '<span>🌙 حالت تیره (Dark)</span>';
      }
    }
  </script>
</body>
</html>
"""

# Write to both locations
p1 = "C:/Users/V I S I O N/nexus-kv/logos/preview.html"
p2 = "C:/Users/V I S I O N/Desktop/nexus_logos_preview.html"

with open(p1, "w", encoding="utf-8") as f:
    f.write(html)

with open(p2, "w", encoding="utf-8") as f:
    f.write(html)

print("Saved logo showcase HTML successfully to both locations!")
