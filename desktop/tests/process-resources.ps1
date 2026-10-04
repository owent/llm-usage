param([int]$RootPid, [int]$Seconds = 600)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$started = [DateTime]::UtcNow
$samples = [Collections.Generic.List[object]]::new()
$cpuByProcess = @{}
$firstCpu = $null
do {
    $tree = @(Get-CimInstance Win32_Process -Property ProcessId,ParentProcessId)
    $owned = [Collections.Generic.HashSet[int]]::new()
    [void]$owned.Add($RootPid)
    do {
        $added = $false
        foreach ($entry in $tree) {
            if ($owned.Contains([int]$entry.ParentProcessId) -and $owned.Add([int]$entry.ProcessId)) { $added = $true }
        }
    } while ($added)
    $private = 0L; $working = 0L; $count = 0
    foreach ($processId in $owned) {
        try {
            $process = Get-Process -Id $processId -ErrorAction Stop
            $private += $process.PrivateMemorySize64
            $working += $process.WorkingSet64
            $cpuByProcess["$processId-$($process.StartTime.Ticks)"] = $process.TotalProcessorTime.TotalSeconds
            $count++
        } catch { }
    }
    $cpu = ($cpuByProcess.Values | Measure-Object -Sum).Sum
    if ($null -eq $firstCpu) { $firstCpu = $cpu }
    $samples.Add([pscustomobject]@{elapsed_seconds=([DateTime]::UtcNow-$started).TotalSeconds;process_count=$count;private_bytes=$private;working_set_bytes=$working;cpu_seconds=$cpu})
    if (-not (Get-Process -Id $RootPid -ErrorAction SilentlyContinue)) { break }
    if (([DateTime]::UtcNow-$started).TotalSeconds -ge $Seconds) { break }
    Start-Sleep -Milliseconds 5000
} while ($true)
$elapsed = ([DateTime]::UtcNow-$started).TotalSeconds
[pscustomobject]@{
    duration_seconds=$elapsed
    sample_count=$samples.Count
    max_process_count=($samples.process_count | Measure-Object -Maximum).Maximum
    peak_private_bytes=($samples.private_bytes | Measure-Object -Maximum).Maximum
    peak_working_set_bytes=($samples.working_set_bytes | Measure-Object -Maximum).Maximum
    mean_private_bytes=($samples.private_bytes | Measure-Object -Average).Average
    single_core_cpu_percent=100*($cpu-$firstCpu)/[Math]::Max(1,$elapsed)
    samples=$samples
} | ConvertTo-Json -Depth 5 -Compress
