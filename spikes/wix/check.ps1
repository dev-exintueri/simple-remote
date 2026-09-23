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

# Read a file inside the SYSTEM-only folder as SYSTEM. PsExec does not reliably forward the
# child's stdout, so SYSTEM copies the content to a temp file the runner account can read.
# Paths here contain no spaces, so no inner quotes are passed to cmd.
function Read-AsSystem([string] $path) {
    $tmp = Join-Path $env:RUNNER_TEMP ("read-as-system-" + [guid]::NewGuid() + ".txt")
    & $PsExec -accepteula -nobanner -s cmd /c "type $path > $tmp 2>&1" 2>$null | Out-Null
    if (Test-Path $tmp) { Get-Content $tmp | Out-String } else { '' }
}

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

icacls $data | Write-Output
$acl = Get-Acl $data -ErrorAction SilentlyContinue
$ids = if ($acl) { @($acl.Access | ForEach-Object { $_.IdentityReference.Value } | Sort-Object -Unique) } else { @() }
Check 'data-acl-system-only' ($acl -and $acl.AreAccessRulesProtected -and $ids.Count -eq 1 -and $ids[0] -eq 'NT AUTHORITY\SYSTEM') "ids=$($ids -join ';')"
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
Check 'trace-programdata-gone' (-not (Test-Path $data))
Check 'trace-firewall-gone' ($null -eq (Get-NetFirewallRule -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -eq 'SimpleRemote Spike' }))

$all = -not ($results.Values -contains $false)
Write-Output "RESULT wix ok=$($all.ToString().ToLower())"
if (-not $all) { exit 1 }
exit 0
