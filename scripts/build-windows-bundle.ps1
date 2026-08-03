[CmdletBinding()]
param(
  [Parameter(Mandatory)] [string]$Plugin,
  [Parameter(Mandatory)] [string]$Agent,
  [Parameter(Mandatory)] [string]$TelegramBotApi,
  [Parameter(Mandatory)] [string]$Output
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$stage = "$Output.stage-$([guid]::NewGuid().ToString('N'))"

function Assert-File([string]$Path) {
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "Arquivo obrigatório não encontrado: $Path" }
}

function Assert-X64Pe([string]$Path) {
  Assert-File $Path
  $bytes = [IO.File]::ReadAllBytes((Resolve-Path -LiteralPath $Path))
  if ($bytes.Length -lt 64 -or $bytes[0] -ne 0x4d -or $bytes[1] -ne 0x5a) {
    throw "O arquivo não é um executável Windows: $Path"
  }
  $offset = [BitConverter]::ToInt32($bytes, 0x3c)
  if ($offset -lt 0 -or $offset + 6 -gt $bytes.Length -or $bytes[$offset] -ne 0x50 -or $bytes[$offset + 1] -ne 0x45) {
    throw "Cabeçalho PE inválido: $Path"
  }
  if ([BitConverter]::ToUInt16($bytes, $offset + 4) -ne 0x8664) {
    throw "O arquivo não é Windows x64: $Path"
  }
}

Assert-X64Pe $Plugin
Assert-X64Pe $Agent
Assert-X64Pe $TelegramBotApi
if (Test-Path -LiteralPath $stage) { throw "Área temporária já existe: $stage" }

try {
  New-Item -ItemType Directory -Path "$stage\plugin", "$stage\bin" | Out-Null
  Copy-Item -LiteralPath $Plugin -Destination "$stage\plugin\obs-telegram-send.dll"
  Copy-Item -LiteralPath $Agent -Destination "$stage\bin\obs-telegram-agent.exe"
  Copy-Item -LiteralPath $TelegramBotApi -Destination "$stage\bin\telegram-bot-api.exe"
  Copy-Item -LiteralPath "$root\installer\windows\install-obs-telegram-send.ps1" -Destination "$stage\Install-OBS-Telegram-Send.ps1"
  Copy-Item -LiteralPath "$root\installer\windows\uninstall-obs-telegram-send.ps1" -Destination "$stage\Uninstall-OBS-Telegram-Send.ps1"

  $manifestFiles = @('plugin/obs-telegram-send.dll', 'bin/obs-telegram-agent.exe', 'bin/telegram-bot-api.exe')
  $manifest = foreach ($relative in $manifestFiles) {
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath "$stage\$relative").Hash.ToLowerInvariant()
    "$hash  $relative"
  }
  Set-Content -LiteralPath "$stage\checksums.sha256" -Value $manifest -NoNewline

  $parent = Split-Path -Parent $Output
  if (-not [string]::IsNullOrWhiteSpace($parent)) { New-Item -ItemType Directory -Force -Path $parent | Out-Null }
  if (Test-Path -LiteralPath $Output) { Remove-Item -LiteralPath $Output -Force }
  Compress-Archive -Path "$stage\*" -DestinationPath $Output -CompressionLevel Optimal
} finally {
  if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
}

Write-Host "Bundle Windows criado: $Output"
