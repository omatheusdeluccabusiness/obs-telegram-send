[CmdletBinding()]
param(
  [Parameter()]
  [string]$BundleRoot = $PSScriptRoot
)

$ErrorActionPreference = 'Stop'
$taskName = 'OBS-Telegram-Send-Agent'
$pluginName = 'obs-telegram-send.dll'
$runtimeRoot = Join-Path $env:LOCALAPPDATA 'OBS-Telegram-Send'
$runtimeBin = Join-Path $runtimeRoot 'bin'
$pluginRoot = Join-Path $env:APPDATA 'obs-studio\plugins\obs-telegram-send\bin\64bit'

function Assert-File([string]$Path) {
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    throw "Arquivo obrigatório não encontrado: $Path"
  }
}

function Assert-Checksums([string]$Root) {
  $manifest = Join-Path $Root 'checksums.sha256'
  Assert-File $manifest
  foreach ($line in Get-Content -LiteralPath $manifest) {
    if ([string]::IsNullOrWhiteSpace($line) -or $line.StartsWith('#')) { continue }
    if ($line -notmatch '^([A-Fa-f0-9]{64})\s+\*?(.+)$') {
      throw "Manifesto de integridade inválido."
    }
    $expected = $matches[1].ToLowerInvariant()
    $relative = $matches[2].Trim()
    if ($relative.Contains('..') -or [IO.Path]::IsPathRooted($relative)) {
      throw "Manifesto de integridade contém um caminho inválido."
    }
    $file = Join-Path $Root $relative
    Assert-File $file
    $actual = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected) {
      throw "A verificação de integridade falhou para $relative. Baixe novamente a release oficial."
    }
  }
}

function Stop-ExistingAgent {
  $task = Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue
  if ($null -ne $task) {
    Stop-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue
    Unregister-ScheduledTask -TaskName $taskName -Confirm:$false
  }
}

function Wait-ForAgentHealth {
  for ($attempt = 0; $attempt -lt 30; $attempt++) {
    try {
      $response = Invoke-RestMethod -Uri 'http://127.0.0.1:43127/health' -TimeoutSec 2
      if ($response.status -eq 'ok') { return }
    } catch {}
    Start-Sleep -Milliseconds 500
  }
  throw 'O serviço local não iniciou. Não abra o OBS ainda; envie esta mensagem ao Codex para diagnóstico.'
}

Assert-File (Join-Path $BundleRoot "plugin\$pluginName")
Assert-File (Join-Path $BundleRoot 'bin\obs-telegram-agent.exe')
Assert-File (Join-Path $BundleRoot 'bin\telegram-bot-api.exe')
Assert-Checksums $BundleRoot

if (Get-Process -Name 'obs64','obs' -ErrorAction SilentlyContinue) {
  throw 'Feche completamente o OBS antes de instalar o OBS Telegram Send e execute este instalador novamente.'
}

Stop-ExistingAgent
New-Item -ItemType Directory -Force -Path $runtimeBin, $pluginRoot | Out-Null

Copy-Item -LiteralPath (Join-Path $BundleRoot 'bin\obs-telegram-agent.exe') -Destination (Join-Path $runtimeBin 'obs-telegram-agent.exe') -Force
Copy-Item -LiteralPath (Join-Path $BundleRoot 'bin\telegram-bot-api.exe') -Destination (Join-Path $runtimeBin 'telegram-bot-api.exe') -Force
Copy-Item -LiteralPath (Join-Path $BundleRoot "plugin\$pluginName") -Destination (Join-Path $pluginRoot $pluginName) -Force

$agentPath = Join-Path $runtimeBin 'obs-telegram-agent.exe'
$action = New-ScheduledTaskAction -Execute $agentPath
$trigger = New-ScheduledTaskTrigger -AtLogOn -User "$env:USERDOMAIN\$env:USERNAME"
$principal = New-ScheduledTaskPrincipal -UserId "$env:USERDOMAIN\$env:USERNAME" -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit (New-TimeSpan -Days 0)
Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Description 'Serviço local do OBS Telegram Send.' | Out-Null
Start-ScheduledTask -TaskName $taskName
Wait-ForAgentHealth

Write-Host 'Instalação concluída. Abra o OBS e use Ferramentas > Telegram Send para configurar seu bot.'
