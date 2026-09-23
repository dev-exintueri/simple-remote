# Spike only (throwaway): install the spike MSI, check the real state, uninstall, check traces.
# Prints "CHECK <name> ok=<bool>" per item and "RESULT wix ok=<bool>" last.
param(
    [Parameter(Mandatory)] [string] $Msi,
    [Parameter(Mandatory)] [string] $PsExec
)
$ErrorActionPreference = 'Continue'
$name = 'SimpleRemoteSpike'
$exe = 'C:\Program Files\SimpleRemoteSpike\spike-service.exe'
$data = 'C:\ProgramData\SimpleRemoteSpike'
$results = [ordered]@{}

function Check([string] $label, [bool] $ok, [string] $detail = '') {
    $results[$label] = $ok
    Write-Output "CHECK $label ok=$($ok.ToString().ToLower()) $detail"
}

# Run a cmd command line as SYSTEM and return its output. PsExec does not reliably forward the
# child's stdout, so SYSTEM writes to a temp file the runner account can read.
# Paths here contain no spaces, so no inner quotes are passed to cmd.
function Invoke-AsSystem([string] $commandLine) {
    $tmp = Join-Path $env:RUNNER_TEMP ("as-system-" + [guid]::NewGuid() + ".txt")
    & $PsExec -accepteula -nobanner -s cmd /c "$commandLine > $tmp 2>&1" 2>$null | Out-Null
    if (Test-Path $tmp) { Get-Content $tmp | Out-String } else { '' }
}

function Read-AsSystem([string] $path) { Invoke-AsSystem "type $path" }

$p = Start-Process msiexec.exe -ArgumentList "/i `"$Msi`" /qn /l*v install.log" -Wait -PassThru
Check 'install-exit-code' ($p.ExitCode -eq 0) "code=$($p.ExitCode)"

$svc = Get-CimInstance Win32_Service -Filter "Name='$name'"
Check 'service-exists' ($null -ne $svc)
Check 'service-account-localsystem' ($svc.StartName -eq 'LocalSystem') "StartName=$($svc.StartName)"
Check 'service-auto-start' ($svc.StartMode -eq 'Auto') "StartMode=$($svc.StartMode)"
Check 'service-running' ($svc.State -eq 'Running') "State=$($svc.State)"

$qf = sc.exe qfailure $name | Out-String
Write-Output $qf
Check 'service-failure-restart' ($qf -match 'RESTART') ''

$rule = Get-NetFirewallRule -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -eq 'SimpleRemote Spike' }
$ruleProgram = if ($rule) { ($rule | Get-NetFirewallApplicationFilter).Program } else { '' }
Check 'firewall-rule-program' ($null -ne $rule -and $ruleProgram -eq $exe) "Program=$ruleProgram"

# The admin account cannot read the DACL of a SYSTEM-only folder (icacls: Access is denied),
# so the ACL is read as SYSTEM and parsed from icacls output.
$aclText = Invoke-AsSystem "icacls $data"
Write-Output $aclText
# icacls prints "<path> <principal>:(flags)" then "<spaces><principal>:(flags)" per ACE.
$aclBody = $aclText.Replace($data, '')
$principals = @([regex]::Matches($aclBody, '(?m)^\s*(.+?):\(') | ForEach-Object { $_.Groups[1].Value.Trim() } | Sort-Object -Unique)
$inherited = $aclText -match '\(I\)'
Check 'data-acl-system-only' ($principals.Count -eq 1 -and $principals[0] -eq 'NT AUTHORITY\SYSTEM' -and -not $inherited) "principals=$($principals -join ';') inherited=$inherited"
$adminRead = Test-Path (Join-Path $data 'service.log') -ErrorAction SilentlyContinue
Write-Output "admin can see service.log: $adminRead"

Start-Sleep -Seconds 12
$log1 = Read-AsSystem (Join-Path $data 'service.log')
Write-Output $log1
Check 'service-writes-log' ($log1 -match 'running') ''

# Simulated crash: marker makes the next start exit with code 1 without reporting Stopped.
& $PsExec -accepteula -nobanner -s cmd /c "echo x> $data\crash-once" 2>$null
Restart-Service $name -Force
Start-Sleep -Seconds 20
$log2 = Read-AsSystem (Join-Path $data 'service.log')
Write-Output $log2
$pids = @([regex]::Matches($log2, 'started pid=(\d+)') | ForEach-Object { $_.Groups[1].Value } | Select-Object -Unique)
$svc2 = Get-CimInstance Win32_Service -Filter "Name='$name'"
Check 'service-restarted-after-crash' (($log2 -match 'crash-once marker found') -and $pids.Count -ge 3 -and $svc2.State -eq 'Running') "started_pids=$($pids -join ',') State=$($svc2.State)"

$u = Start-Process msiexec.exe -ArgumentList "/x `"$Msi`" /qn /l*v uninstall.log" -Wait -PassThru
Check 'uninstall-exit-code' ($u.ExitCode -eq 0) "code=$($u.ExitCode)"
Check 'trace-service-gone' ($null -eq (Get-CimInstance Win32_Service -Filter "Name='$name'"))
Check 'trace-programfiles-gone' (-not (Test-Path 'C:\Program Files\SimpleRemoteSpike'))
$dataGone = -not (Test-Path $data)
Check 'trace-programdata-gone' $dataGone
if (-not $dataGone) {
    Write-Output '--- left in ProgramData (as SYSTEM) ---'
    Invoke-AsSystem "dir /s /a $data" | Write-Output
    Write-Output '--- uninstall.log lines about RemoveFolderEx / SPIKEDATADIR ---'
    Select-String -Path uninstall.log -Pattern 'RemoveFolder|SPIKEDATADIR|WixRemoveFoldersEx|RemoveFiles' | ForEach-Object { $_.Line } | Write-Output
    Write-Output '--- install.log lines about SPIKEDATADIR ---'
    Select-String -Path install.log -Pattern 'SPIKEDATADIR' | Select-Object -First 5 | ForEach-Object { $_.Line } | Write-Output
}
Check 'trace-firewall-gone' ($null -eq (Get-NetFirewallRule -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -eq 'SimpleRemote Spike' }))

$all = -not ($results.Values -contains $false)
Write-Output "RESULT wix ok=$($all.ToString().ToLower())"
if (-not $all) { exit 1 }
exit 0
