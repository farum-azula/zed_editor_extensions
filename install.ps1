# Installs every Zed extension in this repository that Zed does not have yet. An extension is any
# top-level folder with an `extension.toml`; its `id` is read from there, so a new extension needs
# no change to this script.
#
#     powershell -ExecutionPolicy Bypass -File install.ps1
#
# The Windows counterpart of install.sh (it runs under PowerShell 7 on macOS and Linux as well).
# It does what `zed: install dev extension` does: builds the extension's Rust code into
# `extension.wasm` and its tree-sitter grammars into `grammars/<name>.wasm`, then links the folder
# into Zed's installed extensions as a dev extension (a junction on Windows, as Zed makes). A
# running Zed picks it up right away.
#
# Building needs `cargo` (with `rustup`, the wasm32-wasip2 target is added) and `git`; grammars are
# compiled with the wasi-sdk Zed uses, downloaded into Zed's cache unless WASI_SDK_PATH is set.
# ZED_EXTENSIONS_DIR overrides Zed's extensions directory.

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$WasiSdkVersion = '34'

$onWindows = $PSVersionTable.PSEdition -eq 'Desktop' -or $IsWindows
if ($PSVersionTable.PSEdition -eq 'Desktop') {
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
}

$root = $PSScriptRoot
if ($env:ZED_EXTENSIONS_DIR) {
    $extensions = $env:ZED_EXTENSIONS_DIR
} elseif ($onWindows) {
    $extensions = Join-Path $env:LOCALAPPDATA 'Zed\extensions'
} elseif ($IsMacOS) {
    $extensions = Join-Path $HOME 'Library/Application Support/Zed/extensions'
} else {
    $data = if ($env:XDG_DATA_HOME) { $env:XDG_DATA_HOME } else { Join-Path $HOME '.local/share' }
    $extensions = Join-Path $data 'zed/extensions'
}
$installed = Join-Path $extensions 'installed'
$script:clang = $null

function Say($text) {
    [Console]::Error.WriteLine($text)
}

# Runs a native command and fails when it does.
function Run {
    $exe, $rest = $args
    & $exe @rest
    if ($LASTEXITCODE -ne 0) {
        throw "$exe exited with code $LASTEXITCODE"
    }
}

# A string value, at the top level or in [section].
function Get-TomlValue($file, $key, $section = '') {
    $current = ''
    foreach ($line in Get-Content -LiteralPath $file) {
        if ($line -match '^\s*\[([^\]]*)\]\s*$') {
            $current = $Matches[1]
        } elseif ($current -eq $section -and $line -match "^\s*$([regex]::Escape($key))\s*=\s*`"([^`"]*)`"") {
            return $Matches[1]
        }
    }
    return ''
}

function Get-Grammars($file) {
    foreach ($line in Get-Content -LiteralPath $file) {
        if ($line -match '^\s*\[grammars\.([A-Za-z0-9_]+)\]\s*$') {
            $Matches[1]
        }
    }
}

# The wasi-sdk clang, the same version Zed downloads into the same place.
function Get-WasiClang {
    $exe = if ($onWindows) { 'clang.exe' } else { 'clang' }
    if ($env:WASI_SDK_PATH -and (Test-Path -LiteralPath (Join-Path $env:WASI_SDK_PATH "bin/$exe"))) {
        return Join-Path $env:WASI_SDK_PATH "bin/$exe"
    }
    $sdk = Join-Path $extensions 'build/wasi-sdk'
    $clang = Join-Path $sdk "bin/$exe"
    $version = Join-Path $sdk 'VERSION'
    $current = if (Test-Path -LiteralPath $version) { Get-Content -LiteralPath $version -TotalCount 1 }
    if ($current -ne "$WasiSdkVersion.0" -or -not (Test-Path -LiteralPath $clang)) {
        $arch = if ($onWindows) { $env:PROCESSOR_ARCHITECTURE } else { "$([Runtime.InteropServices.RuntimeInformation]::OSArchitecture)" }
        $cpu = switch -regex ($arch) {
            '^(AMD64|X64)$' { 'x86_64' }
            '^(ARM64|Arm64)$' { 'arm64' }
            default { throw "wasi-sdk is not available for $arch" }
        }
        $os = if ($onWindows) { 'windows' } elseif ($IsMacOS) { 'macos' } else { 'linux' }
        Say "downloading wasi-sdk $WasiSdkVersion"
        $tmp = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
        New-Item -ItemType Directory -Path $tmp | Out-Null
        $archive = Join-Path $tmp 'wasi-sdk.tar.gz'
        Invoke-WebRequest -UseBasicParsing -OutFile $archive `
            "https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-$WasiSdkVersion/wasi-sdk-$WasiSdkVersion.0-$cpu-$os.tar.gz"
        Run tar -xzf $archive -C $tmp
        Remove-Item -LiteralPath $archive
        if (Test-Path -LiteralPath $sdk) {
            Remove-Item -LiteralPath $sdk -Recurse -Force
        }
        New-Item -ItemType Directory -Force -Path (Join-Path $extensions 'build') | Out-Null
        Move-Item -LiteralPath (Get-ChildItem -LiteralPath $tmp | Select-Object -First 1).FullName -Destination $sdk
        Remove-Item -LiteralPath $tmp -Recurse -Force
    }
    return $clang
}

function Build-Rust($dir) {
    $package = (Get-TomlValue (Join-Path $dir 'Cargo.toml') 'name' 'package') -replace '-', '_'
    Say '  building extension.wasm'
    if (Get-Command rustup -ErrorAction SilentlyContinue) {
        & { $ErrorActionPreference = 'Continue'; rustup target add wasm32-wasip2 2>&1 | Out-Null }
    }
    $wrapper = $env:RUSTC_WRAPPER
    $env:RUSTC_WRAPPER = ''
    try {
        Run cargo build --quiet --release --target wasm32-wasip2 `
            --manifest-path (Join-Path $dir 'Cargo.toml') --target-dir (Join-Path $dir 'target')
    } finally {
        $env:RUSTC_WRAPPER = $wrapper
    }
    Copy-Item -LiteralPath (Join-Path $dir "target/wasm32-wasip2/release/$package.wasm") `
        -Destination (Join-Path $dir 'extension.wasm')
}

function Build-Grammar($dir, $name) {
    $manifest = Join-Path $dir 'extension.toml'
    $repository = Get-TomlValue $manifest 'repository' "grammars.$name"
    $rev = Get-TomlValue $manifest 'rev' "grammars.$name"
    if (-not $rev) {
        $rev = Get-TomlValue $manifest 'commit' "grammars.$name"
    }
    $path = Get-TomlValue $manifest 'path' "grammars.$name"
    $checkout = Join-Path $dir "grammars/$name"
    Say "  building grammars/$name.wasm"
    if (-not (Test-Path -LiteralPath (Join-Path $checkout '.git'))) {
        New-Item -ItemType Directory -Force -Path $checkout | Out-Null
        Run git -C $checkout init -q
        Run git -C $checkout remote add origin $repository
    }
    Run git -C $checkout fetch -q --depth 1 origin $rev
    Run git -C $checkout -c advice.detachedHead=false checkout -q $rev
    $src = if ($path) { Join-Path (Join-Path $checkout $path) 'src' } else { Join-Path $checkout 'src' }
    if (-not $script:clang) {
        $script:clang = Get-WasiClang
    }
    $sources = @(Join-Path $src 'parser.c')
    if (Test-Path -LiteralPath (Join-Path $src 'scanner.c')) {
        $sources += Join-Path $src 'scanner.c'
    }
    Run $script:clang -fPIC -shared -Os "-Wl,--export=tree_sitter_$name" `
        -o (Join-Path $dir "grammars/$name.wasm") -I $src @sources
}

foreach ($manifest in Get-ChildItem -Path (Join-Path $root '*/extension.toml')) {
    $dir = $manifest.Directory.FullName
    $id = Get-TomlValue $manifest.FullName 'id'
    $dest = Join-Path $installed $id
    # A link left behind by a moved or deleted clone. `Test-Path` does not tell one from a live link;
    # `Directory.Exists` follows it.
    $link = Get-Item -LiteralPath $dest -Force -ErrorAction SilentlyContinue
    if ($link -and $link.LinkType -and -not [IO.Directory]::Exists($dest)) {
        $link.Delete()
    }
    if (Test-Path -LiteralPath $dest) {
        Say "${id}: already installed"
        continue
    }
    Say "${id}: installing from $($manifest.Directory.Name)"
    if (Test-Path -LiteralPath (Join-Path $dir 'Cargo.toml')) {
        Build-Rust $dir
    }
    foreach ($grammar in Get-Grammars $manifest.FullName) {
        Build-Grammar $dir $grammar
    }
    New-Item -ItemType Directory -Force -Path $installed | Out-Null
    $type = if ($onWindows) { 'Junction' } else { 'SymbolicLink' }
    New-Item -ItemType $type -Path $dest -Target $dir | Out-Null
    Say "${id}: installed"
}
