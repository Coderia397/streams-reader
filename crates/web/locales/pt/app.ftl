# Streams Reader, português

app-name = Streams Reader

## Common actions
action-back = Voltar
action-cancel = Cancelar
action-close = Fechar
action-delete = Excluir
action-dismiss = Dispensar
action-not-now = Agora não
action-retry = Tentar de novo
toast-reload = Recarregar
toast-undo = Desfazer
toast-update-ready = Uma nova versão está pronta.

## Navigation
nav-label = Principal
nav-library = Biblioteca
nav-voices = Vozes
nav-settings = Ajustes

## Times and numbers
time-left-under-minute = Falta menos de um minuto
time-left-min = Faltam { $minutes } min
time-left-hours = Faltam { $hours } h { $minutes } min
duration-min = { $minutes } min
duration-hours = { $hours } h { $minutes } min
# Storage sizes (decimal units) and the playback speed; NUMBER() writes the language's decimal separator.
size-b = { NUMBER($n, maximumFractionDigits: 0) } B
size-kb = { NUMBER($n, maximumFractionDigits: 1) } kB
size-mb = { NUMBER($n, maximumFractionDigits: 1) } MB
size-gb = { NUMBER($n, maximumFractionDigits: 1) } GB
speed-value = { NUMBER($rate, maximumFractionDigits: 2) }×
seconds-short = { $seconds } s
wait-under-minute = menos de um minuto
wait-minutes = { $minutes ->
    [one] um minuto
   *[other] { $minutes } minutos
}
wait-hours = { $hours ->
    [one] cerca de uma hora
   *[other] cerca de { $hours } horas
}

## Errors
error-offline = Você está sem conexão. Verifique a internet e tente de novo.
error-unexpected = O servidor respondeu algo inesperado. Tente de novo em instantes.
error-browser = Seu navegador recusou: { $detail }
error-bad-request = A solicitação não estava bem certa. Tente de novo.
error-unauthorized = Entre novamente na sua conta.
error-forbidden = Isso não é permitido.
error-not-found = Esse item não existe mais.
error-conflict = Isso mudou em outro lugar. Atualize e tente de novo.
error-too-large = É grande demais para enviar.
error-rate-limited = Tentativas demais. Espere um pouco e tente de novo.
error-rate-limited-wait = Tentativas demais. Tente de novo em { $wait }.
error-gpu-budget = O tempo de vozes expressivas de hoje acabou. Experimente uma voz padrão.
error-gpu-budget-wait = O tempo de vozes expressivas de hoje acabou. Ele se renova em { $wait }.
error-engine-waking = O mecanismo de voz está acordando. Isso pode levar um minuto.
error-engine-failed = O mecanismo de voz não conseguiu ler esse trecho.
error-upstream = Não foi possível carregar a página. Confira o link ou cole o texto.
error-upgrade = Há uma nova versão disponível. Recarregando…
error-internal = Algo deu errado no servidor. Tente de novo.

## Sign in
sign-in-lead = Sua lista de leitura, lida em voz alta.
sign-in-title = Entre no Streams Reader
sign-in-email-label = E-mail
sign-in-email-placeholder = voce@exemplo.com
sign-in-email-invalid = Isso não parece um endereço de e-mail.
sign-in-continue = Continuar com e-mail
sign-in-or = ou
sign-in-with = Continuar com { $provider }
sign-in-invite-label = Código de convite
sign-in-invite-placeholder = Por exemplo STREAMS-1234
sign-in-invite-hint = Por enquanto o Streams Reader funciona só com convite. Peça o código a quem convidou você.
sign-in-code-title = Confira seu e-mail
sign-in-code-label = Código de 6 dígitos
sign-in-verify = Entrar
sign-in-link-hint = Você também pode abrir o link desse e-mail neste aparelho.
sign-in-resend = Enviar novo código
sign-in-resend-wait = Reenviar em { $seconds } s
sign-in-other-email = Usar outro e-mail
sign-in-code-sent-before = Enviamos um código de 6 dígitos para{" "}
sign-in-code-sent-after = .
sign-in-new-here = É novo por aqui? Adicione seu código de convite para criar sua conta.
sign-in-have-invite = Tenho um código de convite
sign-in-oauth-invite = Para criar uma conta assim, informe primeiro seu e-mail e seu código de convite.
sign-in-legal-before = Ao continuar, você concorda com os{" "}
sign-in-legal-and = {" "}e a{" "}
sign-in-legal-after = .
legal-terms = Termos
legal-privacy = Privacidade
auth-error-offline = Não foi possível acessar o serviço de login. Verifique sua conexão.
auth-error-code = O código está errado ou expirou. Tente de novo ou peça um novo.
auth-invite-needed = Você precisa de um código de convite para criar uma conta.
auth-error-invite = Esse código de convite não é válido ou já foi usado.
auth-error-rate = Tentativas demais. Espere um minuto e tente de novo.
auth-error-session = Sua sessão foi encerrada. Entre novamente.
auth-error-other = Não foi possível entrar. Tente de novo em instantes.
invite-used = Esse código de convite já foi usado.
invite-expired = Esse código de convite expirou. Peça um novo.
invite-taken = Esse código de convite pertence a outro e-mail.
invite-unknown = Não reconhecemos esse código de convite. Confira e tente de novo.
auth-callback-busy = Entrando…
auth-callback-failed = O login não foi concluído
auth-callback-back = Voltar ao login

## First run
welcome-title = Boas-vindas ao Streams Reader
welcome-lead = Dois ajustes rápidos para tocar sem interrupções, mesmo com a tela apagada.
welcome-install-title = Adicione à tela inicial
welcome-install-line = Abre em tela cheia e aparece nos controles de mídia.
welcome-install = Instalar
welcome-installed = Instalado
welcome-installed-toast = Instalado. Abra pela tela inicial.
welcome-install-manual = No Chrome, abra o menu ⋮ e escolha Adicionar à tela inicial.
welcome-background-title = Deixe tocar em segundo plano
welcome-background-line = O Android pode pausar o áudio alguns minutos depois que a tela apaga. Permita que o Chrome funcione em segundo plano:
welcome-android-1 = Abra Configurações, depois Apps e depois Chrome.
welcome-android-2 = Toque em Uso da bateria do app.
welcome-android-3 = Escolha Sem restrições.
welcome-samsung-line = Em celulares Samsung, tire também o Chrome das listas de suspensão:
welcome-samsung-1 = Abra Configurações, depois Bateria e depois Limites de uso em segundo plano.
welcome-samsung-2 = Remova o Chrome de Apps em suspensão e de Apps em suspensão profunda.
welcome-keys-title = Ouça pelo teclado
welcome-keys-line = No leitor, a barra de espaço toca e pausa, J e L pulam, e [ e ] mudam a velocidade.
welcome-start = Começar a ouvir

## Library
library-title = Biblioteca
library-search = Pesquisar na sua biblioteca
library-search-placeholder = Título, autor ou site
library-search-empty-line = Tente outra palavra ou pesquise dentro de um documento pelo leitor.
library-continue = Continuar ouvindo
library-all = Todos os documentos
library-more = Mais ações
library-play = Ouvir
library-delete = Excluir da biblioteca
library-removed = “{ $title }” foi removido.
library-empty-title = Nada para ouvir ainda
library-empty-line = Adicione um artigo, um livro ou suas próprias anotações e aperte o play.
library-share-tip = Dica: compartilhe qualquer página do navegador com o Streams Reader.
library-unreachable = Não foi possível acessar sua biblioteca
library-unreachable-line = Verifique sua conexão e tente de novo.
library-preparing = Preparando…
library-upload-failed = O envio não terminou
library-retry-gone = Este arquivo não está mais neste dispositivo. Exclua e adicione de novo.
doc-finished = Concluído
source-web = Página da web
source-text = Texto
source-book = Livro
source-pdf = PDF
source-text-file = Arquivo de texto
source-markdown = Markdown

## Adding documents
add-title = Adicionar à biblioteca
add-paste = Colar texto
add-paste-hint = Um artigo, um capítulo, o que quiser
add-link = Colar um link
add-link-hint = Funciona melhor com artigos e blogs
add-file = Abrir um arquivo
add-file-hint = EPUB, PDF, TXT, Markdown ou HTML
add-paste-title-label = Título (opcional)
add-paste-text-label = Texto
add-link-label = Endereço web
add-paste-placeholder = Cole ou digite o texto aqui.
add-paste-submit = Adicionar texto
add-paste-empty = Cole algum texto primeiro.
add-link-help = A página é limpa para que só o conteúdo seja lido.
add-link-submit = Adicionar link
add-link-invalid = Isso não parece um endereço da web.
add-file-error = Não foi possível abrir o arquivo: { $detail }
add-share-tip = Você também pode compartilhar uma página ou um arquivo com o Streams Reader a partir de outros apps.
add-done = Adicionado à sua biblioteca.
import-stage-fetching = Buscando a página…
import-stage-reading = Lendo…
import-stage-uploading = Adicionando à sua biblioteca…
import-error-empty = Não há texto legível aí.
import-error-too-large = É grande demais ({ $detail }).
import-error-damaged = O arquivo parece danificado e não pôde ser lido.
import-error-unsupported = Não compatível: { $detail }.
import-error-pdf = Não foi possível ler esse PDF ({ $detail }).
import-error-file-type = Esse tipo de arquivo não é compatível. Tente EPUB, PDF, TXT, Markdown ou HTML.
import-error-worker = O importador não iniciou. Recarregue a página e tente de novo.

## Reader and player
reader-back = Voltar à biblioteca
reader-more = Mais
reader-play-here = Ouvir a partir daqui
reader-back-to-voice = Voltar à voz
reader-end = Fim
reader-no-text = Este documento não tem texto legível.
reader-hide-text = Ouvir sem o texto
reader-hide-text-hint = Mostra só a capa enquanto toca
reader-show-text = Mostrar o texto
reader-text-size = Tamanho do texto
bookmark-add = Marcar este ponto
bookmark-added = Marcador salvo.
bookmark-exists = Este trecho já tem um marcador.
bookmark-remove = Remover marcador
reader-text-smaller = Texto menor
reader-text-larger = Texto maior
player-play = Tocar
player-pause = Pausar
player-now-playing = Tocando agora
player-position = Posição no documento
player-chapter-left = Faltam { $time } no capítulo
player-skip-back = Voltar { $seconds } segundos
player-skip-forward = Avançar { $seconds } segundos
player-error = A reprodução parou de forma inesperada. Toque em tocar para tentar de novo.
contents-title = Sumário
search-doc-title = Pesquisar no texto
search-doc-placeholder = Encontre uma palavra ou frase
search-doc-hint = Digite pelo menos duas letras.
search-doc-count = { $count ->
    [one] Um resultado
   *[other] { $count } resultados
}
search-no-results = Nenhum resultado
keys-title = Atalhos de teclado
keys-toggle = Tocar ou pausar
keys-skip = Voltar ou avançar
keys-speed = Mais devagar ou mais rápido
keys-paragraph = Parágrafo anterior ou seguinte
keys-follow = Voltar à voz
keys-search = Pesquisar no texto

## Speed and sleep
speed-title = Velocidade
speed-slower = Mais devagar
speed-faster = Mais rápido
speed-hint = O tom continua natural em qualquer velocidade.
sleep-title = Timer de sono
sleep-on = Timer de sono ativado
sleep-off = Desligado
sleep-minutes = { $minutes } min
sleep-end-chapter = Fim do capítulo
sleep-stops-in = Para em { $time }
sleep-hint = Os últimos dez segundos diminuem suavemente.
sleep-set = Timer de sono: { $choice }
sleep-cleared = Timer de sono desligado
sleep-ended = O timer terminou. Bom descanso.

## Voices
voices-title = Vozes
voices-lead = Escolha a voz que lê para você. Toque no alto-falante para ouvir antes.
voices-favourites = Favoritas
voices-recent = Recentes
voices-yours = Suas vozes
voices-standard = Padrão
voices-standard-hint = Escuta ilimitada. Começa rápido.
voices-expressive = Expressivas
voices-expressive-hint = Usa o saldo expressivo diário.
voices-expressive-line = Uma leitura mais rica e emotiva. Cada prévia e cada minuto de áudio usam o saldo diário.
voices-budget-today = Saldo expressivo de hoje
voices-budget-numbers = { $left } de { $cap } s · { $runs ->
        [one] { $runs } uso
       *[other] { $runs } usos
    }
voices-more = Mais vozes
voices-fewer = Menos vozes
voice-title = Voz
voice-switched = Agora lendo com { $name }
voice-default-set = { $name } agora é sua voz padrão
voice-budget-left = Restam hoje cerca de { $minutes } min de áudio expressivo e { $runs ->
        [one] { $runs } uso
       *[other] { $runs } usos
    }.
voice-badge-expressive = Expressiva
voice-favourite = Adicionar às favoritas
voice-unfavourite = Remover das favoritas
voice-preview = Ouvir { $name }
voice-manage = Gerenciar voz
voice-manage-line = Ao excluir, a voz e a gravação usada para criá-la são apagadas.
voice-delete = Excluir esta voz
voice-deleted = { $name } foi excluída.
voice-ask-title = Ouvir uma prévia expressiva?
voice-ask-line = Isso gera uma amostra curta na GPU e usa cerca de { $seconds } segundos do saldo de hoje.
voice-ask-left = Restam hoje { $left ->
        [one] { $left } segundo
       *[other] { $left } segundos
    } e { $runs ->
        [one] { $runs } uso
       *[other] { $runs } usos
    }.
voice-ask-play = Ouvir prévia
voice-state-needs-recording = precisa de gravação
voice-state-preparing = preparando…
voice-state-failed = falhou
voice-yours-fallback = Sua voz

accent-american = Americano
accent-british = Britânico
gender-female = Feminina
gender-male = Masculina

## Recording a voice
record-title = Grave sua voz
record-cta = Grave sua voz
record-cta-line = Leia oito frases curtas. Cerca de dois minutos.
record-tips = Procure um lugar silencioso, segure o celular a um palmo da boca e leia cada frase em inglês com a emoção indicada.
record-name = Nome
record-default-name = Minha voz
record-start = Gravar
record-stop = Parar
record-again = Gravar de novo
record-progress = { $done } de { $total } frases gravadas
record-create = Criar voz
record-uploading = Enviando sua gravação…
record-preparing = Preparando sua voz…
record-wait = Leva um ou dois minutos. Você pode sair desta tela; o processo continua.
record-ready = Sua voz está pronta
record-ready-line = Ela lê com o mecanismo expressivo, que usa o saldo diário.
record-use = Usar minha voz
record-failed-title = Não deu certo
record-back = Voltar à gravação
record-line-good = A frase { $line } ficou boa.
record-failed = A gravação falhou: { $detail }
record-every-line = Grave todas as frases primeiro.
record-prepare-failed = Não foi possível transformar a gravação em uma voz. Tente de novo num lugar mais silencioso.
record-vanished = A voz sumiu enquanto era preparada.
record-still-preparing = Ainda preparando. Ela aparecerá em Suas vozes quando estiver pronta.
record-error-blocked = O acesso ao microfone está bloqueado. Permita nas configurações do site no navegador e tente de novo.
record-error-no-mic = Nenhum microfone encontrado.
record-error-other = Não foi possível começar a gravar: { $detail }
record-issue-clipping = Alto demais: distorceu. Afaste-se um pouco e leia de novo.
record-issue-noisy = Há ruído de fundo. Procure um lugar mais silencioso, se puder.
record-issue-quiet = Muito baixo. Aproxime um pouco o celular.
record-issue-short = Ficou muito curto. Leia a frase inteira.
mood-warm = Acolhedor
mood-calm = Calmo
mood-delighted = Encantado
mood-sorry = Arrependido
mood-firm = Firme
mood-curious = Curioso
mood-amused = Divertido
mood-gentle = Suave

## Settings
settings-title = Ajustes
settings-account = Conta
settings-listening = Audição
settings-appearance = Aparência
settings-storage = Armazenamento
settings-engine = Mecanismo de voz
settings-about = Sobre
settings-default-voice = Voz padrão
settings-default-speed = Velocidade padrão
settings-skip-back = Voltar
settings-skip-forward = Avançar
settings-theme = Tema
settings-language = Idioma
theme-ink = Tinta
theme-paper = Papel
theme-night = Noite
theme-system = Sistema
language-auto = Igual ao aparelho
language-auto-named = Automático ({ $language })
account-email = E-mail
account-offline = Sem conexão
account-method-email = Código por e-mail
account-unlink = Desvincular
account-unlinked = Método de login removido.
account-methods = Formas de entrar
account-linked = Vinculado
account-not-linked = Não vinculado
account-link-short = Vincular
account-sign-out = Sair
account-sign-out-all = Sair em todos os aparelhos
account-sign-out-all-confirm = Sair em todos os aparelhos?
account-sign-out-all-line = Cada celular, tablet e computador conectado a esta conta precisará entrar de novo.
account-sign-out-all-action = Sair em todos
storage-used = Em uso neste aparelho
storage-clear = Limpar áudio salvo
storage-hint = O áudio que você já ouviu fica salvo para tocar sem conexão.
storage-clear-title = Limpar o áudio salvo?
storage-clear-line = Limpar libera espaço. Os trechos são baixados de novo quando necessário.
storage-clear-confirm = Limpar
storage-cleared = Áudio salvo apagado.
engine-voice = Status
engine-awake = Ativo
engine-asleep = Em repouso (acorda no primeiro uso)
engine-starting = Iniciando
engine-unavailable = Indisponível
engine-gpu-today = Saldo expressivo de hoje
engine-gpu-left = { $left } de { $cap } s · { $runs ->
        [one] { $runs } uso
       *[other] { $runs } usos
    }
engine-queue = Na fila
engine-refresh = Atualizar status
about-version = Versão
about-setup = Instalação e reprodução em segundo plano
about-source = Código-fonte
about-colophon = Streams Reader · gratuito e de código aberto

## Not found
not-found-title = Nada por aqui
not-found-line = Essa página não existe.
not-found-back = Voltar à biblioteca
