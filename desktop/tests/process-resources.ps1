param([int]$RootPid, [ValidateRange(0,1800)][int]$Seconds = 600, [ValidateRange(100,10000)][int]$IntervalMs = 5000, [string]$ReadyFile, [string]$StopFile)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$started = [DateTime]::UtcNow
$samples = [Collections.Generic.List[object]]::new()
$cpuByProcess = @{}
$roleByProcess = @{}
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
    $private = 0L; $working = 0L; $count = 0; $roles = @{}
    foreach ($processId in $owned) {
        try {
            $process = Get-Process -Id $processId -ErrorAction Stop
            $private += $process.PrivateMemorySize64
            $working += $process.WorkingSet64
            $cpuByProcess["$processId-$($process.StartTime.Ticks)"] = $process.TotalProcessorTime.TotalSeconds
            $identity = "$processId-$($process.StartTime.Ticks)"
            if (-not $roleByProcess.ContainsKey($identity)) {
                $role = 'webview-other'
                if ($processId -eq $RootPid) { $role = 'app' }
                else {
                    # Inspect only descendants of this test's root. Persist the
                    # fixed role label, never command lines or local paths.
                    try {
                        $command = (Get-CimInstance Win32_Process -Filter "ProcessId=$processId" -Property CommandLine).CommandLine
                        if ($command -match '--type=gpu-process') { $role = 'webview-gpu' }
                        elseif ($command -match '--type=renderer') { $role = 'webview-renderer' }
                        elseif ($command -match '--type=utility') { $role = 'webview-utility' }
                        elseif ($command -and $command -notmatch '--type=') { $role = 'webview-browser' }
                    } catch { }
                }
                $roleByProcess[$identity] = $role
            }
            $role = $roleByProcess[$identity]
            if (-not $roles.ContainsKey($role)) { $roles[$role] = 0L }
            $roles[$role] += $process.PrivateMemorySize64
            $count++
        } catch { }
    }
    $cpu = ($cpuByProcess.Values | Measure-Object -Sum).Sum
    if ($null -eq $firstCpu) { $firstCpu = $cpu }
    $samples.Add([pscustomobject]@{elapsed_seconds=([DateTime]::UtcNow-$started).TotalSeconds;process_count=$count;private_bytes=$private;working_set_bytes=$working;cpu_seconds=$cpu;private_bytes_by_role=$roles})
    if ($ReadyFile -and $samples.Count -eq 1) { [IO.File]::WriteAllText($ReadyFile, 'ready') }
    if (-not (Get-Process -Id $RootPid -ErrorAction SilentlyContinue)) { break }
    if ($StopFile -and [IO.File]::Exists($StopFile)) { break }
    if (([DateTime]::UtcNow-$started).TotalSeconds -ge $Seconds) { break }
    Start-Sleep -Milliseconds $IntervalMs
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
