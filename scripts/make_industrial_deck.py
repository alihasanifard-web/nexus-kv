# -*- coding: utf-8 -*-
import sys
import os
from pptx import Presentation
from pptx.util import Inches, Pt
from pptx.enum.text import PP_ALIGN
from pptx.dml.color import RGBColor

def set_shape_bg(shape, hex_color):
    fill = shape.fill
    fill.solid()
    r = int(hex_color[0:2], 16)
    g = int(hex_color[2:4], 16)
    b = int(hex_color[4:6], 16)
    fill.fore_color.rgb = RGBColor(r, g, b)

def apply_rtl(p):
    p_pr = p._p.get_or_add_pPr()
    p_pr.set('rtl', '1')

def add_header(slide, title_text, category_text="NEXUS-KV // DISTRIBUTED SYSTEMS ENGINE"):
    header_box = slide.shapes.add_textbox(Inches(0.8), Inches(0.4), Inches(11.7), Inches(1.1))
    tf = header_box.text_frame
    tf.word_wrap = True
    tf.margin_left = tf.margin_top = tf.margin_right = tf.margin_bottom = 0
    
    p_cat = tf.paragraphs[0]
    p_cat.alignment = PP_ALIGN.RIGHT
    apply_rtl(p_cat)
    r_cat = p_cat.add_run()
    r_cat.text = "// " + category_text
    r_cat.font.name = "Space Mono"
    r_cat.font.size = Pt(12)
    r_cat.font.bold = True
    r_cat.font.color.rgb = RGBColor(212, 255, 0) # Acid Volt
    
    p_title = tf.add_paragraph()
    p_title.alignment = PP_ALIGN.RIGHT
    apply_rtl(p_title)
    r_title = p_title.add_run()
    r_title.text = title_text
    r_title.font.name = "Vazirmatn"
    r_title.font.size = Pt(22)
    r_title.font.bold = True
    r_title.font.color.rgb = RGBColor(248, 250, 252)

def add_footer(slide, current_idx, total_slides):
    footer_box = slide.shapes.add_textbox(Inches(0.8), Inches(6.9), Inches(11.7), Inches(0.4))
    tf = footer_box.text_frame
    tf.word_wrap = True
    tf.margin_left = tf.margin_top = tf.margin_right = tf.margin_bottom = 0
    
    p = tf.paragraphs[0]
    p.alignment = PP_ALIGN.RIGHT
    apply_rtl(p)
    r = p.add_run()
    r.text = f"NEXUS-KV DISTRIBUTED ENGINE  //  ARCHITECT: ALI HASANIFARD  //  SLIDE {current_idx:02d} OF {total_slides:02d}"
    r.font.name = "Space Mono"
    r.font.size = Pt(10)
    r.font.color.rgb = RGBColor(120, 120, 120)

def add_card(slide, left, top, width, height, title, items, badge_text=None, border_color="333333", bg_color="0f0f12"):
    card = slide.shapes.add_shape(1, Inches(left), Inches(top), Inches(width), Inches(height))
    set_shape_bg(card, bg_color)
    card.line.color.rgb = RGBColor(int(border_color[0:2], 16), int(border_color[2:4], 16), int(border_color[4:6], 16))
    card.line.width = Pt(1.5)
    
    tf = card.text_frame
    tf.word_wrap = True
    tf.margin_left = tf.margin_right = Inches(0.25)
    tf.margin_top = Inches(0.22)
    tf.margin_bottom = Inches(0.2)
    
    p_title = tf.paragraphs[0]
    p_title.alignment = PP_ALIGN.RIGHT
    apply_rtl(p_title)
    
    if badge_text:
        r_b = p_title.add_run()
        r_b.text = f"[{badge_text}] "
        r_b.font.name = "Space Mono"
        r_b.font.size = Pt(12)
        r_b.font.bold = True
        r_b.font.color.rgb = RGBColor(255, 92, 0) # Safety Orange
        
    r_t = p_title.add_run()
    r_t.text = title
    r_t.font.name = "Vazirmatn"
    r_t.font.size = Pt(15)
    r_t.font.bold = True
    r_t.font.color.rgb = RGBColor(212, 255, 0) # Acid Volt
    
    for item in items:
        p_item = tf.add_paragraph()
        p_item.space_before = Pt(8)
        p_item.alignment = PP_ALIGN.RIGHT
        apply_rtl(p_item)
        r_bullet = p_item.add_run()
        r_bullet.text = "• "
        r_bullet.font.name = "Space Mono"
        r_bullet.font.size = Pt(12)
        r_bullet.font.color.rgb = RGBColor(255, 92, 0) # Orange
        
        r_text = p_item.add_run()
        r_text.text = item
        r_text.font.name = "Vazirmatn"
        r_text.font.size = Pt(12)
        r_text.font.color.rgb = RGBColor(230, 230, 230)

def add_code_card(slide, left, top, width, height, title, code_snippet):
    card = slide.shapes.add_shape(1, Inches(left), Inches(top), Inches(width), Inches(height))
    set_shape_bg(card, "050507")
    card.line.color.rgb = RGBColor(45, 45, 45)
    card.line.width = Pt(1.5)
    
    tf = card.text_frame
    tf.word_wrap = True
    tf.margin_left = tf.margin_right = Inches(0.25)
    tf.margin_top = Inches(0.2)
    
    p_title = tf.paragraphs[0]
    p_title.alignment = PP_ALIGN.RIGHT
    apply_rtl(p_title)
    r_t = p_title.add_run()
    r_t.text = "TELEMETRY / PROTOCOL: " + title
    r_t.font.name = "Space Mono"
    r_t.font.size = Pt(12)
    r_t.font.bold = True
    r_t.font.color.rgb = RGBColor(255, 92, 0) # Safety Orange
    
    p_code = tf.add_paragraph()
    p_code.space_before = Pt(8)
    p_code.alignment = PP_ALIGN.LEFT
    r_c = p_code.add_run()
    r_c.text = code_snippet
    r_c.font.name = "Space Mono"
    r_c.font.size = Pt(11)
    r_c.font.color.rgb = RGBColor(212, 255, 0) # Acid Volt

def create_deck():
    prs = Presentation()
    prs.slide_width = Inches(13.333)
    prs.slide_height = Inches(7.5)
    blank_layout = prs.slide_layouts[6]
    
    TOTAL_SLIDES = 10
    
    def make_base_slide(chapter_tag, title_text, idx):
        s = prs.slides.add_slide(blank_layout)
        bg = s.shapes.add_shape(1, 0, 0, Inches(13.333), Inches(7.5))
        set_shape_bg(bg, "050505")
        bg.line.fill.background()
        add_header(s, title_text, chapter_tag)
        add_footer(s, idx, TOTAL_SLIDES)
        return s

    # SLIDE 1: Cover
    s1 = prs.slides.add_slide(blank_layout)
    bg1 = s1.shapes.add_shape(1, 0, 0, Inches(13.333), Inches(7.5))
    set_shape_bg(bg1, "050505")
    bg1.line.fill.background()
    
    box1 = s1.shapes.add_shape(1, Inches(1.5), Inches(1.0), Inches(10.333), Inches(5.5))
    set_shape_bg(box1, "0e0e11")
    box1.line.color.rgb = RGBColor(212, 255, 0)
    box1.line.width = Pt(2.0)
    
    tf1 = box1.text_frame
    tf1.word_wrap = True
    p = tf1.paragraphs[0]
    p.alignment = PP_ALIGN.CENTER
    apply_rtl(p)
    r = p.add_run()
    r.text = "⚡ NEXUS-KV: DISTRIBUTED STORAGE ENGINE"
    r.font.name = "Space Mono"
    r.font.size = Pt(28)
    r.font.bold = True
    r.font.color.rgb = RGBColor(212, 255, 0)
    
    p2 = tf1.add_paragraph()
    p2.space_before = Pt(14)
    p2.alignment = PP_ALIGN.CENTER
    apply_rtl(p2)
    r2 = p2.add_run()
    r2.text = "INDUSTRIAL RAFT CONSENSUS & LSM-TREE IN PURE RUST"
    r2.font.name = "Space Mono"
    r2.font.size = Pt(16)
    r2.font.bold = True
    r2.font.color.rgb = RGBColor(255, 92, 0)
    
    p3 = tf1.add_paragraph()
    p3.space_before = Pt(24)
    p3.alignment = PP_ALIGN.CENTER
    apply_rtl(p3)
    r3 = p3.add_run()
    r3.text = "مهندسی سیستم‌های توزیع‌شده مقیاس‌پذیر با توان پردازش +۱۰۲ هزار درخواست بر ثانیه و سازگاری کامل با پروتکل Redis RESP"
    r3.font.name = "Vazirmatn"
    r3.font.size = Pt(14)
    r3.font.color.rgb = RGBColor(230, 230, 230)
    
    p4 = tf1.add_paragraph()
    p4.space_before = Pt(40)
    p4.alignment = PP_ALIGN.CENTER
    apply_rtl(p4)
    r4 = p4.add_run()
    r4.text = "LEAD SYSTEMS ARCHITECT: ALI HASANIFARD (علی حسنی فرد)\ngithub.com/alihasanifard-web/nexus-kv"
    r4.font.name = "Space Mono"
    r4.font.size = Pt(13)
    r4.font.bold = True
    r4.font.color.rgb = RGBColor(160, 160, 160)
    add_footer(s1, 1, TOTAL_SLIDES)

    # SLIDE 2: High-Level Architecture
    s2 = make_base_slide("SECTION 01 // ARCHITECTURE", "معماری سه‌لایه سیستم پایگاه داده توزیع‌شده NexusKV", 2)
    add_card(s2, 0.8, 1.8, 5.7, 4.8, "لایه‌های سه‌گانه صنعتی", [
        "لایه ۱ (شبکه و پروتکل): سرور ناهمگام Tokio با پروتکل Redis RESP v2.",
        "لایه ۲ (اجماع توزیع‌شده): ماشین حالت Raft برای تکثیر لاگ و انتخابات لیدر.",
        "لایه ۳ (موتور ذخیره‌سازی): ساختار LSM-Tree با MemTable، WAL و SSTable.",
        "پروتکل دو فازی 2PC: تراکنش‌های ACID با ساعت منطقی هیبریدی HLC."
    ], "LAYER 1-3")
    add_code_card(s2, 6.8, 1.8, 5.7, 4.8, "WRITE PATH EXECUTION",
        "[Client: SET key val]\n         ↓ (RESP Protocol)\n[Tokio Async TCP Worker]\n         ↓\n[Raft Propose -> Append Log]\n         ↓ (Quorum 2/3 Nodes)\n[Commit Log Entry]\n         ↓\n[WAL Append (CRC32)] -> [MemTable Put]\n         ↓ (Size >= Threshold)\n[Flush to Disk: SSTable]"
    )

    # SLIDE 3: LSM-Tree Storage
    s3 = make_base_slide("SECTION 02 // LSM-TREE", "موتور ذخیره‌سازی درخت LSM در برابر B-Tree سنتی", 3)
    add_card(s3, 0.8, 1.8, 5.7, 4.8, "مزایای LSM-Tree در نوشتن سنگین", [
        "تبدیل نوشتن‌های تصادفی به نوشتن ترتیبی متوالی روی دیسک (Sequential I/O).",
        "عدم وجود قفل‌شدگی سراسری در حافظه رم با استفاده از RwLock BTreeMap.",
        "علامت‌گذاری حذفی با Tombstone جهت حذف بدون افت پرفورمنس.",
        "فشرده‌سازی در پس‌زمینه (Background Compaction) برای بازیافت فضا."
    ], "STORAGE")
    add_card(s3, 6.8, 1.8, 5.7, 4.8, "اجزای داخلی ذخیره‌سازی", [
        "MemTable: بافر رم مرتب بر پایه RwLock با پایش لحظه‌ای بایت‌ها.",
        "WAL: لاگ مکتوب دیسک با فریم‌های اعتبارسنجی شده توسط CRC32.",
        "SSTable: فایل‌های تغییرناپذیر دیسک با شاخص تنک برای جستجوی دودویی.",
        "فیلتر بلوم: ساختار احتمالی دابل-هشینگ با خطای منفی صفر و پرش از دیسک."
    ], "MODULES")

    # SLIDE 4: Raft Consensus
    s4 = make_base_slide("SECTION 03 // CONSENSUS", "الگوریتم اجماع Raft و مقاومت در برابر قطع شبکه", 4)
    add_card(s4, 0.8, 1.8, 5.7, 4.8, "ماشین حالت و قوانین اجماع", [
        "نقش‌های گره: Follower, Candidate, Leader.",
        "انتخابات لیدر با تایمرهای تصادفی ۱۵۰ تا ۳۰۰ میلی‌ثانیه برای جلوگیری از Split-Vote.",
        "ارسال ضربان قلب (Heartbeat) در هر ۵۰ میلی‌ثانیه توسط لیدر.",
        "قانون حد نصاب اکثریت (Quorum = N/2 + 1) برای ثبت قطعی لاگ‌ها."
    ], "RAFT CORE")
    add_code_card(s4, 6.8, 1.8, 5.7, 4.8, "RPC PROTOCOL SCHEMAS",
        "// RPC درخواست رای\nRequestVoteArgs {\n    term: u64,\n    candidate_id: NodeId,\n    last_log_index: u64,\n    last_log_term: u64\n}\n\n// RPC تکثیر لاگ و هارت‌بیت\nAppendEntriesArgs {\n    term: u64,\n    leader_id: NodeId,\n    entries: Vec<LogEntry>,\n    leader_commit: u64\n}"
    )

    # SLIDE 5: Redis Compatibility
    s5 = make_base_slide("SECTION 04 // PROTOCOL", "سازگاری ۱۰۰٪ با پروتکل استاندارد Redis RESP v2", 5)
    add_card(s5, 0.8, 1.8, 5.7, 4.8, "کلاینت‌های بدون نیاز به درایور", [
        "پشتیبانی بدون واسطه از redis-cli، redis-py، ioredis و go-redis.",
        "پارس استریم بدون کپی حافظه (Zero-Copy) با بافرهای BytesMut.",
        "پشتیبانی از دستورات کلیدی PING، SET، GET، DEL، ROLE و FLUSHALL.",
        "دستورات اسکن پیشرفته KEYS pattern و SCAN cursor برای پیمایش بازه‌ای."
    ], "RESP WIRE")
    add_code_card(s5, 6.8, 1.8, 5.7, 4.8, "OFFICIAL PYTHON DRIVER",
        "from nexus_kv import NexusClient\n\nwith NexusClient(port=6379) as client:\n    client.set('user:101', 'Ali Hasanifard')\n    val = client.get('user:101')\n    print('Retrieved:', val)\n\n    # اسکن با وایلدکارد\n    keys = client.keys('user:*')\n    print('Matched keys:', keys)"
    )

    # SLIDE 6: Multi-Raft & Sharding
    s6 = make_base_slide("SECTION 05 // SCALE", "معماری چندگروهه Multi-Raft و تقسیم خودکار Region", 6)
    add_card(s6, 0.8, 1.8, 5.7, 4.8, "افراز داده‌ها (Dynamic Sharding)", [
        "تقسیم فضای کلیدها به بازه‌های مشخص به نام Region.",
        "هر Region دارای یک گروه مستقل Raft روی سرورهای مختلف است.",
        "عدم افت پرفورمنس با افزایش حجم داده تا ترابایت و پتابایت.",
        "عملیات split_region برای دو نیم کردن Regionهای پرحجم درجا."
    ], "MULTI-RAFT")
    add_code_card(s6, 6.8, 1.8, 5.7, 4.8, "REGION ROUTING ENGINE",
        "// روتینگ کلید به ریجن مربوطه\nlet router = ShardRouter::new();\n\n// تقسیم ریجن در کلید 'm'\nrouter.split_region(1, b'm'.to_vec(), 2)?;\n\n// کلید 'apple' به ریجن ۱ می‌رود\nlet r1 = router.locate_region(b'apple');\n\n// کلید 'zebra' به ریجن ۲ می‌رود\nlet r2 = router.locate_region(b'zebra');"
    )

    # SLIDE 7: Distributed ACID 2PC
    s7 = make_base_slide("SECTION 06 // TRANSACTIONS", "پروتکل ۲ فازی 2PC و ساعت منطقی هیبریدی (HLC)", 7)
    add_card(s7, 0.8, 1.8, 5.7, 4.8, "تضمین کامل ویژگی‌های ACID", [
        "ساعت منطقی هیبریدی HLC برای ترتیب‌دهی خطی تراکنش‌ها.",
        "کنترل همزمانی خوش‌بینانه (OCC) جهت کشف تضادهای نوشتن-نوشتن.",
        "فاز ۱ (Prewrite): اخذ قفل‌های محلی و ثبت تغییرات موقت.",
        "فاز ۲ (Commit): تخصیص Commit Timestamp و آزادسازی اتمیک قفل‌ها."
    ], "2PC ENGINE")
    add_code_card(s7, 6.8, 1.8, 5.7, 4.8, "ATOMIC TRANSACTION SPEC",
        "let mut txn = Transaction::begin();\n\n// انتقال پول بدون ریسک رقابت\ntxn.set(b'acc:alice'.to_vec(), b'900'.to_vec());\ntxn.set(b'acc:bob'.to_vec(), b'1100'.to_vec());\n\n// فاز اول: قفل‌گذاری\ncoordinator.prewrite(&txn)?;\n\n// فاز دوم: تایید و اعمال\nlet commit_ts = coordinator.commit(&mut txn)?;"
    )

    # SLIDE 8: Benchmarks & Latency
    s8 = make_base_slide("SECTION 07 // BENCHMARKS", "نتایج بنچمارک رسمی و تاخیر صدک‌ها (P99)", 8)
    add_card(s8, 0.8, 1.8, 5.7, 4.8, "پرفورمنس و نرخ پردازش", [
        "تست‌شده با ابزار اختصاصی چندترده nexus-bench روی حافظه NVMe.",
        "بیش از ۱۰۲ هزار درخواست در ثانیه در بار ترکیبی ۵۰ ورکر همزمان.",
        "میانگین تاخیر میانه (P50) زیر ۳۴۰ میکروثانیه.",
        "حداکثر تاخیر صدک ۹۹ (P99) در مرز ۱.۲ میلی‌ثانیه."
    ], "PERFORMANCE")
    add_card(s8, 6.8, 1.8, 5.7, 4.8, "جدول تاخیرات رسمی", [
        "• P50  Latency (Median):    340 µs",
        "• P90  Latency:             780 µs",
        "• P95  Latency:             960 µs",
        "• P99  Latency:           1,240 µs (1.2 ms)",
        "• P99.9 Latency:          2,100 µs (2.1 ms)",
        "• نرخ توان عملیاتی:        102,100 QPS"
    ], "EMPIRICAL DATA", border_color="d4ff00")

    # SLIDE 9: Chaos Testing & K8s
    s9 = make_base_slide("SECTION 08 // RESILIENCE", "تست آشوب خودکار (Chaos Test) و استقرار کوبرنتیز", 9)
    add_card(s9, 0.8, 1.8, 5.7, 4.8, "نتایج تست آشوب (Chaos Testing)", [
        "اسکریپت اتوماتیک scripts/chaos_test.py.",
        "تزریق ۲۰ رکورد -> شلیک SIGKILL به پروسس لیدر در حین کار.",
        "استارت سرد نود: بازخوانی ۱۰۰٪ سالم تمام داده‌ها از لاگ مکتوب WAL.",
        "صفر درصد افت یا فساد داده در بدترین سناریوهای قطعی برق."
    ], "CHAOS LAB")
    add_card(s9, 6.8, 1.8, 5.7, 4.8, "استقرار مدرن ابری", [
        "ایمیج فوق‌سبک Docker کمتر از ۲۰ مگابایت با ساخت دو مرحله‌ای Alpine.",
        "مانیفست رسمی Kubernetes StatefulSet با ۳ رپلیکا.",
        "سرویس Headless برای شناسایی و تبادل پکت‌های Raft.",
        "پایش سلامت زنده روی اندپوینت با پروتکل پرومتئوس (METRICS)."
    ], "CLOUD DEPLOY")

    # SLIDE 10: Conclusion & Author
    s10 = prs.slides.add_slide(blank_layout)
    bg10 = s10.shapes.add_shape(1, 0, 0, Inches(13.333), Inches(7.5))
    set_shape_bg(bg10, "050505")
    bg10.line.fill.background()
    add_header(s10, "جمع‌بندی و هویت پروژه NexusKV", "CREDITS & SPEC")
    add_footer(s10, 10, TOTAL_SLIDES)
    
    box_c = s10.shapes.add_shape(1, Inches(1.2), Inches(1.8), Inches(10.933), Inches(4.8))
    set_shape_bg(box_c, "0e0e11")
    box_c.line.color.rgb = RGBColor(212, 255, 0)
    box_c.line.width = Pt(2.0)
    
    tfc = box_c.text_frame
    tfc.word_wrap = True
    
    p1 = tfc.paragraphs[0]
    p1.alignment = PP_ALIGN.CENTER
    apply_rtl(p1)
    r1 = p1.add_run()
    r1.text = "🏆 NEXUS-KV: INDUSTRIAL DISTRIBUTED STORAGE PLATFORM"
    r1.font.name = "Space Mono"
    r1.font.size = Pt(22)
    r1.font.bold = True
    r1.font.color.rgb = RGBColor(212, 255, 0)
    
    p2 = tfc.add_paragraph()
    p2.space_before = Pt(20)
    p2.alignment = PP_ALIGN.CENTER
    apply_rtl(p2)
    r2 = p2.add_run()
    r2.text = "پایپ‌لاین CI گیت‌هاب • کلاینت تعاملی nexus-cli • درایور رسمی پایتون • مستندات و وایت‌پیپر مهندسی"
    r2.font.name = "Vazirmatn"
    r2.font.size = Pt(15)
    r2.font.color.rgb = RGBColor(230, 230, 230)
    
    p3 = tfc.add_paragraph()
    p3.space_before = Pt(36)
    p3.alignment = PP_ALIGN.CENTER
    apply_rtl(p3)
    r3 = p3.add_run()
    r3.text = "LEAD SYSTEMS ARCHITECT & DESIGNER:"
    r3.font.name = "Space Mono"
    r3.font.size = Pt(16)
    r3.font.bold = True
    r3.font.color.rgb = RGBColor(255, 92, 0)
    
    p4 = tfc.add_paragraph()
    p4.space_before = Pt(14)
    p4.alignment = PP_ALIGN.CENTER
    apply_rtl(p4)
    r4 = p4.add_run()
    r4.text = "ALI HASANIFARD (علی حسنی فرد)\nEmail: hasanifardalipa@gmail.com\nGitHub: https://github.com/alihasanifard-web/nexus-kv"
    r4.font.name = "Space Mono"
    r4.font.size = Pt(14)
    r4.font.bold = True
    r4.font.color.rgb = RGBColor(212, 255, 0)

    out_file = r"C:\Users\V I S I O N\Desktop\معرفی_معماری_NexusKV.pptx"
    prs.save(out_file)
    print(f"[OK] Industrial System Design Presentation saved to: {out_file}")

if __name__ == "__main__":
    create_deck()
