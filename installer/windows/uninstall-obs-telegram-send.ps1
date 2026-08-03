[CmdletBinding()]
param(
  [switch]$RemoveData
)

$ErrorActionPreference = 'Stop'
$taskName = 'OBS-Telegram-Send-Agent'
$runtimeRoot = Join-Path $env:LOCALAPPDATA 'OBS-Telegram-Send'
$runtimeBin = Join-Path $runtimeRoot 'bin'
$pluginRoot = Join-Path $env:APPDATA 'obs-studio\plugins\obs-telegram-send'

Stop-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue
Unregister-ScheduledTask -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue
if (Test-Path -LiteralPath $pluginRoot) { Remove-Item -LiteralPath $pluginRoot -Recurse -Force }
if (Test-Path -LiteralPath $runtimeBin) { Remove-Item -LiteralPath $runtimeBin -Recurse -Force }
if ($RemoveData -and (Test-Path -LiteralPath $runtimeRoot)) {
  Remove-Item -LiteralPath $runtimeRoot -Recurse -Force
}
Write-Host 'OBS Telegram Send removido. Gravações e credenciais do Windows foram preservadas.'
