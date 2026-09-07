# Install a checksum-verified native Windows release without administrator access.
[CmdletBinding()]
param(
    [ValidatePattern('^(latest|v[0-9]+\.[0-9]+\.[0-9]+([-\.][a-zA-Z0-9\.]+)?)$')]
    [string]$Version = 'latest',
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\they-work'),
    [switch]$NoPath
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$architecture = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
$target = switch ($architecture) {
    'AMD64' { 'x86_64-pc-windows-msvc' }
    'ARM64' { 'aarch64-pc-windows-msvc' }
    default { throw "No native release for $architecture. See INSTALL.md to build from source." }
}
$asset = "they-work-$target.zip"
$base = 'https://github.com/Giuseppecarte/they-work/releases'
$url = if ($Version -eq 'latest') { "$base/latest/download" } else { "$base/download/$Version" }
$work = Join-Path ([IO.Path]::GetTempPath()) ("they-work-install-" + [guid]::NewGuid())
$staged = $null
New-Item -ItemType Directory -Path $work | Out-Null
try {
    Write-Host "Downloading $asset ($Version) ..."
    try {
        Invoke-WebRequest "$url/$asset" -OutFile (Join-Path $work $asset) -UseBasicParsing
        Invoke-WebRequest "$url/SHA256SUMS" -OutFile (Join-Path $work 'SHA256SUMS') -UseBasicParsing
    } catch {
        throw "Release download failed; nothing installed. The older v0.1.0 release is Docker-only. See INSTALL.md to build this checkout. $($_.Exception.Message)"
    }
    $lines = @(Get-Content (Join-Path $work 'SHA256SUMS') | Where-Object { $_ -match ('^[a-fA-F0-9]{64}\s+' + [regex]::Escape($asset) + '$') })
    if ($lines.Count -ne 1) { throw 'Release checksum missing or duplicated; nothing installed.' }
    $expected = ($lines[0] -split '\s+')[0]
    $actual = (Get-FileHash (Join-Path $work $asset) -Algorithm SHA256).Hash
    if ($actual -ne $expected) { throw 'SHA256 verification failed; nothing installed.' }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [IO.Compression.ZipFile]::OpenRead((Join-Path $work $asset))
    try {
        $entries = @($archive.Entries | Where-Object { $_.FullName -eq 'they-work.exe' })
        if ($entries.Count -ne 1) { throw 'Release archive must contain one they-work.exe.' }
        [IO.Compression.ZipFileExtensions]::ExtractToFile($entries[0], (Join-Path $work 'they-work.exe'))
    } finally { $archive.Dispose() }
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    $destination = Join-Path $InstallDir 'they-work.exe'
    $staged = Join-Path $InstallDir ('.they-work-' + [guid]::NewGuid() + '.exe')
    Copy-Item (Join-Path $work 'they-work.exe') $staged
    if (Test-Path $destination) {
        [IO.File]::Replace($staged, $destination, $null)
    } else {
        [IO.File]::Move($staged, $destination)
    }
    $staged = $null
    if (-not $NoPath) {
        $userPath = [string][Environment]::GetEnvironmentVariable('Path', 'User')
        if (($userPath -split ';') -notcontains $InstallDir) {
            [Environment]::SetEnvironmentVariable('Path', (($userPath.TrimEnd(';') + ';' + $InstallDir).TrimStart(';')), 'User')
        }
        if (($env:Path -split ';') -notcontains $InstallDir) { $env:Path += ";$InstallDir" }
    }
    Write-Host "Installed: $destination"
    Write-Host "Start with: & '$destination' --setup (your sources), or --demo"
    if (-not $NoPath) { Write-Host 'Future terminals can run they-work from PATH.' }
} finally {
    Remove-Item -LiteralPath $work -Recurse -Force
    if ($staged -and (Test-Path $staged)) { Remove-Item -LiteralPath $staged -Force }
}
