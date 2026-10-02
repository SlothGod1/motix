# MOTIX build server setup (ADR-036). Started by "Set up MOTIX build server.cmd".
# 1. Installs the free build tools (Rust and the Visual Studio C++ Build Tools) if missing.
# 2. Stores the update-signing key in a private folder only this Windows account can read.
# 3. Asks which folder to publish updates to.
# 4. Adds a background task that builds and publishes whenever Claude leaves an update.

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
function Say([string]$text) {
    [void][System.Windows.Forms.MessageBox]::Show($text, 'MOTIX build server')
}
function Ask([string]$text) {
    return ([System.Windows.Forms.MessageBox]::Show($text, 'MOTIX build server', 'YesNo') -eq 'Yes')
}

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$source = (Resolve-Path (Join-Path $here '..\..')).Path
$private = Join-Path $env:LOCALAPPDATA 'MOTIX-build'
New-Item -ItemType Directory -Force -Path $private | Out-Null
Write-Host "MOTIX source folder: $source"

# 1. Build tools ------------------------------------------------------------------------
$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
if (-not (Test-Path $cargo)) {
    Write-Host 'Installing Rust (free)...'
    winget install --id Rustlang.Rustup -e --accept-source-agreements --accept-package-agreements
}
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$hasVc = $false
if (Test-Path $vswhere) {
    $hasVc = [bool](& $vswhere -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath)
}
if (-not $hasVc) {
    Write-Host 'Installing the Visual Studio C++ Build Tools (free, a few GB; Windows may ask for permission)...'
    winget install --id Microsoft.VisualStudio.2022.BuildTools -e --accept-source-agreements --accept-package-agreements `
        --override '--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
}

# 2. Signing key ------------------------------------------------------------------------
$keyFile = Join-Path $private 'signing-key.txt'
if (-not (Test-Path $keyFile)) {
    Say ("Next, choose your MOTIX update-signing key file (MOTIX_UPDATE_SIGNING_KEY ... .txt).`n`n" +
        'It will be copied to a private folder on this PC that only your Windows account can read, ' +
        'and used to sign every update so MOTIX knows it is really from you.')
    $dialog = New-Object System.Windows.Forms.OpenFileDialog
    $dialog.Title = 'Choose your MOTIX signing key file'
    $dialog.Filter = 'Text files (*.txt)|*.txt|All files (*.*)|*.*'
    if ($dialog.ShowDialog() -ne 'OK') { Say 'Setup cancelled.'; exit 1 }
    $text = (Get-Content -Raw -LiteralPath $dialog.FileName).Trim()
    if ($text -notmatch '^[0-9a-fA-F]{64}$') {
        Say "That file doesn't look like a MOTIX signing key (it should be 64 letters and numbers)."
        exit 1
    }
    Set-Content -LiteralPath $keyFile -Value $text -NoNewline
    icacls $keyFile /inheritance:r /grant:r "$($env:USERDOMAIN)\$($env:USERNAME):(R,W)" | Out-Null
    if (Ask ("The key is now stored privately.`n`nDelete the copy you picked?`n$($dialog.FileName)`n`n" +
            'Only say Yes if you also keep your own backup somewhere safe (for example a USB stick).')) {
        Remove-Item -LiteralPath $dialog.FileName
    }
}

# 3. Where to publish -------------------------------------------------------------------
Say ("Now choose the folder to publish updates to.`n`n" +
    "If you shared MOTIX on your network, pick the 'updates' folder inside that shared folder. " +
    'PCs with their own copy of MOTIX can use the same folder: Help > Check for updates > ' +
    'Get updates from a folder on your network.')
$folder = New-Object System.Windows.Forms.FolderBrowserDialog
$folder.Description = 'Folder to publish MOTIX updates to'
if ($folder.ShowDialog() -ne 'OK') { Say 'Setup cancelled.'; exit 1 }
@{ source = $source; publish = $folder.SelectedPath; key = $keyFile } |
    ConvertTo-Json | Set-Content -LiteralPath (Join-Path $private 'config.json')

# 4. Background task --------------------------------------------------------------------
# conhost --headless runs PowerShell with no window at all, so nothing flashes every minute.
$script = Join-Path $here 'build.ps1'
$action = New-ScheduledTaskAction -Execute 'conhost.exe' `
    -Argument "--headless powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File `"$script`""
$trigger = New-ScheduledTaskTrigger -Once -At (Get-Date).AddMinutes(1) -RepetitionInterval (New-TimeSpan -Minutes 1)
$settings = New-ScheduledTaskSettingsSet -MultipleInstances IgnoreNew -ExecutionTimeLimit (New-TimeSpan -Hours 3) `
    -StartWhenAvailable -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
Register-ScheduledTask -TaskName 'MOTIX build server' -Action $action -Trigger $trigger -Settings $settings `
    -Description 'Builds and publishes MOTIX updates when Claude leaves one (ADR-036).' -Force | Out-Null

Say ("All set!`n`nThis PC now builds MOTIX whenever Claude delivers an update and publishes it to:`n" +
    "$($folder.SelectedPath)`n`nIt works while you're signed in to Windows on this PC. " +
    "Progress is written to 'MOTIX build status.txt' next to the motix folder.")
