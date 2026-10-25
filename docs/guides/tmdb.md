# Guia: chave do TMDB e primeiro catálogo

O catálogo de filmes (títulos em português, gêneros, elenco, trailers, pôsteres) vem do TMDB. Um job diário do GitHub Actions monta o catálogo com a chave do dono e publica como arquivo de release; o app só baixa esse arquivo. **A chave nunca vai para dentro do app.**

Faça isto uma vez, antes de abrir o app pela primeira vez.

## 1. Criar a conta e pedir a chave

1. Crie uma conta gratuita em `https://www.themoviedb.org/signup` e confirme o e-mail.
2. Logado, abra `https://www.themoviedb.org/settings/api`.
3. Clique em **Create** (ou **Request an API Key**) e escolha **Developer**.
4. Aceite os termos de uso.
5. Preencha o formulário:
   - **Type of use**: Personal
   - **Application name**: Cinelens
   - **Application URL**: `https://github.com/LucasJLBraz/cinelens`
   - **Application summary**: "Personal, non-commercial movie recommender for my own use."
   - Os demais campos (endereço, telefone) com seus dados.
6. Envie. A chave sai na hora.

## 2. Copiar o token certo

Na mesma página (`settings/api`) aparecem dois valores. Copie o **API Read Access Token** (o texto longo que começa com `eyJ`), **não** a "API Key" curta.

## 3. Colocar o token como secret do repositório

No GitHub, abra o repositório → **Settings** → **Secrets and variables** → **Actions** → **New repository secret**:

| Nome | Valor |
|---|---|
| `TMDB_TOKEN` | o API Read Access Token do passo 2 |

## 4. Gerar o primeiro catálogo

O job roda sozinho todo dia de madrugada. Para não esperar:

1. No GitHub, aba **Actions** → workflow **Catalog** → **Run workflow** → **Run workflow**.
2. Espere terminar (a primeira vez leva perto de uma hora: são dezenas de milhares de filmes).
3. Confira em **Releases** que existe a release **Catálogo (formato 1)** com o arquivo `catalog.json.gz`. Ela fica marcada como pré-lançamento de propósito, para o Obtainium não confundir com uma versão do app.

Pronto. Na primeira abertura o app baixa esse arquivo mostrando o progresso; depois abre direto com os dados do aparelho.

## Se o job falhar

- Mensagem `TMDB_TOKEN secret missing`: o secret do passo 3 não existe ou tem outro nome.
- Mensagem com `HTTP 401`: o token foi copiado errado (talvez a "API Key" curta). Refaça o passo 2 e atualize o secret.
- Outras falhas de rede: o job não publica catálogo pela metade; o app continua com o catálogo anterior. Rode de novo pelo passo 4 ou espere o dia seguinte.
