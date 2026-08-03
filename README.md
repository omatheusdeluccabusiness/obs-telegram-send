# OBS Telegram Send

Plugin para OBS que oferece o envio manual de uma gravação concluída para o
seu próprio Telegram. Nenhuma gravação é enviada até que você marque
**Enviar este vídeo ao Telegram** na janela exibida ao final da gravação.

## Instalação para clientes

No Windows 10/11 x64, use o [guia de instalação via Codex](docs/instalar-com-codex-pt-BR.md).
Ele instala apenas no perfil do usuário, confere os hashes do pacote e abre o
OBS para o onboarding. A primeira distribuição é uma **prévia para instalação
via Codex**, pois não possui assinatura Microsoft.

O pacote macOS assinado para clientes continua dependente de certificados Apple
Developer. O código e a instalação local já funcionam no Mac de desenvolvimento,
mas não há um instalador macOS público para clientes leigos neste momento.

O pacote instala:

- `obs-telegram-send.plugin` em `/Library/Application Support/obs-studio/plugins/`;
- o agente local e o servidor oficial Telegram Bot API em
  `/Library/Application Support/OBS-Telegram-Send/`;
- um LaunchAgent que inicia somente o agente local em sua sessão.

O agente escuta apenas em `127.0.0.1`. No primeiro início ele cria um bearer
local aleatório em `~/Library/Application Support/OBS-Telegram-Send/`, com
permissão de usuário; o pacote não contém tokens, `api_id`, `api_hash` nem
caminhos de gravações. A configuração do Telegram fica no Keychain do macOS.
Os dados operacionais do servidor oficial ficam em
`~/Library/Application Support/OBS-Telegram-Send/telegram-bot-api-data/` e sua pasta
`temp/`, ambas privadas (`0700`); o agente passa esses caminhos explicitamente
ao processo e não depende do diretório atual do LaunchAgent.
O servidor oficial só é iniciado depois de uma ação explícita do assistente de
configuração. Antes da primeira inicialização local, o agente chama `logOut` na
API cloud do Telegram, como exigido pelo Telegram para que o `/start` possa ser
recebido localmente. Depois do sucesso, grava somente um marcador privado
(`0600`) identificado pelo SHA-256 do token — o token nunca é gravado nesse
marcador. Assim, se o processo local falhar, a próxima tentativa pula o
`logOut` já concluído e valida o bot local com `getMe` antes de aceitar o início.

Leia o guia para clientes em [docs/onboarding-pt-BR.md](docs/onboarding-pt-BR.md)
e a solução de problemas em [docs/troubleshooting-pt-BR.md](docs/troubleshooting-pt-BR.md).

## Limites e privacidade

- O arquivo original é enviado como vídeo para MP4 e como documento para os
  demais formatos; ele nunca é apagado pelo produto.
- O limite é 2 GiB. Arquivos maiores são recusados antes do upload.
- É preciso internet para a configuração e para cada envio ao Telegram.
- O envio é para um chat escolhido por você. Não há nuvem, conta, banco de
  dados remoto ou envio automático do produto.

## Pacotes: desenvolvimento e release

`--development` gera `OBS-Telegram-Send-macOS-development.pkg`: ele é unsigned
no nível do `.pkg`, usa assinatura ad-hoc nos binários e é marcado **não
publicável**. Serve apenas para validação local.

`--release` só gera `OBS-Telegram-Send-macOS.pkg` com Developer ID Application,
Developer ID Installer e perfil do `notarytool`; o script assina, notariza,
stapla e falha se algum requisito ou screenshot de release estiver ausente.
Não publique, compartilhe com clientes ou chame de release o pacote de
desenvolvimento.

Após uma instalação, o postinstall registra o LaunchAgent para o usuário que
está na tela e tenta iniciar somente o agente. Se não houver sessão gráfica, o
log do sistema orienta a encerrar e entrar novamente no macOS.

## Construir o pacote

São necessários macOS arm64, OBS Studio 32.1.1, CMake 3.28+, Rust estável e o
binário oficial arm64 `telegram-bot-api`. O binário oficial não é enviado para
o repositório: informe uma cópia local no momento do empacotamento.

```sh
cmake --build build --target obs-telegram-send
cargo build --manifest-path agent/Cargo.toml --release
bash installer/macos/build-package.sh --development \
  --telegram-bot-api /caminho/para/telegram-bot-api
pkgutil --check-signature dist/OBS-Telegram-Send-macOS-development.pkg
```

`--plugin`, `--agent` e as variáveis `OBS_TELEGRAM_PLUGIN_PATH`,
`OBS_TELEGRAM_AGENT_PATH` e `OBS_TELEGRAM_BOT_API_PATH` permitem apontar para
artefatos diferentes. O script falha se qualquer executável não for arm64 e
gera `dist/OBS-Telegram-Send-macOS.pkg`. Para desenvolvimento fora do pacote,
`OBS_TELEGRAM_BOT_API_PATH` também pode indicar temporariamente o binário
oficial; em uma instalação normal o agente usa sempre o caminho empacotado,
sem consultar o `PATH`. Para `--release`, informe os três dados de assinatura
por flags ou pelas variáveis `OBS_TELEGRAM_DEVELOPER_ID_APPLICATION`,
`OBS_TELEGRAM_DEVELOPER_ID_INSTALLER` e `OBS_TELEGRAM_NOTARY_PROFILE`.

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
