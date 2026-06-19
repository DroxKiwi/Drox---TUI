//! Classification heuristique d’une sous-commande Bash (premier mot exécutable).

use std::sync::LazyLock;

use regex::Regex;

/// Catégorie grossière pour l’UI / logs (pas encore branchée sur deny automatique).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BashCommandKind {
    /// Lecture ou introspection (`ls`, `cat`, `git status`, …).
    ReadOnly,
    /// Modification locale probable (`touch`, `chmod`, `npm install`, …).
    Mutating,
    /// Accès réseau (`curl`, `wget`, `ssh`, …).
    Network,
    /// Opération à risque élevé (`rm -rf`, `dd`, …) — voir aussi [`destructive_hint`].
    Destructive,
    /// Non classé.
    Unknown,
}

static ENV_ASSIGN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*=").expect("valid regex"));

/// Retire les préfixes `VAR=value` « sûrs » en tête (même heuristique minimale que le TS).
#[must_use]
pub fn first_executable_token(segment: &str) -> Option<&str> {
    let mut rest = segment.trim();
    while let Some(tok) = rest.split_whitespace().next() {
        if ENV_ASSIGN.is_match(tok) {
            rest = rest[tok.len()..].trim_start();
            continue;
        }
        return Some(tok);
    }
    None
}

/// Classe une sous-commande à partir de son premier token.
#[must_use]
pub fn kind_of_segment(segment: &str) -> BashCommandKind {
    let Some(first) = first_executable_token(segment) else {
        return BashCommandKind::Unknown;
    };
    let base = first
        .rsplit_once('/')
        .map_or(first, |(_, name)| name)
        .to_ascii_lowercase();

    if READ_ONLY.contains(&base.as_str()) {
        return BashCommandKind::ReadOnly;
    }
    if NETWORK.contains(&base.as_str()) {
        return BashCommandKind::Network;
    }
    if DESTRUCTIVE_PREFIX.contains(&base.as_str()) {
        return BashCommandKind::Destructive;
    }
    if MUTATING.contains(&base.as_str()) {
        return BashCommandKind::Mutating;
    }

    // `git` : sous-commandes read vs write
    if base == "git" {
        return classify_git(segment);
    }

    BashCommandKind::Unknown
}

fn classify_git(segment: &str) -> BashCommandKind {
    let mut it = segment.split_whitespace();
    let _git = it.next();
    let Some(sub) = it.next() else {
        return BashCommandKind::ReadOnly;
    };
    let sub = sub.to_ascii_lowercase();
    if GIT_READ.contains(&sub.as_str()) {
        BashCommandKind::ReadOnly
    } else if GIT_MUTATING.contains(&sub.as_str()) {
        BashCommandKind::Mutating
    } else {
        BashCommandKind::Unknown
    }
}

/// Motifs destructifs (sous-ensemble porté depuis `destructiveCommandWarning.ts`).
#[must_use]
pub fn destructive_hint(segment: &str) -> Option<&'static str> {
    let s = segment;
    for (re, msg) in DESTRUCTIVE_PATTERNS.iter() {
        if re.is_match(s) {
            return Some(*msg);
        }
    }
    None
}

/// Drapeaux permissions pour une sous-commande (utilisés par `drox-engine`).
#[must_use]
pub const fn permission_flags(kind: BashCommandKind) -> (bool, bool) {
    match kind {
        BashCommandKind::ReadOnly => (false, true),
        BashCommandKind::Mutating | BashCommandKind::Network | BashCommandKind::Destructive => {
            (true, false)
        }
        BashCommandKind::Unknown => (true, false),
    }
}

/// Message de refus automatique pour motifs à haut risque (§2.27).
///
/// Distinct de [`destructive_hint`] informatif : ici on **bloque** en mode défaut
/// si la politique ne contient pas déjà une règle `Allow` explicite.
#[must_use]
pub fn auto_deny_message(segment: &str) -> Option<&'static str> {
    destructive_hint(segment).or_else(|| {
        let kind = kind_of_segment(segment);
        if kind == BashCommandKind::Destructive {
            Some("destructive command (rm/dd/mkfs/shred or similar)")
        } else {
            None
        }
    })
}

static READ_ONLY: &[&str] = &[
    "ls", "dir", "pwd", "echo", "printf", "cat", "tac", "head", "tail", "wc", "sort", "uniq",
    "less", "more", "file", "stat", "whoami", "id", "uname", "date", "true", "false", "test", "[",
    "which", "whereis", "type", "command", "help", "man", "basename", "dirname", "realpath",
    "readlink", "find",
];

static NETWORK: &[&str] = &[
    "curl",
    "wget",
    "ssh",
    "scp",
    "rsync",
    "nc",
    "netcat",
    "telnet",
    "dig",
    "nslookup",
    "host",
    "ping",
    "traceroute",
    "tracepath",
    "ftp",
    "sftp",
];

static MUTATING: &[&str] = &[
    "touch",
    "mkdir",
    "mv",
    "cp",
    "ln",
    "chmod",
    "chown",
    "chgrp",
    "install",
    "tee",
    "sed",
    "awk",
    "perl",
    "python",
    "python3",
    "node",
    "npm",
    "pnpm",
    "yarn",
    "cargo",
    "make",
    "cmake",
    "ninja",
    "rustc",
    "gcc",
    "g++",
    "clang",
    "clang++",
    "strip",
    "ar",
    "tar",
    "zip",
    "unzip",
    "gzip",
    "gunzip",
    "bzip2",
    "xz",
    "docker",
    "kubectl",
    "terraform",
    "ansible",
    "helm",
];

static DESTRUCTIVE_PREFIX: &[&str] = &["rm", "dd", "mkfs", "shred"];

static GIT_READ: &[&str] = &[
    "status",
    "diff",
    "log",
    "show",
    "branch",
    "tag",
    "remote",
    "fetch",
    "grep",
    "rev-parse",
    "describe",
    "ls-files",
    "ls-tree",
    "config",
    "help",
    "version",
];

static GIT_MUTATING: &[&str] = &[
    "add",
    "commit",
    "push",
    "pull",
    "merge",
    "rebase",
    "cherry-pick",
    "stash",
    "checkout",
    "switch",
    "restore",
    "reset",
    "clean",
    "mv",
    "rm",
];

// git push --force etc. : voir `DESTRUCTIVE_PATTERNS`

static DESTRUCTIVE_PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    vec![
        (
            Regex::new(r"\bgit\s+reset\s+--hard\b").expect("regex"),
            "may discard uncommitted changes",
        ),
        (
            Regex::new(r"\bgit\s+push\b[^;&|\n]*[ \t](--force|--force-with-lease|-f)\b")
                .expect("regex"),
            "may overwrite remote history",
        ),
        (
            Regex::new(r"\bgit\s+clean\b[^\n;|&]*-[a-zA-Z]*f").expect("regex"),
            "may permanently delete untracked files",
        ),
        (
            Regex::new(r"(^|[;&|\n]\s*)rm\s+-[a-zA-Z]*[rR][a-zA-Z]*f").expect("regex"),
            "may recursively force-remove files",
        ),
        (
            Regex::new(r"(^|[;&|\n]\s*)rm\s+-[a-zA-Z]*f[a-zA-Z]*[rR]").expect("regex"),
            "may recursively force-remove files",
        ),
        (
            Regex::new(r"(?i)\b(DROP|TRUNCATE)\s+(TABLE|DATABASE|SCHEMA)\b").expect("regex"),
            "may drop or truncate database objects",
        ),
        (
            Regex::new(r"\bkubectl\s+delete\b").expect("regex"),
            "may delete Kubernetes resources",
        ),
        (
            Regex::new(r"\bterraform\s+destroy\b").expect("regex"),
            "may destroy Terraform infrastructure",
        ),
        (
            Regex::new(r"\bgit\s+checkout\s+(--\s+)?\.[ \t]*($|[;&|\n])").expect("regex"),
            "may discard all working tree changes",
        ),
        (
            Regex::new(r"\bgit\s+restore\s+(--\s+)?\.[ \t]*($|[;&|\n])").expect("regex"),
            "may discard all working tree changes",
        ),
        (
            Regex::new(r"\bgit\s+stash[ \t]+(drop|clear)\b").expect("regex"),
            "may permanently remove stashed changes",
        ),
    ]
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_token_skips_env_assign() {
        assert_eq!(
            first_executable_token("NODE_ENV=prod npm install"),
            Some("npm")
        );
    }

    #[test]
    fn ls_is_read_only() {
        assert_eq!(kind_of_segment("ls -la"), BashCommandKind::ReadOnly);
    }

    #[test]
    fn curl_is_network() {
        assert_eq!(kind_of_segment("curl https://x"), BashCommandKind::Network);
    }

    #[test]
    fn rm_is_destructive_prefix() {
        assert_eq!(kind_of_segment("rm -f a"), BashCommandKind::Destructive);
    }

    #[test]
    fn git_status_read() {
        assert_eq!(kind_of_segment("git status"), BashCommandKind::ReadOnly);
    }

    #[test]
    fn destructive_hint_git_reset() {
        assert!(destructive_hint("git reset --hard").is_some());
    }

    #[test]
    fn permission_flags_read_only() {
        assert_eq!(
            permission_flags(BashCommandKind::ReadOnly),
            (false, true)
        );
    }

    #[test]
    fn auto_deny_rm_rf() {
        assert!(auto_deny_message("rm -rf /tmp/x").is_some());
    }

    #[test]
    fn auto_deny_skips_ls() {
        assert!(auto_deny_message("ls -la").is_none());
    }
}
