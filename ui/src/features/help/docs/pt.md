## 1. O que é o Container Manage Center {#about}

O Container Manage Center é uma aplicação de ambiente de trabalho para administrar o **Docker Engine** no Linux sem depender do terminal. Tudo o que mostra são dados reais que pede ao Docker nesse momento, e cada ação é uma operação real: nada é simulado.

- Comunica com o Docker Engine através do seu **socket Unix**, com a API oficial.
- Executa o **Docker Compose** com o seu programa oficial, com os argumentos separados e sem shell.
- Cada operação fica registada no **painel de atividade** com o comando `docker` equivalente, para que saiba exatamente o que foi feito.

A janela tem uma **barra lateral** com as secções (Ctrl+B recolhe-a), a **área de trabalho**, uma **barra superior** com o indicador de ligação e o botão **Atividade**, e o **painel de atividade** à direita (Ctrl+Shift+A).

## 2. Requisitos do sistema e dependências {#requirements}

- **Sistema**: Debian 12 ou posterior, ou Ubuntu 22.04 ou posterior, de 64 bits (x86_64).
- **Docker Engine 20.10 ou posterior** (API 1.41+), em modo normal ou rootless.
- **Docker Compose v2 ou posterior**, apenas para a secção Docker Compose (como plugin `docker compose` ou binário `docker-compose` v2). O Compose v1 não é suportado.
- **Dependências de execução**: WebKitGTK 4.1, GTK 3, glibc e bibliotecas associadas. O APT instala-as com o pacote.

A aplicação não precisa de Internet. Só as operações que o Docker Engine faz contra registos remotos (transferir imagens, pesquisar no Docker Hub, construir com imagens base que não existem no computador) precisam de rede no Docker.

## 3. Instalação e primeira execução {#install}

Instale o pacote com o APT, que resolve as dependências:

```
sudo apt install ./container-manage-center_X.Y.Z_amd64.deb
```

A aplicação aparece no menu como **Container Manage Center**. É executada com o seu utilizador normal: **não a execute com sudo** (não arranca como root).

Ao abrir, procura o Docker Engine automaticamente. Se o encontrar, o **Painel** mostra o seu estado; caso contrário, mostra um diagnóstico com a causa e os passos para a resolver.

Para atualizar, instale o novo pacote da mesma forma: a sua configuração e o histórico são mantidos.

## 4. Configuração inicial {#first-setup}

Não é preciso configurar nada para começar: a ligação ao Docker é automática. Em **Definições** pode escolher:

- O **tema** (Sistema, Claro ou Escuro) e o **idioma**. A cor principal é o roxo.
- Se o painel de atividade se abre sozinho quando uma operação falha, e quantas operações guarda.
- Se o histórico de operações e eventos é guardado entre sessões.
- Um socket do Docker concreto, se não quiser a deteção automática.

Tudo é guardado de imediato em `~/.config/container-manage-center/config.toml`.

## 5. Ligação ao Docker Engine {#connection}

O **indicador de ligação** da barra superior mostra o estado:

| Aspeto | Significado |
|---|---|
| Ponto verde e `Docker 29.x` | Ligado. Ao passar o rato vê-se o socket |
| `A ligar ao Docker…` | A detetar ou a ligar |
| Vermelho, `Sem ligação ao Docker` | Sem ligação. Clique para ver o diagnóstico |

Em modo **automático**, a aplicação experimenta por esta ordem: a variável `DOCKER_HOST` (apenas `unix://`), o contexto ativo do Docker, o seu socket rootless e o socket do sistema `/var/run/docker.sock`. Vigia a ligação a cada 10 segundos e volta a ligar-se sozinha quando o Docker fica novamente disponível.

A aplicação nunca pede sudo, não altera permissões nem grupos e não executa as instruções do diagnóstico por si. O seu utilizador tem de conseguir abrir o socket: com o Docker rootless ou pertencendo ao grupo `docker` (o que, na prática, equivale a ter privilégios de administrador).

## 6. Docker rootless e ligações através de socket {#rootless}

Com o **Docker rootless**, o daemon é executado com o seu utilizador e o socket está em `$XDG_RUNTIME_DIR/docker.sock` (normalmente `/run/user/1000/docker.sock`). A aplicação deteta-o sozinha, também através do contexto `rootless`, e assinala-o com a etiqueta **rootless**. É a opção recomendada por segurança.

Em **Definições › Ligação ao Docker** pode:

- Escolher **Socket personalizado** e escrever um caminho absoluto.
- **Testar ligação**: verifica o caminho sem o guardar.
- **Guardar e ligar**: aplica a configuração.
- **Voltar a ligar**: repete a deteção.

As ligações `tcp://` não são suportadas. Os servidores remotos administram-se por SSH a partir de **Servidores** (secção seguinte).

## 7. Servidores remotos por SSH {#remote-servers}

Em **Servidores** pode administrar o Docker de outros computadores Linux a que tenha acesso por **SSH**, ao mesmo tempo que o deste computador. A ligação usa o cliente SSH do sistema (`ssh`) e executa `docker system dial-stdio` no servidor: o Docker **nunca** é exposto na rede (nada de TCP 2375) e não é aberta nenhuma porta.

**Requisitos do servidor**: servidor SSH em execução, Docker Engine 20.10 ou posterior (API 1.41) e um utilizador que possa usar o Docker. Para o Compose, `docker compose` ou `docker-compose` no servidor. A aplicação **não instala o Docker, não altera permissões, não mexe na configuração SSH nem abre portas**: se faltar algo, diz-lhe o quê e como resolver.

**Adicionar servidor** (assistente): dados gerais (nome, anfitrião, utilizador, porta 22), autenticação, Docker (predefinido, um socket concreto ou um contexto) e **Testar ligação**, que verifica sete passos: DNS, ligação SSH, identidade do servidor, autenticação, Docker disponível, permissões e compatibilidade da API.

- **Autenticação**: agente SSH (recomendado), chave privada, chave com frase-passe ou palavra-passe (desaconselhada). A chave privada **nunca é copiada**: só é guardado o caminho.
- **Credenciais**: a frase-passe ou a palavra-passe são pedidas ao ligar. Se escolher **lembrá-la**, é guardada no **armazenamento seguro do sistema** (GNOME Keyring, KWallet…); se não houver nenhum, é pedida de cada vez e **nunca** é guardada em texto simples. Pode apagá-la ao editar o servidor.
- **Identidade do servidor**: na primeira vez é mostrada a **impressão digital** (`SHA256:…`) para a verificar e confirmar. Se mudar, a ligação é interrompida com um aviso de possível **ataque de intermediário**; só é substituída depois de a rever. Se a chave anterior estiver no seu `~/.ssh/known_hosts`, a aplicação não modifica esse ficheiro e dá-lhe o comando para o fazer.

**Vários servidores**: o **seletor** da barra superior indica em que servidor está a trabalhar, com o seu estado, e distingue os remotos. Ao mudar de servidor, todo o ecrã passa para esse servidor. **Vista geral** mostra todos ao mesmo tempo (estado, versão, contentores e recursos). Cada servidor é independente: um em baixo não afeta os outros, e cada operação vai para o servidor em que a iniciou. As confirmações indicam **servidor e endereço**, e o painel de atividade mostra o servidor de cada operação e permite filtrar por ele.

**Num servidor remoto** funcionam contentores, imagens, redes, volumes, terminais e Compose como em local. Os caminhos (ficheiros Compose, pastas dos bind mounts) são **do servidor**: escrevem-se à mão e são verificados lá; antes de escrever um ficheiro no servidor confirma-se o destino. Em **Terminais** também pode abrir a **shell SSH do servidor**, um tipo de sessão à parte.

**Desligar** fecha a sessão SSH e os seus terminais; **eliminar o perfil** apaga só os dados da aplicação (perfil, credenciais e identidade guardada). Nenhuma das duas coisas mexe nos contentores, imagens, redes, volumes ou dados do servidor.

Pertencer ao grupo `docker` do servidor equivale a ter acesso de **root** nele: decida com o administrador; o Docker rootless é a alternativa mais segura.

## 8. Painel e diagnóstico {#dashboard}

Com ligação, o **Painel** mostra dados obtidos nesse momento: versão do Docker Engine e da API, socket e origem da ligação, contentores (em execução, em pausa, parados), imagens, dados do computador (sistema, kernel, CPU, memória), armazenamento e execução (controlador, diretório de dados, cgroups, registos) e avisos do daemon. **Atualizar** volta a pedi-los.

Sem ligação, mostra o **diagnóstico**: a causa concreta, os passos para a resolver (com comandos para copiar para um terminal), os sockets verificados com o seu resultado e a instalação detetada. **Voltar a ligar** e **Configurar ligação** estão no cabeçalho.

## 9. Gestão de contentores {#containers}

A secção **Contentores** lista todos os contentores, como `docker ps -a`: estado, nome e ID curto, projeto Compose, imagem, estado do Docker, portas publicadas e antiguidade. Pode filtrar por **Todos / Em execução / Parados** e pesquisar por nome, imagem, ID ou projeto. O menu **⋯** de cada linha (ou o botão direito) oferece **Editar configuração** e **Criar imagem a partir deste contentor**.

| Estado | Ações disponíveis |
|---|---|
| Em execução | Parar, Reiniciar, Pausar, Eliminar |
| Em pausa | Retomar, Parar, Eliminar |
| Criado ou parado | Iniciar, Eliminar |

Parar e reiniciar esperam 10 segundos que o processo termine antes de o forçar, como o Docker. Depois de cada ação, é consultado o estado real do contentor. Se o Docker recusar uma ação, a sua mensagem aparece por cima da tabela e no painel de atividade. As listas atualizam-se sozinhas quando algo muda no Docker, mesmo a partir do terminal.

## 10. Criar, duplicar, inspecionar e eliminar contentores {#container-lifecycle}

**Criar contentor** abre um assistente com seis separadores (só a imagem é obrigatória): Geral, Ambiente, Rede, Armazenamento, Recursos e Avançado.

- **Comando** e **Ponto de entrada** escrevem-se como num terminal e são divididos em argumentos, que vê por baixo. Não é executada nenhuma shell.
- O painel **Equivalente** mostra o `docker run` equivalente e o corpo exato que é enviado para a API.
- A **revisão** assinala erros (impedem a criação) e avisos (portas privilegiadas, pastas sensíveis com escrita, modo privilegiado…).
- **Criar** cria o contentor sem o iniciar; **Criar e iniciar** também o inicia.

Ao clicar no nome de um contentor abre-se o seu **detalhe**: Resumo, Registos, Terminal, Estatísticas e Inspeção JSON (a resposta completa de `docker inspect`, com botão Copiar).

**Duplicar** abre o assistente com a configuração do contentor e o nome `<nome>-copia`, e indica as definições que não são transferidas.

**Eliminar** pede confirmação. Um contentor em execução ou em pausa só é eliminado se marcar **Forçar paragem e eliminar**. Opcionalmente são eliminados os seus volumes anónimos; os volumes com nome e as pastas ligadas não são tocados. Não se pode desfazer.

## 11. Editar a configuração de um contentor {#container-edit}

**Editar configuração** está no menu **⋯** de cada contentor (também com o botão direito sobre a linha) e no cabeçalho do detalhe. A configuração está organizada em separadores: **Geral**, **Rede**, **Armazenamento**, **Variáveis**, **Portas**, **Recursos** e **Avançado**. Cada alteração indica como é aplicada:

| Alteração | Como é aplicada |
|---|---|
| Nome | Diretamente (`docker rename`) |
| Política de reinício, CPU, memória e memória reservada | Diretamente (`docker update`), sem reiniciar o contentor |
| Ligar ou desligar redes | Diretamente e de imediato (`docker network connect` / `disconnect`) |
| Variáveis de ambiente, portas, volumes e pastas, descrição | É preciso **recriar** o contentor |
| Retirar um limite de CPU ou de memória | É preciso recriar (`docker update` não sabe retirá-lo) |
| Imagem, comando, utilizador, diretório de trabalho e o resto das opções do separador **Avançado** | Só de leitura: o Docker não permite alterá-las. Use **Duplicar** |

- **Rede**: mostra as redes, o IP, o MAC e os aliases. O Docker não permite alterar os aliases ou o IP de uma ligação existente: é preciso desligar e voltar a ligar. Se for a única rede do contentor, é dado um aviso antes de a desligar.
- **Variáveis**: os valores que parecem segredos são ocultados no ecrã e nunca são guardados no painel de atividade.
- **Descrição**: é guardada na etiqueta `cmc.description` do contentor.

**Rever alterações** compara a configuração atual com a nova: o que muda (antes e depois, elemento a elemento), o que se mantém e o que exige recriar. Nada é aplicado até que confirme.

**Recriação segura.** Quando uma alteração exige recriar, o assistente:

1. Verifica a imagem, os conflitos de nome e de portas com outros contentores e os motivos que impedem recriar com segurança: contentores do Compose ou do Swarm, criados com `--rm`, em pausa ou a reiniciar. Nesses casos não deixa continuar.
2. Mostra os riscos que tem de aceitar um a um: ficheiros escritos fora dos volumes (não passam para o contentor novo), montagens que deixam de estar montadas, contentores que usam a rede deste…
3. Ao confirmar: para o contentor se estiver em execução, muda-lhe o nome para `<nome>-cmc-backup-<data>` (a cópia de segurança), cria o novo com a configuração **completa** do original mais as suas alterações, liga as suas redes e, se estava em execução, inicia-o e verifica que continua a funcionar durante alguns segundos.
4. Se algo falhar, elimina o contentor novo (sem tocar nos seus volumes), devolve o nome ao original e volta a iniciá-lo.

**Nunca são eliminados volumes, pastas nem imagens.** Os volumes anónimos são montados de novo com o seu conteúdo. A cópia fica parada em **Geral › Cópias de segurança**; a partir daí pode **Restaurar** (o contentor atual é posto de parte, não é eliminado) ou **Eliminar** quando já não precisar dela. Todas as operações ficam no painel de atividade, sem valores sensíveis.

## 12. Criar uma imagem a partir de um contentor {#container-commit}

**Criar imagem a partir deste contentor** (menu **⋯** da lista e cabeçalho do detalhe) equivale a `docker commit`: guarda o estado atual do sistema de ficheiros do contentor como uma imagem nova.

- Indique o **repositório** e a **etiqueta** (vazia = `latest`); opcionalmente, o **autor** e uma **mensagem**. O nome completo é mostrado e validado antes de criar.
- **Não inclui os volumes nem as pastas montadas**: apenas a camada de escrita do contentor. A caixa de diálogo enumera as montagens que ficam de fora.
- A imagem **guarda a configuração do contentor**, incluindo as suas variáveis de ambiente (também as que contêm palavras-passe ou tokens): não a publique num registo se contiver segredos.
- Se o contentor estiver em execução, por omissão é **posto em pausa** durante alguns segundos enquanto a imagem é criada, para que fique coerente; pode desmarcar essa opção. Nunca é parado.
- Se já existir uma imagem com esse nome, é dado um aviso e tem de o aceitar expressamente: a anterior não é eliminada, fica sem etiqueta.
- O contentor original não é alterado. No fim, **Ver imagem** abre o seu detalhe em Imagens.

## 13. Registos, estatísticas e terminais {#logs-stats-terminal}

No detalhe de um contentor:

- **Registos**: a saída em direto (stderr a vermelho). Pode escolher quantas linhas anteriores carregar, filtrar, ver as marcas temporais, seguir o final, copiar o que está visível e deixar de seguir. São guardadas as últimas 10 000 linhas.
- **Estatísticas**: CPU, memória, rede, disco e processos a cada segundo, calculados como `docker stats`.
- **Terminal**: **Abrir terminal** inicia uma shell com `docker exec -it` (Automático usa `bash` se existir e, caso contrário, `sh`; também pode escolher uma). **Terminar sessão** termina a shell e os processos que tenha deixado dentro do contentor.

São operações reais, aparecem no painel de atividade enquanto estão abertas e fecham ao sair do detalhe. Para trabalhar com vários terminais ao mesmo tempo, use a secção **Terminais**.

## 14. Gestão de imagens {#images}

A secção **Imagens** lista as imagens locais, como `docker image ls`: etiquetas, ID curto, tamanho, antiguidade e que contentores as usam. Filtros **Todas / Em uso / Sem uso / Sem etiqueta** e pesquisa por etiqueta ou ID.

- **Detalhe**: Resumo (com os valores sensíveis das variáveis ocultos), Camadas (como `docker image history`), os Contentores que a usam e a Inspeção JSON.
- **Etiquetar** acrescenta um nome `repositório:etiqueta`.
- **Eliminar**: se a imagem tiver várias etiquetas, pode retirar apenas uma; eliminar a imagem inteira exige forçar. O Docker não elimina imagens de contentores em execução.
- **Limpar** elimina as imagens sem etiqueta que nenhum contentor usa ou, com uma confirmação adicional, todas as que não estão em uso.

## 15. Transferir e construir imagens {#pull-build}

**Transferir imagem** pede a referência (`nginx:1.27`, `utilizador/app`, `registo.exemplo.com:5000/app:v2`; sem etiqueta é usada `latest`). Credenciais:

| Opção | O que faz |
|---|---|
| As do Docker | Usa o que foi guardado com `docker login` (ou o seu *credential helper*) |
| Nenhuma | Transferência anónima |
| Utilizador e palavra-passe | Apenas para esta transferência: não são guardadas nem aparecem no painel de atividade |

O painel **Transferências** mostra o progresso total e por camada; entretanto pode mudar de secção e **Cancelar**. **Pesquisar no Docker Hub** pesquisa como `docker search` (fá-lo o Docker Engine, e precisa de Internet).

**Construir imagem** abre um assistente: contexto (a pasta que é enviada, respeitando o `.dockerignore`) e Dockerfile, nomes (`--tag`), argumentos (`--build-arg`), metadados (`--label`), etapa final, sem cache, procurar imagens base mais recentes e construtor (**BuildKit** ou **clássico**). Vêem-se os passos com o seu estado e a saída como `docker build --progress=plain`; pode ser cancelada.

## 16. Redes do Docker {#networks}

A secção **Redes** lista as redes, como `docker network ls`, com o controlador, as sub-redes e os contentores. **bridge**, **host** e **none** são predefinidas e não se podem eliminar.

- **Criar rede**: nome, controlador (`bridge`, `macvlan`, `ipvlan`, `overlay` ou outro), rede interna, endereços IPv4 (sub-rede, gateway e intervalo) e IPv6, opções do controlador e metadados. Vê-se o comando equivalente e o pedido exato, e os erros são assinalados antes de enviar o que quer que seja ao Docker.
- **Ligar** um contentor (em execução ou parado), com aliases e IP fixo opcionais. **Desligar** pede confirmação.
- **Eliminar** e **Limpar** avisam se houver contentores parados que tenham a rede configurada e depois não possam arrancar.

## 17. Volumes {#volumes}

A secção **Volumes** lista os volumes, como `docker volume ls`, com o seu controlador e os contentores que os montam. Os anónimos são assinalados como tal. **Calcular tamanhos** pede ao Docker o espaço de cada um (como `docker system df -v`); não é feito automaticamente porque pode demorar.

- **Criar volume**: nome (opcional) e tipo: local, tmpfs, NFS, uma pasta do computador ou outro controlador.
- **Detalhe**: diretório, opções, metadados, tamanho, contentores com o seu caminho e modo, e Inspeção JSON. **Criar contentor com ele** monta-o em `/data`.
- **Eliminar**: o Docker não elimina um volume que algum contentor monte, mesmo que esteja parado.
- **Limpar**: elimina os volumes anónimos sem uso ou, com confirmação, todos os volumes sem uso.

## 18. Diretórios ligados (bind mounts) {#binds}

Esta secção mostra as pastas e ficheiros do seu computador montados dentro de contentores, agrupados por caminho: tipo, permissões e proprietário, se existem, que contentores os montam, onde e com que acesso. São assinalados os que merecem atenção: o **socket do Docker** (dá ao contentor controlo sobre o Docker), a raiz ou diretórios do sistema e caminhos que já não existem.

O Docker não permite acrescentar uma pasta a um contentor existente. **Ligar uma pasta…** abre o seletor de pastas e depois o assistente de contentores com essa pasta montada em `/mnt/<nome>`.

## 19. Docker Compose {#compose}

A secção **Docker Compose** precisa do Compose v2 ou posterior; se não estiver instalado, explica como o instalar. Todas as operações executam o Docker Compose oficial e aparecem no painel de atividade com o comando exato.

A lista mostra os projetos com contentores no Docker (também os implementados a partir do terminal) e os que acrescentar, com o seu estado (em execução, parcial, parado ou não implementado), pasta e serviços.

- **Abrir projeto…**: escolha o seu `compose.yaml` (e ficheiros adicionais, pela ordem em que são combinados).
- **Novo projeto**: escolha uma pasta, escreva o conteúdo (parte de um modelo) e **Criar projeto**: é validado com o Docker Compose antes de criar o `compose.yaml`.
- **Retirar da lista** apenas o retira da lista; não toca no Docker nem nos ficheiros.

## 20. Gestão de projetos e perfis do Compose {#compose-projects}

No detalhe de um projeto:

- **Serviços**: cada serviço com a sua imagem, estado, contentores, portas e **perfis**. Em cada linha é possível implementar, iniciar, parar, reiniciar ou ver os registos desse serviço.
- **Implementar** (`docker compose up --detach`): todos os serviços ou os escolhidos, os **perfis** a ativar, construir antes, a política de transferência de imagens, recriar os contentores e eliminar os órfãos. A caixa de diálogo mostra o comando exato.
- **Iniciar, Parar, Reiniciar**, **Transferir imagens** e **Construir**.
- **Eliminar** (`docker compose down`): contentores e redes do projeto; opcionalmente os seus volumes com nome (os dados perdem-se) e as imagens. Tem de ser confirmado.
- **Saída** da operação em curso (pode ser **Interrompida**) e **Registos** dos serviços em direto.
- **Ficheiro**: edita os ficheiros; **Validar** verifica-os com o Docker Compose sem guardar, e **Guardar** valida-os antes e não os sobrescreve se outro programa os tiver alterado.
- **Configuração**: a configuração final, segundo o Docker Compose. Pode conter segredos, por isso fica oculta até a pedir e não é guardada no painel de atividade.

## 21. Terminais simultâneos {#terminals}

A secção **Terminais** permite abrir e ver ao mesmo tempo vários terminais reais (`docker exec -it`) em contentores em execução, no mesmo contentor ou em contentores diferentes.

- **Novo terminal**: escolha um contentor em execução, a shell (Automático, bash ou sh) e, se quiser, um nome para a sessão.
- Cada painel mostra o **contentor**, o **nome da sessão** e um **indicador de ligação** (A ligar, Ligado, Terminado ou Erro), com botões para **mudar o nome**, **maximizar** e **fechar**. Também se muda o nome com um duplo clique sobre ele.
- **Disposição**: um terminal ocupa todo o espaço; dois, duas colunas; três, um à esquerda e dois à direita; quatro, uma grelha 2×2; mais de quatro, filas de dois com deslocamento vertical.
- Os **separadores** arrastam-se com o rato. Com o foco num separador, as setas movem-no, Início e Fim levam-no aos extremos, e um duplo clique reparte o espaço por igual.
- Cada sessão é independente: tem a sua própria shell, entrada e saída, tamanho e estado. O que fizer numa não afeta as outras.
- As sessões **continuam abertas quando muda de secção**. **Fechar todas** pede confirmação. São permitidas até 12 sessões em simultâneo.
- Ao **fechar** uma sessão termina a sua shell e os processos que tenha deixado em segundo plano dentro do contentor. Se o contentor **parar** ou for **eliminado**, o painel indica-o; uma sessão terminada pode **voltar a abrir-se** se o contentor ainda existir.
- Ao **fechar a aplicação** terminam todas as sessões.

O painel de atividade regista a abertura, o fecho e os erros de cada sessão, mas não o que escreve nem a saída do terminal.

## 22. Painel de atividade e registo de operações {#activity}

Cada operação aparece no **painel de atividade**: hora, distintivo **API** (pedido ao Docker Engine através do socket), **CLI** (processo, como `docker compose` ou `apt-get`) ou **APP** (operação da própria aplicação, como uma mudança de idioma, ou o resumo de uma recriação cujos passos aparecem à parte), tipo, recurso, duração e estado. A linha principal é o comando equivalente, que pode copiar. Ao expandir uma entrada vê a operação realmente executada, o resultado, o erro e o seu código HTTP se falhar, e as saídas dos processos.

**Eventos e registos** mostra os eventos do Docker em direto, os guardados, uma consulta ao Docker sobre um período (`docker events --since … --until …`; o Docker só guarda os últimos 256) e o **histórico de operações** desta e de outras sessões.

O histórico guarda o que foi feito, quando e com que resultado, mas nunca o corpo dos pedidos, o comando nem as saídas, porque podem conter palavras-passe. Fica em `~/.local/state/container-manage-center/` e só o seu utilizador o pode ler.

## 23. Definições, temas e idiomas {#settings}

- **Aspeto**: tema (Sistema, Claro, Escuro) e idioma, por esta ordem: **English**, **Español** (predefinido), **Català**, **Galego**, **Euskara**, **Français**, **Italiano** e **Português**. Cada idioma aparece com o seu próprio nome. O idioma é guardado, mantém-se ao voltar a abrir a aplicação e as suas mudanças ficam no painel de atividade. A cor principal é o **roxo** (desde a versão 1.0.0 não é possível escolher outra; se tinha o verde-azulado, muda para roxo sem perder o resto da configuração).
- **Painel de atividade**: abrir perante erros, operações guardadas (50–10 000) e **Guardar histórico entre sessões**.
- **Ligação ao Docker** e verificação do **Docker Compose**.
- **Atualizações**: onde são publicadas as versões e **Verificar agora**. A aplicação nunca transfere nem instala nada.
- **Sistema**: versões da aplicação, do pacote, do WebKitGTK e da glibc, do sistema e do Docker; **Copiar** gera um relatório sem dados sensíveis.
- **Desinstalação e componentes** e **Ajuda e documentação**.
- **Repor preferências**, depois de confirmar. Não altera a ligação ao Docker.

| Atalho | Ação |
|---|---|
| Ctrl+B | Recolher ou expandir a barra lateral |
| Ctrl+Shift+A | Abrir ou fechar o painel de atividade |
| Escape | Fechar uma caixa de diálogo |

As mensagens geradas pelo próprio Docker (por exemplo, um erro do Docker Engine) e a saída dos programas mostram-se tal como chegam, sem tradução. Os resumos técnicos que o motor da aplicação escreve no painel de atividade (por exemplo, como terminou um terminal) e as mensagens de erro com dados concretos são gerados em espanhol em todos os idiomas.

## 24. Desinstalação e gestão de dependências {#uninstall}

**Definições › Desinstalação e componentes** mostra o que a aplicação instalou e permite retirá-lo sem usar o terminal. **Analisar** consulta o dpkg e o APT sem alterar nada:

- O **pacote da aplicação** (`container-manage-center`), com o seu espaço.
- As **dependências que se podem retirar**: apenas as que o APT instalou automaticamente **com a aplicação** (segundo o seu histórico) e que nenhum outro pacote necessita.
- As que **são mantidas**, com o motivo: dependências partilhadas (com os pacotes que as usam), instaladas manualmente, que já existiam antes, de origem incerta (se o histórico do APT não mostrar como a aplicação foi instalada) e componentes do sistema.
- As **ferramentas externas**: Docker Engine, Docker CLI e Docker Compose. **São sempre mantidas.**
- Os **dados que não são eliminados**: a sua configuração, o seu histórico e os dados da janela. No Docker também não se toca em nada: imagens, contentores, redes, volumes e projetos Compose.
- A **simulação do APT** completa da desinstalação.

Pode escolher **Apenas Container Manage Center** ou **com as dependências que se podem retirar**, ou marcar pacotes um a um; o resumo indica quanto espaço é recuperado. **Rever e desinstalar…** volta a verificar a seleção e simula o comando exato: se o APT fosse retirar algo que não escolheu, não é possível continuar. A caixa de diálogo mostra o comando que vai ser executado:

```
pkexec /usr/bin/apt-get remove -y --no-auto-remove container-manage-center …
```

Ao confirmar, o sistema pede a sua palavra-passe de administrador (pkexec) **apenas para esse comando**; a aplicação continua sem privilégios. Se cancelar a autenticação, nada é retirado. Não é usado `purge`. Tudo fica no painel de atividade. Se a própria aplicação for retirada, feche-a no fim.

Se o `pkexec` não estiver instalado, a aplicação não pode pedir autorização: a caixa de diálogo mostra o comando equivalente com `sudo` para que o execute num terminal.

## 25. Segurança e tratamento de credenciais {#security}

- A aplicação **não é executada como root**, nunca pede sudo para funcionar e não altera as permissões do socket nem os grupos.
- Os comandos externos são executados **sem shell**, com os argumentos separados: o que escreve nunca é interpretado como um comando.
- **Não guarda credenciais.** As de um registo são usadas apenas para uma transferência e não aparecem no painel de atividade. Com «As do Docker» são usadas as de `docker login`.
- Os valores de variáveis como `PASSWORD`, `TOKEN` ou `SECRET` são ocultados no ecrã e no painel (`********`), embora o Docker os receba completos.
- O histórico persistente nunca guarda os corpos dos pedidos, os comandos nem as saídas.
- As operações destrutivas pedem confirmação.
- A janela bloqueia ligações externas. A única ligação da aplicação é a verificação de versões, desativada enquanto não indicar uma fonte. O botão do GitHub da ajuda só pode abrir esse endereço, no seu navegador.
- Montar o socket do Docker num contentor ou pertencer ao grupo `docker` dá controlo total sobre o computador: a aplicação assinala-o.

## 26. Resolução de problemas comuns {#troubleshooting}

| Causa mostrada | O que fazer |
|---|---|
| O Docker não parece estar instalado | `sudo apt install docker.io` ou o guia oficial do Docker |
| O Docker Engine não é encontrado / está parado | `sudo systemctl enable --now docker` (rootless: `systemctl --user enable --now docker`) |
| Sem permissão para usar o socket | Docker rootless, ou o grupo `docker` e voltar a iniciar sessão |
| Já pertence ao grupo, mas não funciona | Termine a sessão e volte a entrar |
| Ligação do Docker não compatível | `tcp://` ou `ssh://` na ligação local: use um socket local (`docker context use default`); para um servidor remoto, adicione-o em **Servidores** |
| O Docker não responde a tempo | `sudo systemctl restart docker` |
| A versão do Docker é demasiado antiga | Atualize o Docker para 20.10 ou posterior |

- **O Docker Compose não aparece**: instale-o (Debian 13: `sudo apt install docker-compose`; Ubuntu: `sudo apt install docker-compose-v2`; repositório do Docker: `docker-compose-plugin`) e clique em **Verificar** em Definições.
- **Uma transferência ou pesquisa falha**: o Docker Engine precisa de acesso à Internet; a mensagem de erro é do Docker.
- **Um terminal não abre**: o contentor tem de estar em execução e ter uma shell (`sh` ou `bash`).
- **Não é permitido recriar um contentor**: o motivo aparece na revisão (contentor do Compose, criado com `--rm`, em pausa, imagem que já não existe…).
- **A desinstalação não oferece uma dependência**: outro programa usa-a, já estava instalada antes ou não se pode ter a certeza; o motivo aparece ao lado.
- **Para comunicar um problema**: Definições › Sistema › **Copiar** gera um relatório sem dados sensíveis.

## 27. Perguntas frequentes {#faq}

**A aplicação executa comandos `docker`?** Não: usa a API do Docker Engine. O comando que vê no painel de atividade é o equivalente, para que o possa reproduzir. O Docker Compose, por outro lado, é executado com o seu programa oficial.

**Preciso de ser administrador?** Não. Só precisa de acesso ao socket do Docker. Apenas a desinstalação de pacotes pede autorização, e só para esse comando.

**Funciona sem Internet?** Sim. Só as operações que o Docker faz contra registos remotos precisam de rede.

**O que acontece aos meus contentores se fechar a aplicação?** Continuam a funcionar: é o Docker que os gere. Ao fechar terminam os terminais abertos e deixa de se seguir os registos e as estatísticas.

**Posso perder dados ao editar um contentor?** Não são eliminados volumes, pastas nem imagens, e o original é guardado como cópia. Só os ficheiros escritos fora dos volumes ficam na cópia, e a revisão avisa-o antes.

**Os meus dados perdem-se ao desinstalar?** Não. A sua configuração, o seu histórico e tudo o que está no Docker são mantidos.

**Onde ficam guardados a configuração e o histórico?** Em `~/.config/container-manage-center/` e `~/.local/state/container-manage-center/`.

**Como mudo o idioma?** Em Definições › Aspeto › Idioma.

## 28. Legal {#legal}

**Nome:** Container Manage Center (abreviado **CMC**).

**Licença:** GNU General Public License, versão 3 ou posterior (identificador SPDX `GPL-3.0-or-later`).

**Copyright © 2026 Sergio Alfaro Lorite.**

Este programa é software livre: pode redistribuí-lo e/ou modificá-lo nos termos da Licença Pública Geral GNU publicada pela Free Software Foundation, seja a versão 3 da Licença ou (à sua escolha) qualquer versão posterior. É distribuído na esperança de que seja útil, mas SEM QUALQUER GARANTIA; nem mesmo a garantia implícita de COMERCIALIZAÇÃO ou ADEQUAÇÃO A UM FIM ESPECÍFICO. Consulte a Licença Pública Geral GNU para mais detalhes.

Texto completo da licença: https://www.gnu.org/licenses/gpl-3.0.html (também em `/usr/share/doc/container-manage-center/LICENSE`).

**Componentes de terceiros.** A aplicação inclui bibliotecas e tipos de letra de terceiros que mantêm as suas próprias licenças (MIT, Apache-2.0, MPL-2.0, OFL-1.1 e outras) e os avisos de direitos de autor dos seus autores; não passam a ser GPL nem de Sergio Alfaro Lorite. Os textos completos estão em `/usr/share/doc/container-manage-center/THIRD_PARTY_NOTICES.md`, e o aviso de direitos de autor do pacote em `/usr/share/doc/container-manage-center/copyright`.

**Código-fonte.** O código-fonte correspondente é publicado com cada versão do pacote (`container-manage-center-X.Y.Z-src.tar.gz`).

**Marcas.** Container Manage Center é um projeto independente: não é afiliado à Docker, Inc. nem apoiado por ela. «Docker» é uma marca da Docker, Inc. e é usada aqui apenas para designar a tecnologia com que a aplicação trabalha (Docker Engine, a sua API, Docker Compose e a CLI do Docker).

Os mesmos dados estão em **Definições › Legal**.

## 29. Windows {#windows}

No Windows, o Container Manage Center funciona como no Linux, com estas diferenças:

- **Docker local**: liga-se ao **Docker Desktop** através do seu pipe com nome (`\\.\pipe\docker_engine`). O Docker Desktop tem de estar em execução e o seu utilizador no grupo local `docker-users` (o instalador do Docker Desktop adiciona-o).
- **Servidores remotos por SSH**: usam o cliente OpenSSH do Windows (`ssh.exe`, incluído no Windows 10 e 11). Como não suporta ligações multiplexadas, cada operação abre a sua própria ligação: na primeira vez a identidade do servidor é verificada como no Linux, e a palavra-passe ou frase-passe guardada é usada automaticamente. Os **servidores intermédios** (ProxyJump) ainda não estão disponíveis e, no **terminal do servidor**, a alteração do tamanho da janela não chega ao servidor.
- **Palavras-passe guardadas**: no **Gestor de credenciais** do Windows (credenciais do Windows), com o nome `container-manage-center/…`.
- **Cópias agendadas**: com o **Agendador de Tarefas**, na pasta «Container Manage Center», com o seu utilizador e sem privilégios de administrador. São executadas com a sessão iniciada; se o computador estava desligado a essa hora, a cópia é feita assim que possível.
- **Notificações**: as do sistema (centro de notificações).
- **Dados**: configuração em `%APPDATA%\container-manage-center` e histórico em `%LOCALAPPDATA%\container-manage-center`.
- **Desinstalar**: em Definições do Windows › Aplicações › Aplicações instaladas. Os seus dados e o Docker Desktop não são alterados. A secção «Desinstalação e componentes» só existe no Debian e no Ubuntu.

## 30. Gestão em massa {#bulk}

**Gestão em massa** (em «Recursos») aplica a mesma operação a muitos contentores de uma vez, também em vários servidores.

- **Vista geral**: total, em execução, parados, em pausa e com erro. Clicar num contador filtra por esse estado. Reúnem-se os contentores deste computador e dos servidores remotos **ligados**; os restantes são indicados num aviso.
- **Atualização**: botão **Atualizar** e atualização automática (de 5 s a 5 min, pode ser desativada).
- **Tabela**: procure por nome ou ID; filtre por estado, imagem, servidor e etiqueta (`chave` ou `chave=valor`); ordene clicando no título de uma coluna.
- **Seleção**: caixa por linha, da página ou de **tudo o filtrado**. A barra indica a quantos contentores da seleção se aplica cada operação.
- **Operações**: iniciar, parar, reiniciar, pausar, retomar, eliminar e alterar a política de reinício (a quente com `docker update`; não nos criados com `--rm`). Os que não o permitem no seu estado são **ignorados**, com o motivo.
- **Confirmação**: operação, servidores, contentores afetados e ignorados, consequências e operações em simultâneo (1 a 16). **Eliminar** pede para escrever o número de contentores e **nunca elimina volumes nem imagens**.
- **Etiquetas**: o Docker não permite alterá-las num contentor que já existe (teria de ser recriado); o diálogo explica-o e mostra as atuais. Para um contentor concreto, use **Duplicar**.
- **Resultados**: cada contentor tem o seu: correto (confirmado pelo Docker), falhado (com o erro), ignorado, cancelado ou **desconhecido** (a ligação caiu antes da confirmação; atualize para ver o estado). Se um falhar, os restantes continuam; nada é desfeito nem repetido automaticamente. **Cancelar o pendente** deixa de enviar ordens.
- **Atividade**: cada ordem fica registada com o distintivo «Massa»; **Ver na atividade** mostra só essa operação.
- **Secções**: Resumo, Contentores, Operações, Configuração em massa, Comandos em massa e Histórico (operações e comandos guardados, com as ordens de cada contentor).
- **Comandos em massa**: execute um mesmo comando dentro de vários contentores em execução com o Docker Exec, sem passar por nenhuma shell deste computador. Escolha «Executável com argumentos» (um por linha, sem shell) ou «Sequência com um intérprete» (`/bin/sh`, `bash`, `python3`…), utilizador, diretório, tempo máximo e execuções em simultâneo. A pré-visualização mostra os argumentos exatos e o risco; os administrativos ou destrutivos pedem para escrever o número de contentores. Cada contentor dá o estado, o código de saída, `stdout` e `stderr` (copiáveis e exportáveis com os segredos ocultos). O Docker não permite parar um processo já iniciado: com o tempo esgotado ou ao interromper só se deixa de esperar. Podem guardar-se comandos frequentes (nunca com segredos).
