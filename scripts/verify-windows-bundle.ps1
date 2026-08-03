[CmdletBinding()]
param(
  [Parameter(Mandatory)] [string]$Bundle
)

$ErrorActionPreference = 'Stop'
if (-not (Test-Path -LiteralPath $Bundle -PathType Leaf)) { throw "Bundle não encontrado: $Bundle" }
$temporary = Join-Path ([IO.Path]::GetTempPath()) "obs-telegram-send-verify-$([guid]::NewGuid().ToString('N'))"
try {
  Expand-Archive -LiteralPath $Bundle -DestinationPath $temporary
  foreach ($relative in @('plugin/obs-telegram-send.dll', 'bin/obs-telegram-agent.exe', 'bin/telegram-bot-api.exe', 'checksums.sha256', 'Install-OBS-Telegram-Send.ps1')) {
    if (-not (Test-Path -LiteralPath "$temporary\$relative" -PathType Leaf)) { throw "Bundle sem $relative" }
  }
  foreach ($script in @('Install-OBS-Telegram-Send.ps1', 'Uninstall-OBS-Telegram-Send.ps1')) {
    $tokens = $null
    $errors = $null
    [System.Management.Automation.Language.Parser]::ParseFile("$temporary\$script", [ref]$tokens, [ref]$errors) | Out-Null
    if ($errors.Count -gt 0) { throw "Script PowerShell inválido: $script" }
  }
} finally {
  if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary -Recurse -Force }
}
