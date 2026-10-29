import base64

with open("C:/Users/V I S I O N/nexus-kv/assets/cockpit.png", "rb") as f:
    b64_png = base64.b64encode(f.read()).decode('utf-8')

icon_tag = f'<link rel="icon" type="image/png" href="data:image/png;base64,{b64_png}">'

for path in ["C:/Users/V I S I O N/nexus-kv/web/index.html", "C:/Users/V I S I O N/Desktop/nexus_dashboard.html"]:
    with open(path, "r", encoding="utf-8") as f:
        content = f.read()
    
    # replace the favicon link
    import re
    content = re.sub(r'<link rel="icon"[^>]*>', icon_tag, content)
    
    with open(path, "w", encoding="utf-8") as f:
        f.write(content)

print("Embedded Base64 favicon successfully into both HTML files!")
