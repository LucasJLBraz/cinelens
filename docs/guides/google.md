# Guia: projeto Google Cloud para o sync

Passos únicos do dono. O sync guarda as notas e marcas num arquivo da `appDataFolder` do seu Google Drive: uma pasta escondida que só o Cinelens vê (escopo `drive.appdata`, não sensível). Nada vai para outro lugar.

Precisa da impressão digital SHA-1 da chave de assinatura (passo 1 de `release.md`).

## 1. Criar o projeto

1. Abra <https://console.cloud.google.com/> com a sua conta Google.
2. No seletor de projetos (topo da página) → **Novo projeto** → nome `Cinelens` → **Criar**. Confira que o projeto novo está selecionado.
3. Menu → **APIs e serviços** → **Biblioteca** → procure **Google Drive API** → **Ativar**.

## 2. Tela de consentimento

1. Menu → **APIs e serviços** → **Tela de permissão OAuth** (ou **Google Auth Platform**) → **Começar**.
2. Nome do app `Cinelens`, e-mail de suporte: o seu. Público: **Externo**. Contato: o seu e-mail. Aceite a política e **Criar**.
3. Em **Acesso a dados** → **Adicionar ou remover escopos** → marque `.../auth/drive.appdata` (ou cole `https://www.googleapis.com/auth/drive.appdata` em "Adicionar escopos manualmente") → **Atualizar** → **Salvar**.
4. Em **Público** → **Usuários de teste** → **Adicionar usuários** → o seu e-mail do Google → **Salvar**.
5. **Não** clique em "Publicar app": o app fica em **Teste**. Publicar exige página inicial e política de privacidade num domínio seu, e não vale o trabalho para um app só seu.

O custo de ficar em Teste: o Google derruba o login a cada 7 dias. Quando isso acontece, o Perfil volta a mostrar **Entrar com Google**; é um toque por aparelho. Até lá o sync só espera: as mudanças ficam no aparelho e vão juntas quando você entrar de novo.

Ao entrar, o Google mostra "O Google não verificou este app": clique em **Continuar** (ou **Avançado** → **Acessar Cinelens**).

## 3. Cliente Android

1. Em **Clientes** → **Criar cliente** → tipo **Android**.
2. Nome do pacote: `io.github.lucasjlbraz.cinelens`.
3. Impressão digital SHA-1: a da chave de assinatura (`keytool -list -v -keystore cinelens-release.jks -alias cinelens`, linha `SHA1`).
4. **Criar**. Não há nada a copiar: o Play Services reconhece o app pelo pacote e pela chave que o assinou.

Para testar um APK de debug (assinado com a chave de debug do Android Studio), crie um segundo cliente Android com o SHA-1 dela:

```bash
keytool -list -v -keystore ~/.android/debug.keystore -alias androiddebugkey -storepass android
```

Se a chave de assinatura mudar, o login do Android para de funcionar até o SHA-1 do cliente ser trocado.

## 4. Cliente desktop (opcional, para o app no computador)

1. Em **Clientes** → **Criar cliente** → tipo **App para computador** → nome `Cinelens desktop` → **Criar**.
2. Copie o **ID do cliente** e a **Chave secreta do cliente**. Num app instalado, o Google não trata essa chave como segredo.
3. Compile o app desktop com elas no ambiente:

```bash
CINELENS_GOOGLE_CLIENT_ID=<id do cliente> CINELENS_GOOGLE_CLIENT_SECRET=<chave secreta> npm run tauri build
```

Sem essas variáveis o app desktop funciona, só que o botão "Entrar com Google" responde que foi compilado sem elas.

## 5. Entrar no app

Em cada aparelho: **Perfil** → **Entrar com Google** → escolha a conta → **Permitir**. No Android a tela é a do próprio sistema; no computador abre o navegador. Depois disso o app sincroniza sozinho: ao abrir, depois de cada mudança e a cada 10 minutos enquanto está aberto. Se duas mudanças no mesmo item se cruzarem, vale a mais recente; o histórico de notas guarda todas.
