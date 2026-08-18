# Roteiro da demonstração

Alvo: **90 segundos**. Quem assiste decide nos primeiros 10 se continua, então o
gancho vem antes de qualquer explicação.

Grave contra `demo/cadastro-exemplo.html` — é uma tela fictícia, feita para isso.
**Nunca grave contra um sistema real da empresa**: a captura pega a tela inteira
da janela, incluindo nomes, documentos e o que mais estiver ali.

## Antes de gravar

- [ ] Feche tudo o que não entra na demo (e-mail, chat, abas do navegador)
- [ ] Navegador em janela anônima, sem barra de favoritos e sem extensões visíveis
- [ ] Área de trabalho neutra e área de notificação sem ícones de trabalho
- [ ] Resolução **1920×1080**, escala **100%** (150% deixa a fonte gigante no vídeo)
- [ ] Tema do Windows claro — combina com o tema padrão do StepEasy
- [ ] Uma pasta limpa para salvar a gravação de exemplo
- [ ] Rode a demo inteira uma vez sem gravar, para não titubear no take

## Sequência

| Tempo | O que acontece | Por que está aqui |
|---|---|---|
| 0:00–0:08 | A tela de gravação do StepEasy. Escolher **Janela ativa** no seletor. Clicar em **Iniciar gravação**. A janela some. | Mostra que começar custa dois cliques |
| 0:08–0:30 | No formulário: clicar em **Razão social**, digitar `Transportes Aurora Ltda`, clicar em **E-mail**, digitar um endereço, escolher **30/60 dias** na lista, clicar em **Salvar** | São os quatro tipos de evento que viram passo: clique, digitação, seleção e confirmação |
| 0:30–0:34 | `Ctrl+Shift+F9`. O StepEasy volta sozinho, já no editor, com os passos prontos | O momento "ah, entendi" — nada foi escrito à mão |
| 0:34–0:50 | Percorrer os passos na timeline. Parar num deles e mostrar a legenda: *"Clicou no botão **Salvar** da janela..."* | O diferencial: legenda em texto, não coordenada |
| 0:50–1:05 | Arrastar um passo na timeline. Excluir outro. Corrigir um texto. `Ctrl+Z` para desfazer | A gravação é rascunho, não documento pronto — é o que o PSR não fazia |
| 1:05–1:20 | Ferramenta **Borrão** sobre o campo CPF. Depois uma **Seta** apontando para o botão Salvar | O borrão é o argumento de venda para quem documenta sistema com dado de cliente |
| 1:20–1:30 | **Exportar → HTML**, abrir o arquivo gerado no navegador, rolar mostrando os passos com as anotações gravadas na imagem | Fecha o ciclo: entra clique, sai documento |

## Depois

```powershell
# Converte o take bruto em MP4 (README/release) e GIF (topo do README)
demo\gerar-midia.ps1 -Entrada .\take-bruto.mkv -Inicio 00:00:03 -Duracao 90
```

## Onde publicar

- **GIF** no topo do README, logo abaixo do logo. É o que aparece sem clique.
- **MP4** arrastado para dentro de uma issue ou release do GitHub: o upload
  devolve uma URL que o README renderiza como player.

Um GIF de 90 s fica pesado demais. Corte para os **15 segundos** mais
convincentes (gravar → editor com os passos prontos) e deixe o vídeo completo
para quem quiser mais.
