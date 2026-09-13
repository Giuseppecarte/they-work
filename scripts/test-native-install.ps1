# Offline Windows installer regression tests. No network or user PATH changes.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$scratch = Join-Path $root ('target\installer-tests\' + [guid]::NewGuid())
New-Item -ItemType Directory -Force -Path $scratch | Out-Null
$oldTemp = $env:TEMP
$oldTmp = $env:TMP
$env:TEMP = $scratch
$env:TMP = $scratch
$global:TheyWorkFixture = $scratch
$global:TheyWorkDownloadFails = $false
function global:Invoke-WebRequest {
    param([string]$Uri, [string]$OutFile, [switch]$UseBasicParsing)
    if ($global:TheyWorkDownloadFails) { throw 'Simulated download failure' }
    $name = if ($Uri.EndsWith('/SHA256SUMS')) { 'SHA256SUMS' } else { 'release.zip' }
    Copy-Item (Join-Path $global:TheyWorkFixture $name) $OutFile
}
function Assert-True([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}
function Expect-Failure([scriptblock]$Action) {
    $failed = $false
    try { & $Action } catch { $failed = $true }
    Assert-True $failed 'Expected the installer to fail'
}
try {
    $architecture = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
    $target = if ($architecture -eq 'ARM64') { 'aarch64-pc-windows-msvc' } else { 'x86_64-pc-windows-msvc' }
    $asset = "they-work-$target.zip"
    $payload = Join-Path $scratch 'they-work.exe'
    # Bind Value explicitly: PowerShell's dynamic NoNewline parameter can
    # silently skip creating the file when Value is bound positionally.
    Set-Content -LiteralPath $payload -Value 'release fixture' -NoNewline
    Compress-Archive -Path $payload -DestinationPath (Join-Path $scratch 'release.zip')
    $digest = (Get-FileHash (Join-Path $scratch 'release.zip') -Algorithm SHA256).Hash
    $checksums = Join-Path $scratch 'SHA256SUMS'
    Set-Content -LiteralPath $checksums -Value "$digest  $asset"
    $destination = Join-Path $scratch 'path with spaces'
    $installer = Join-Path $PSScriptRoot 'install.ps1'
    $pathBefore = [Environment]::GetEnvironmentVariable('Path', 'User')
    & $installer -Version v0.2.0 -InstallDir $destination -NoPath
    $installed = Join-Path $destination 'they-work.exe'
    Assert-True ((Get-Content -Raw $installed) -eq 'release fixture') 'Valid installation failed'
    Assert-True ([Environment]::GetEnvironmentVariable('Path', 'User') -eq $pathBefore) 'NoPath modified PATH'
    Write-Host 'PASS: valid archive, path with spaces, NoPath'

    Set-Content -LiteralPath $installed -Value 'previous executable' -NoNewline
    Set-Content -LiteralPath $checksums -Value (('0' * 64) + "  $asset")
    Expect-Failure { & $installer -InstallDir $destination -NoPath }
    Assert-True ((Get-Content -Raw $installed) -eq 'previous executable') 'Checksum failure replaced existing install'
    Write-Host 'PASS: checksum failure preserves previous executable'

    $global:TheyWorkDownloadFails = $true
    Expect-Failure { & $installer -InstallDir $destination -NoPath }
    Assert-True ((Get-Content -Raw $installed) -eq 'previous executable') 'Download failure replaced existing install'
    $global:TheyWorkDownloadFails = $false
    Write-Host 'PASS: download failure preserves previous executable'

    Set-Content -LiteralPath $checksums -Value "$digest  $asset`n$digest  $asset"
    Expect-Failure { & $installer -InstallDir $destination -NoPath }
    Write-Host 'PASS: duplicate checksum rejected'

    Set-Content -LiteralPath $checksums -Value "$digest  $asset"
    & $installer -InstallDir $destination -NoPath
    Assert-True ((Get-Content -Raw $installed) -eq 'release fixture') 'Update failed'
    Write-Host 'PASS: successful update'
} finally {
    Remove-Item Function:\Invoke-WebRequest
    Remove-Variable TheyWorkFixture, TheyWorkDownloadFails -Scope Global
    $env:TEMP = $oldTemp
    $env:TMP = $oldTmp
    Remove-Item -LiteralPath $scratch -Recurse -Force
}
