"""Builds the release artifact: one self-contained HTML file that opens from
disk (file://) with no network access.

    python3 scripts/build.py                        # -> dist/private-key-convertor.html,
                                                    #    dist/demo.html, dist/site/
    python3 scripts/build.py --verify-reproducible  # build twice, compare bytes

Steps: compile pkc-wasm with the pinned toolchain (rust-toolchain.toml) and
the `html` profile; run wasm-bindgen, pinned to the version in Cargo.lock;
then inline everything into web/index.html (the wasm as base64, the
wasm-bindgen glue and app.js as one inline module script) and replace the
page's development CSP with a strict one: default-src 'none', the script
and the stylesheet allowed only by hash.

Reproducible per platform: the same commit gives byte-identical output on
any machine of the same host platform (checked: two Linux x86-64
environments agree; macOS ARM, Linux ARM and Linux x86-64 each differ, since
the host feeds into Cargo's crate hashes). The reference build is therefore
scripts/build-reproducible.sh, which runs this script in one pinned Linux
x86-64 container. Paths that would otherwise leak into the wasm (the
checkout, cargo's home directory, the toolchain's rust-src) are remapped to
fixed names. No wasm-opt: its output varies by
version, and the size it saves doesn't justify another tool to trust.

Every text transformation below asserts the exact shape it expects, so a
change in wasm-bindgen's output or in the page fails the build instead of
silently producing a broken file. Standard library only.
"""
import argparse
import base64
import hashlib
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
WEB = ROOT / "web"
TOOLS = ROOT / "target" / "tools"
DIST = ROOT / "dist"
SITE = ROOT / "site"
RELEASE_NAME = "private-key-convertor.html"

APP_IMPORT = re.compile(r'^import init, \{ ([\w, ]+) \} from "\./pkg/pkc_wasm\.js";\n', re.M)
GLUE_EXPORT_TAIL = "export { initSync, __wbg_init as default };\n"
DEV_ONLY = re.compile(r"[ \t]*// BEGIN dev-only.*?// END dev-only\n", re.S)
SCRIPT_TAG = '<script type="module" src="./app.js"></script>'
CSP_META = re.compile(r'<meta http-equiv="Content-Security-Policy" content="[^"]*">')
# Anchored to line starts, so a "<style>" mentioned in a comment can't match.
STYLE_BLOCK = re.compile(r"^<style>(.*?)^</style>", re.S | re.M)


def fail(message):
    sys.exit(f"build.py: {message}")


def run(cmd, **kwargs):
    subprocess.run(cmd, check=True, cwd=ROOT, **kwargs)


def locked_version(crate):
    lock = (ROOT / "Cargo.lock").read_text()
    m = re.search(rf'\[\[package\]\]\nname = "{re.escape(crate)}"\nversion = "([^"]+)"', lock)
    if not m:
        fail(f"{crate} not found in Cargo.lock")
    return m.group(1)


def ensure_wasm_bindgen():
    version = locked_version("wasm-bindgen")
    binary = TOOLS / "bin" / "wasm-bindgen"
    if binary.exists():
        out = subprocess.run([binary, "--version"], capture_output=True, text=True).stdout.split()
        if out[-1:] == [version]:
            return binary
    run(["cargo", "install", "--quiet", "wasm-bindgen-cli", "--version", version, "--locked", "--root", str(TOOLS)])
    return binary


def rustc_info():
    sysroot = subprocess.run(["rustc", "--print", "sysroot"], capture_output=True, text=True, check=True, cwd=ROOT).stdout.strip()
    verbose = subprocess.run(["rustc", "-vV"], capture_output=True, text=True, check=True, cwd=ROOT).stdout
    commit = re.search(r"^commit-hash: (\w+)$", verbose, re.M)
    if not commit:
        fail("can't read rustc's commit hash")
    return Path(sysroot), commit.group(1)


def build_wasm(target_dir):
    cargo_home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo")).resolve()
    sysroot, commit = rustc_info()
    # With the rust-src component installed, rustc swaps the canonical
    # /rustc/<commit>/ paths of std for the local copy's: which names the
    # machine and differs per host. Map them back.
    rust_src = sysroot / "lib" / "rustlib" / "src" / "rust"
    rustflags = " ".join([
        f"--remap-path-prefix={cargo_home}=/cargo",
        f"--remap-path-prefix={ROOT}=/build",
        f"--remap-path-prefix={rust_src}=/rustc/{commit}",
    ])
    env = dict(os.environ, RUSTFLAGS=rustflags)
    env.pop("CARGO_ENCODED_RUSTFLAGS", None)
    run(
        ["cargo", "build", "--quiet", "--locked", "--profile", "html", "--target", "wasm32-unknown-unknown",
         "-p", "pkc-wasm", "--target-dir", str(target_dir)],
        env=env,
    )
    return target_dir / "wasm32-unknown-unknown" / "html" / "pkc_wasm.wasm"


def bindgen(wasm_bindgen, wasm, out_dir):
    if out_dir.exists():
        shutil.rmtree(out_dir)
    run([str(wasm_bindgen), "--target", "web", "--no-typescript", "--out-dir", str(out_dir), str(wasm)])
    return (out_dir / "pkc_wasm.js").read_text(), (out_dir / "pkc_wasm_bg.wasm").read_bytes()


def wrap_glue(glue):
    """Turn wasm-bindgen's ES module into a plain scope returning its exports:
    two modules can't import each other inside one HTML file, and the glue's
    own exported `init` (our panic-hook start function) would collide with
    the name app.js uses for the loader."""
    if glue.count(GLUE_EXPORT_TAIL) != 1 or not glue.endswith(GLUE_EXPORT_TAIL):
        fail("wasm-bindgen glue no longer ends with the expected export list")
    body = glue[: -len(GLUE_EXPORT_TAIL)]
    functions = re.findall(r"^export function (\w+)\(", body, re.M)
    body = re.sub(r"^export (class|function) ", r"\1 ", body, flags=re.M)
    if re.search(r"^export\b", body, re.M):
        fail("wasm-bindgen glue has an export this script doesn't handle")
    exports = ", ".join(functions)
    return f"const wasmBindings = (() => {{\n{body}\nreturn {{ {exports}, load: __wbg_init }};\n}})();\n"


def transform_app(app):
    m = APP_IMPORT.search(app)
    if not m or len(APP_IMPORT.findall(app)) != 1:
        fail("app.js no longer starts with the expected pkc_wasm import")
    names = m.group(1)
    seam = (
        f"const {{ {names} }} = wasmBindings;\n"
        "const init = () => wasmBindings.load({ module_or_path: wasmBytes });\n"
    )
    app = app[: m.start()] + seam + app[m.end():]
    if len(DEV_ONLY.findall(app)) != 1:
        fail("app.js should have exactly one dev-only block")
    return DEV_ONLY.sub("", app)


def csp_hash(text):
    return "'sha256-" + base64.b64encode(hashlib.sha256(text.encode()).digest()).decode() + "'"


DEMO_SWITCH = "let DEMO_MODE = false;\n"


def assemble(html, glue, app, wasm_bytes, demo=False):
    if app.count(DEMO_SWITCH) != 1:
        fail("app.js should have exactly one DEMO_MODE switch")
    if demo:
        app = app.replace(DEMO_SWITCH, "let DEMO_MODE = true;\n")
    wasm_b64 = base64.b64encode(wasm_bytes).decode()
    script = (
        "\n"
        f'const wasmBytes = Uint8Array.from(atob("{wasm_b64}"), (c) => c.charCodeAt(0));\n'
        + wrap_glue(glue)
        + transform_app(app)
    )
    if html.count(SCRIPT_TAG) != 1:
        fail("index.html should load app.js with exactly one module script tag")
    html = html.replace(SCRIPT_TAG, f'<script type="module">{script}</script>')

    styles = STYLE_BLOCK.findall(html)
    if len(styles) != 1:
        fail("index.html should have exactly one <style> block")
    if re.search(r"\sstyle=\"", html):
        fail("index.html has an inline style= attribute, which the release CSP blocks")
    csp = (
        "default-src 'none'; "
        f"script-src {csp_hash(script)} 'wasm-unsafe-eval'; "
        f"style-src {csp_hash(styles[0])}; "
        "base-uri 'none'; form-action 'none'"
    )
    if len(CSP_META.findall(html)) != 1:
        fail("index.html should have exactly one CSP meta tag")
    return CSP_META.sub(lambda _: f'<meta http-equiv="Content-Security-Policy" content="{csp}">', html)


def landing_page(html):
    """The static landing page: no script at all, its stylesheet allowed by
    hash, same as the tool."""
    styles = STYLE_BLOCK.findall(html)
    if len(styles) != 1:
        fail("site/index.html should have exactly one <style> block")
    if re.search(r"<script|\sstyle=\"", html):
        fail("site/index.html must have no script and no style= attribute")
    csp = f"default-src 'none'; style-src {csp_hash(styles[0])}; base-uri 'none'; form-action 'none'"
    if len(CSP_META.findall(html)) != 1:
        fail("site/index.html should have exactly one CSP meta tag")
    return CSP_META.sub(lambda _: f'<meta http-equiv="Content-Security-Policy" content="{csp}">', html)


def build(target_dir):
    """Every output, as {path under dist/: bytes}: the release file, the
    demo (the same page with DEMO_MODE on), and the website."""
    wasm = build_wasm(target_dir)
    glue, wasm_bytes = bindgen(ensure_wasm_bindgen(), wasm, target_dir / "bindgen")
    page, app = (WEB / "index.html").read_text(), (WEB / "app.js").read_text()
    release = assemble(page, glue, app, wasm_bytes).encode()
    demo = assemble(page, glue, app, wasm_bytes, demo=True).encode()
    outputs = {RELEASE_NAME: release, "demo.html": demo}
    if (SITE / "index.html").exists():
        outputs["site/index.html"] = landing_page((SITE / "index.html").read_text()).encode()
        outputs["site/demo.html"] = demo
    return outputs


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--verify-reproducible", action="store_true",
                        help="build a second time from a clean target directory and compare the bytes")
    args = parser.parse_args()

    first = build(ROOT / "target" / "html")
    for name, data in first.items():
        path = DIST / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        print(f"{hashlib.sha256(data).hexdigest()}  {path.relative_to(ROOT)} ({len(data)} bytes)")

    if args.verify_reproducible:
        clean = ROOT / "target" / "html-verify"
        if clean.exists():
            shutil.rmtree(clean)
        second = build(clean)
        if second != first:
            changed = sorted(name for name in first if first[name] != second.get(name))
            fail(f"not reproducible: {', '.join(changed)} differ on a clean second build")
        print("reproducible: a clean second build is byte-identical")


if __name__ == "__main__":
    main()
