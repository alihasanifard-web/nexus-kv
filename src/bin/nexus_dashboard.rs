// ==============================================================================
// 🔭 NEXUS-DASHBOARD: NATIVE DESKTOP COCKPIT LAUNCHER
// Architect: Ali Hasanifard
// ==============================================================================

use std::path::Path;
use std::process::Command;

fn main() {
    let dashboard_path = r"C:\Users\V I S I O N\Desktop\nexus_dashboard.html";
    let edge_path = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe";

    // Launch in borderless desktop application window mode
    if Path::new(edge_path).exists() {
        let _ = Command::new(edge_path)
            .arg(format!("--app=file:///{}", dashboard_path.replace('\\', "/")))
            .arg("--window-size=1220,860")
            .spawn();
    } else {
        let _ = Command::new("cmd")
            .args(["/c", "start", "", dashboard_path])
            .spawn();
    }
}
