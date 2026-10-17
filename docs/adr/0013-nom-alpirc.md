# ADR 0013 — Le client s'appelle AlpIRC

- Statut : acceptée (17/10/2026, grilling par lots, Q1, Q14, Q20)

## Contexte

Il faut un nom à nous (ADR 0001). Guillaume voulait « IRC » dans le nom, quelque chose de simple et compris à
l'international. Noms écartés : Alpagirc, Alpachat, Batifel, Barjaque, Bagoule, BagoulIRC (trop régionaux ou peu
exportables). Vérification RDAP du 17/10/2026 : `alpirc` est libre en .com, .net, .org, .app, .dev, .io et .fr ;
aucun logiciel homonyme trouvé. À éviter : bIRC (client Mac sorti en juillet 2026), gIRC (bibliothèque Go).

## Décision

**AlpIRC** : « Alp », la montagne (compris en anglais, allemand, français), clin d'œil aux canaux d'Undernet de
Guillaume au pied des Alpes (Grenoble, Chambéry) et frère d'Alpagamp ; « IRC » écrit en capitales comme dans mIRC.

- Nom affiché : `AlpIRC` ; dépôt, crates, identifiants : `alpirc` ; paquet Android : `fr.guiwald.alpirc`.
- Fichiers livrés : `AlpIRC-X.Y.Z.dmg`, `AlpIRC-X.Y.Z.apk`, `AlpIRC-X.Y.Z-linux-x86_64.tar.gz`,
  `alpirc_X.Y.Z_amd64.deb`.
