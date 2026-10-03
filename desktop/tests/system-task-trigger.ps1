# Read only: observe the test-owned minute trigger installed through real IPC.
# Inputs are confined to this native test process; this script creates no tasks.
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$service = New-Object -ComObject Schedule.Service
$service.Connect()
$folder = $service.GetFolder('\')
$expectedArguments = '--headless --data-dir "' + $env:LLM_USAGE_ACCEPTANCE_DATA + '"'
$collection = $folder.GetTasks(0)
$matches = @(for ($index = 1; $index -le $collection.Count; $index++) {
    $candidate = $collection.Item($index)
    $definition = $candidate.Definition
    $description = [string]$definition.RegistrationInfo.Description
    if ($description.StartsWith('llm-usage:') -and
        $definition.Actions.Count -eq 1 -and
        $definition.Actions.Item(1).Path -eq $env:LLM_USAGE_ACCEPTANCE_EXE -and
        $definition.Actions.Item(1).Arguments -eq $expectedArguments) { $candidate }
})
if ($matches.Count -ne 1) { throw 'expected exactly one test-owned task' }
$taskName = $matches[0].Name
$beforeRun = $matches[0].LastRunTime
$deadline = [DateTime]::UtcNow.AddSeconds(80)
do {
    $task = $folder.GetTask($taskName)
    if ($task.LastRunTime -gt $beforeRun -and $task.State -eq 3) {
        if ($task.LastTaskResult -ne 0) { throw ('task exit code: ' + $task.LastTaskResult) }
        [pscustomobject]@{trigger='minute';exit_code=0;completed=$true} | ConvertTo-Json -Compress
        exit 0
    }
    Start-Sleep -Milliseconds 250
} while ([DateTime]::UtcNow -lt $deadline)
throw 'minute trigger did not finish within the acceptance deadline'
