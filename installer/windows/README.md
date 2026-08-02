# Instalador Windows via Codex

O pacote Windows é instalado por usuário, sem gravar em `Program Files` e sem
pedir permissões de administrador. O script verifica os hashes SHA-256 do
bundle, instala o plugin no diretório de plugins do usuário do OBS e cria uma
tarefa do Agendador de Tarefas somente para o usuário atual.

O agente e o Telegram Bot API são instalados em `%LOCALAPPDATA%\OBS-Telegram-Send\bin`.
Os dados privados do agente permanecem fora dessa pasta `bin`, permitindo
atualizações sem apagar a configuração.
