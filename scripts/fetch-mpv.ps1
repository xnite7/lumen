# Downloads the latest libmpv developer build (shinchiro/mpv-winbuild-cmake)
# into src-tauri/mpv and creates the import library MSVC links against.
# Run from a "Developer PowerShell for VS" (needs lib.exe) or let CI do it.
$ErrorActionPreference = "Stop"
$dest = Join-Path $PSScriptRoot "..\src-tauri\mpv"
New-Item -ItemType Directory -Force -Path $dest | Out-Null

$headers = @{ "User-Agent" = "lumen-build" }
if ($env:GITHUB_TOKEN) { $headers["Authorization"] = "Bearer $env:GITHUB_TOKEN" }
$rel = Invoke-RestMethod -Headers $headers "https://api.github.com/repos/shinchiro/mpv-winbuild-cmake/releases/latest"
# The plain x86_64 build (not -v3) runs on any x64 CPU.
$asset = $rel.assets | Where-Object { $_.name -match '^mpv-dev-x86_64-\d{8}-git-[0-9a-f]+\.7z$' } | Select-Object -First 1
if (-not $asset) { throw "No mpv-dev asset found in $($rel.tag_name)" }
Write-Host "Downloading $($asset.name)"
$archive = Join-Path $env:TEMP $asset.name
Invoke-WebRequest -Headers $headers $asset.browser_download_url -OutFile $archive
7z x -y "-o$dest" $archive | Out-Null

Push-Location $dest
$def = Get-ChildItem -Filter "*.def" | Select-Object -First 1
if (-not $def) {
  # Newer archives ship only the DLL: derive the .def from its exports.
  "EXPORTS" | Out-File -Encoding ascii mpv.def
  (& dumpbin /exports libmpv-2.dll) | Select-String '^\s+\d+\s+[0-9A-F]+\s+[0-9A-F]+\s+(mpv_\w+)' | ForEach-Object { $_.Matches[0].Groups[1].Value } | Out-File -Append -Encoding ascii mpv.def
  $def = Get-Item mpv.def
}
lib /nologo /def:$($def.Name) /name:libmpv-2.dll /out:mpv.lib /MACHINE:X64
Pop-Location
Write-Host "libmpv ready in $dest"
