# OBS Telegram Send

Plugin para macOS que oferece o envio manual de uma gravação concluída do OBS
para o seu próprio Telegram. Nenhuma gravação é enviada até que você marque
**Enviar este vídeo ao Telegram** na janela exibida ao final da gravação.

O pacote instala:

- `obs-telegram-send.plugin` em `/Library/Application Support/obs-studio/plugins/`;
- o agente local e o servidor oficial Telegram Bot API em
  `/Library/Application Support/OBS-Telegram-Send/`;
- um LaunchAgent que inicia somente o agente local em sua sessão.

O agente escuta apenas em `127.0.0.1`. No primeiro início ele cria um bearer
local aleatório em `~/Library/Application Support/OBS-Telegram-Send/`, com
permissão de usuário; o pacote não contém tokens, `api_id`, `api_hash` nem
caminhos de gravações. A configuração do Telegram fica no Keychain do macOS.
O servidor oficial só é iniciado depois de uma ação explícita do assistente de
configuração. Antes de iniciar o servidor local, o agente chama `logOut` na API
cloud do Telegram, como exigido pelo Telegram para que o `/start` possa ser
recebido localmente.

Leia o guia para clientes em [docs/onboarding-pt-BR.md](docs/onboarding-pt-BR.md)
e a solução de problemas em [docs/troubleshooting-pt-BR.md](docs/troubleshooting-pt-BR.md).

## Limites e privacidade

- O arquivo original é enviado como vídeo para MP4 e como documento para os
  demais formatos; ele nunca é apagado pelo produto.
- O limite é 2 GiB. Arquivos maiores são recusados antes do upload.
- É preciso internet para a configuração e para cada envio ao Telegram.
- O envio é para um chat escolhido por você. Não há nuvem, conta, banco de
  dados remoto ou envio automático do produto.

## Instalação local do pacote

Baixe `OBS-Telegram-Send-macOS.pkg`, abra-o no Finder e siga o instalador. Este
primeiro pacote é **unsigned no nível do .pkg** e **não é notarizado**; os
executáveis internos recebem apenas assinatura ad-hoc. Se o macOS bloquear a
abertura, use a orientação de Segurança e Privacidade descrita no onboarding.

Após instalar, encerre e abra o OBS (ou encerre e entre novamente na sessão do
macOS), abra **Ferramentas → Telegram Send** e siga o assistente.

## Construir o pacote

São necessários macOS arm64, OBS Studio 32.1.1, CMake 3.28+, Rust estável e o
binário oficial arm64 `telegram-bot-api`. O binário oficial não é enviado para
o repositório: informe uma cópia local no momento do empacotamento.

```sh
cmake --build build --target obs-telegram-send
cargo build --manifest-path agent/Cargo.toml --release
bash installer/macos/build-package.sh \
  --telegram-bot-api /caminho/para/telegram-bot-api
pkgutil --check-signature dist/OBS-Telegram-Send-macOS.pkg
```

`--plugin`, `--agent` e as variáveis `OBS_TELEGRAM_PLUGIN_PATH`,
`OBS_TELEGRAM_AGENT_PATH` e `OBS_TELEGRAM_BOT_API_PATH` permitem apontar para
artefatos diferentes. O script falha se qualquer executável não for arm64 e
gera `dist/OBS-Telegram-Send-macOS.pkg`. Para desenvolvimento fora do pacote,
`OBS_TELEGRAM_BOT_API_PATH` também pode indicar temporariamente o binário
oficial; em uma instalação normal o agente usa sempre o caminho empacotado,
sem consultar o `PATH`.

## Desenvolvimento e validação

```sh
cargo test --manifest-path agent/Cargo.toml
cmake -S . -B build \
  -DOBS_APP_BUNDLE=/Applications/OBS.app \
  -DOBS_INCLUDE_DIR=/caminho/para/headers-do-obs-32.1.1
cmake --build build
ctest --test-dir build --output-on-failure
bash installer/macos/package-layout-test.sh
```

Para remover a instalação do sistema sem apagar suas gravações, configuração
do Keychain ou estado local do usuário:

```sh
sudo bash installer/macos/uninstall.sh
```

Detalhes para apagar dados locais de forma opcional estão no troubleshooting.
