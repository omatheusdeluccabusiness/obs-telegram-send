# Solução de problemas — OBS Telegram Send

> **Capturas de tela pendentes antes do lançamento:** o verificador de release
> exige as imagens reais em `images/macos/01-installer.pkg.png`,
> `images/macos/02-tools-menu.png`, `images/macos/03-onboarding.png` e
> `images/macos/04-send-confirmation.png`. Esta versão não usa imagens
> fictícias.

## O menu “Telegram Send” não aparece no OBS

1. Confirme que o pacote foi instalado e feche completamente o OBS.
2. Abra o OBS novamente. Se necessário, encerre e entre na sessão do macOS.
3. Confira se está usando a versão de OBS compatível com a release do pacote.
4. Reinstale somente o pacote obtido da release confiável.

Não mova manualmente a pasta do plugin: ela é instalada em
`/Library/Application Support/obs-studio/plugins/obs-telegram-send.plugin`.

## O macOS bloqueia o instalador de desenvolvimento

O arquivo `OBS-Telegram-Send-macOS-development.pkg` é unsigned e não
notarizado. Ele é somente para testes locais, nunca para distribuição. Em
**Ajustes do Sistema → Privacidade e Segurança**, use **Abrir Mesmo Assim**
somente depois de confirmar que você mesmo gerou o pacote local. Uma release
pública precisa vir assinada e notarizada; se ela for bloqueada, não a contorne
antes de confirmar a origem e comunicar o suporte.

## “Detectar meu chat” não encontra nada

1. Abra a conversa privada correta com o bot.
2. Envie o comando `/start` completo mostrado pelo OBS; ele muda a cada nova
   tentativa do assistente.
3. Volte ao OBS e clique em **Detectar meu chat** novamente.
4. Desative outra automação que esteja recebendo updates desse mesmo bot.

O serviço local precisa chamar `logOut` na API cloud antes de receber updates.
Isso é normal e significa que o mesmo bot não deve receber updates em dois
lugares ao mesmo tempo. Se essa etapa já terminou mas o servidor local falhou,
a próxima tentativa usa um marcador privado sem o token e tenta iniciar
diretamente. O início só é aceito quando a API local confirma o bot com
`getMe`; token inválido continua sendo recusado.

## A mensagem de teste ou o envio falha

- Confirme que há internet e tente novamente.
- No assistente, confira se token, `api_id` e `api_hash` pertencem ao mesmo bot
  e aplicativo do Telegram.
- Confira se a mensagem `/start` chegou ao bot.
- Em uma gravação já autorizada, use **Tentar novamente**. Não é criado envio
  automático de uma gravação nova.

Nunca envie token, `api_hash` ou uma captura desses campos ao pedir ajuda.

O servidor local grava somente em
`~/Library/Application Support/OBS-Telegram-Send/telegram-bot-api/` e em sua
pasta `temp/`. Essas pastas pertencem ao usuário da sessão e têm permissão
privada; ele não tenta criar arquivos em `/` nem usa a pasta do usuário root.

## O arquivo é maior que 2 GiB

O produto recusa o arquivo antes de iniciar o upload. Grave com bitrate/duração
menor, compacte fora do produto ou divida o arquivo manualmente. Compactação e
divisão não pertencem a esta primeira versão.

## Remover o produto

No repositório/clonar que contém o instalador, execute:

```sh
sudo bash installer/macos/uninstall.sh
```

O script para o LaunchAgent da sessão atual e remove exatamente:

- `/Library/Application Support/obs-studio/plugins/obs-telegram-send.plugin`
- `/Library/Application Support/OBS-Telegram-Send`
- `/Library/LaunchAgents/com.obs-telegram-send.agent.plist`

Ele **não** remove gravações e preserva o Keychain e o estado local por usuário.
Isso permite reinstalar sem perder a configuração, mas exige uma limpeza manual
se você quiser remover tudo relacionado ao produto.

### Limpeza manual opcional da configuração

Faça isso somente se quiser esquecer o bot neste Mac. No Terminal, como o mesmo
usuário que configurou o OBS:

```sh
security delete-generic-password \
  -s com.obs-telegram-send.agent \
  -a telegram-configuration
rm -rf "$HOME/Library/Application Support/OBS-Telegram-Send"
```

O primeiro comando remove as credenciais do Keychain; o segundo remove o bearer
local e o estado de fila desse usuário. Nenhum dos dois remove gravações. A
próxima configuração criará um bearer novo.
