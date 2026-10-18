# AlpIRC

**Le client de discussion des années 2000, refait de zéro en Rust.**
Tu te souviens de mIRC ? Les canaux `#lyon` ou `#grenoble` sur Undernet, la liste des pseudos à droite, les
couleurs, les scripts… AlpIRC, c'est ce client-là, réécrit aujourd'hui. Il marche sur **Mac**, **Android** et
**Linux**.

C'est gratuit, sans pub, sans compte à créer, et tout le code est ici, lisible par tout le monde.

*English speakers: jump to the [English section](#english) at the bottom.*

> AlpIRC n'a aucun lien avec mIRC ni avec mIRC Co. Ltd. Il s'en inspire, mais n'utilise ni son code ni ses images :
> tout a été réécrit, et ses icônes sont originales ([pourquoi et comment](docs/adr/0001-reimplementation-propre.md)).

## Sommaire

1. [C'est quoi, exactement ?](#cest-quoi-exactement-)
2. [Où on en est](#où-on-en-est)
3. [Installer](#installer) — [Mac](#sur-mac) · [Android](#sur-android) · [Linux](#sur-linux)
4. [Premiers pas](#premiers-pas)
5. [Les scripts](#les-scripts)
6. [Le relais : le même IRC sur le téléphone et l'ordinateur](#le-relais--le-même-irc-sur-le-téléphone-et-lordinateur)
7. [Vie privée](#vie-privée)
8. [Ça ne marche pas ?](#ça-ne-marche-pas-)
9. [Pour les curieux et les développeurs](#pour-les-curieux-et-les-développeurs)
10. [English](#english)

---

## C'est quoi, exactement ?

**IRC**, c'est le grand ancêtre des messageries de groupe : des **réseaux** de serveurs (Undernet, Libera.Chat,
IRCnet…), et sur chacun des milliers de **canaux**, des salons de discussion dont le nom commence par `#`. On s'y
connecte avec un **pseudo**, sans numéro de téléphone ni compte obligatoire. IRC existe toujours, et on y parle
encore de tout.

Au début des années 2000, sous Windows, on y allait presque tous avec **mIRC**. AlpIRC reprend sa façon de faire :

- **Tout dans une grande fenêtre** : la fenêtre Status de chaque réseau, une fenêtre par canal et par discussion
  privée, une barre de boutons en haut pour passer de l'une à l'autre, la liste des pseudos à droite, la ligne de
  saisie en bas, une police à chasse fixe, des boîtes de dialogue grises en relief.
- **Les couleurs et la mise en forme** de l'époque : Ctrl+K pour la couleur, Ctrl+B pour le gras…
- **Plusieurs réseaux à la fois**, Undernet en tête, avec une connexion chiffrée par défaut.
- **Les favoris** (canaux à rejoindre d'un clic), les **amis** qu'on voit arriver, les gens qu'on **ignore**, les
  commandes lancées à la connexion, les **journaux** de discussion, les envois de fichiers (**DCC**).
- **Les scripts mIRC** : alias, menus du clic droit, réactions automatiques, boîtes de dialogue — dans une boîte
  fermée, pour qu'un script ne puisse pas abîmer ton ordinateur.
- **Sur le téléphone**, une disposition pensée pour le pouce (une fenêtre à la fois, un tiroir pour les canaux), le
  vrai clavier d'Android (correction, dictée, emoji), et la connexion qui reste ouverte en arrière-plan.

## Où on en est

AlpIRC est **presque prêt** : tout ce qui est prévu pour la 1.0 est écrit et testé automatiquement (plus de
850 tests). Il reste à l'essayer pour de vrai sur un Mac et sur un téléphone Android, puis la 1.0 sortira.

| Partie | Où on en est |
|---|---|
| Se connecter et discuter : Status, canaux, privés, Undernet et les autres réseaux, TLS, SASL, compte X | écrit, testé |
| Le confort de mIRC : Options, favoris, Notify, Ignore, Control, Perform, journaux, sons, thèmes | écrit, testé |
| Les scripts mSL (éditeur Alt+R, menus, dialogues) et le DCC | écrit, testé |
| Android : disposition téléphone, connexion permanente, notifications, saisie native | écrit, à essayer sur un vrai téléphone |
| Le relais : le téléphone et l'ordinateur sur la même session | écrit, testé ; pas encore installé sur un serveur |
| Les 7 langues (anglais, français, espagnol, allemand, italien, valencien, irlandais) | écrit ; relecture bienvenue |
| 1.0 : version finale, essayée sur Mac, Android et Linux | en cours d'essai |

Ce qu'il faut savoir avant d'installer :

- **Mac** : un seul fichier pour les Mac à puce Apple (M1, M2, M3, M4…) et les Mac Intel, macOS 11 ou plus récent.
- **Android** : Android 8 ou plus récent.
- **Linux** : PC 64 bits avec Ubuntu 22.04, Debian 12, Fedora 36, Mint 21 ou plus récent.
- **Windows, iPhone** : pas encore, c'est prévu plus tard.
- Les applis ne sont **pas signées** (Apple fait payer ça chaque année). Ton Mac va donc afficher un avertissement
  la première fois : c'est normal, et on t'explique quoi faire juste en dessous.

---

## Installer

### Étape 1 : télécharger le bon fichier

Va sur la page des versions : **<https://github.com/Guiwald/alpirc/releases/latest>**

Tout en bas de la version, dans **Assets**, prends le fichier qui correspond à ton appareil (le numéro change à
chaque version : prends simplement ceux de la dernière) :

| Ton appareil | Le fichier à télécharger |
|---|---|
| Mac (puce Apple ou Intel) | `AlpIRC-X.Y.Z.dmg` |
| Téléphone Android 8 ou plus récent | `AlpIRC-X.Y.Z.apk` |
| PC Linux : Debian, Ubuntu, Mint | `alpirc_X.Y.Z_amd64.deb` |
| PC Linux : les autres (Fedora…) | `AlpIRC-X.Y.Z-linux-x86_64.tar.gz` |

### Sur Mac

1. Double-clique sur `AlpIRC-X.Y.Z.dmg`. Une fenêtre s'ouvre avec l'icône d'AlpIRC et un dossier Applications.
2. **Glisse AlpIRC sur le dossier Applications.** C'est installé !
3. Ouvre AlpIRC depuis le dossier **Applications**. La première fois, macOS refuse (« Apple ne peut pas vérifier… ») :
   clique sur **OK**, puis ouvre **Réglages Système** → **Confidentialité et sécurité**, descends tout en bas : une
   ligne parle d'AlpIRC, avec un bouton **Ouvrir quand même**. Clique dessus, confirme avec ton mot de passe ou
   Touch ID, puis **Ouvrir**. Il ne faut le faire qu'une seule fois : ensuite, AlpIRC s'ouvre normalement, depuis le
   Launchpad ou le Dock.

**Pourquoi cet avertissement ?** Apple demande aux développeurs de payer 99 $ par an pour « signer » leurs
applis. AlpIRC est un projet gratuit, il n'est donc pas signé. Ce n'est pas un virus : tout son code est sur cette
page.

**Si macOS dit que l'appli est « endommagée »** : ouvre l'appli **Terminal**, colle cette ligne puis appuie sur
Entrée, et rouvre AlpIRC :

```sh
xattr -dr com.apple.quarantine /Applications/AlpIRC.app
```

Les liens `irc://` (sur une page web, dans un mail) s'ouvrent dans AlpIRC.

### Sur Android

1. Sur ton téléphone, ouvre la page des versions dans Chrome et touche le fichier **`.apk`** de la dernière version
   pour le télécharger.
2. Ouvre le fichier téléchargé (la notification « Téléchargement terminé », ou l'appli **Fichiers** →
   **Téléchargements**).
3. Android te demande d'autoriser Chrome (ou Fichiers) à **installer des applis inconnues** : touche
   **Paramètres**, active l'interrupteur, puis reviens en arrière.
4. Touche **Installer**. Si Play Protect affiche un avertissement, touche **Plus de détails** → **Installer quand même**.
5. Au premier lancement, **autorise les notifications** : c'est ce qui te prévient quand quelqu'un te parle, et
   ce qui permet à AlpIRC de rester connecté quand tu passes à une autre appli.

Pour mettre à jour plus tard, installe simplement le nouveau fichier `.apk` par-dessus : tes réglages restent.

### Sur Linux

**Debian, Ubuntu, Mint** : double-clique sur `alpirc_X.Y.Z_amd64.deb` : le gestionnaire de logiciels s'ouvre,
clique sur **Installer** et tape ton mot de passe. S'il ne propose rien, ouvre un terminal dans le dossier
Téléchargements (clic droit → **Ouvrir dans un terminal**) et tape (avec le bon numéro) :

```sh
sudo apt install ./alpirc_X.Y.Z_amd64.deb
```

AlpIRC est alors dans le menu des applications. Le paquet ajoute aussi le **dépôt d'AlpIRC** : les versions
suivantes arrivent toutes seules avec les mises à jour du système. Pour le désinstaller : `sudo apt remove alpirc`
(le dépôt part avec).

**Sans télécharger le .deb**, quatre lignes suffisent (elles ajoutent le dépôt, puis installent) :

```sh
sudo wget -qO /usr/share/keyrings/alpirc-archive-keyring.gpg https://guiwald.github.io/alpirc/apt/alpirc-archive-keyring.gpg
printf 'Types: deb\nURIs: https://guiwald.github.io/alpirc/apt\nSuites: stable\nComponents: main\nArchitectures: amd64\nSigned-By: /usr/share/keyrings/alpirc-archive-keyring.gpg\n' | sudo tee /etc/apt/sources.list.d/alpirc.sources
sudo apt update
sudo apt install alpirc
```

**Les autres Linux** (Fedora, Arch…) : décompresse `AlpIRC-X.Y.Z-linux-x86_64.tar.gz` (clic droit →
**Extraire ici**), ouvre le dossier dans un terminal et lance `./install.sh` : AlpIRC s'installe dans ton dossier
personnel, sans mot de passe, et apparaît dans le menu des applications. `./install.sh --remove` le retire.

**Bon à savoir** : pour les boîtes « Ouvrir un fichier » (envoyer un fichier, charger un script), le bureau doit
avoir le **portail** `xdg-desktop-portal` (GNOME, KDE et la plupart des bureaux l'ont) ou `zenity`.

---

## Premiers pas

*Ce qui suit décrit AlpIRC tel qu'il sera en 1.0 ; le tableau [Où on en est](#où-on-en-est) dit ce qui marche
déjà.*

### Se connecter

1. Au premier lancement, AlpIRC demande ton **pseudo** (le nom sous lequel les autres te verront) et un pseudo de
   secours, au cas où le premier serait déjà pris.
2. Dans la liste des réseaux, **Undernet** est en tête (puis Libera.Chat, OFTC, IRCnet, EFnet, DALnet, QuakeNet,
   Rizon, EpiKnet). Choisis-en un et clique sur **Connecter**.
3. La fenêtre **Status** du réseau s'ouvre et affiche les messages du serveur. Tu es connecté !

### Rejoindre un canal

- Ouvre les **favoris** (Channels Folder, dans la barre d'outils) : `#lyon`, `#grenoble` et `#chambery` sur
  Undernet y sont déjà. Double-clic pour rejoindre.
- Ou tape dans la ligne de saisie : `/join #lyon`.

Chaque canal a sa fenêtre, avec la liste des pseudos à droite. Un **double-clic sur un pseudo** ouvre une
discussion privée ; un **clic droit** propose whois, privé, CTCP, DCC… et, si tu es op du canal, op, voice, kick
et ban.

### Les commandes à connaître

| Tape… | Pour… |
|---|---|
| `/join #canal` | rejoindre un canal |
| `/part` | quitter le canal où tu es |
| `/msg pseudo bonjour` | écrire en privé |
| `/me danse` | raconter une action (« \* Guiwald danse ») |
| `/nick nouveau` | changer de pseudo |
| `/whois pseudo` | savoir qui se cache derrière un pseudo |
| `/away parti manger` | te dire absent (`/away` seul pour revenir) |
| `/quit à plus` | te déconnecter du réseau |

Pour **écrire en couleur** : Ctrl+K, puis le numéro de la couleur (Ctrl+B gras, Ctrl+U souligné, Ctrl+I italique,
Ctrl+O pour revenir au normal). Sur Mac, c'est Cmd quand la combinaison n'est pas réservée par macOS.

### Undernet et le compte X

Sur Undernet, les comptes et les canaux enregistrés sont tenus par **X** (pas par NickServ comme ailleurs). Crée
ton compte sur le site d'Undernet, puis indique-le dans les réglages du réseau (champ **Compte Undernet**) : AlpIRC
s'identifie dès la connexion et masque ton adresse IP.

## Les scripts

Les scripts mIRC (fichiers `.mrc` et `.ini`) se chargent dans AlpIRC : nouvelles commandes, menus du clic droit,
réponses automatiques, boîtes de dialogue. L'**éditeur de scripts** s'ouvre par Alt+R (Option+R sur Mac).

Un script tourne dans une **boîte fermée** : il ne peut pas lancer de programme, il ne lit et n'écrit que dans le
dossier des scripts d'AlpIRC, et il ne peut pas accepter un fichier à ta place. Quand un script veut s'exécuter
au démarrage, AlpIRC te le dit. Les scripts qui ont besoin de bibliothèques Windows (DLL) ne marchent qu'en partie.
Les détails : [ADR 0007](docs/adr/0007-scripts-msl.md).

## Le relais : le même IRC sur le téléphone et l'ordinateur

*Facultatif : AlpIRC marche très bien sans.*

Quand on se connecte à IRC depuis deux appareils, on y apparaît deux fois, sous deux pseudos. Le **relais**
(`alpirc-relay`, un « bouncer ») règle ça : c'est un petit programme qui tourne sur une machine toujours allumée
(un serveur à la maison ou un petit serveur loué). Il reste connecté aux réseaux sous **un seul pseudo**, garde
l'**historique**, et ton Mac comme ton téléphone se connectent à lui : ce qui est écrit sur l'un apparaît sur
l'autre, et ce que tu as lu sur le téléphone est marqué lu sur le Mac.

Il s'installe sur un serveur Debian ou Ubuntu par le même dépôt APT (`sudo apt install alpirc-relay`), comme un
service du système. D'autres applis IRC peuvent aussi s'y connecter (il parle comme le bouncer soju).

En bref, sur le serveur :

```sh
sudo apt install alpirc-relay
sudo -u alpirc-relay alpirc-relay --init      # crée le certificat et affiche son empreinte
sudo alpirc-relay --add-user guillaume        # demande le mot de passe (deux fois)
sudo nano /etc/alpirc-relay/relay.toml        # tes réseaux, pseudos et canaux
sudo systemctl enable --now alpirc-relay
```

Puis dans AlpIRC, ajoute un réseau dont le serveur est le relais : TLS, vérification « épinglée » avec l'empreinte
affichée par `--init`, et le mot de passe de serveur `guillaume:ton-mot-de-passe`. Les détails :
[ADR 0009](docs/adr/0009-relais.md) et [ADR 0014](docs/adr/0014-relais-realisation.md).

## Vie privée

AlpIRC ne contient **ni publicité, ni pisteur, ni statistiques**, et ne demande aucun compte. Il se connecte
seulement aux réseaux IRC que tu choisis (ou à ton propre relais), **directement**, sans passer par un serveur
d'AlpIRC. Tes réglages, tes scripts et tes journaux restent sur ton appareil.

À savoir sur IRC lui-même : ce que tu écris dans un canal est lu par tous ceux qui y sont, et les serveurs voient
ton adresse IP — sauf si le réseau la masque (sur Undernet, avec le compte X). AlpIRC n'affiche pas d'aperçu des
liens et n'ouvre rien tout seul : cela donnerait ton adresse aux sites.

## Ça ne marche pas ?

*Cette partie se remplira au fil des versions.*

**Mac : « impossible d'ouvrir », « endommagée »** → voir [Sur Mac](#sur-mac) (« Ouvrir quand même » dans les
Réglages, ou la ligne à coller dans le Terminal).

**Android : « Application non installée »** → si tu avais une ancienne version installée autrement, désinstalle-la
d'abord.

**« Pseudo déjà utilisé »** → quelqu'un l'a pris sur ce réseau : AlpIRC essaie ton pseudo de secours ; tu peux en
changer avec `/nick`.

**Linux : les boîtes « Ouvrir un fichier » ne s'ouvrent pas** → installe `xdg-desktop-portal` avec
`xdg-desktop-portal-gtk` (ou `-kde`), ou simplement `zenity` (`sudo apt install zenity`), puis relance AlpIRC.

**Un autre souci, une idée ?** Ouvre un ticket dans l'onglet [Issues](https://github.com/Guiwald/alpirc/issues) :
explique ce que tu as fait, ce qui s'est passé, et sur quel appareil.

---

## Pour les curieux et les développeurs

AlpIRC est écrit en [Rust](https://www.rust-lang.org/), sans bibliothèque C ([ADR 0003](docs/adr/0003-tout-en-rust.md)).
Tout est fait main : le protocole IRC, le moteur de scripts mSL, l'interface façon Windows 98 dessinée au pixel
près (avec [winit](https://github.com/rust-windowing/winit), [softbuffer](https://github.com/rust-windowing/softbuffer)
et [cosmic-text](https://github.com/pop-os/cosmic-text)), le relais. Le réseau passe par
[tokio](https://tokio.rs) et [rustls](https://github.com/rustls/rustls) (cryptographie
[graviola](https://github.com/ctz/graviola)). Sur Android, quelques classes Java seulement (service de connexion,
champ de saisie).

### Compiler toi-même

1. Installe Rust : <https://rustup.rs> (une ligne à copier dans le Terminal).
2. Sur Linux, installe aussi la bibliothèque son : `sudo apt install libasound2-dev pkg-config`.
3. Puis :

```sh
git clone https://github.com/Guiwald/alpirc
cd alpirc
cargo run --release
```

### Les commandes utiles

```sh
cargo run --release                      # le client
cargo test --workspace                   # les tests
cargo xtask help                         # l'outillage (versions, icônes, paquets, licences)
cargo xtask icons                        # icônes .icns, Linux et Android depuis assets/icons/app
cargo build --release && cargo xtask bundle-mac target/release/alpirc   # AlpIRC.app et .dmg (sur macOS)
cargo xtask apk                          # APK sans Gradle (ANDROID_HOME, NDK, JDK ; --all-abis pour l'émulateur x86_64)
cargo build --release && cargo xtask dist-linux && cargo xtask deb      # .tar.gz et .deb
cargo xtask deb-relay target/release/alpirc-relay                        # .deb du relais (service systemd)
cargo xtask bump minor                   # nouvelle version (VERSION, Cargo.toml, CHANGELOG datés)
cargo xtask changelog-page               # page « Versions d'AlpIRC » tirée du CHANGELOG
cargo xtask licenses                     # licences des crates utilisées
```

Les paquets vont dans `dist/` (ou `$ALPIRC_DIST`) ; une version déjà fabriquée n'est jamais écrasée.

### Comment c'est rangé

| Dossier | Contenu |
|---|---|
| `crates/proto` | Le protocole IRC : messages, étiquettes IRCv3, ISUPPORT, CTCP, couleurs mIRC, encodages, SASL |
| `crates/core` | Le cœur du client : sessions, réseaux, réglages, DCC, langues |
| `crates/net` | Les connexions TCP et TLS (client et serveur), sans C |
| `crates/script` | Le moteur de scripts mSL, en bac à sable |
| `crates/ui` | La boîte à outils graphique façon Windows 98 : widgets, fenêtres internes, vue de discussion |
| `crates/relay` | Le relais (bouncer) |
| `app/desktop` | L'appli Mac et Linux |
| `app/android` | L'appli Android (Rust, et les classes Java dans `app/android/java`) |
| `xtask` | L'outillage : versions, icônes, paquets, APK, licences |
| `packaging`, `scripts` | Fichiers des paquets Linux, dépôt APT, notes de version, publication |
| `assets` | Polices et icônes |

Les choix importants sont expliqués dans [`docs/adr`](docs/adr/README.md), le vocabulaire dans
[`GLOSSARY.md`](GLOSSARY.md), les changements dans [`CHANGELOG.md`](CHANGELOG.md) (en anglais :
[`CHANGELOG.en.md`](CHANGELOG.en.md)), la façon de numéroter et de sortir une version dans
[`docs/versions.md`](docs/versions.md).

### Compilation automatique et versions

À chaque envoi sur GitHub, [GitHub Actions](.github/workflows/ci.yml) lance les tests sous Linux et compile AlpIRC
pour macOS, Android et Linux. Pour publier une version : `cargo xtask bump minor` (ou `patch`), compléter le
CHANGELOG, faire un commit, puis envoyer une étiquette `vX.Y.Z` : [le flux de publication](.github/workflows/release.yml)
fabrique le `.dmg` (Mac universel), l'`.apk`, le `.tar.gz` et le `.deb`, les met sur la page des versions avec les
notes en français et en anglais, et publie le dépôt APT signé sur GitHub Pages.

---

## English

**AlpIRC is the IRC client of the 2000s, rewritten from scratch in Rust.** It looks and feels like mIRC 5/6: one
main window holding the Status, channel and private windows, a switchbar (or a treebar), the nickname list on the
right, the input line at the bottom, a fixed-width font, grey Windows 98-style dialogs, and mIRC colour codes. It
connects to several networks at once (Undernet first, then Libera.Chat, OFTC, IRCnet, EFnet, DALnet, QuakeNet,
Rizon, EpiKnet) over TLS, with SASL, Undernet's X login, IRCv3, favourites (Channels Folder), Notify, Ignore,
Control, Perform, logs, DCC, and sandboxed mIRC scripts (mSL: aliases, popups, remotes, dialogs). On Android it
uses a phone-friendly layout, the native keyboard, and keeps connections open in the background. An optional relay
(bouncer, `alpirc-relay`) lets your phone and your computer share the same session and history. It runs on
**macOS**, **Android** and **Linux**. Free, no ads, no tracking, MIT licensed. Not affiliated with mIRC or
mIRC Co. Ltd.; no mIRC code or images are used.

**Status:** almost there. Everything planned for 1.0 is written and covered by automated tests (over 850); what
remains is trying it for real on a Mac and an Android phone, then 1.0 ships. The interface speaks English, French,
Spanish, German, Italian, Valencian and Irish (reviews from native speakers welcome).

**Install:** download the file for your device from the
[latest release](https://github.com/Guiwald/alpirc/releases/latest):

- **Mac** (Apple silicon and Intel, macOS 11+): open the `.dmg` and drag AlpIRC to Applications. The app isn't
  notarized: the first time, open it, then go to System Settings → Privacy & Security → **Open Anyway**. If macOS
  says the app is "damaged", run `xattr -dr com.apple.quarantine /Applications/AlpIRC.app` in Terminal.
- **Android 8+**: download the `.apk` on the phone, allow installing unknown apps for your browser, install, and
  allow notifications on first launch.
- **Linux** (64-bit; Ubuntu 22.04+, Debian 12+, Fedora 36+, Mint 21+): on Debian, Ubuntu or Mint, open the `.deb`
  (or `sudo apt install ./alpirc_X.Y.Z_amd64.deb`); it also adds AlpIRC's APT repository
  (`https://guiwald.github.io/alpirc/apt`), so updates come with your system's. Elsewhere, unpack the `.tar.gz`
  and run `./install.sh` (`--remove` to uninstall). File dialogs need `xdg-desktop-portal` or `zenity`.

**First steps:** pick a nickname, choose a network (Undernet is first) and click Connect; open the Channels Folder
(`#lyon`, `#grenoble` and `#chambery` on Undernet are there) or type `/join #channel`. Double-click a nickname for a
private chat, right-click it for whois, CTCP, DCC and op commands. Ctrl+K writes in colour. Alt+R opens the script
editor.

**Privacy:** no ads, trackers or statistics, no account. AlpIRC only connects to the IRC networks you choose (or
your own relay), directly. Your settings, scripts and logs stay on your device.

**What's new:** see [`CHANGELOG.en.md`](CHANGELOG.en.md), or the notes of each
[release](https://github.com/Guiwald/alpirc/releases) (French first, then English).

**Build from source:** install Rust (<https://rustup.rs>), then `git clone https://github.com/Guiwald/alpirc`,
`cd alpirc`, `cargo run --release` (on Linux, first `sudo apt install libasound2-dev pkg-config`). Developer
tooling: `cargo xtask help`.

## Licence

MIT — voir [`LICENSE`](LICENSE). Les composants tiers (polices) et leurs licences sont listés dans
[`THIRD_PARTY.md`](THIRD_PARTY.md) ; celles des crates Rust par `cargo xtask licenses`.
