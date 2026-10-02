# Builds and publishes MOTIX when Claude leaves an update (ADR-036). Runs every minute from
# the "MOTIX build server" scheduled task; does nothing unless '.motix-build-ready' exists.

$ErrorActionPreference = 'Stop'
$private = Join-Path $env:LOCALAPPDATA 'MOTIX-build'
$cfg = Get-Content -Raw -LiteralPath (Join-Path $private 'config.json') | ConvertFrom-Json
$ready = Join-Path $cfg.source '.motix-build-ready'
if (-not (Test-Path -LiteralPath $ready)) { exit 0 }

$editing = Split-Path -Parent $cfg.source
$log = Join-Path $editing 'MOTIX build log.txt'
$status = Join-Path $editing 'MOTIX build status.txt'
$notes = Join-Path $private 'notes.txt'
Move-Item -Force -LiteralPath $ready -Destination $notes

$base = (Select-String -LiteralPath (Join-Path $cfg.source 'Cargo.toml') -Pattern '^version = "(.+)"' |
    Select-Object -First 1).Matches[0].Groups[1].Value
$version = "$base-server.$(Get-Date -Format 'yyyyMMddHHmm')"
"Building MOTIX $version (started $(Get-Date))..." | Set-Content -LiteralPath $status

$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
if (-not (Test-Path $cargo)) { $cargo = 'cargo.exe' }
$env:MOTIX_VERSION = $version
$env:CARGO_TARGET_DIR = Join-Path $private 'target'
$out = Join-Path $private 'out.txt'
$err = Join-Path $private 'err.txt'

function Run([string]$exe, [string[]]$arguments) {
    # Quote every argument: the folders have spaces in their names.
    $quoted = ($arguments | ForEach-Object { '"' + $_ + '"' }) -join ' '
    $p = Start-Process -FilePath $exe -ArgumentList $quoted -WorkingDirectory $cfg.source -NoNewWindow -Wait `
        -PassThru -RedirectStandardOutput $out -RedirectStandardError $err
    Get-Content -LiteralPath $out, $err | Add-Content -LiteralPath $log
    return $p.ExitCode
}

"=== MOTIX $version - $(Get-Date)" | Set-Content -LiteralPath $log
try {
    if ((Run $cargo @('build', '--release', '--locked', '-p', 'motix', '-p', 'xtask')) -ne 0) {
        throw 'the build failed'
    }
    $release = Join-Path $env:CARGO_TARGET_DIR 'release'
    $publish = @('publish', $version, (Join-Path $release 'motix.exe'), (Join-Path $cfg.source 'LICENSE'),
        $cfg.key, $cfg.publish, $notes)
    if ((Run (Join-Path $release 'xtask.exe') $publish) -ne 0) { throw 'publishing failed' }
    "Published MOTIX $version to $($cfg.publish) ($(Get-Date))" | Set-Content -LiteralPath $status
} catch {
    "FAILED: MOTIX $version - $($_.Exception.Message) ($(Get-Date)). See 'MOTIX build log.txt'." |
        Set-Content -LiteralPath $status
}
