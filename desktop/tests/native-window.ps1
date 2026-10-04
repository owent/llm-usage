param([int]$RootPid, [ValidateSet('State','Close','Accessibility')][string]$Action='State')
$ErrorActionPreference='Stop'
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class OwnedWindow {
    public delegate bool EnumProc(IntPtr window,IntPtr argument);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback,IntPtr argument);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window,out uint process);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr window);
    [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr window,StringBuilder text,int capacity);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr window,uint message,IntPtr wParam,IntPtr lParam);
    public static IntPtr Find(int process) {
        IntPtr result=IntPtr.Zero;
        EnumWindows((window,argument)=>{uint owner;GetWindowThreadProcessId(window,out owner);if(owner==process){var title=new StringBuilder(256);GetWindowText(window,title,title.Capacity);if(title.ToString()=="LLM Usage")result=window;}return true;},IntPtr.Zero);
        return result;
    }
}
'@
$window=[OwnedWindow]::Find($RootPid)
if($window -eq [IntPtr]::Zero){throw 'owned native window not found'}
if($Action -eq 'Close'){
    if(-not [OwnedWindow]::PostMessage($window,0x0010,[IntPtr]::Zero,[IntPtr]::Zero)){throw 'close request failed'}
}
$result=@{visible=[OwnedWindow]::IsWindowVisible($window);dpi=[OwnedWindow]::GetDpiForWindow($window)}
if($Action -eq 'Accessibility'){
    Add-Type -AssemblyName UIAutomationClient
    Add-Type -AssemblyName UIAutomationTypes
    $element=[Windows.Automation.AutomationElement]::FromHandle($window)
    $controls=$element.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)
    $names=[Collections.Generic.List[string]]::new()
    foreach($control in $controls){
        if($control.Current.ControlType -eq [Windows.Automation.ControlType]::Button -and $control.Current.Name){$names.Add($control.Current.Name)}
    }
    $result.button_names=$names.ToArray()
    $result.control_count=$controls.Count
}
$result | ConvertTo-Json -Depth 4 -Compress
