<#
.SYNOPSIS
Read-only inventory of Visual Studio instances, Copilot components and local files.
.DESCRIPTION
Uses the official Setup Configuration client (vswhere), across all products and
prerelease instances. Reads assembly metadata and file counts only; never reads
chat bodies, credentials or account reports. Does not install or configure VS.
Finding an exporter type does not establish that it emitted usage.
#>
[CmdletBinding()]
param([string]$VsWherePath)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$warnings = [System.Collections.Generic.List[string]]::new()

function Get-JsonlExporterEvidence([string]$AssemblyPath) {
    $stream = $null
    $reader = $null
    try {
        $stream = [System.IO.File]::OpenRead($AssemblyPath)
        $reader = [System.Reflection.PortableExecutable.PEReader]::new($stream)
        $metadata = [System.Reflection.Metadata.PEReaderExtensions]::GetMetadataReader($reader)
        foreach ($handle in $metadata.TypeDefinitions) {
            $type = $metadata.GetTypeDefinition($handle)
            if ($metadata.GetString($type.Name) -eq 'JsonlOtlpTraceExporter' -and
                $metadata.GetString($type.Namespace) -eq 'Microsoft.VisualStudio.Copilot.Instrumentation.Exporters') {
                return 'present'
            }
        }
        return 'absent'
    } catch {
        return 'unreadable'
    } finally {
        if ($null -ne $reader) { $reader.Dispose() }
        if ($null -ne $stream) { $stream.Dispose() }
    }
}

if ([string]::IsNullOrWhiteSpace($VsWherePath)) {
    $command = Get-Command vswhere.exe -ErrorAction SilentlyContinue
    if ($null -ne $command) {
        $VsWherePath = $command.Source
    } else {
        # This is Microsoft's documented Installer tool location, not a VS IDE
        # installation path. IDE paths come exclusively from vswhere's results.
        $programFiles = [Environment]::GetFolderPath('ProgramFilesX86')
        if (-not [string]::IsNullOrWhiteSpace($programFiles)) {
            $VsWherePath = Join-Path $programFiles 'Microsoft Visual Studio/Installer/vswhere.exe'
        }
    }
}
$instances = @()
$componentFiles = @()
$discoveryStatus = 'vswhere_unavailable'
if (-not [string]::IsNullOrWhiteSpace($VsWherePath) -and (Test-Path -LiteralPath $VsWherePath -PathType Leaf)) {
    # Do not use -latest, a year range, or a Community-only product selector.
    $query = @('-all', '-prerelease', '-products', '*', '-utf8')
    try {
        $LASTEXITCODE = 0
        $raw = & $VsWherePath @query -format json
        if ($LASTEXITCODE -ne 0) { throw 'vswhere instance query failed' }
        $instances = @((($raw -join "`n") | ConvertFrom-Json))
        $LASTEXITCODE = 0
        $componentFiles = @(& $VsWherePath @query -find '**\Microsoft.VisualStudio.Copilot.Core.dll')
        if ($LASTEXITCODE -ne 0) { throw 'vswhere component query failed' }
        $discoveryStatus = 'queried'
    } catch {
        $warnings.Add('vswhere_query_failed')
        $discoveryStatus = 'query_failed'
    }
}

$installationEvidence = @(
    foreach ($instance in $instances) {
        $components = @(
            foreach ($file in $componentFiles) {
                if ([string]::IsNullOrWhiteSpace($file)) { continue }
                $relative = [System.IO.Path]::GetRelativePath($instance.installationPath, $file)
                if ([System.IO.Path]::IsPathRooted($relative) -or $relative -eq '..' -or
                    $relative.StartsWith('..\') -or $relative.StartsWith('../')) { continue }
                try {
                    $version = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($file)
                    [pscustomobject]@{
                        relative_path = $relative
                        product_version = $version.ProductVersion
                        jsonl_exporter_type = Get-JsonlExporterEvidence $file
                    }
                } catch {
                    $warnings.Add('component_metadata_unreadable')
                }
            }
        )
        [pscustomobject]@{
            instance_id = $instance.instanceId
            product_id = $instance.productId
            installation_version = $instance.installationVersion
            installation_path = $instance.installationPath
            copilot_components = $components
        }
    }
)

# The current runtime result is authoritative for this process. VS may have
# inherited a different environment; keep user defaults and both variables as
# separate candidates and report only directories that can be inspected.
$tempCandidates = @([System.IO.Path]::GetTempPath(), $env:TMP, $env:TEMP)
foreach ($name in @('TMP', 'TEMP')) {
    $tempCandidates += [Environment]::GetEnvironmentVariable($name, 'User')
}
$local = [Environment]::GetFolderPath('LocalApplicationData')
if (-not [string]::IsNullOrWhiteSpace($local)) {
    $tempCandidates += Join-Path $local 'Temp'
}
$seen = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
$carriers = @(
    foreach ($temp in $tempCandidates) {
        if ([string]::IsNullOrWhiteSpace($temp)) { continue }
        $logs = [System.IO.Path]::GetFullPath((Join-Path $temp 'VSGitHubCopilotLogs'))
        if (-not $seen.Add($logs)) { continue }
        $traces = Join-Path $logs 'traces'
        try {
            $files = @()
            if (Test-Path -LiteralPath $traces -PathType Container) {
                $files = @(Get-ChildItem -LiteralPath $traces -File -Filter '*.jsonl' | Select-Object -First 1024)
            }
            $bytes = if ($files.Count -gt 0) { ($files | Measure-Object -Property Length -Sum).Sum } else { 0 }
            $last = $files | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1
            [pscustomobject]@{
                traces_directory = $traces
                jsonl_files_up_to_1024 = $files.Count
                bytes = if ($null -eq $bytes) { 0 } else { $bytes }
                newest_write_utc = if ($null -eq $last) { $null } else { $last.LastWriteTimeUtc.ToString('o') }
                legacy_usage_details_exists = Test-Path -LiteralPath (Join-Path $logs 'UsageDetails') -PathType Leaf
                inspection_status = 'inspected'
            }
        } catch {
            $warnings.Add('carrier_directory_unreadable')
            [pscustomobject]@{ traces_directory = $traces; inspection_status = 'unreadable' }
        }
    }
)
[pscustomobject]@{
    schema_version = 1
    installation_discovery = $discoveryStatus
    instances = $installationEvidence
    local_carriers = $carriers
    warnings = @($warnings)
    coverage_note = 'Component presence does not certify usage. No JSONL means no verified trace carrier; legacy UsageDetails has no event time and is not ingested.'
} | ConvertTo-Json -Depth 8
