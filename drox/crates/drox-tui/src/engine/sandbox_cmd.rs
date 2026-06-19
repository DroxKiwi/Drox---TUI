//! `/sandbox` — état sandbox bash (informatif pour drox-tui local).

use super::EngineRuntime;

/// Lignes d'état sandbox / permissions bash.
#[must_use]
pub fn format_sandbox_lines(runtime: &EngineRuntime) -> Vec<String> {
    let mut lines = vec![
        "Sandbox bash (isolation noyau)".into(),
        "  drox-tui : sandbox kernel **non actif** sur cette plateforme.".into(),
        "  Les commandes `bash` passent par le moteur de permissions (Ask/Allow/Deny).".into(),
        format!(
            "  Mode permission courant : {}",
            runtime.permission_mode().short_title()
        ),
        format!(
            "  Écritures disque (--apply) : {}",
            if runtime.apply_fs_writes {
                "activées"
            } else {
                "simulation (dry-run)"
            }
        ),
    ];
    let settings = runtime.workspace.join(".drox/settings.json");
    if settings.is_file() {
        lines.push(format!("  Réglages : {settings}"));
    } else {
        lines.push("  Astuce : `.drox/settings.json` pour règles permission persistantes.".into());
    }
    lines.push(
        "  Leak `SandboxManager` (Linux/macOS/WSL) — hors scope TUI Windows natif.".into(),
    );
    lines
}
