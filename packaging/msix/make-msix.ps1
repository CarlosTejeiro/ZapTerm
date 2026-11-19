<#
  Build the MSIX package of ZapTerm for the Microsoft Store.

    pwsh packaging/msix/make-msix.ps1 -Exe target\release\zapterm.exe -Version 1.7.9 -Out ZapTerm_1.7.9_x64.msix

  The package is NOT signed here: the Store signs what it distributes. To try
  one locally it has to be signed with a certificate whose subject is the
  manifest's Publisher (see .github/workflows/msix.yml, which does that).
#>
param(
  [Parameter(Mandatory)] [string] $Exe,
  [Parameter(Mandatory)] [string] $Version,
  [Parameter(Mandatory)] [string] $Out,
  # Override the Publisher (the CI smoke test uses its own test certificate).
  [string] $Publisher = ''
)
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$repo = Resolve-Path (Join-Path $here '..\..')

if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "version must be X.Y.Z, got '$Version'" }
if (-not (Test-Path $Exe)) { throw "exe not found: $Exe" }

$identity = Get-Content (Join-Path $here 'identity.json') -Raw | ConvertFrom-Json
if (-not $Publisher) { $Publisher = $identity.publisher }

$layout = Join-Path ([IO.Path]::GetTempPath()) ("zapterm-msix-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path (Join-Path $layout 'Assets') | Out-Null
try {
  Copy-Item $Exe (Join-Path $layout 'zapterm.exe')
  foreach ($logo in 'StoreLogo', 'Square44x44Logo', 'Square71x71Logo', 'Square150x150Logo', 'Square310x310Logo') {
    Copy-Item (Join-Path $repo "crates\app\icons\$logo.png") (Join-Path $layout "Assets\$logo.png")
  }

  # The Store requires the fourth number of a package version to be 0.
  $manifest = (Get-Content (Join-Path $here 'AppxManifest.xml') -Raw).
    Replace('{{NAME}}', $identity.name).
    Replace('{{PUBLISHER}}', $Publisher).
    Replace('{{PUBLISHER_DISPLAY_NAME}}', $identity.publisherDisplayName).
    Replace('{{VERSION}}', "$Version.0")
  Set-Content -Path (Join-Path $layout 'AppxManifest.xml') -Value $manifest -Encoding utf8

  $makeappx = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin\*\x64\makeappx.exe' |
    Sort-Object FullName -Descending | Select-Object -First 1
  if (-not $makeappx) { throw 'makeappx.exe (Windows SDK) not found' }

  if (Test-Path $Out) { Remove-Item $Out }
  & $makeappx.FullName pack /d $layout /p $Out /o
  if ($LASTEXITCODE -ne 0) { throw "makeappx failed ($LASTEXITCODE)" }
  Write-Host "built $Out ($([math]::Round((Get-Item $Out).Length / 1MB, 1)) MB)"
}
finally {
  Remove-Item $layout -Recurse -Force -ErrorAction SilentlyContinue
}
