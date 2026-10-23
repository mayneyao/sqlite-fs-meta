param(
  [string]$Version = $env:XMETA_VERSION,
  [string]$InstallDir = $env:XMETA_INSTALL_DIR
)

$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
$LatestUrl = if ($env:XMETA_LATEST_URL) { $env:XMETA_LATEST_URL } else { 'https://fsmeta.eidos.space/cli/latest.txt' }
$DownloadBase = if ($env:XMETA_DOWNLOAD_BASE) { $env:XMETA_DOWNLOAD_BASE } else { 'https://github.com/mayneyao/sqlite-fs-meta/releases/download' }
if (-not $InstallDir) { $InstallDir = Join-Path $HOME '.local\bin' }
if (-not [IO.Path]::IsPathRooted($InstallDir)) { throw 'Install directory must be an absolute path' }
$InstallDir = [IO.Path]::GetFullPath($InstallDir)
$Architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
if ($Architecture -ne 'X64') { throw "Unsupported Windows architecture: $Architecture" }

if (-not $Version) { $Version = (Invoke-WebRequest -UseBasicParsing -Uri $LatestUrl).Content.Trim() }
$Version = $Version -replace '^v', ''
if ($Version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$') { throw "Invalid version: $Version" }
$Binary = 'xmeta-x86_64-pc-windows-msvc.exe'
$ReleaseUrl = "$DownloadBase/v$Version"
$TemporaryDirectory = Join-Path ([IO.Path]::GetTempPath()) ('xmeta-install-' + [guid]::NewGuid())
$TemporaryTarget = $null

try {
  New-Item -ItemType Directory -Path $TemporaryDirectory | Out-Null
  $Download = Join-Path $TemporaryDirectory 'xmeta.exe'
  $Checksums = Join-Path $TemporaryDirectory 'SHA256SUMS'
  Write-Host "Installing xmeta $Version (Windows x86-64)..."
  Invoke-WebRequest -UseBasicParsing -Uri "$ReleaseUrl/$Binary" -OutFile $Download
  Invoke-WebRequest -UseBasicParsing -Uri "$ReleaseUrl/SHA256SUMS" -OutFile $Checksums
  $Lines = @(Get-Content $Checksums | Where-Object { $_ -match ('\s' + [regex]::Escape($Binary) + '$') })
  if ($Lines.Count -ne 1) { throw "SHA256SUMS has no unique entry for $Binary" }
  $Expected = ($Lines[0] -split '\s+')[0].ToLowerInvariant()
  $Stream = [IO.File]::OpenRead($Download)
  $Hasher = [Security.Cryptography.SHA256]::Create()
  try { $Actual = [BitConverter]::ToString($Hasher.ComputeHash($Stream)).Replace('-', '').ToLowerInvariant() }
  finally { $Stream.Dispose(); $Hasher.Dispose() }
  if ($Actual -ne $Expected) { throw "Checksum mismatch for $Binary" }
  $ActualVersion = & $Download --version
  if ($LASTEXITCODE -ne 0 -or $ActualVersion -ne "xmeta $Version") { throw "Unexpected binary version: $ActualVersion" }

  $Target = Join-Path $InstallDir 'xmeta.exe'
  if (Test-Path $Target -PathType Container) { throw "Destination is a directory: $Target" }
  New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
  $TemporaryTarget = Join-Path $InstallDir ('.xmeta-' + [guid]::NewGuid() + '.tmp')
  Copy-Item $Download $TemporaryTarget
  Move-Item -Force $TemporaryTarget $Target
  $TemporaryTarget = $null

  if (-not $env:XMETA_NO_MODIFY_PATH) {
    $UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $Entries = @($UserPath -split ';' | Where-Object { $_ })
    if (-not ($Entries | Where-Object { $_.TrimEnd('\') -ieq $InstallDir.TrimEnd('\') })) {
      [Environment]::SetEnvironmentVariable('Path', ((@($Entries) + $InstallDir) -join ';'), 'User')
    }
    if (-not (($env:Path -split ';') | Where-Object { $_.TrimEnd('\') -ieq $InstallDir.TrimEnd('\') })) {
      $env:Path = "$InstallDir;$env:Path"
    }
  }
  Write-Host "Installed xmeta $Version to $Target"
  Write-Host 'Run: xmeta --help'
}
finally {
  if ($TemporaryTarget -and (Test-Path $TemporaryTarget)) { Remove-Item -Force $TemporaryTarget }
  if (Test-Path $TemporaryDirectory) { Remove-Item -Recurse -Force $TemporaryDirectory }
}
