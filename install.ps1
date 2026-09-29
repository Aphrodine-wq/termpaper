# Install termpaper on Windows.
#
#   irm https://raw.githubusercontent.com/Aphrodine-wq/termpaper/main/install.ps1 | iex
#
# Downloads the latest release, puts termpaper.exe in
# %LOCALAPPDATA%\Programs\termpaper and adds that folder to your user PATH.
# Nothing needs administrator rights.
#
# Options (environment variables):
#   TERMPAPER_RELEASE_REPO   GitHub owner/repo to download from
#   TERMPAPER_INSTALL_DIR    install somewhere else

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'   # Invoke-WebRequest is slow with the bar

$repo = if ($env:TERMPAPER_RELEASE_REPO) { $env:TERMPAPER_RELEASE_REPO } else { 'Aphrodine-wq/termpaper' }
$dest = if ($env:TERMPAPER_INSTALL_DIR) { $env:TERMPAPER_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\termpaper' }
$arch = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'aarch64' } else { 'x86_64' }
$asset = "termpaper-$arch-pc-windows-msvc.zip"
$url = "https://github.com/$repo/releases/latest/download/$asset"

Write-Host "==> downloading $asset"
$tmp = Join-Path ([IO.Path]::GetTempPath()) $asset
try {
    Invoke-WebRequest -Uri $url -OutFile $tmp -UseBasicParsing
} catch {
    Write-Error "could not download $url - is there a GitHub release yet? ($_)"
}

New-Item -ItemType Directory -Force -Path $dest | Out-Null
Expand-Archive -Path $tmp -DestinationPath $dest -Force
Remove-Item $tmp -Force
if (-not (Test-Path (Join-Path $dest 'termpaper.exe'))) {
    Write-Error "the archive did not contain termpaper.exe"
}

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($null -eq $userPath) { $userPath = '' }
$parts = $userPath -split ';' | Where-Object { $_ -ne '' }
if (-not ($parts -contains $dest)) {
    $new = (@($parts) + $dest) -join ';'
    [Environment]::SetEnvironmentVariable('Path', $new, 'User')
    $env:Path = "$env:Path;$dest"
    Write-Host "==> added $dest to your PATH (new terminals pick it up)"
}

Write-Host ""
Write-Host "termpaper is installed."
Write-Host ""
Write-Host "  termpaper          start; press ? for the menu"
Write-Host "  termpaper tokyo    a Studio scene (uses your GPU)"
Write-Host "  termpaper list     the full catalog"
Write-Host ""
Write-Host "Best in Windows Terminal with the Cascadia Mono font, which draws every pixel mode."
