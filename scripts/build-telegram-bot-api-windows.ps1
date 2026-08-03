[CmdletBinding()]
param(
  [Parameter(Mandatory)] [string]$WorkDirectory,
  [Parameter(Mandatory)] [string]$Output
)

$ErrorActionPreference = 'Stop'
$revision = 'adfd7f6a8e990272851777eeb3ae0def4216f161'
$source = Join-Path $WorkDirectory 'telegram-bot-api'
$build = Join-Path $source 'build-windows-x64'
$vcpkg = 'C:\vcpkg\vcpkg.exe'

if (-not (Test-Path -LiteralPath $vcpkg -PathType Leaf)) { throw 'vcpkg não está disponível no runner Windows.' }
if (-not (Get-Command gperf -ErrorAction SilentlyContinue)) {
  choco install gperf --yes --no-progress --limit-output
}

if (-not (Test-Path -LiteralPath "$source\.git")) {
  git init $source
  git -C $source remote add origin https://github.com/tdlib/telegram-bot-api.git
}
git -C $source fetch --depth 1 origin $revision
$resolved = (git -C $source rev-parse FETCH_HEAD).Trim()
if ($resolved -ne $revision) { throw 'A revisão verificada do Telegram Bot API não confere.' }
git -C $source checkout --detach FETCH_HEAD
git -C $source submodule update --init --recursive --depth 1

& $vcpkg install 'openssl:x64-windows-static' 'zlib:x64-windows-static'
if ($LASTEXITCODE -ne 0) { throw 'Não foi possível instalar as dependências verificadas do Telegram Bot API.' }

cmake -S $source -B $build -G 'Visual Studio 17 2022' -A x64 `
  -DCMAKE_BUILD_TYPE=Release `
  -DCMAKE_TOOLCHAIN_FILE='C:\vcpkg\scripts\buildsystems\vcpkg.cmake' `
  -DVCPKG_TARGET_TRIPLET=x64-windows-static
if ($LASTEXITCODE -ne 0) { throw 'A configuração do Telegram Bot API para Windows falhou.' }
cmake --build $build --target telegram-bot-api --config Release --parallel
if ($LASTEXITCODE -ne 0) { throw 'A compilação do Telegram Bot API para Windows falhou.' }

$candidate = Join-Path $build 'Release\telegram-bot-api.exe'
if (-not (Test-Path -LiteralPath $candidate -PathType Leaf)) { throw 'O executável Windows do Telegram Bot API não foi criado.' }
$outputParent = Split-Path -Parent $Output
if (-not [string]::IsNullOrWhiteSpace($outputParent)) { New-Item -ItemType Directory -Force -Path $outputParent | Out-Null }
Copy-Item -LiteralPath $candidate -Destination $Output -Force
