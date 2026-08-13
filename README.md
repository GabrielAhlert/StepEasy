# StepEasy

Gravador de passo a passo local, open source e multiplataforma, escrito em Rust — a evolução do **Gravador de Passos (PSR)** do Windows.

O PSR foi descontinuado, exporta só MHTML e não deixa editar nada depois de gravar. O StepEasy grava a interação (clique/tecla → screenshot + descrição) e, principalmente, deixa **editar a gravação depois**: reordenar, excluir, mesclar e reescrever passos antes de exportar.

> Status: **em desenvolvimento** (pré-v0.1). Ainda não há release utilizável.

## Princípios

- **100% local.** Nada sai da máquina. Zero telemetria.
- **Formato aberto.** `.stepeasy` é um zip com `manifest.json` + PNGs. Sem lock-in.
- **Binário único.** Interface em [egui](https://github.com/emilk/egui), sem runtime web, sem instalador obrigatório.

## Escopo de captura

Escolhido antes de gravar:

| Modo | O que entra na imagem |
|---|---|
| Tela sob o cursor | O monitor onde o clique aconteceu (padrão) |
| Tela específica | Um monitor fixo escolhido por você |
| Todas as telas | O canvas virtual inteiro |
| Janela ativa | Só a janela em foco, recortada |
| Região | Um retângulo fixo que você seleciona |

## Roadmap

**v0.1 (em andamento)** — captura no Windows, editor com reordenação/edição/mesclagem e undo/redo, formato `.stepeasy`, export Markdown e HTML.

**Depois** — Linux (AT-SPI) e macOS (AX API), anotações (setas, retângulos, blur), redação automática de dados sensíveis, export PDF/DOCX, vídeo/GIF, diff entre gravações, export para Playwright.

## Compilando

```bash
cargo run --release
```

## Estrutura

```
crates/
  stepeasy-core/     modelo de dados, formato .stepeasy, exportadores
  stepeasy-capture/  hooks de entrada, screenshot e acessibilidade por plataforma
  stepeasy-ui/       interface egui (gravador + editor)
  stepeasy/          binário
```

## Licença

MIT
