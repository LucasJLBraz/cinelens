# Guia: chave de assinatura, publicar versão e Obtainium

Passos únicos do dono. Nenhum exige conhecimento de programação além de copiar comandos no terminal.

## 1. Gerar a chave de assinatura (uma vez só, para sempre)

O Android só aceita uma atualização se ela for assinada pela mesma chave da versão instalada. O login do Google (sync) também fica preso a ela. **Se a chave se perder, o app precisa ser desinstalado e o login refeito.** Guarde o arquivo e a senha num lugar seguro (gerenciador de senhas, pendrive).

No terminal do computador (precisa de Java instalado; `keytool` vem com ele):

```bash
keytool -genkeypair -v -keystore cinelens-release.jks -keyalg RSA -keysize 4096 -validity 36500 -alias cinelens
```

- Ele pede uma senha: escolha uma forte e anote (sem o caractere `\`, que quebra a leitura no CI). Quando perguntar a senha da chave, use **a mesma**.
- As perguntas de nome e organização podem ser respondidas com qualquer coisa (ou Enter).
- O resultado é o arquivo `cinelens-release.jks`. **Nunca** coloque esse arquivo dentro da pasta do repositório.

Anote também a impressão digital SHA-1 (vai ser usada no passo do login Google):

```bash
keytool -list -v -keystore cinelens-release.jks -alias cinelens
```

Copie a linha `SHA1: ...`.

## 2. Colocar a chave como secret do repositório

Converta o arquivo da chave em texto:

```bash
base64 -w0 cinelens-release.jks > chave.txt
```

No GitHub, abra o repositório → **Settings** → **Secrets and variables** → **Actions** → **New repository secret**. Crie três secrets:

| Nome | Valor |
|---|---|
| `ANDROID_KEY_BASE64` | todo o conteúdo de `chave.txt` |
| `ANDROID_KEY_ALIAS` | `cinelens` |
| `ANDROID_KEY_PASSWORD` | a senha do passo 1 |

Depois apague `chave.txt` (o `.jks` continua guardado no lugar seguro).

## 3. Publicar uma versão

Nesta ordem. Cada passo depende do anterior:

1. **Merge e push da `main`.** Antes, rode `cargo bench -p cinelens-core` e confira que ele saiu com código 0: um `grep` na saída esconde um panic.
2. **Dados novos, se a versão trouxer algum.** Rode o workflow **Catalog** e espere terminar. Depois rode **Anime model** ou **Model**: o de animes baixa `animes.json.gz`, que só o Catalog publica.
3. **CI verde na `main`.** Espere o workflow **CI** do último commit. A release roda o mesmo CI e para se ele falhar.
4. **Tag:**

```bash
git tag v0.1.0
```

```bash
git push origin v0.1.0
```

O GitHub Actions (aba **Actions**, workflow **Release**) compila o APK, assina com a chave e cria a release `v0.1.0` com o arquivo `cinelens-0.1.0.apk`. Leva cerca de 15 minutos. Cada nova versão usa um número maior (`v0.1.1`, `v0.2.0`...).

Se a release falhar, corrija e publique o próximo patch (`v0.1.1`). A tag que falhou fica sem release: tags publicadas não se movem.

5. **Medir no celular:** instale por cima da versão anterior e rode o script abaixo, antes e depois de atualizar, para comparar o Perfil.

```bash
scripts/device-speed.py SERIAL
```

`SERIAL` vem de `adb devices`. Depois de uma atualização, abra o app uma vez e espere os downloads: a ordem e as previsões de animes mudam quando o modelo deles chega. O script só toca em rótulos de navegação e na primeira capa de Animes, e para se não achar o rótulo na tela, então pode rodar na conta real. Dar nota, medir o tempo depois de uma nota e testar marcas exige uma conta de teste.

O bench não prevê o celular: no S20 FE, na v3, a busca com erro de digitação levou 1,7 vez o tempo do bench, e voltar o filtro para Tudo, cerca de 1,9 vez. Passar no bench não basta: compare no aparelho.

## 4. Instalar e atualizar pelo Obtainium

1. No celular, instale o Obtainium a partir da página de releases dele no GitHub (`github.com/ImranR98/Obtainium/releases`, arquivo `app-arm64-v8a-release.apk`) ou pelo F-Droid.
2. Abra o Obtainium → **Adicionar app**.
3. Em URL, cole `https://github.com/LucasJLBraz/cinelens` e toque **Adicionar**.
4. Toque em **Instalar**. Na primeira vez o Android pede para permitir instalar apps do Obtainium: permita.
5. Pronto. O Obtainium verifica novas releases em segundo plano e avisa quando houver atualização; um toque instala.
