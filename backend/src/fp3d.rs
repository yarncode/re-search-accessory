//! Resolve a KiCad footprint (`Lib:Name`) to its 3D model entries.
//!
//! A BOM only carries the footprint name. To preview its 3D we resolve the
//! library through `fp-lib-table`, read the `.kicad_mod`, and return every
//! `(model ...)` entry with its path, transform and file bytes.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use serde::Serialize;

const MAX_MODELS: usize = 8;
const MAX_TOTAL_BYTES: u64 = 40 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct ModelEntry {
    pub path: String,
    pub exists: bool,
    pub size: u64,
    pub format: String,
    pub offset: [f64; 3],
    pub scale: [f64; 3],
    pub rotate: [f64; 3],
    pub data: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Resolved {
    pub footprint: String,
    pub library: String,
    pub found: bool,
    pub footprint_path: Option<String>,
    pub models: Vec<ModelEntry>,
    pub error: Option<String>,
    pub searched: Vec<String>,
}

fn kicad_share_dir() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        candidates.push(
            PathBuf::from(local)
                .join("Programs")
                .join("KiCad")
                .join("10.0")
                .join("share")
                .join("kicad"),
        );
    }

    if let Ok(program_files) = std::env::var("ProgramFiles") {
        candidates.push(
            PathBuf::from(program_files)
                .join("KiCad")
                .join("10.0")
                .join("share")
                .join("kicad"),
        );
    }

    // Linux packages ("kicad" or the versioned data dir) and macOS bundles.
    candidates.push(PathBuf::from("/usr/share/kicad"));
    candidates.push(PathBuf::from("/usr/local/share/kicad"));
    candidates.push(PathBuf::from("/opt/homebrew/share/kicad"));
    candidates.push(PathBuf::from(
        "/Applications/KiCad/KiCad.app/Contents/SharedSupport",
    ));

    candidates.into_iter().find(|dir| dir.is_dir())
}

/// Per-user KiCad configuration dir (`kicad_common.json`, `fp-lib-table`):
/// `%APPDATA%\kicad\10.0` on Windows, `$XDG_CONFIG_HOME/kicad/10.0` (or
/// `~/.config/kicad/10.0`) on Linux, `~/Library/Application Support/kicad/10.0`
/// on macOS.
fn user_config_dir() -> Option<PathBuf> {
    if let Ok(appdata) = std::env::var("APPDATA") {
        return Some(PathBuf::from(appdata).join("kicad").join("10.0"));
    }

    if let Ok(config) = std::env::var("XDG_CONFIG_HOME") {
        if !config.trim().is_empty() {
            return Some(PathBuf::from(config).join("kicad").join("10.0"));
        }
    }

    let home = std::env::var("HOME").ok()?;

    if cfg!(target_os = "macos") {
        Some(
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("kicad")
                .join("10.0"),
        )
    }
    else {
        Some(PathBuf::from(home).join(".config").join("kicad").join("10.0"))
    }
}

fn user_vars() -> HashMap<String, String> {
    let mut vars = HashMap::new();
    let Some(config_dir) = user_config_dir() else {
        return vars;
    };
    let path = config_dir.join("kicad_common.json");

    let Ok(text) = std::fs::read_to_string(&path) else {
        return vars;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return vars;
    };

    if let Some(entries) = json.pointer("/environment/vars").and_then(|v| v.as_object()) {
        for (key, value) in entries {
            if let Some(value) = value.as_str() {
                vars.insert(key.clone(), value.to_string());
            }
        }
    }

    vars
}

fn global_vars() -> &'static HashMap<String, String> {
    static VARS: OnceLock<HashMap<String, String>> = OnceLock::new();

    VARS.get_or_init(|| {
        let mut vars = user_vars();

        if let Some(share) = kicad_share_dir() {
            let defaults = [
                ("KICAD10_FOOTPRINT_DIR", share.join("footprints")),
                ("KICAD10_3DMODEL_DIR", share.join("3dmodels")),
                ("KICAD10_SYMBOL_DIR", share.join("symbols")),
                ("KICAD10_TEMPLATE_DIR", share.join("template")),
            ];

            for (key, value) in defaults {
                vars.entry(key.to_string())
                    .or_insert_with(|| value.to_string_lossy().replace('\\', "/"));
            }
        }

        // Environment overrides win over the JSON file (same as KiCad).
        for (key, value) in std::env::vars() {
            if key.starts_with("KICAD") {
                vars.insert(key, value);
            }
        }

        vars
    })
}

fn expand(value: &str, vars: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;

    while let Some(position) = rest.find("${") {
        out.push_str(&rest[..position]);

        let after = &rest[position + 2..];

        if let Some(end) = after.find('}') {
            let name = &after[..end];

            match vars.get(name) {
                Some(replacement) => out.push_str(replacement),
                None => {
                    out.push_str("${");
                    out.push_str(name);
                    out.push('}');
                }
            }

            rest = &after[end + 1..];
        }
        else {
            out.push_str(&rest[position..]);
            rest = "";
        }
    }

    out.push_str(rest);
    out
}

fn read_text(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

fn quoted_after(block: &str, key: &str) -> Option<String> {
    let start = block.find(key)?;
    let rest = &block[start + key.len()..];
    let first_quote = rest.find('"')?;
    let after = &rest[first_quote + 1..];
    let end = after.find('"')?;

    Some(after[..end].to_string())
}

fn numbers_after(block: &str, key: &str) -> Option<[f64; 3]> {
    let start = block.find(key)?;
    let rest = &block[start + key.len()..];
    let open = rest.find('(')?;
    let close = rest[open..].find(')')?;
    let inner = &rest[open + 1..open + close];
    let numbers: Vec<f64> = inner
        .split_whitespace()
        .filter_map(|token| token.parse::<f64>().ok())
        .collect();

    if numbers.len() >= 3 {
        Some([numbers[0], numbers[1], numbers[2]])
    }
    else {
        None
    }
}

/// Split a parenthesised block starting at `start` (which must point at `(`).
fn block_at(text: &str, start: usize) -> Option<&str> {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut index = start;
    let mut in_string = false;

    while index < bytes.len() {
        let byte = bytes[index];

        if in_string {
            if byte == b'"' {
                in_string = false;
            }
        }
        else {
            match byte {
                b'"' => in_string = true,
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(&text[start..=index]);
                    }
                }
                _ => {}
            }
        }

        index += 1;
    }

    None
}

pub fn parse_models(text: &str) -> Vec<(String, [f64; 3], [f64; 3], [f64; 3])> {
    let mut models = Vec::new();
    let mut cursor = 0;

    while let Some(offset) = text[cursor..].find("(model") {
        let start = cursor + offset;
        let Some(block) = block_at(text, start) else {
            break;
        };

        if let Some(path) = quoted_after(block, "(model") {
            models.push((
                path,
                numbers_after(block, "(offset").unwrap_or([0.0, 0.0, 0.0]),
                numbers_after(block, "(scale").unwrap_or([1.0, 1.0, 1.0]),
                numbers_after(block, "(rotate").unwrap_or([0.0, 0.0, 0.0]),
            ));
        }

        cursor = start + block.len();
    }

    models
}

fn parse_lib_table(text: &str) -> HashMap<String, String> {
    let mut libs = HashMap::new();
    let mut cursor = 0;

    while let Some(offset) = text[cursor..].find("(lib") {
        let start = cursor + offset;
        let Some(block) = block_at(text, start) else {
            break;
        };

        if let (Some(name), Some(uri)) = (quoted_after(block, "(name"), quoted_after(block, "(uri")) {
            libs.insert(name, uri);
        }

        cursor = start + block.len();
    }

    libs
}

fn library_table_paths(project: Option<&str>) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(config_dir) = user_config_dir() {
        paths.push(config_dir.join("fp-lib-table"));
    }

    if let Some(project) = project {
        let candidate = PathBuf::from(project).join("fp-lib-table");

        if candidate.is_file() {
            paths.push(candidate);
        }
    }

    paths
}

fn model_format(path: &str) -> String {
    let lower = path.to_lowercase();

    if lower.ends_with(".step") || lower.ends_with(".stp") {
        "step".to_string()
    }
    else if lower.ends_with(".wrl") || lower.ends_with(".vrml") {
        "wrl".to_string()
    }
    else {
        "unknown".to_string()
    }
}

fn base64(bytes: &[u8]) -> String {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    STANDARD.encode(bytes)
}

pub fn resolve(footprint: &str, project: Option<&str>) -> Resolved {
    let footprint = footprint.trim().to_string();
    let mut searched = Vec::new();

    let (library, name) = match footprint.split_once(':') {
        Some((library, name)) if !library.is_empty() && !name.is_empty() => {
            (library.to_string(), name.to_string())
        }
        _ => {
            return Resolved {
                footprint,
                library: String::new(),
                found: false,
                footprint_path: None,
                models: Vec::new(),
                error: Some("Footprint phải có dạng Lib:Name".to_string()),
                searched,
            };
        }
    };

    let mut vars = global_vars().clone();

    if let Some(project) = project {
        vars.insert("KIPRJMOD".to_string(), project.replace('\\', "/"));
    }

    let mut libs: HashMap<String, String> = HashMap::new();

    for path in library_table_paths(project) {
        if let Some(text) = read_text(&path) {
            for (key, value) in parse_lib_table(&text) {
                libs.entry(key).or_insert(value);
            }
        }
    }

    let Some(uri) = libs.get(&library) else {
        searched.push(format!(
            "Không thấy thư viện `{library}` trong fp-lib-table (global hoặc project)."
        ));

        return Resolved {
            footprint,
            library,
            found: false,
            footprint_path: None,
            models: Vec::new(),
            error: Some("Không tìm thấy thư viện footprint".to_string()),
            searched,
        };
    };

    let base = expand(uri, &vars);
    let footprint_path = PathBuf::from(&base).join(format!("{name}.kicad_mod"));

    searched.push(footprint_path.display().to_string());

    let Some(text) = read_text(&footprint_path) else {
        return Resolved {
            footprint,
            library,
            found: false,
            footprint_path: Some(footprint_path.display().to_string()),
            models: Vec::new(),
            error: Some("Không đọc được file .kicad_mod".to_string()),
            searched,
        };
    };

    let mut models = Vec::new();
    let mut total = 0u64;

    for (raw_path, offset, scale, rotate) in parse_models(&text).into_iter().take(MAX_MODELS) {
        let resolved = expand(&raw_path, &vars);
        let path = PathBuf::from(&resolved);
        let exists = path.is_file();
        let size = if exists {
            std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0)
        }
        else {
            0
        };

        let mut data = None;

        if exists && total + size <= MAX_TOTAL_BYTES {
            if let Ok(bytes) = std::fs::read(&path) {
                total += bytes.len() as u64;
                data = Some(base64(&bytes));
            }
        }

        models.push(ModelEntry {
            path: resolved,
            exists,
            size,
            format: model_format(&raw_path),
            offset,
            scale,
            rotate,
            data,
        });
    }

    Resolved {
        footprint,
        library,
        found: true,
        footprint_path: Some(footprint_path.display().to_string()),
        models,
        error: None,
        searched,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_models_with_transform() {
        let text = r#"
        (footprint "Test" (version 20240108)
          (model "${KICAD10_3DMODEL_DIR}/Package_SO.3dshapes/SOIC-8.step"
            (offset (xyz 1 2 3))
            (scale (xyz 1 1 1))
            (rotate (xyz 0 0 90))
          )
          (model "${DATALOGGER_LIB}/x.wrl")
        )
        "#;

        let models = parse_models(text);

        assert_eq!(models.len(), 2);
        assert_eq!(models[0].0, "${KICAD10_3DMODEL_DIR}/Package_SO.3dshapes/SOIC-8.step");
        assert_eq!(models[0].1, [1.0, 2.0, 3.0]);
        assert_eq!(models[0].3, [0.0, 0.0, 90.0]);
        assert_eq!(models[1].1, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn parses_lib_table_entries() {
        let text = r#"(fp_lib_table
          (lib (name "Package_SO") (type "KiCad") (uri "${KICAD10_FOOTPRINT_DIR}/Package_SO.pretty") (options ""))
          (lib (name "datalogger_lib") (type "KiCad") (uri "${DATALOGGER_LIB}/datalogger_lib.pretty") (options ""))
        )"#;

        let libs = parse_lib_table(text);

        assert_eq!(libs.get("Package_SO").unwrap(), "${KICAD10_FOOTPRINT_DIR}/Package_SO.pretty");
        assert_eq!(libs.get("datalogger_lib").unwrap(), "${DATALOGGER_LIB}/datalogger_lib.pretty");
    }

    #[test]
    fn expands_known_vars_only() {
        let mut vars = HashMap::new();

        vars.insert("A".to_string(), "/a".to_string());

        assert_eq!(expand("${A}/x", &vars), "/a/x");
        assert_eq!(expand("${B}/x", &vars), "${B}/x");
    }
}
