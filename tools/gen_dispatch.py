# Regenerate src-tauri/src/dispatch.rs from the commands in commands.rs.
# Run from the repo root: python3 -I tools/gen_dispatch.py
import re, os
root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "src-tauri", "src")
s = open(os.path.join(root, "commands.rs")).read()
pat = re.compile(r'#\[cfg_attr\(feature = "tauri-app", tauri::command\)\]\s*\n(?:\s*#\[[^\]]*\]\s*\n)*\s*pub (async )?fn (\w+)\((.*?)\)\s*(?:->\s*(.*?))?\s*\{', re.S)
out = []
for m in pat.finditer(s):
    asy, name, params, ret = m.group(1), m.group(2), m.group(3), (m.group(4) or "()").strip()
    args, lets = [], []
    for p in [x.strip() for x in re.split(r',(?![^<]*>)', params) if x.strip()]:
        n, t = p.split(":", 1)
        n = n.replace("mut ", "").strip(); t = t.strip()
        if t.startswith("S<"):
            args.append("st"); continue
        camel = re.sub(r'_(\w)', lambda m: m.group(1).upper(), n)
        lets.append(f'            let {n}: {t} = arg(&a, "{camel}")?;')
        args.append(n)
    call = f'commands::{name}({", ".join(args)})' + (".await" if asy else "")
    conv = f"out({call}.map_err(|e| e.to_string())?)" if ret.startswith(("AppResult", "Result")) else f"out({call})"
    out.append(f'        "{name}" => {{\n' + "\n".join(lets) + ("\n" if lets else "") + f'            {conv}\n        }}')
code = '''//! The same commands the Svelte UI calls, by name with JSON arguments, for
//! the GPUI app. Generated from commands.rs (tools/gen_dispatch.py).
#![allow(unused_mut, clippy::all)]

use crate::addons::CatalogRef;
use crate::commands;
use crate::player::PlayRequest;
use crate::settings::Settings;
use serde::de::DeserializeOwned;
use serde_json::Value;

fn arg<T: DeserializeOwned>(a: &Value, k: &str) -> Result<T, String> {
    serde_json::from_value(a.get(k).cloned().unwrap_or(Value::Null)).map_err(|e| format!("argument {k}: {e}"))
}
fn out<T: serde::Serialize>(v: T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

/// Run a command: `invoke("meta_get", json!({ "kind": "movie", "id": "tt.." }))`.
pub async fn invoke(cmd: &str, a: Value) -> Result<Value, String> {
    let st = crate::state::get();
    match cmd {
''' + "\n".join(out) + '''
        _ => Err(format!("unknown command {cmd}")),
    }
}
'''
open(os.path.join(root, "dispatch.rs"), "w").write(code)
print(len(out), "commands")
