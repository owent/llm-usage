param([Parameter(Mandatory)][string]$Action)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$OutputEncoding = [Console]::OutputEncoding
$installPath = $env:LLM_USAGE_INSTALL_PATH
$taskLabel = $env:LLM_USAGE_TASK_LABEL
$workspace=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
$allowedRoot=Join-Path $workspace 'build/install-lifecycle/windows'
if (!$installPath -or ![IO.Path]::GetFullPath($installPath).StartsWith($allowedRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Installation target must stay within this test workspace' }
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\LLMUsage'
function Snapshot {
  $product = Get-ItemProperty -LiteralPath $uninstallKey -ErrorAction SilentlyContinue
  $machineProducts=@(foreach($key in @('HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\LLMUsage','HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\LLMUsage')) {
    $entry=Get-ItemProperty -LiteralPath $key -ErrorAction SilentlyContinue
    if($entry){@{key=$key;version=$entry.DisplayVersion;location=$entry.InstallLocation}}
  })
  $startup = Get-ItemProperty -LiteralPath $runKey -Name LLMUsage -ErrorAction SilentlyContinue
  $shortcutPaths=@((Join-Path ([Environment]::GetFolderPath('DesktopDirectory')) 'LLMUsage.lnk'),(Join-Path ([Environment]::GetFolderPath('Programs')) 'LLMUsage.lnk'))
  $shell=New-Object -ComObject WScript.Shell
  $shortcuts=@(foreach($path in $shortcutPaths) { if(Test-Path -LiteralPath $path) { @{path=$path;target=$shell.CreateShortcut($path).TargetPath} } })
  $service = New-Object -ComObject Schedule.Service
  $service.Connect()
  $collection=$service.GetFolder('\').GetTasks(1)
  $tasks=@(for($index=1;$index -le $collection.Count;$index++) {
    $candidate=$collection.Item($index)
    if($candidate.Name -like 'LLMUsageDataRefresh-*' -and $candidate.Definition.Actions.Count -eq 1 -and $candidate.Definition.Actions.Item(1).Path -eq "$installPath\LLMUsage.exe") {
      @{ name=$candidate.Name;enabled=$candidate.Enabled;description=$candidate.Definition.RegistrationInfo.Description;user=$candidate.Definition.Principal.UserId;logon=$candidate.Definition.Principal.LogonType;level=$candidate.Definition.Principal.RunLevel;xml=$candidate.Xml }
    }
  })
  @{ product=if($product){@{version=$product.DisplayVersion;location=$product.InstallLocation;uninstall=$product.UninstallString}}else{$null};
     machineProducts=$machineProducts;startup=if($startup){$startup.LLMUsage}else{$null}; tasks=$tasks;shortcuts=$shortcuts;
     running=@(Get-Process LLMUsage -ErrorAction SilentlyContinue | ForEach-Object {$_.Id}) } | ConvertTo-Json -Depth 8 -Compress
}
switch ($Action) {
  'snapshot' { Snapshot }
  'installer' {
    $arguments = '/S /NS /UPDATE /D=' + $installPath
    if ($env:LLM_USAGE_FRESH_INSTALL -eq '1') { $arguments='/S /D='+$installPath }
    if ($env:LLM_USAGE_UNINSTALL -eq '1') { $arguments = '/S _?=' + $installPath }
    $process = Start-Process -FilePath $env:LLM_USAGE_INSTALLER -ArgumentList $arguments -WindowStyle Hidden -PassThru
    if (!$process.WaitForExit(180000)) { $process.Kill(); throw 'Owned installer timed out' }
    @{code=$process.ExitCode;arguments=$arguments} | ConvertTo-Json -Compress
  }
  'foreign-startup-create' {
    if ((Get-ItemProperty -LiteralPath $runKey -Name LLMUsage -ErrorAction SilentlyContinue)) { throw 'Startup entry already occupied' }
    $null=New-Item -Path $runKey -Force
    Set-ItemProperty -LiteralPath $runKey -Name LLMUsage -Value ('"'+$installPath+'\foreign-LLMUsage.exe"')
    'ok'
  }
  'foreign-create' {
    if (!$taskLabel.StartsWith('LLMUsageDataRefresh-lifecycle-')) { throw 'Invalid owned decoy name' }
    $service = New-Object -ComObject Schedule.Service
    $service.Connect()
    $folder=$service.GetFolder('\')
    $collection=$folder.GetTasks(1)
    $existing=@(for($index=1;$index -le $collection.Count;$index++){if($collection.Item($index).Name -eq $taskLabel){$collection.Item($index)}})
    if ($existing) { throw 'Decoy already exists' }
    $definition=$service.NewTask(0)
    $command=[System.Security.SecurityElement]::Escape("$installPath\LLMUsage.exe")
    $sid=[System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    $definition.XmlText=@"
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task"><RegistrationInfo><Description>lifecycle-decoy:$taskLabel</Description></RegistrationInfo><Principals><Principal id="Author"><UserId>$sid</UserId><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals><Settings><Enabled>false</Enabled></Settings><Actions Context="Author"><Exec><Command>$command</Command><Arguments>--headless --data-dir "foreign-lifecycle-decoy"</Arguments></Exec></Actions></Task>
"@
    $null=$folder.RegisterTaskDefinition($taskLabel,$definition,2,$null,$null,3,$null)
    'ok'
  }
  'cleanup-test' {
    # Only exact test identities, created by this script or the isolated application.
    $service = New-Object -ComObject Schedule.Service
    $service.Connect()
    $folder=$service.GetFolder('\')
    $collection=$folder.GetTasks(1)
    for($index=1;$index -le $collection.Count;$index++) {
      $task=$collection.Item($index)
      if ($task.Name -eq $taskLabel -and $task.Definition.RegistrationInfo.Description -eq ('lifecycle-decoy:'+$taskLabel)) {
        $folder.DeleteTask($task.Name,0)
      } elseif ($task.Name -like 'LLMUsageDataRefresh-*' -and $task.Definition.RegistrationInfo.Description -eq ('llm-usage:'+$task.Name)) {
        $actions=$task.Definition.Actions
        if ($actions.Count -eq 1 -and $actions.Item(1).Path -eq "$installPath\LLMUsage.exe" -and $actions.Item(1).Arguments -eq ('--headless --data-dir "'+$env:LLM_USAGE_TEST_DATA+'"')) {
          $folder.DeleteTask($task.Name,0)
        }
      }
    }
    $startup=Get-ItemProperty -LiteralPath $runKey -Name LLMUsage -ErrorAction SilentlyContinue
    if ($startup -and ($startup.LLMUsage -eq ('"'+$installPath+'\LLMUsage.exe"') -or $startup.LLMUsage -eq ('"'+$installPath+'\foreign-LLMUsage.exe"'))) {
      Remove-ItemProperty -LiteralPath $runKey -Name LLMUsage
    }
    $productKey='HKCU:\Software\llmusage\LLMUsage'
    $productEntry=[Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Software\llmusage\LLMUsage',$true)
    try { if ($productEntry -and $productEntry.GetValue('') -eq $installPath) { $productEntry.DeleteValue('', $false) } }
    finally { if ($productEntry) { $productEntry.Dispose() } }
    'ok'
  }
  default { throw 'Unknown action' }
}
