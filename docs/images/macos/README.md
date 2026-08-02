# Capturas de release do macOS

Antes de gerar `--release`, capture no produto realmente instalado e salve
exatamente estes arquivos neste diretório:

- `01-installer.pkg.png` — instalador do pacote de release.
- `02-tools-menu.png` — OBS com **Ferramentas → Telegram Send**.
- `03-onboarding.png` — assistente sem credenciais visíveis.
- `04-send-confirmation.png` — janela de confirmação com checkbox inicialmente
  desmarcado.

Não substitua essas imagens por mockups, e não inclua token, `api_hash`, chat ID
ou caminhos de gravações. `installer/macos/verify-release-assets.sh --release`
bloqueia a criação de release enquanto qualquer uma estiver ausente.
