# Guia: chave da OMDb (pontuação da crítica)

A lista **Aclamados pela crítica** usa a pontuação da crítica (Metacritic e, na falta dele, Rotten Tomatoes) que a OMDb fornece. O job diário do catálogo busca essas pontuações com a chave do dono; o app só baixa o catálogo pronto. **A chave nunca vai para dentro do app.**

Sem a chave tudo funciona: a lista vira **Aclamados pelo público**, com a média do público no TMDB.

## 1. Pedir a chave

1. Abra `https://www.omdbapi.com/apikey.aspx`.
2. Escolha **FREE! (1,000 daily limit)**.
3. Preencha o e-mail e, em **Use**, algo como "Personal movie recommender, non-commercial."
4. Envie. A chave chega por e-mail, junto com um link de ativação.
5. **Clique no link de ativação** do e-mail. Sem isso a chave não funciona.

## 2. Colocar a chave como secret do repositório

No GitHub, abra o repositório → **Settings** → **Secrets and variables** → **Actions** → **New repository secret**:

| Nome | Valor |
|---|---|
| `OMDB_KEY` | a chave do e-mail (8 caracteres) |

## 3. Esperar as pontuações chegarem

A chave gratuita permite 1000 consultas por dia. O job usa até 900 por dia, somando as execuções à mão (sobra margem para novas tentativas quando a OMDb oscila) e guarda o que já sabe entre uma execução e outra, começando pelos filmes mais votados. Com dezenas de milhares de filmes no catálogo, as pontuações vão aparecendo ao longo de semanas; depois disso o job passa a atualizar as mais antigas.

Enquanto um filme não tem pontuação da crítica, ele entra em Aclamados pela crítica pela média do público, como se a lista fosse Aclamados pelo público para ele.

## Se algo der errado

O catálogo é publicado mesmo quando a OMDb falha; só ficam faltando as pontuações novas daquele dia. O aviso aparece no resumo da execução, em **Actions** → **Catalog**:

- `OMDB_KEY is not set`: o secret do passo 2 não existe ou tem outro nome.
- `Invalid API key!`: a chave foi copiada errado ou não foi ativada (passo 1.5).
- `Request limit reached!`: as 1000 consultas do dia acabaram (por exemplo, a chave foi usada em outro lugar). O job continua no dia seguinte.
