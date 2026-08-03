# Instalar OBS Telegram Send com Codex — Windows

Este método é para Windows 10 ou 11 de 64 bits com OBS Studio 32.1.1. Você
precisa usar o app do Codex no próprio computador onde grava no OBS.

## Copie e cole este pedido no Codex

> Instale o OBS Telegram Send para Windows a partir da release oficial do
> repositório `omatheusdeluccabusiness/obs-telegram-send`. Baixe somente o
> arquivo `OBS-Telegram-Send-Windows-x64-Codex.zip` da release mais recente,
> confirme o hash publicado na release, feche o OBS, extraia o ZIP e execute
> `Install-OBS-Telegram-Send.ps1` no PowerShell como meu usuário atual. Não
> instale nada em Program Files, não peça senha de administrador e não me peça
> token do Telegram no chat. Verifique `http://127.0.0.1:43127/health`, abra o
> OBS e depois me guie pelo menu Ferramentas > Telegram Send para eu inserir
> meus dados diretamente no OBS.

## O que o Codex faz

- Confere os hashes SHA-256 antes de instalar os arquivos.
- Instala o plugin somente no seu diretório de plugins do OBS.
- Instala o serviço local somente em `%LOCALAPPDATA%` e o inicia para você.
- Deixa o bot e as credenciais para serem preenchidos exclusivamente nas telas
  do OBS, nunca no chat.

O Windows pode mostrar um aviso do SmartScreen por este pacote não ter
assinatura Microsoft. Antes de permitir, confirme que o arquivo veio da
release oficial indicada acima e que o Codex validou seu hash.

Depois da instalação, siga o [onboarding](onboarding-pt-BR.md). O plugin não
envia nada automaticamente: após cada gravação, a caixa **Enviar este vídeo ao
Telegram** começa desmarcada.
