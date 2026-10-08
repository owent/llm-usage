$ErrorActionPreference = 'Stop'
[Console]::InputEncoding = [System.Text.UTF8Encoding]::new($false)
$body = ConvertFrom-Json -InputObject ([Console]::In.ReadToEnd())
$tokens = $null
$errors = $null
$null = [System.Management.Automation.Language.Parser]::ParseInput($body, [ref]$tokens, [ref]$errors)
if ($errors.Count -ne 0) {
    throw ('PowerShell source has parse errors: ' + ($errors.ErrorId -join ', '))
}
$ranges = @($tokens | Where-Object Kind -EQ 'Comment' | ForEach-Object {
    @{ start = $_.Extent.StartOffset; end = $_.Extent.EndOffset; kind = 'comment' }
})
ConvertTo-Json -InputObject $ranges -Compress
