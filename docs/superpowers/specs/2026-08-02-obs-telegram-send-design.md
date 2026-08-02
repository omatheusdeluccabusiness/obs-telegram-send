# OBS Telegram Send — Design

## Objetivo

Entregar um plugin público para OBS Studio que permita a cada criador enviar manualmente uma gravação concluída para o próprio chat no Telegram, preservando o arquivo original em qualidade integral, até 2 GB, sem uma infraestrutura central operada pelo mantenedor.

## Escopo de lançamento

O primeiro lançamento é para macOS e será validado em uma instalação real do OBS. A arquitetura e o protocolo entre plugin e serviço auxiliar serão independentes de plataforma; a segunda etapa entrega o mesmo fluxo em Windows com instalador e armazenamento seguro de credenciais específicos da plataforma.

## Experiência do cliente

1. O cliente instala um pacote único que adiciona o plugin ao OBS e o serviço auxiliar local ao computador.
2. No OBS, abre **Telegram Send** e inicia o assistente de conexão.
3. O assistente mostra, em linguagem simples, como criar um bot no BotFather, obter o token, obter `api_id` e `api_hash` em `my.telegram.org`, enviar `/start` ao bot e detectar o chat de destino.
4. O assistente realiza um teste de conexão e informa sucesso ou explica o próximo passo para corrigir o problema.
5. Quando uma gravação termina, o plugin abre uma janela central no OBS. Ela apresenta nome, duração e tamanho do arquivo.
6. A caixa **Enviar este vídeo ao Telegram** começa desmarcada. O botão **Enviar agora** fica indisponível até o cliente confirmar a caixa. **Não enviar** fecha a janela e não altera nem apaga o arquivo.
7. Depois da confirmação, a janela mostra progresso, sucesso ou erro. Em caso de erro, oferece **Tentar novamente**. O arquivo original permanece no computador em todos os casos.

## Arquitetura

### Plugin OBS

Um módulo nativo C++/Qt usa a Frontend API do OBS para receber a notificação de que a gravação terminou. Ele resolve o arquivo concluído, apresenta todas as janelas do produto e envia comandos ao serviço auxiliar em `127.0.0.1`. O plugin nunca transmite automaticamente um take: somente a ação explícita na janela de confirmação cria um trabalho de envio.

### Serviço auxiliar local

O serviço roda somente no computador do cliente e escuta exclusivamente em loopback. Ele inicia e supervisiona o Telegram Bot API Server oficial em modo local, o que permite o envio de arquivos de até 2 GB por caminho local. O serviço mantém uma fila persistente de envios, reporta progresso ao plugin e tenta novamente falhas de rede somente quando o cliente clicar em **Tentar novamente**.

### Telegram

Cada cliente usa o próprio bot, token, `api_id`, `api_hash` e chat de destino. A configuração local do Bot API Server é obrigatória para o modo de arquivos grandes. O bot usado nessa configuração não deve ser usado ao mesmo tempo por outra automação que receba updates do Telegram.

### Credenciais

O token, `api_id` e `api_hash` não são registrados em logs, cenas, perfil do OBS, repositório ou arquivos de texto. No macOS, são armazenados no Keychain. O plugin recebe apenas um estado de conexão e o chat selecionado; o serviço auxiliar é o único processo que lê as credenciais ao enviar arquivos.

## Limites e decisões explícitas

- A integração deve aceitar MP4 e MKV e enviar o arquivo original; quando o formato não for reproduzível nativamente no Telegram, ele será enviado como documento.
- O limite de envio no modo local é 2 GB. Acima disso, o produto deve informar que o arquivo não pode ser enviado sem compactação ou divisão, funcionalidades que não pertencem ao primeiro lançamento.
- Não há envio automático, exclusão automática, upload em nuvem do produto, banco de dados remoto, conta do cliente no produto ou mensalidade.
- A internet do cliente continua necessária para o envio ao Telegram.
- A janela de confirmação usa checkbox desmarcado por padrão e botões **Enviar agora** e **Não enviar**.

## Empacotamento e distribuição

O repositório público contém código-fonte, instruções, licenças, testes e pipeline de releases. O GitHub Releases entrega um pacote único do macOS e, na segunda etapa, um instalador do Windows. O pacote instala plugin e serviço auxiliar, mas não inicia nem acessa o bot antes de o cliente concluir o onboarding.

## Validação

- Testes unitários cobrem a validação de configuração, regra de consentimento manual, elegibilidade por tamanho, serialização de comandos e estados de fila.
- Testes de integração cobrem a comunicação loopback com um servidor Telegram simulado.
- Antes de release, validação manual confirma: configuração por cliente leigo, detecção de chat, gravação terminada, checkbox inicialmente desmarcado, nenhum envio sem consentimento, envio real de um MP4 e recuperação manual após erro.

## Fora de escopo inicial

- Compressão, divisão de arquivos e links de download para arquivos maiores que 2 GB.
- Vários chats por usuário.
- Agendamento de publicação ou integração com Instagram.
- Operação de bot compartilhado, armazenamento na nuvem ou painel web.
