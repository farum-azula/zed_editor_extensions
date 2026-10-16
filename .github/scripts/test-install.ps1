# Tests install.sh / install.ps1 against a scratch Zed extensions directory:
#
#     pwsh .github/scripts/test-install.ps1 -Installer sh|pwsh|powershell
#
# 1. A first run installs every extension of the repository: each is linked into `installed/<id>`
#    (a junction on Windows), and what it builds is valid WebAssembly of the kind Zed loads.
# 2. A second run changes nothing.
# 3. A link left dangling by a moved clone is replaced.
#
# `wasm-tools`, when in PATH, also validates the built modules.

param(
    [Parameter(Mandatory)] [ValidateSet('sh', 'pwsh', 'powershell')] [string] $Installer
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$extensions = Join-Path ([IO.Path]::GetTempPath()) "zed-extensions-$([IO.Path]::GetRandomFileName())"
$installed = Join-Path $extensions 'installed'
$onWindows = $PSVersionTable.PSEdition -eq 'Desktop' -or $IsWindows
$script:failures = 0

function Check($ok, $what) {
    if ($ok) {
        Write-Host "ok    $what"
    } else {
        Write-Host "FAIL  $what"
        $script:failures++
    }
}

function Install {
    $env:ZED_EXTENSIONS_DIR = $extensions
    $output = switch ($Installer) {
        'sh' { & sh (Join-Path $root 'install.sh') 2>&1 }
        'pwsh' { & pwsh -NoProfile -File (Join-Path $root 'install.ps1') 2>&1 }
        'powershell' { & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $root 'install.ps1') 2>&1 }
    }
    $code = $LASTEXITCODE
    $output = @($output | ForEach-Object { "$_" })
    $output | ForEach-Object { Write-Host "      | $_" }
    if ($code -ne 0) {
        throw "the installer exited with code $code"
    }
    return $output
}

function TomlValue($file, $key, $section = '') {
    $current = ''
    foreach ($line in Get-Content -LiteralPath $file) {
        if ($line -match '^\s*\[([^\]]*)\]\s*$') {
            $current = $Matches[1]
        } elseif ($current -eq $section -and $line -match "^\s*$([regex]::Escape($key))\s*=\s*`"([^`"]*)`"") {
            return $Matches[1]
        }
    }
}

# Checks a built `.wasm`: its header (`component` or core `module`), a marker it must contain, and
# what `wasm-tools validate` says.
function CheckWasm($path, $kind, $marker) {
    $name = $path.Substring($root.Length + 1)
    if (-not (Test-Path -LiteralPath $path)) {
        Check $false "$name exists"
        return
    }
    $bytes = [IO.File]::ReadAllBytes($path)
    $header = ($bytes[0..7] | ForEach-Object { $_.ToString('x2') }) -join ''
    $expected = if ($kind -eq 'component') { '0061736d0d000100' } else { '0061736d01000000' }
    Check ($header -eq $expected) "$name is a wasm $kind"
    Check ([Text.Encoding]::ASCII.GetString($bytes).Contains($marker)) "$name contains $marker"
    if (Get-Command wasm-tools -ErrorAction SilentlyContinue) {
        & wasm-tools validate --features all $path
        Check ($LASTEXITCODE -eq 0) "$name passes wasm-tools validate"
    }
}

$manifests = @(Get-ChildItem -Path (Join-Path $root '*/extension.toml'))
Check ($manifests.Count -gt 0) "the repository has extensions"

try {
    Write-Host "== first run ($Installer)"
    $first = Install
    foreach ($manifest in $manifests) {
        $dir = $manifest.Directory.FullName
        $id = TomlValue $manifest.FullName 'id'
        $dest = Join-Path $installed $id
        Check ($first -contains "${id}: installed") "$id reported as installed"
        $link = Get-Item -LiteralPath $dest -Force -ErrorAction SilentlyContinue
        $type = if ($onWindows) { 'Junction' } else { 'SymbolicLink' }
        Check ($link -and $link.LinkType -eq $type) "installed/$id is a $type"
        $linked = Join-Path $dest 'extension.toml'
        Check ((Test-Path -LiteralPath $linked) -and
            (Get-FileHash -LiteralPath $linked).Hash -eq (Get-FileHash -LiteralPath $manifest.FullName).Hash) `
            "installed/$id leads to $($manifest.Directory.Name)"
        if (Test-Path -LiteralPath (Join-Path $dir 'Cargo.toml')) {
            CheckWasm (Join-Path $dir 'extension.wasm') 'component' 'zed:api-version'
        }
        foreach ($line in Get-Content -LiteralPath $manifest.FullName) {
            if ($line -match '^\s*\[grammars\.([A-Za-z0-9_]+)\]\s*$') {
                $grammar = $Matches[1]
                CheckWasm (Join-Path $dir "grammars/$grammar.wasm") 'module' "tree_sitter_$grammar"
            }
        }
    }

    Write-Host "== second run"
    $second = Install
    foreach ($manifest in $manifests) {
        $id = TomlValue $manifest.FullName 'id'
        Check ($second -contains "${id}: already installed") "$id is left alone"
    }
    Check (-not ($second -match 'building')) "nothing is rebuilt"

    Write-Host "== dangling link"
    $id = TomlValue $manifests[0].FullName 'id'
    $dest = Join-Path $installed $id
    (Get-Item -LiteralPath $dest -Force).Delete()
    $gone = Join-Path ([IO.Path]::GetTempPath()) "moved-clone-$([IO.Path]::GetRandomFileName())"
    New-Item -ItemType Directory -Path $gone | Out-Null
    $type = if ($onWindows) { 'Junction' } else { 'SymbolicLink' }
    New-Item -ItemType $type -Path $dest -Target $gone | Out-Null
    Remove-Item -LiteralPath $gone
    $third = Install
    Check ($third -contains "${id}: installed") "$id is reinstalled over a dangling link"
    Check ([IO.File]::Exists((Join-Path $dest 'extension.toml'))) "installed/$id leads to the extension again"
} finally {
    if (Test-Path -LiteralPath $installed) {
        # Links only, never what they lead to.
        Get-ChildItem -LiteralPath $installed -Force | ForEach-Object { $_.Delete() }
    }
    if (Test-Path -LiteralPath $extensions) {
        Remove-Item -LiteralPath $extensions -Recurse -Force
    }
}

if ($script:failures -gt 0) {
    Write-Host "$($script:failures) check(s) failed"
    exit 1
}
Write-Host "all checks passed"
