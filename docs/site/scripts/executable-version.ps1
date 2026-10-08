param([Parameter(Mandatory = $true)][string]$Executable)
$ErrorActionPreference = 'Stop'
$version = [Diagnostics.FileVersionInfo]::GetVersionInfo($Executable).ProductVersion
if ($version -notmatch '^\d+\.\d+\.\d+') { throw 'Executable has no valid product-version metadata' }
[Console]::Out.Write($version)
