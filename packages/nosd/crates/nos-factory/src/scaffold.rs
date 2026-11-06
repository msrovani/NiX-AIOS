//! Fábrica de apps **reais**: gera um projecto Rust ou Python executável, com
//! `flake.nix` incluído (Nix-native), constrói-o e corre-o.
//!
//! Honestidade: sem toolchain, o resultado é `engine_absent` — nunca um
//! "sucesso" fictício. O `placeholder` da fábrica wasm continua separado.

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lang {
    Rust,
    Python,
}

impl Lang {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "rust" | "rs" | "cargo" => Some(Self::Rust),
            "python" | "py" | "python3" => Some(Self::Python),
            _ => None,
        }
    }

    /// Linguagem pedida no intent; default Rust (o AIOS é Rust-first).
    pub fn detect(intent: &str) -> Self {
        // C5.3: `Self::parse(&s)` recebia a frase inteira e nunca casava (o
        // parse espera um token). Procurar o token no texto; o primeiro
        // explicito ganha, e sem nenhum o default continua a ser Rust.
        let s = intent.to_lowercase();
        let toks: Vec<&str> = s
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .filter(|t| !t.is_empty())
            .collect();
        let python_at = toks
            .iter()
            .position(|t| matches!(*t, "python" | "python3" | "py"));
        let rust_at = toks
            .iter()
            .position(|t| matches!(*t, "rust" | "rustc" | "cargo" | "rs"));
        match (python_at, rust_at) {
            (Some(p), Some(r)) => {
                if p < r {
                    Self::Python
                } else {
                    Self::Rust
                }
            }
            (Some(_), None) => Self::Python,
            _ => Self::Rust,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaffoldFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Toolchain {
    pub tool: String,
    pub available: bool,
    pub path: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaffoldPlan {
    pub name: String,
    pub lang: Lang,
    pub title: String,
    pub files: Vec<ScaffoldFile>,
    /// BLAKE3 dos ficheiros (path+conteúdo ordenados) — commitment do projecto.
    pub content_id: String,
    pub toolchain: Toolchain,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunOutcome {
    /// `ok` | `build_failed` | `run_failed` | `engine_absent` | `timeout`
    pub status: String,
    pub exit: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub ms: u64,
    pub tool: Option<String>,
    pub note: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ScaffoldError {
    #[error("nome de app inválido")]
    BadName,
    #[error("índice: {0}")]
    Io(String),
    #[error("app já existe: {0}")]
    Exists(String),
}

/// Nome seguro a partir do intent: `[a-z0-9-]`, ≤32 chars, prefixo `app-`
/// **sem duplicar** quando o intent já começa por "app" (`app-rust-…`, não
/// `app-app-rust-…`).
pub fn app_name(intent: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = true;
    for c in intent.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
        if slug.len() >= 28 {
            break;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.len() < 3 {
        return "app-nixaios".to_string();
    }
    if slug.starts_with("app") {
        slug
    } else {
        format!("app-{slug}")
    }
}

fn content_id(files: &[ScaffoldFile]) -> String {
    let mut sorted: Vec<&ScaffoldFile> = files.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let mut h = blake3::Hasher::new();
    for f in sorted {
        h.update(f.path.as_bytes());
        h.update(b"\0");
        h.update(f.content.as_bytes());
        h.update(b"\0");
    }
    h.finalize().to_hex().to_string()
}

fn rust_main(name: &str) -> String {
    // Template literal + substituição: evita a dupla-escapagem de `{` entre o
    // `format!` da fábrica e o `println!` do código gerado (já partiu um build).
    const TPL: &str = r#"//! `__APP__` — app gerada pela fábrica NixAIOS (sem dependências).
//! Contrato: imprime uma linha JSON em stdout e sai 0.

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_else(|| "hello".to_string());
    let suffix = if arg.is_empty() { "n/a" } else { arg.as_str() };
    println!(
        "{{\"app\":\"__APP__\",\"lang\":\"rust\",\"ok\":true,\"arg\":\"{}\"}}",
        suffix
    );
}
"#;
    TPL.replace("__APP__", name)
}

fn rust_cargo(name: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]

[[bin]]
name = "{name}"
path = "main.rs"
"#
    )
}

fn python_app(name: &str) -> String {
    format!(
        r#"#!/usr/bin/env python3
"""`{name}` — app gerada pela fábrica NixAIOS.

Contrato: imprime uma linha JSON em stdout e sai 0.
"""
import json
import sys


def main(argv: list[str]) -> int:
    arg = argv[1] if len(argv) > 1 else "hello"
    sys.stdout.write(
        json.dumps(
            {{"app": "{name}", "lang": "python", "ok": True, "arg": arg, "py": sys.version.split()[0]}},
            ensure_ascii=False,
        )
        + "\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
"#
    )
}

fn flake(name: &str, lang: Lang) -> String {
    match lang {
        // Build directo por `rustc`: sem Cargo.lock não há `buildRustPackage`,
        // e uma flake que não constrói é pior que nenhuma.
        Lang::Rust => format!(
            r#"{{
  description = "{name} — app gerada (fábrica NixAIOS)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = {{ self, nixpkgs }}:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (s: f nixpkgs.legacyPackages.${{s}});
    in {{
      packages = forAll (pkgs: {{
        default = pkgs.stdenv.mkDerivation {{
          pname = "{name}";
          version = "0.1.0";
          src = ./.;
          nativeBuildInputs = [ pkgs.rustc ];
          buildPhase = ''
            rustc -O main.rs -o app
          '';
          installPhase = ''
            install -Dm755 app $out/bin/{name}
          '';
        }};
      }});
    }};
}}
"#
        ),
        Lang::Python => format!(
            r#"{{
  description = "{name} — app gerada (fábrica NixAIOS)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = {{ self, nixpkgs }}:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (s: f nixpkgs.legacyPackages.${{s}});
    in {{
      packages = forAll (pkgs: {{
        default = pkgs.writeShellApplication {{
          name = "{name}";
          runtimeInputs = [ pkgs.python3 ];
          text = ''
            exec python3 ${{./app.py}} "$@"
          '';
        }};
      }});
    }};
}}
"#
        ),
    }
}

fn readme(name: &str, lang: Lang) -> String {
    let (run, build) = match lang {
        Lang::Rust => (
            "cargo run -- \"hello\"   # ou: rustc -O main.rs -o app && ./app",
            "nix build .#default",
        ),
        Lang::Python => ("python3 app.py \"hello\"", "nix build .#default"),
    };
    format!(
        "# {name}\n\nApp gerada pela fábrica NixAIOS (lang={}).\n\n```bash\n{run}\n{build}\n```\n\nContrato: uma linha JSON em stdout, exit 0.\n",
        lang.as_str()
    )
}

/// Candidatos de binário por plataforma (regra 35: só caminhos absolutos).
fn which(tool: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        format!("{tool}.exe")
    } else {
        tool.to_string()
    };
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        if !dir.is_absolute() {
            continue;
        }
        let cand = dir.join(&exe);
        if cand.is_file() {
            return Some(cand);
        }
    }
    // Fallbacks absolutos conhecidos (NixOS/profile), nunca relativos.
    for base in ["/run/current-system/sw/bin", "/usr/bin", "/usr/local/bin"] {
        let cand = Path::new(base).join(&exe);
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

fn tool_version(cmd: &Path) -> Option<String> {
    let out = std::process::Command::new(cmd)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let line = s.lines().next().unwrap_or("").trim();
    if line.is_empty() {
        None
    } else {
        Some(line.chars().take(48).collect())
    }
}

/// Toolchain que vai construir/correr a app desta linguagem.
pub fn toolchain(lang: Lang) -> Toolchain {
    let candidates: &[&str] = match lang {
        Lang::Rust => &["cargo", "rustc"],
        Lang::Python => &["python3", "python"],
    };
    for t in candidates {
        if let Some(p) = which(t) {
            return Toolchain {
                tool: (*t).to_string(),
                available: true,
                version: tool_version(&p),
                path: Some(p.to_string_lossy().to_string()),
            };
        }
    }
    Toolchain {
        tool: candidates.first().copied().unwrap_or("python3").to_string(),
        available: false,
        path: None,
        version: None,
    }
}

/// Plano determinístico: mesmo intent + lang ⇒ mesmos ficheiros + content_id.
pub fn plan(intent: &str, lang: Option<Lang>) -> ScaffoldPlan {
    let lang = lang.unwrap_or_else(|| Lang::detect(intent));
    let name = app_name(intent);
    let title = intent.trim().chars().take(120).collect::<String>();
    let files = match lang {
        Lang::Rust => vec![
            ScaffoldFile {
                path: "main.rs".into(),
                content: rust_main(&name),
            },
            ScaffoldFile {
                path: "Cargo.toml".into(),
                content: rust_cargo(&name),
            },
            ScaffoldFile {
                path: "flake.nix".into(),
                content: flake(&name, lang),
            },
            ScaffoldFile {
                path: "README.md".into(),
                content: readme(&name, lang),
            },
        ],
        Lang::Python => vec![
            ScaffoldFile {
                path: "app.py".into(),
                content: python_app(&name),
            },
            ScaffoldFile {
                path: "flake.nix".into(),
                content: flake(&name, lang),
            },
            ScaffoldFile {
                path: "README.md".into(),
                content: readme(&name, lang),
            },
        ],
    };
    let tc = toolchain(lang);
    let note = if tc.available {
        format!(
            "{} ficheiros; toolchain {} ({})",
            files.len(),
            tc.tool,
            tc.version.as_deref().unwrap_or("versão desconhecida")
        )
    } else {
        format!(
            "{} ficheiros; toolchain ausente ({}) — o plano é válido, o build honestamente falhará",
            files.len(),
            tc.tool
        )
    };
    ScaffoldPlan {
        content_id: content_id(&files),
        name,
        lang,
        title,
        files,
        toolchain: tc,
        note,
    }
}

/// Escreve o projecto em `root/<name>` (recusa symlinks e nomes estranhos).
pub fn write(plan: &ScaffoldPlan, root: &Path) -> Result<PathBuf, ScaffoldError> {
    if plan.name.is_empty()
        || !plan
            .name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(ScaffoldError::BadName);
    }
    std::fs::create_dir_all(root).map_err(|e| ScaffoldError::Io(e.to_string()))?;
    let dir = root.join(&plan.name);
    if let Ok(md) = std::fs::symlink_metadata(&dir) {
        if md.file_type().is_symlink() {
            return Err(ScaffoldError::Exists(format!(
                "{} é symlink (recusado)",
                dir.display()
            )));
        }
    }
    // C5.2: reescrever por cima apagava trabalho do utilizador sem aviso. Se o
    // destino ja tem ficheiros, recusar — quem chamou tem de pedir overwrite.
    if dir.is_dir() {
        let existing = std::fs::read_dir(&dir)
            .map(|rd| rd.flatten().count())
            .unwrap_or(0);
        if existing > 0 {
            return Err(ScaffoldError::Exists(format!(
                "{} ja tem {existing} ficheiro(s) (recusado sem overwrite)",
                dir.display()
            )));
        }
    }
    std::fs::create_dir_all(&dir).map_err(|e| ScaffoldError::Io(e.to_string()))?;
    for f in &plan.files {
        // Sem `..` nem absolutos: o plano é server-authored, mas não confiamos.
        if f.path.contains("..") || Path::new(&f.path).is_absolute() {
            return Err(ScaffoldError::BadName);
        }
        let dest = dir.join(&f.path);
        let mut fh = std::fs::File::create(&dest).map_err(|e| ScaffoldError::Io(e.to_string()))?;
        fh.write_all(f.content.as_bytes())
            .map_err(|e| ScaffoldError::Io(e.to_string()))?;
        fh.sync_all().ok();
    }
    Ok(dir)
}

fn run_cmd(dir: &Path, tool: &Path, args: &[&str], limit: Duration) -> RunOutcome {
    let start = Instant::now();
    let mut child = match std::process::Command::new(tool)
        .args(args)
        .current_dir(dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return RunOutcome {
                status: "engine_absent".into(),
                exit: None,
                stdout: String::new(),
                stderr: e.to_string(),
                ms: start.elapsed().as_millis() as u64,
                tool: Some(tool.to_string_lossy().to_string()),
                note: "spawn falhou".into(),
            }
        }
    };
    let mut timed_out = false;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if start.elapsed() > limit {
                    timed_out = true;
                    let _ = child.kill();
                    let _ = child.wait();
                    break;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(_) => break,
        }
    }
    let mut out = String::new();
    let mut err = String::new();
    if let Some(mut s) = child.stdout.take() {
        let mut b = Vec::new();
        let _ = s.read_to_end(&mut b);
        out = String::from_utf8_lossy(&b).to_string();
    }
    if let Some(mut s) = child.stderr.take() {
        let mut b = Vec::new();
        let _ = s.read_to_end(&mut b);
        err = String::from_utf8_lossy(&b).to_string();
    }
    const CAP: usize = 4096;
    truncate(&mut out, CAP);
    truncate(&mut err, CAP);
    let code = child.try_wait().ok().flatten().and_then(|s| s.code());
    let status = if timed_out {
        "timeout"
    } else if code == Some(0) {
        "ok"
    } else {
        "run_failed"
    };
    RunOutcome {
        status: status.to_string(),
        exit: code,
        stdout: out,
        stderr: err,
        ms: start.elapsed().as_millis() as u64,
        tool: Some(tool.to_string_lossy().to_string()),
        note: if timed_out {
            "prazo excedido; processo morto".into()
        } else {
            String::new()
        },
    }
}

fn truncate(s: &mut String, cap: usize) {
    if s.len() > cap {
        let cut = s
            .char_indices()
            .map(|(i, _)| i)
            .take_while(|i| *i <= cap)
            .last()
            .unwrap_or(0);
        s.truncate(cut);
        s.push_str("…[truncado]");
    }
}

/// Constrói e corre a app. Rust: `cargo run` se houver cargo, senão
/// `rustc -O`; Python: `python3 app.py`.
pub fn build_and_run(plan: &ScaffoldPlan, dir: &Path) -> RunOutcome {
    const LIMIT: Duration = Duration::from_secs(120);
    // C5.1: a app nasce com `flake.nix`; construir pelo PATH ignorava-o. Se o
    // nix responde, construir a derivacao e correr `out/bin/<name>`.
    if let Some(outcome) = nix_build_outcome(plan, dir, LIMIT) {
        return outcome;
    }
    match plan.lang {
        Lang::Rust => {
            let arg = "hello";
            if let Some(cargo) = which("cargo") {
                let mut o = run_cmd(dir, &cargo, &["run", "--quiet", "--", arg], LIMIT);
                if o.status != "ok" {
                    o.note = format!("cargo run: {}", o.note);
                }
                return o;
            }
            if let Some(rustc) = which("rustc") {
                let bin = if cfg!(windows) { "app.exe" } else { "app" };
                let build = run_cmd(dir, &rustc, &["-O", "main.rs", "-o", bin], LIMIT);
                if build.status != "ok" {
                    return RunOutcome {
                        status: "build_failed".into(),
                        note: format!("rustc: {}", first_line(&build.stderr)),
                        ..build
                    };
                }
                let exe = dir.join(bin);
                return run_cmd(dir, &exe, &[arg], LIMIT);
            }
            absent("rustc/cargo")
        }
        Lang::Python => match which("python3").or_else(|| which("python")) {
            Some(py) => run_cmd(dir, &py, &["app.py", "hello"], Duration::from_secs(30)),
            None => absent("python3"),
        },
    }
}

/// C5.1: `nix build --no-link --print-out-paths` sobre o `flake.nix` gerado.
/// `None` = nix ausente ou daemon sem resposta (cai para o PATH, declarado).
/// Sem `trusted-users`, o `nosd` nao escreve no store: `nix store ping` falha
/// e o caminho antigo fica — honesto, nao um `ok` inventado.
fn nix_build_outcome(plan: &ScaffoldPlan, dir: &Path, limit: Duration) -> Option<RunOutcome> {
    let nix = which("nix")?;
    let ping = run_cmd(dir, &nix, &["store", "ping"], Duration::from_secs(20));
    if ping.status != "ok" {
        return None;
    }
    let build = run_cmd(
        dir,
        &nix,
        &["build", "--no-link", "--print-out-paths"],
        limit,
    );
    if build.status != "ok" {
        return Some(RunOutcome {
            status: "build_failed".into(),
            note: format!("nix build: {}", first_line(&build.stderr)),
            ..build
        });
    }
    let out_path = build
        .stdout
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string();
    let bin = Path::new(&out_path).join("bin").join(&plan.name);
    if out_path.is_empty() || !bin.is_file() {
        return Some(RunOutcome {
            status: "build_failed".into(),
            note: format!("nix out sem bin/{}: {out_path}", plan.name),
            ..build
        });
    }
    let mut o = run_cmd(dir, &bin, &["hello"], limit);
    o.note = format!("nix:{out_path}");
    Some(o)
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").chars().take(160).collect()
}

fn absent(tool: &str) -> RunOutcome {
    RunOutcome {
        status: "engine_absent".into(),
        exit: None,
        stdout: String::new(),
        stderr: String::new(),
        ms: 0,
        tool: Some(tool.to_string()),
        note: format!("{tool} não está no PATH (EngineAbsent, não sucesso)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn plan_is_deterministic_and_has_flake() {
        let a = plan("gerar app rust de saudação", None);
        let b = plan("gerar app rust de saudação", None);
        assert_eq!(a.content_id, b.content_id);
        assert_eq!(a.lang, Lang::Rust);
        assert!(a.files.iter().any(|f| f.path == "flake.nix"), "Nix-native");
        assert!(a.files.iter().any(|f| f.path == "main.rs"));
        assert_eq!(a.content_id.len(), 64);
    }

    #[test]
    fn lang_detected_from_intent() {
        assert_eq!(Lang::detect("uma app em python"), Lang::Python);
        assert_eq!(Lang::detect("uma app em rust"), Lang::Rust);
        assert_eq!(Lang::detect("monitor de rede"), Lang::Rust);
        assert_eq!(Lang::parse("py"), Some(Lang::Python));
    }

    #[test]
    fn name_is_sanitized() {
        let n = app_name("../../etc/passwd; rm -rf /");
        assert!(n.starts_with("app-"));
        assert!(n
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert!(!n.contains(".."));
    }

    #[test]
    fn name_does_not_double_app_prefix() {
        assert_eq!(app_name("app rust de saudação"), "app-rust-de-sauda-o");
        assert_eq!(app_name("monitor de rede"), "app-monitor-de-rede");
        assert!(app_name("x").starts_with("app-"));
    }

    #[cfg(unix)]
    #[test]
    fn write_refuses_symlink_target() {
        let d = tempdir().unwrap();
        let p = plan("app symlink", Some(Lang::Python));
        let outside = d.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, d.path().join(&p.name)).unwrap();
        let e = write(&p, d.path()).unwrap_err();
        assert!(matches!(e, ScaffoldError::Exists(_)), "{e:?}");
    }

    #[test]
    fn write_then_run_python_or_absent() {
        let d = tempdir().unwrap();
        let p = plan("app de demonstração python", Some(Lang::Python));
        let dir = write(&p, d.path()).unwrap();
        assert!(dir.join("app.py").is_file());
        let out = build_and_run(&p, &dir);
        match out.status.as_str() {
            "ok" => {
                assert!(out.stdout.contains(&p.name), "{}", out.stdout);
                assert!(out.stdout.contains("\"ok\": true") || out.stdout.contains("\"ok\":true"));
            }
            "engine_absent" => assert!(out.exit.is_none(), "ausente não tem exit"),
            other => panic!("estado inesperado: {other} — {}", out.stderr),
        }
    }

    #[test]
    fn write_then_run_rust_or_absent() {
        let d = tempdir().unwrap();
        let p = plan("app de demonstração rust", Some(Lang::Rust));
        let dir = write(&p, d.path()).unwrap();
        assert!(dir.join("main.rs").is_file());
        let out = build_and_run(&p, &dir);
        match out.status.as_str() {
            "ok" => assert!(out.stdout.contains(&p.name), "{}", out.stdout),
            "engine_absent" => {}
            other => panic!("estado inesperado: {other} — {}", out.stderr),
        }
    }
}

#[cfg(test)]
mod f5_tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn detect_finds_the_language_token_in_a_phrase() {
        assert_eq!(Lang::detect("gerar app py de saudacao"), Lang::Python);
        assert_eq!(Lang::detect("quero um cargo project"), Lang::Rust);
        // O primeiro token explicito ganha.
        assert_eq!(
            Lang::detect("script em python que chama rust"),
            Lang::Python
        );
        // Sem token, o default honesto continua Rust.
        assert_eq!(Lang::detect("monitor de rede"), Lang::Rust);
    }

    #[test]
    fn write_refuses_to_overwrite_an_existing_app() {
        let d = tempdir().unwrap();
        let p = plan("app python de saudacao", Some(Lang::Python));
        let dir = write(&p, d.path()).unwrap();
        assert!(dir.join("app.py").is_file());
        let second = write(&p, d.path());
        assert!(
            matches!(second, Err(ScaffoldError::Exists(_))),
            "{second:?}"
        );
    }
}
