# Smoke nosd over TCP (Windows host) — onda5/6: hermes continuous + route/chat/submit
$ErrorActionPreference = "Stop"
$nixaios = Split-Path -Parent $PSScriptRoot
$repo = Split-Path -Parent $nixaios
$manifest = Join-Path $nixaios "packages\nosd\Cargo.toml"
$data = Join-Path $env:TEMP "nosd-smoke-data"
New-Item -ItemType Directory -Force -Path $data | Out-Null
$env:CARGO_TARGET_DIR = Join-Path $repo "target\nixaios-check"
$errLog = Join-Path $env:TEMP "nosd-smoke-err.log"

function Invoke-Nosctl([string[]]$CtlArgs) {
  # cargo escreve progresso em stderr — não tratar como erro do PowerShell
  $prev = $ErrorActionPreference
  $ErrorActionPreference = "Continue"
  $out = & cargo run --manifest-path $manifest -p nosd --bin nosctl -- @CtlArgs 2>&1 | Out-String
  $code = $LASTEXITCODE
  $ErrorActionPreference = $prev
  Write-Host $out
  if ($code -ne 0) {
    throw "nosctl failed exit=$code args=$($CtlArgs -join ' ')"
  }
  return $out
}

$proc = Start-Process -FilePath "cargo" -ArgumentList @(
  "run","--manifest-path",$manifest,"-p","nosd","--bin","nosd","--",
  "--foreground","--tcp","127.0.0.1:7700","--data-dir",$data,"--hermes-tick-secs","1"
) -PassThru -NoNewWindow -RedirectStandardError $errLog
Start-Sleep -Seconds 5
$ok = $false
try {
  if ($proc.HasExited) {
    $tail = if (Test-Path $errLog) { Get-Content $errLog -Raw } else { "(no log)" }
    throw "nosd exited early code=$($proc.ExitCode)`n$tail"
  }
  $health = Invoke-Nosctl @("--tcp","127.0.0.1:7700","health")
  if ($health -notmatch '"hermes"\s*:\s*"running"') {
    Start-Sleep -Seconds 2
    $health = Invoke-Nosctl @("--tcp","127.0.0.1:7700","health")
  }
  if ($health -notmatch '"hermes"\s*:\s*"running"') {
    throw "expected hermes=running in health: $health"
  }
  Invoke-Nosctl @("--tcp","127.0.0.1:7700","models") | Out-Null
  $sug = Invoke-Nosctl @("--tcp","127.0.0.1:7700","suggest")
  if ($sug -notmatch '"quant"') { throw "model.suggest missing quant: $sug" }
  Invoke-Nosctl @("--tcp","127.0.0.1:7700","unload") | Out-Null
  Invoke-Nosctl @("--tcp","127.0.0.1:7700","tick") | Out-Null
  Invoke-Nosctl @("--tcp","127.0.0.1:7700","route","implemente uma arquitetura") | Out-Null
  Invoke-Nosctl @("--tcp","127.0.0.1:7700","route","oi") | Out-Null
  Invoke-Nosctl @("--tcp","127.0.0.1:7700","chat","oi nixaios") | Out-Null
  Invoke-Nosctl @("--tcp","127.0.0.1:7700","submit","oi async") | Out-Null
  Invoke-Nosctl @("--tcp","127.0.0.1:7700","spawn","smoke-agent") | Out-Null
  Invoke-Nosctl @("--tcp","127.0.0.1:7700","recall","nixaios") | Out-Null
  $ok = $true
  Write-Host "SMOKE PASS (onda7: hermes + suggest/unload + health/models/tick/route/chat/submit/spawn)"
} finally {
  if (-not $proc.HasExited) {
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
  }
}
if (-not $ok) { exit 1 }
