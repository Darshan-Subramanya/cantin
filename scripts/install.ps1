# Install the `cantin` command-line tool on Windows. No Rust needed.
#   irm https://raw.githubusercontent.com/Darshan-Subramanya/cantin/main/scripts/install.ps1 | iex
$ErrorActionPreference = "Stop"
$base = if ($env:CANTIN_BASE_URL) { $env:CANTIN_BASE_URL } else { "https://github.com/Darshan-Subramanya/cantin/releases/latest/download" }
$dir = Join-Path $env:LOCALAPPDATA "Programs\cantin"
New-Item -ItemType Directory -Force -Path $dir | Out-Null

$zip = Join-Path $env:TEMP "cantin-cli.zip"
$url = "$base/cantin-x86_64-pc-windows-msvc.zip"
Write-Host "Downloading $url"
Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing
Expand-Archive -Path $zip -DestinationPath $dir -Force
Remove-Item $zip

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (-not ($userPath -split ";" | Where-Object { $_ -eq $dir })) {
  [Environment]::SetEnvironmentVariable("Path", "$userPath;$dir", "User")
  Write-Host "Added $dir to your PATH. Open a new terminal to use it."
}
Write-Host "Installed cantin to $dir\cantin.exe. Try: cantin scan $HOME"
