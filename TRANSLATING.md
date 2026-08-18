# Translating StepEasy

Adding a language takes one file and no Rust. You copy `locales/en.yml`,
translate the values, and open a pull request. The application picks the new
language up automatically — nothing in the code needs to change.

## Steps

1. **Open an issue first** using the *New language* template, so two people do
   not translate the same language at the same time.
2. Copy `locales/en.yml` to `locales/<code>.yml`, where `<code>` is the
   [language code](https://en.wikipedia.org/wiki/IETF_language_tag):
   `es.yml`, `de.yml`, `fr-CA.yml`, `pt-PT.yml`…
3. Translate the **values**. Never translate the keys on the left.
4. Set `idioma.nome` to the name of the language **in that language** —
   `Español`, not `Spanish`. People look for the name they recognise.
5. Open the pull request.

You do not need to build the project. If you can, running
`cargo test -p stepeasy-core --test traducoes` checks your file for missing
keys before you submit — but CI runs the same checks on your pull request.

## The two rules that matter

**Keep every `%{name}` exactly as it is.** Those are values the program
inserts:

```yaml
# en.yml
capturados: "%{n} step(s) captured. Review and save."

# es.yml — correct
capturados: "%{n} paso(s) capturado(s). Revisa y guarda."

# es.yml — WRONG, renamed the parameter
capturados: "%{numero} paso(s) capturado(s)."
```

You may move a `%{name}` anywhere in the sentence, or use it more than once.
You may not rename it or drop it. A test catches this, because the failure is
otherwise invisible until a user sees a raw `%{n}` on screen.

**Translate the whole sentence, not the words.** Word order is yours to
choose — that is the point of the design.

## The interesting part: step captions

StepEasy writes a sentence for every recorded step. Those sentences are
assembled from the `legenda:` section, and **the grammar lives in your file,
not in the code**. Compare:

```yaml
# en.yml
alvo:
  com_nome: "the **%{nome}** %{tipo}"
controle:
  botao: button
clique: "Clicked %{alvo} with the %{botao} mouse button"
# → Clicked the **Save** button with the left mouse button
```

```yaml
# pt-BR.yml
alvo:
  com_nome: "%{tipo} **%{nome}**"
controle:
  botao: no botão          # the preposition is part of the value
clique: "Clicou com o botão %{botao} %{alvo}"
# → Clicou com o botão esquerdo no botão **Salvar**
```

In English the article belongs to the target and the type comes after the
name. In Portuguese the preposition is glued to the control type and the order
is reversed. Both work because the whole sentence is yours.

Use this freedom. If your language needs a case ending on the control type,
put it in `controle:`. If it needs a different word order, write a different
sentence. If your language needs something the current structure genuinely
cannot express — a gender agreement that depends on the control type, for
example — **open an issue instead of forcing it**. That report is as valuable
as the translation.

The `**` around a word makes it bold in the exported documents. Keep it around
the same word.

## What is not translated

- `StepEasy` — the product name.
- Keyboard shortcuts like `Ctrl+Shift+F9`.
- The control type **keys** (`botao`, `caixa_edicao`) — those are identifiers
  the program uses; only their values are text.

## Keeping a translation alive

When someone adds a screen, `en.yml` gains a key and every other language
becomes incomplete. CI fails immediately and says exactly which keys are
missing, so nothing silently drifts. If that happens to your language and you
are around, a small pull request fixes it. If not, someone else will — that is
what the failing test is for.
