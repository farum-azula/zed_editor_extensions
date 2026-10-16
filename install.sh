#!/bin/sh
# Installs every Zed extension in this repository that Zed does not have yet. An extension is any
# top-level folder with an `extension.toml`; its `id` is read from there, so a new extension needs
# no change to this script.
#
#     ./install.sh
#
# It does what `zed: install dev extension` does: builds the extension's Rust code into
# `extension.wasm` and its tree-sitter grammars into `grammars/<name>.wasm`, then links the folder
# into Zed's installed extensions as a dev extension. A running Zed picks it up right away.
#
# Building needs `cargo` (with `rustup`, the wasm32-wasip2 target is added) and `git`; grammars are
# compiled with the wasi-sdk Zed uses, downloaded into Zed's cache unless WASI_SDK_PATH is set.
# ZED_EXTENSIONS_DIR overrides Zed's extensions directory.

set -eu

WASI_SDK_VERSION=34

root=$(cd "$(dirname "$0")" && pwd)
case $(uname -s) in
    Darwin) data="$HOME/Library/Application Support/Zed" ;;
    *) data="${XDG_DATA_HOME:-$HOME/.local/share}/zed" ;;
esac
extensions="${ZED_EXTENSIONS_DIR:-$data/extensions}"
installed="$extensions/installed"
clang=

say() {
    printf '%s\n' "$*" >&2
}

# toml_value FILE KEY [SECTION]: a string value, at the top level or in [SECTION].
toml_value() {
    awk -v key="$2" -v want="${3:-}" '
        /^[ \t]*\[/ { section = $0; gsub(/^[ \t]*\[|\][ \t]*$/, "", section); next }
        section == want && $1 == key { sub(/^[^=]*=[ \t]*"/, ""); sub(/".*$/, ""); print; exit }
    ' "$1"
}

grammars() {
    sed -n 's/^[ \t]*\[grammars\.\([A-Za-z0-9_]*\)\][ \t]*$/\1/p' "$1"
}

# The wasi-sdk clang, the same version Zed downloads into the same place.
wasi_clang() {
    if [ -n "${WASI_SDK_PATH:-}" ] && [ -x "$WASI_SDK_PATH/bin/clang" ]; then
        printf '%s\n' "$WASI_SDK_PATH/bin/clang"
        return
    fi
    sdk="$extensions/build/wasi-sdk"
    if [ "$(head -n 1 "$sdk/VERSION" 2>/dev/null)" != "$WASI_SDK_VERSION.0" ] || [ ! -x "$sdk/bin/clang" ]; then
        case $(uname -s)-$(uname -m) in
            Darwin-arm64) platform=arm64-macos ;;
            Darwin-x86_64) platform=x86_64-macos ;;
            Linux-x86_64) platform=x86_64-linux ;;
            Linux-aarch64 | Linux-arm64) platform=arm64-linux ;;
            *) say "wasi-sdk is not available for $(uname -s) $(uname -m)"; exit 1 ;;
        esac
        say "downloading wasi-sdk $WASI_SDK_VERSION"
        tmp=$(mktemp -d)
        curl -fsSL "https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-$WASI_SDK_VERSION/wasi-sdk-$WASI_SDK_VERSION.0-$platform.tar.gz" |
            tar -xzf - -C "$tmp"
        rm -rf "$sdk"
        mkdir -p "$extensions/build"
        mv "$tmp"/* "$sdk"
        rmdir "$tmp"
    fi
    printf '%s\n' "$sdk/bin/clang"
}

build_rust() {
    dir=$1
    package=$(toml_value "$dir/Cargo.toml" name package | tr - _)
    say "  building extension.wasm"
    if command -v rustup >/dev/null; then
        rustup target add wasm32-wasip2 >/dev/null 2>&1 || true
    fi
    RUSTC_WRAPPER= cargo build --quiet --release --target wasm32-wasip2 \
        --manifest-path "$dir/Cargo.toml" --target-dir "$dir/target"
    cp "$dir/target/wasm32-wasip2/release/$package.wasm" "$dir/extension.wasm"
}

build_grammar() {
    dir=$1
    name=$2
    manifest="$dir/extension.toml"
    repository=$(toml_value "$manifest" repository "grammars.$name")
    rev=$(toml_value "$manifest" rev "grammars.$name")
    [ -n "$rev" ] || rev=$(toml_value "$manifest" commit "grammars.$name")
    path=$(toml_value "$manifest" path "grammars.$name")
    checkout="$dir/grammars/$name"
    say "  building grammars/$name.wasm"
    if [ ! -d "$checkout/.git" ]; then
        mkdir -p "$checkout"
        git -C "$checkout" init -q
        git -C "$checkout" remote add origin "$repository"
    fi
    git -C "$checkout" fetch -q --depth 1 origin "$rev"
    git -C "$checkout" -c advice.detachedHead=false checkout -q "$rev"
    src="$checkout${path:+/$path}/src"
    [ -n "$clang" ] || clang=$(wasi_clang)
    set -- -fPIC -shared -Os "-Wl,--export=tree_sitter_$name" -o "$dir/grammars/$name.wasm" -I "$src" "$src/parser.c"
    [ ! -f "$src/scanner.c" ] || set -- "$@" "$src/scanner.c"
    "$clang" "$@"
}

for manifest in "$root"/*/extension.toml; do
    dir=${manifest%/extension.toml}
    id=$(toml_value "$manifest" id)
    dest="$installed/$id"
    # A link left behind by a moved or deleted clone.
    if [ -L "$dest" ] && [ ! -e "$dest" ]; then
        rm "$dest"
    fi
    if [ -e "$dest" ]; then
        say "$id: already installed"
        continue
    fi
    say "$id: installing from ${dir#"$root"/}"
    if [ -f "$dir/Cargo.toml" ]; then
        build_rust "$dir"
    fi
    for grammar in $(grammars "$manifest"); do
        build_grammar "$dir" "$grammar"
    done
    mkdir -p "$installed"
    ln -s "$dir" "$dest"
    say "$id: installed"
done
