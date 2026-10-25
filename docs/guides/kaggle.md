# Guia: token do Kaggle para a base Letterboxd

O modelo de recomendação é treinado uma vez por mês no GitHub Actions com duas bases públicas: MovieLens `ml-latest` (baixada direto do GroupLens) e a amostra Letterboxd de abril de 2026, que cobre os filmes de 2023 até abril de 2026. A amostra Letterboxd fica no Kaggle, e o Kaggle só libera o download com um token da sua conta. **O token fica só nos secrets do repositório; nunca vai para dentro do app.**

Faça isto uma vez. Enquanto o secret não existir, o workflow **Model** treina só com o MovieLens.

## 1. Criar a conta no Kaggle

1. Crie uma conta gratuita em `https://www.kaggle.com/account/login?phase=startRegisterTab` e confirme o e-mail.
2. Abra `https://www.kaggle.com/datasets/tliocastro/letterboxd-sample-apr-2026` logado e confira que a página abre. Se o Kaggle pedir para aceitar algum termo do dataset, aceite (a licença é CC BY-NC-SA 4.0: uso não comercial, com atribuição).

## 2. Gerar o token

1. Abra `https://www.kaggle.com/settings/api` (ou: foto do perfil → **Settings** → seção **API**).
2. Clique em **Generate New Token**.
3. Dê um nome, por exemplo `cinelens-ci`, e confirme.
4. Copie o token mostrado (um texto longo que começa com `KGAT_`). **Ele aparece uma vez só**; se fechar a janela sem copiar, gere outro.

Não use o botão **Create Legacy API Key** (o arquivo `kaggle.json` antigo): o workflow espera o token novo.

## 3. Colocar o token como secret do repositório

No GitHub, abra o repositório → **Settings** → **Secrets and variables** → **Actions** → **New repository secret**:

| Nome | Valor |
|---|---|
| `KAGGLE_API_TOKEN` | o token do passo 2 |

## 4. Gerar o modelo

O workflow precisa do catálogo já publicado (ver `docs/guides/tmdb.md`), porque é pelo catálogo que os filmes do Letterboxd são ligados aos do app.

1. No GitHub, aba **Actions** → workflow **Model** → **Run workflow** → **Run workflow**.
2. Espere terminar (pode levar até duas horas: são quase 50 milhões de notas).
3. Confira em **Releases** que a release **Modelo (formato 1)** tem o arquivo `model.json.gz` atualizado.

## 5. (Opcional) Usar o token no seu computador

Para medir localmente o erro da nota prevista com a base Letterboxd (`docs/guides/accuracy.md`), salve o mesmo token em `~/.kaggle/access_token`:

```bash
mkdir -p ~/.kaggle && chmod 700 ~/.kaggle
```

Depois abra um editor, cole o token num arquivo novo `~/.kaggle/access_token` e salve. Não cole o token em nenhum outro lugar (chat, issue, commit).

## Se o job falhar

- Aviso `KAGGLE_API_TOKEN secret missing` e modelo treinado só com o MovieLens: o secret do passo 3 não existe ou tem outro nome.
- Mensagem com `401` ou `Unauthorized`: o token foi copiado errado ou foi revogado. Gere outro (passo 2) e atualize o secret.
- Mensagem com `403`: falta aceitar os termos do dataset (passo 1.2).
- Mensagem `release not found` no passo **Download catálogo**: o catálogo ainda não foi publicado; rode o workflow **Catalog** antes.
- O token não expira sozinho, mas pode ser revogado em `https://www.kaggle.com/settings/api`. Se revogar, gere outro e atualize o secret.
