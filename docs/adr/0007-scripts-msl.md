# ADR 0007 — Scripts mSL : l'essentiel et les dialogues, en bac à sable

- Statut : acceptée (17/10/2026, grilling par lots, Q4)

## Contexte

Le langage de script de mIRC (mSL) en est l'âme : alias, popups, réactions aux événements, scripts complets de
l'époque. Les vers mIRC (script.ini en 1997) ont montré qu'un script peut nuire : lancer des programmes, écraser des
fichiers, accepter des envois automatiques.

## Décision

1. **Un interpréteur mSL en Rust**, dans la 1.0 :
   - alias, popups (menus des fenêtres, de la liste des pseudos, de la barre de menus), remotes (`on TEXT`,
     `on JOIN`, `on CONNECT`… , `ctcp`, `raw`), groupes `#nom on|off` ;
   - `%variables` (locales et globales, enregistrées), `$identifiants` avec leurs propriétés, `[ ]` et `$+` ;
   - `if`/`elseif`/`else`, `while`, `goto`, `return`, `halt`, `haltdef` ;
   - timers, tables de hachage, fichiers texte (`/write`, `$read`), fenêtres `@` personnalisées ;
   - **dialogues personnalisés** (`dialog`, `/did`, `$did`, `on DIALOG`), dessinés par la boîte à outils d'AlpIRC ;
   - l'éditeur de scripts (Alt+R), avec ses onglets Aliases, Popups, Remote, Users, Variables.
2. **Bac à sable** :
   - pas de `/run` ;
   - lecture et écriture de fichiers limitées au dossier des scripts d'AlpIRC ;
   - aucun envoi DCC accepté automatiquement par un script ;
   - avertissement quand on charge un script et que celui-ci s'exécute au démarrage.
3. **1.1** : sockets de script, avec une autorisation à donner pour chaque script.
4. **Jamais** : `$dll`, `$com`, et tout ce qui dépend de Windows.

## Conséquences

- Les scripts mIRC existants se chargent (`.mrc`, `.ini`) ; ceux qui demandent des DLL ou des sockets ne marchent
  qu'en partie.
- Le même interpréteur tourne sur Mac, Linux et Android.
