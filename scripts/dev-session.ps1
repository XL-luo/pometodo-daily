param(
    [Parameter(Mandatory)][ValidateSet('status','acquire','release')][string]$Action,
    [string]$Owner,
    [string]$Task,
    [string]$Repository = (Split-Path $PSScriptRoot -Parent)
)
$ErrorActionPreference = 'Stop'
$gitDirectory = git -C $Repository rev-parse --absolute-git-dir
if ($LASTEXITCODE -ne 0) { throw 'Repository must be an existing Git checkout.' }
$recordPath = Join-Path $gitDirectory 'codex-dev-session.json'
# Serialize all operations, including ownership checks and release. The guard file
# persists; its exclusive handle is automatically released even after a crash.
$guard = $null
try {
    $guard = [IO.File]::Open((Join-Path $gitDirectory 'codex-dev-session.guard'), 'OpenOrCreate', 'ReadWrite', 'None')
    $record = if (Test-Path -LiteralPath $recordPath) {
        Get-Content -LiteralPath $recordPath -Raw | ConvertFrom-Json
    } else { $null }
    if ($Action -eq 'status') {
        if ($record) { $record | ConvertTo-Json } else { Write-Output 'FREE' }
        exit 0
    }
    if ([string]::IsNullOrWhiteSpace($Owner)) { throw 'Owner (chat ID) is required.' }
    if ($Action -eq 'acquire') {
        if ($record) { throw "Occupied by $($record.owner): $($record.task). Inspect the owner chat; never expire automatically." }
        if ([string]::IsNullOrWhiteSpace($Task)) { throw 'Task is required.' }
        $json = [ordered]@{owner=$Owner; task=$Task; startedAt=[DateTimeOffset]::Now.ToString('o')} | ConvertTo-Json
        $stream = [IO.File]::Open($recordPath, 'CreateNew', 'Write', 'None')
        try {
            $bytes = [Text.Encoding]::UTF8.GetBytes($json)
            $stream.Write($bytes, 0, $bytes.Length)
            $stream.Flush($true)
        } finally { $stream.Dispose() }
        Write-Output $json
    } else {
        if (-not $record) { throw 'No active session to release.' }
        if ($record.owner -cne $Owner) { throw "Only owner $($record.owner) can release this session." }
        Remove-Item -LiteralPath $recordPath
        Write-Output 'RELEASED'
    }
} catch {
    Write-Error $_ -ErrorAction Continue
    exit 1
} finally {
    if ($guard) { $guard.Dispose() }
}
