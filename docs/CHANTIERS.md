# Chantiers en cours

**C'est ici que vit le travail non terminé.** Chaque entrée porte trois choses :
ce qui est fait, **ce qui reste**, et les pièges déjà payés une fois. Une case
`- [ ]` non cochée = un chantier ouvert. `CLAUDE.md` n'en garde qu'un tableau
d'une ligne par chantier, pour savoir lequel existe sans lire tout ce fichier.

Le mot « journal » n'en dit que le tiers : on n'y écrit pas que ce qu'on a
appris, on y écrit aussi ce qu'il reste à faire. C'est le document qu'on ouvre
pour reprendre un chantier à froid.

Deux règles, et la seconde est la raison d'être du fichier :

1. **Retirer une entrée dès qu'elle est faite.** Une liste qu'on n'élague pas
   cesse d'être lue.
2. **Ce qui s'écrit ici, c'est ce qu'on ne retrouverait pas deux fois** — une
   mesure faite sur la bibliothèque réelle, une piste essayée puis abandonnée
   et pourquoi, un bug qui se reproduira à l'identique si personne ne l'a noté.
   L'avancement pur (« lot 3 fait ») vieillit tout seul ; la mesure qui a
   décidé du lot 3, non.

Quand un chantier a sa propre spec dans `docs/`, c'est **elle** qui décrit ce
que l'app fait ; l'entrée d'ici dit où on en est et ce qu'il faut savoir avant
de reprendre. En cas d'écart, la spec fait foi.

---

- [ ] **Story Mode et Create Your Own — phase 0 (2026-10-17).** Spec générique
      dans `docs/story/` ; le document complet et les maquettes restent en
      local (`docs/story/private/`, hors dépôt). Premier chantier long derrière un
      drapeau (`CLAUDE.md`, « Chantiers longs derrière un drapeau ») : tout
      sur `main`, rien de visible pour un utilisateur.
      **Fait** : le drapeau (toujours allumé sous `tauri dev`,
      `--enable-story-studio` / `--disable-story-studio` dans un build
      livré), l'entrée Story du rail, un hub vide chargé paresseusement, la
      règle `story-isolation` de `check-conventions.mjs`.
      **Reste, dans l'ordre** :
      - **L0, hors drapeau — fait sauf une vérification en jeu
        (2026-10-17).** Code de session partagé, testé comme tel :
        - *Configs CSP* : **le problème n'existe plus** depuis juillet, quand
          le lancement est passé de `race/config` à `race/quick`
          (`docs/L4-cm-launch-research.md`). Revérifié sur le journal de CM
          d'un lancement Pit Box du 2026-10-17 : `TriggerAutoLoadAsync` pour le
          circuit et la voiture, `ID to install` sur les patchs VAO. Le
          « Prerequisite » de la spec Story est donc périmé.
        - *Résultat de session* : `raceresult.rs` et `read_session_result`
          (SESSION§2.6), testés sur quatre fichiers réels. Aucun écran ne s'en
          sert encore.
        - *Grille par pilote* : force, nom et nationalité par ligne existaient
          déjà ; l'agressivité par ligne est ajoutée (`Opponent.aggression`,
          `Auto` par défaut, sans colonne à l'écran).
        - **Reste** : prouver en jeu que la force **par pilote** est
          respectée sur la version de CSP visée (installée : 0.3.0-preview622,
          build 4157 ; la spec ne nomme pas de version). L'historique de CM
          va dans ce sens sans le prouver : sur deux sessions (Zandvoort 2024,
          Imola 2026), sept paires de voitures identiques à forces
          différentes sur huit ont la plus forte devant — mais sur peu de
          tours, dont un track day de deux tours dans le trafic. Protocole : six voitures identiques à
          70, 76, 82, 88, 94 et 100, cinq tours, le joueur garé sur la
          grille, puis comparer les temps au tour dans le résultat.
        - **Repéré, non traité** : `cmimport.rs` et `sessionpreset.rs` ne
          relisent pas `AiAggressions` par ligne. Sans effet aujourd'hui
          (aucun écran ne pose cette valeur) ; à reprendre si une grille
          enregistrée doit un jour la porter.
      - **L1 — fait (2026-10-18).** `crates/story-engine/` (pur, sans
        Tauri, que rien dans l'app n'importe encore) et `crates/story-tool/`
        (`check`, `play`, résultats tapés « P3 1:18.2 », « dnf »,
        « failed »). Une histoire d'exemple neutre, `story-engine/sample/`,
        se joue de bout en bout, en test comme dans la CLI. Le format
        réellement lu est décrit dans la spec (« Engine file format »).
        **Décidé avec l'utilisateur** : l'échelle des relations est un
        réglage de l'histoire (cinq crans par défaut, la sauvegarde retient
        le *nom* du cran) ; des compteurs libres en plus des drapeaux ; une
        action du joueur se calcule **dans le moteur** — les règles d'achat
        y viendront avec les catalogues (L6), parce que le simulateur
        d'équilibrage joue sans écran. **Choisi en route** : YAML lu par
        `serde-saphyr` (Rust pur, sans `unsafe`, conçu pour ne pas
        paniquer ; `serde_yaml` est abandonné) ; conditions et effets en
        courtes phrases ; sauvegarde en JSON, `format: 1`, figée par
        `save_v1_still_reads` ; tirage des pools par un générateur dont
        l'état est sauvegardé (recharger ne relance pas le tirage) ; frais
        d'inscription payés au lancement, pas au résultat ; une entrée
        refusée ne change rien à l'état ; un événement repris par un second
        nœud déclenche un avertissement (ils partageraient tentatives et
        frais : une revanche a son propre fichier d'événement) ; une
        sauvegarde reprise après une mise à jour de l'histoire reçoit les
        personnages et compteurs ajoutés depuis.
        **Laissé pour plus tard, à savoir** :
        - le moteur a son propre type de résultat (`EventResult`) : le pont
          de L3 le remplira depuis `raceresult` (L0), sans que le moteur
          dépende du format de session de l'app ;
        - les identifiants de voiture (`car.<id>`) ne sont pas vérifiés :
          le paquet n'existe pas encore (ceux de pilote le sont depuis L2.6) ;
        - `wealth`, `economy` et `garage_slots` attendent l'économie (L6) ;
        - un contenu à réponses portait ses effets dans son en-tête
          (`replies`) : migrés dans le texte Ink en L2.4, la clé est
          désormais refusée.
      - **Format des dialogues — décidé (2026-10-18)** : Ink, dans une
        seule langue d'abord, pour valider tout le concept avant la
        traduction (décision de l'utilisateur, à condition que la bascule
        vers un format à identifiants de ligne reste peu coûteuse). D'où les
        règles de la spec (« Dialogue text ») : un sous-ensemble d'Ink, sans
        variables propres, conditions et effets écrits avec les phrases de
        l'histoire. Yarn écarté parce que son portage Rust se dit « en
        chantier, sans support » et vise Bevy. `blade-ink-rs`, prévu pour
        exécuter Ink, a été **écarté à l'essai en L2.4** (voir plus bas) : le
        moteur lit et joue le sous-ensemble lui-même.
      - **L2, Create Your Own — découpé en six sous-lots (validé
        2026-10-18)** : L2.1 ouvrir une histoire et construire son fil ;
        L2.2 le monde et les réglages (personnages, drapeaux, compteurs,
        échelle des relations, départ, `story.yaml`) ; L2.3 les événements
        (circuit et voiture tirés de la bibliothèque), puis L2.3b les
        dialogues en personne et le cadre dans le fil (ajouté le
        2026-10-20) ; L2.4 les contenus en
        Ink (vue Formulaire et vue Script, le moteur exécute Ink) ; L2.5
        l'aperçu dans le téléphone (modèle fil de discussion d'abord) ;
        L2.6 les pilotes (facultatif).
        - **L2.1 — fait (2026-10-18).** Le hub liste « Mes histoires »
          (`app_config_dir/story/studio.json`), en crée une dans un dossier
          vide ou en ouvre une existante ; le studio montre les chapitres et
          le fil de l'un d'eux, nœud par nœud, avec le panneau du nœud
          sélectionné et la vérification du moteur relancée après chaque
          enregistrement. Le moteur entre dans l'app par le seul domaine
          Story (`src-tauri/src/story/studio.rs`, `commands/story.rs`,
          `src/lib/story/`). Il sait désormais **écrire** un chapitre
          (`write.rs`, forme canonique relue avant d'être rendue) et
          rattache un diagnostic à son nœud. **Choisi en route, avec
          l'accord de l'utilisateur** : une colonne disposée par l'ordre
          plutôt qu'une toile (rien à enregistrer sur la place d'une boîte) ;
          chaque changement écrit aussitôt, fichier réécrit en entier
          (commentaires tapés à la main perdus dans ce fichier) ; messages
          de vérification en anglais tant que l'éditeur est réservé.
          **Choisi sans demander** : un nœud neuf porte toutes les clés de
          son type, et ce qui reste à choisir (l'événement d'un nœud
          Event, la cible d'une branche) est laissé vide pour que la
          vérification le montre, plutôt qu'une valeur devinée ; la langue
          d'une histoire neuve est celle de l'app ; l'identifiant du nœud
          de départ ne se change pas dans le fil (il est dans `story.yaml`,
          que L2.2 éditera).
          **Limites connues** : un nœud Message ou Event ne vise que des
          contenus et événements déjà écrits à la main (leurs écrans sont
          L2.3 et L2.4 ; levée pour les événements, L2.3 fait) ; ouvrir une histoire puis changer d'écran ne laisse
          pas d'entrée dans l'historique de navigation (le studio est un
          état interne de l'écran Story, comme un onglet) ; un chapitre ne se
          supprime pas encore dans l'éditeur.
        - **L2.2 — fait (2026-10-18).** Deux sections de plus dans le
          studio : **Monde** (personnages avec nom affiché et relation de
          départ, drapeaux, compteurs avec leur valeur de départ) et
          **Réglages** (titre, auteurs, version, langue ; nœud, date, heure,
          réputation, abonnés et argent de départ ; échec, tentatives,
          reprises payantes, aide après échecs ; économie, sauvegardes ; date
          affichée, protagoniste ; échelle des relations). Chaque nom montre
          les lignes qui le citent. Le moteur écrit `story.yaml` et `world/`
          comme un chapitre (`write.rs`, relu avant d'être rendu) et sait
          trouver et renommer les citations (`refs.rs`).
          **Décidé avec l'utilisateur** : renommer un personnage, un
          drapeau, un compteur ou un cran fait suivre toutes les phrases qui
          le citent, après confirmation quand il y en a (option A, sur
          l'exemple du compteur cité trois fois de l'histoire d'exemple).
          **Choisi sans demander** : les citations se cherchent dans le
          texte des fichiers et non dans l'histoire analysée, pour ne changer
          que le mot dans les fichiers que l'éditeur n'écrit pas encore et
          pour atteindre les scripts Ink plus tard ; une ligne de commentaire
          qui cite un nom compte donc comme une citation. Le renommage ne
          prend pas d'empreinte : il relit les fichiers au moment où il
          s'exécute et ne change que les mots cités, donc une modification
          faite ailleurs entre-temps est gardée. Un nom est une lettre puis
          lettres, chiffres et `_` (plus strict que ce que le moteur lit) ;
          celui d'un nouveau personnage vient de son nom affiché. L'échelle
          des relations est dans Réglages, comme dans le tableau de la spec,
          et non dans Monde. Un groupe de réglages égal à son défaut n'est
          pas écrit ; différent, il l'est en entier. Les compteurs s'écrivent
          dans l'ordre alphabétique, quel que soit l'ordre de saisie. La
          langue se choisit parmi les six de l'app (plus celle de l'histoire
          si elle est autre) ; les traductions déclarées sont gardées. Les
          reprises payantes ne s'affichent qu'avec l'économie, la limite de
          tentatives qu'avec « recommencer » ; allumées, la limite vaut 3 et
          l'aide 1 cran tous les 2 échecs, au plus 2. La date de départ est
          toujours une date complète : une histoire en « année N » montre le
          1ᵉʳ janvier de l'an 1. Les erreurs de `story.yaml` comptent dans
          Réglages, celles de `world/` dans Monde, le reste dans le Fil.
          **Limites connues** : les voitures de départ et les versions
          requises ne s'éditent pas (bibliothèque en L2.3, paquet en L4) ;
          renommer dans une histoire **déjà publiée** fait perdre à une
          partie en cours la valeur de l'ancien drapeau ou compteur (repris à
          sa valeur de départ), et une sauvegarde sur un cran renommé est
          refusée — un avertissement à la publication serait à prévoir (L4).
        - **Annuler et copies de sécurité — fait (2026-10-19).** Demandé
          par l'utilisateur après L2.2, les deux à la fois : Ctrl+Z / Ctrl+Y
          (et boutons ↶ ↷ sous le titre) pour l'erreur d'il y a dix
          secondes, une copie par jour pour la mauvaise surprise du
          lendemain (`story/history.rs`, `story/backups.rs`, section
          « Copies de sécurité » du studio). **Choisi sans demander** :
          l'historique garde les 100 dernières modifications, en mémoire
          seulement ; une modification faite ailleurs entre-temps vide
          l'historique plutôt que de n'en annuler qu'une partie ; les
          touches restent au champ de saisie quand on y tape ; une copie
          à la **première ouverture du jour** (et non à chaque ouverture,
          que le studio refait à chaque rechargement), plus une avant
          chaque restauration, dix gardées ; les copies vont dans
          `app_config_dir/story/backups/`, pas dans le dossier de
          l'histoire, souvent un dépôt git où elles finiraient commitées ;
          une copie ne contient que les fichiers que l'éditeur écrit, pas
          les médias ; restaurer retire les fichiers de l'histoire ajoutés
          depuis (pas les autres fichiers du dossier) et s'annule par
          Ctrl+Z. Créer une histoire ne s'annule pas.
        - **L2.3 — fait (2026-10-19).** Section **Événements** du studio :
          la liste dans l'ordre de jeu (par chapitre, puis épreuves annexes,
          puis « pas encore utilisés »), la fiche (nom affiché, type,
          circuit et voiture choisis dans la bibliothèque, session, réussite,
          effets et avance du temps, pool d'échec et nombre de variantes,
          inscription si l'histoire a une économie), le nœud Event du Fil
          avec « Ouvrir » et « Nouvel événement ». Le moteur type la session
          (`model::Session`, liste fermée) au lieu d'une valeur opaque, écrit
          `events/<id>.yaml` en forme canonique et sait renommer un
          événement (`refs.rs`, `Changes` : fichiers écrits et retirés).
          **Décidé avec l'utilisateur** : pour ce lot, l'auteur règle circuit,
          layout, voiture, heure, météo et, pour une course, tours, grille,
          niveau des adversaires et départ ; le reste est au joueur ; grille
          monomarque (la voiture de l'événement) ; voiture imposée.
          **Demandé pour la suite** (écrit dans la spec, « Events, retries and
          failure ») : l'auteur choisit réglage par réglage ce qui est imposé
          ou laissé au joueur ; la grille complète est celle de l'auteur
          (reprendre l'écran de session de Pit Box, mais rempli par
          l'auteur) ; la voiture est imposée ou choisie dans le garage de
          l'histoire, éventuellement parmi une liste permise.
          **Choisi sans demander** : la session a ses propres noms (`track`,
          `car`, `laps`, `grid`, `ai_level`, `start`, `time`, `weather`) que
          le lancement (L3) traduira vers `RaceSetup`, plutôt que le format
          de `launch.rs` lui-même (le moteur est pur, et ce format porte des
          détails de CM) ; `grid` compte le joueur ; la météo est l'intention
          de Pit Box (`clear`, `rain`…), pas un dossier ; les quatre départs
          de l'écran de session (aléatoire, premier, deuxième, dernier) ;
          sans circuit ou sans voiture, un événement est en erreur ; changer
          de type retire tours, grille, niveau et départ ; un circuit à
          layouts prend le premier ; un nouvel événement est une course ;
          renommer un événement déplace son fichier quand il porte son nom
          (seule exception à « les noms de fichiers ne changent pas ») ;
          supprimer laisse les citations aux vérifications ; Ctrl+Z reste sur
          la section affichée quand la modification annulée la touchait ;
          libellé « Hotlap » comme ailleurs dans l'app.
          **Limites connues** : les messages d'échec ne s'écrivent pas encore
          (L2.4) — on choisit un pool existant et on voit ses variantes ;
          `ahead_of` n'a pas de contrôle (il attend les pilotes, L2.6 ; levée,
          L2.6 fait) ; la bibliothèque est lue une fois par lancement de
          l'app ; aucune vérification qu'un circuit accueille la taille de
          grille demandée (le jeu la plafonne au nombre de stands).
        - **L2.3b — fait (2026-10-20).** Sous-lot ajouté avant L2.4 :
          les dialogues en personne et le cadre dans le fil. Le moteur
          connaît le type de contenu `dialogue` (joué sur la scène, il n'a
          besoin d'aucun cadre), le cadre de départ (`start.frame`), le champ
          `frame` d'un Beat (un cadre ou `none`, appliqué avant ses effets),
          le cadre actif dans l'état et la sauvegarde, et les portraits d'un
          personnage par expression (`world/characters.yaml`, chemins dans
          `media/`). Les cadres vivent dans `frame.rs` : une table des cadres
          livrés (le téléphone seul pour l'instant), pas des cas dans le
          moteur. La vérification suit le cadre le long du fil et signale un
          contenu que le cadre actif ne peut pas montrer. Dans l'éditeur :
          le cadre se choisit à la création de l'histoire et dans Réglages,
          le nœud Beat a un champ Cadre, et Monde ouvre les portraits d'un
          personnage.
          **Décidé avec l'utilisateur** : **aucun cadre par défaut**, nulle
          part : Pit Box est une plateforme pour d'autres auteurs, et le
          téléphone n'est qu'un cadre parmi d'autres. `start.frame` est donc
          obligatoire, la fenêtre de création ne présélectionne rien, et un
          nouveau Beat laisse le cadre inchangé. Les histoires écrites avant
          ne se lisent plus sans ajouter la clé (l'utilisateur a jugé que
          c'était sans enjeu : l'auteur n'avait encore rien écrit). « Rien
          pour l'afficher » est un **avertissement**, levé dès qu'un chemin
          arrive sans cadre capable, en distinguant « toujours » de « sur
          certains chemins » ; les conditions des branches ne sont pas
          évaluées, donc un chemin impossible en pratique peut donner une
          fausse alerte. Un portrait se choisit **parmi les images déjà dans
          `media/`** : l'éditeur n'écrit aucun média.
          **Choisi sans demander** : une sauvegarde sans cadre (antérieure à
          L2.3b) reprend le cadre de départ de son histoire, sans changer de
          format ; un cadre autre que `phone` ou `none` est une erreur tant
          que `frames/` n'existe pas ; un portrait introuvable, un chemin
          qui sort de `media/` ou des portraits sans `neutral` sont des
          erreurs ; le chemin est comparé avec la casse du disque (une
          histoire partagée est un zip) ; les médias ne sont que listés,
          jamais lus, et restent hors des copies de sécurité ; la liste se
          relit à chaque vérification, donc une image ajoutée pendant que le
          studio est ouvert apparaît à l'enregistrement suivant ; une
          nouvelle expression commence sans image, pour que la vérification
          le montre plutôt qu'un choix deviné ; un changement de cadre est
          une sortie du moteur (`Output::FrameChanged`) et pas une ligne
          d'historique ; un contenu réactif n'est signalé que si aucun
          cadre de toute l'histoire ne peut le montrer.
          **Laissé pour plus tard, à savoir** :
          - *Un contenu envoyé sans cadre capable de le montrer attend un
            cadre* (la spec le dit) : **reporté à L3**. Aujourd'hui le
            moteur le livre comme un autre, et seule la vérification le
            signale. À trancher en L3 : un contenu en attente compte-t-il
            pour `content.<id>.delivered` ?
          - *« Choisir un fichier » pour un portrait* : une boîte de
            sélection qui copie l'image dans `media/portraits/`. Plus
            confortable que la liste actuelle, mais l'éditeur se mettrait à
            écrire des médias, que les copies de sécurité ne contiennent pas
            (demandé par l'utilisateur, pas urgent).
          - L'en-tête d'un dialogue (personnages présents, scène) et les tags
            `#expression:` / `#position:` vérifiés contre les portraits :
            faits en L2.4.
          - Un nœud Action sans cadre n'est pas signalé (en roman visuel,
            les actions du joueur passent par les réponses).
          - Le type `illustration` et les transitions arrivent avec L3b.
        - **L2.4 — fait (2026-10-20)**, avec son complément (dialogues en
          personne et sorties par réponse), en quatre commits : le moteur,
          les sorties par réponse dans le Fil, la section Contenus avec la
          vue Script, la vue Formulaire.
          Le moteur lit le texte d'un contenu en arbre et le joue
          (`script.rs`) : répliques avec orateur et tags, conditions
          `check()`, effets `effect()`, choix étiquetés imbriqués, points de
          rencontre, `-> DONE` ; tout le reste est une erreur sur sa ligne.
          Livrer un contenu joue sa conversation jusqu'au premier choix ; la
          réponse se donne par son étiquette ; une conversation en cours se
          sauvegarde et se reprend (la sauvegarde garde le format 1). Un
          dialogue déclare ses personnages présents et sa scène. Un nœud
          Message a des sorties par réponse (`on_reply`).
          **Décidé avec l'utilisateur** :
          - **notre propre interpréteur du sous-ensemble plutôt que
            blade-ink-rs**, écarté à l'essai (`bladeink` et
            `bladeink-compiler` 2.1.0) : son compilateur refuse
            `~ effect("x += 1")` (« unterminated string literal ») dès que le
            texte entre guillemets contient un opérateur, et l'étiquette d'un
            choix n'est pas accessible au moteur. Les scripts restent de
            l'Ink valide ; aucune dépendance ajoutée ;
          - l'orateur s'écrit par l'**identifiant** du personnage
            (`friend: …`), pas son nom affiché : vérifiable, et un
            renommage le fait suivre ;
          - avec plusieurs moments de choix, `content.x.reply == yes` est
            vrai si `yes` a été choisi à n'importe lequel, et le nœud sort par
            l'exit de **la réponse la plus récente qui en a une** ;
          - coupure nette pour `replies` (clé refusée, exemple converti) ;
          - les quatre étapes enchaînées, chacune commitée.
          **Choisi sans demander** : une réplique sans préfixe est dite par
          l'expéditeur (un mail, un article s'écrivent en texte simple) ; un
          préfixe qui ressemble à un nom sans être un personnage est un
          avertissement ; seulement `*` (pas les choix collants `+`), le texte
          d'un choix toujours entre crochets, pas de commentaires `//` ; un
          point de choix dont aucun choix ne passe ses conditions est sauté ;
          un dialogue à choix retient toujours le fil ; un nœud à sorties par
          réponse attend comme `wait_reply` (la case est alors cochée et
          verrouillée) ; livrer un contenu à nouveau ouvre une nouvelle
          conversation et oublie les réponses de la précédente ; deux contenus
          qui s'envoient l'un l'autre sont une boucle (erreur), pas un gel ;
          une sauvegarde qui attend une réponse sans conversation en cours, ou
          dont le point de choix a disparu, est refusée comme périmée ; les
          positions sont `left`, `center`, `right` ; un nouveau contenu a son
          fichier `content/<id>.story`, et supprimer le dernier contenu d'un
          fichier retire le fichier ; le texte s'écrit même avec des erreurs
          d'Ink (une conversation se tape en plusieurs fois), jamais avec une
          ligne `---` seule ; une nouvelle réplique, un nouveau choix
          commencent vides, pour que la vérification le montre, et un choix
          reçoit l'étiquette `reply_<n>` libre ; l'écrivain d'Ink ajoute le
          point de rencontre qui manque après des choix ; un texte qui ne se
          lit pas entièrement ne s'ouvre qu'en vue Script ; la vue Formulaire
          s'ouvre par défaut. **Refactorings en passant** : `PickList` (la
          liste des Événements, reprise par les Contenus, commit à part) ;
          `side_lists` de `refs.rs` généralisé en `listed` pour la liste
          `characters` d'un dialogue, dans le commit du moteur.
          **Limites connues** :
          - renommer une **étiquette** ne fait pas suivre les sorties et les
            conditions qui la citent, et renommer un **contenu** ne fait pas
            suivre ses citations (signalées par les vérifications) ;
          - la scène d'un dialogue est un nom non vérifié tant que les scènes
            n'existent pas ;
          - le texte ne s'enregistre qu'en quittant le champ (vue Script) ;
          - `//` est refusé partout dans le texte (Ink le lit comme un
            commentaire), donc une adresse web ne s'écrit pas encore : il
            faudrait accepter l'échappement `\/` d'Ink, et le retirer à la
            lecture ;
          - l'aperçu dans le cadre et sur la scène est L2.5.
        - **L2.5 — fait (2026-10-20)**, avec son complément (l'aperçu d'un
          dialogue). La colonne d'aperçu de la section Contenus : le contenu
          joué par le moteur seul (`engine::preview`, hors du fil, réponses
          rejouées depuis le début à chaque fois, donc juste quand le texte
          change), dans le téléphone (modèle fil de discussion) ou sur la
          scène 16:9 pour un dialogue, ou l'avertissement de la vérification
          quand aucun cadre ne peut le montrer. « Aperçu avec » règle les
          variables que le contenu lit ; ce que la conversation a changé et
          ce qu'elle a envoyé s'affichent dessous. Les vues sont celles du
          rendu joueur (`components/story/player/` : `PhoneFrame`,
          `ThreadView`, `DialogueStage`, et leur logique pure dans
          `src/lib/story/player/`) : elles ne connaissent pas l'éditeur, L3
          les reprendra telles quelles.
          **Décidé avec l'utilisateur** : l'état de l'aperçu part de l'état
          de départ et ne règle que les variables que le contenu lit,
          gardées en mémoire par contenu, jamais écrites ; le plateau d'un
          dialogue (tous les présents visibles, celui qui parle mis en avant ;
          `neutral` jusqu'à une expression, qui reste ; sans position, l'ordre
          de `characters` à gauche, à droite, au centre, et une position
          donnée reste). Le cadre par lequel un contenu peut arriver est
          calculé à un seul endroit (`frame::FrameMap`), que la vérification
          « rien pour l'afficher » et l'aperçu partagent.
          **Choisi sans demander** : seul `message` a un modèle dans le
          téléphone pour l'instant (la question pour `call` est restée sans
          réponse) ; les autres types affichent « pas encore d'aperçu » ;
          l'heure du téléphone est celle du contenu, sinon celle du départ ; la
          date suit le réglage d'affichage (saisons de l'hémisphère nord, le
          réglage de calendrier n'existant pas encore) ; l'avatar est le
          portrait `neutral`, sinon les initiales ; les réponses du joueur sont
          en bleu (le rouge a son barème) ; changer un réglage recommence la
          conversation ; un `send` n'est que nommé ; l'aperçu passe sous la
          fiche quand la place manque, par un simple retour à la ligne, sans
          seuil. Le message « rien pour l'afficher » d'un contenu réactif
          change de mots (« no frame it can arrive in »), puisque la même
          phrase sert aux contenus envoyés par le fil.
          **Limites connues** : pas d'aperçu pour les autres types que
          `message` dans le téléphone ; faits d'épreuve et de contenu non
          réglables ; la scène est un fond uni.
        - **L2.6 — fait (2026-10-20).** Section **Pilotes** du studio (le
          roster, `drivers/roster.yaml`) et bloc **Adversaires** de la fiche
          d'une course. Le moteur lit le roster (nom, personnage, nationalité,
          âge, rôle, niveau, agressivité), la grille d'un événement
          (`session.opponents` : pilote, voiture, livrée, niveau, lest,
          bride), vérifie l'une et l'autre et `ahead_of`, et sait renommer un
          pilote (grilles et critères suivent) ; renommer un personnage fait
          suivre le pilote qu'il est.
          **Décidé avec l'utilisateur** : périmètre roster + grille (force par
          chapitre, livrée par série, carrières et classements reportés) ; le
          tableau des adversaires de l'écran de session est **extrait**
          (`OpponentTable`, et `NationalityPicker` pour la liste des pays,
          commit de refactoring à part) et repris tel quel avec le sélecteur
          de voitures (`OpponentPicker`), plutôt qu'une grille Story écrite à
          côté.
          **Choisi sans demander** : un pilote peut **être** un personnage
          (clé `character`), dont il prend le nom quand il n'en a pas, et un
          personnage n'est qu'un pilote ; pas de portrait propre au pilote
          tant que rien ne l'affiche (celui du personnage suffira) ; le
          pilote d'une ligne se choisit dans le roster, jamais tapé, et la
          nationalité suit le pilote (celle de la livrée sans pilote) ; une
          ligne sans niveau à elle affiche celui de son pilote (sans
          pilote ni niveau : `Auto`, celui de l'événement) ; passer
          l'événement à un autre type que la course retire aussi ses
          `ahead_of` ; pas
          d'agressivité par ligne, c'est celle du pilote ; une voiture
          choisie s'écrit toujours (une ligne sans `car` ne vient que d'un
          fichier tapé à la main) ; remplacer la voiture retire la livrée ;
          une ligne copiée n'a pas de pilote ; `grid` plus petit que la grille
          posée est une erreur, et sans `grid` la grille posée est tout le
          plateau ; `ahead_of` d'un pilote absent de la grille est une
          **erreur** (l'épreuve ne se gagnerait jamais) ; un pilote sans nom
          ni personnage est un avertissement ; niveau et agressivité de 0 à
          100 pour le moteur, mais 70 à 100 dans l'éditeur pour le niveau
          (la plage de Content Manager, comme l'écran de session) ; la fiche
          d'un événement s'élargit pour le tableau (les autres blocs gardent
          leur largeur), qui défile en largeur sous 1 040 px plutôt que
          d'écraser le nom de la voiture ; le roster est un fichier qui
          s'annule et se copie comme les autres.
          **Limites connues** :
          - la grille n'est pas encore lancée : c'est L3 qui la traduira en
            `RaceSetup` (pilotes nommés, places restantes en noms de
            remplissage au niveau de l'événement) ;
          - le niveau de l'événement (`ai_level`, L2.3) s'éditait de 0 à 100
            quand celui d'un pilote ou d'une ligne s'arrête à 70 : aligné sur
            70 à 100 (2026-10-20). Un événement à 70 ne laisse donc aucune
            marge à l'aide après échecs, ce que la taille d'un cran (L3)
            devra prendre en compte ;
          - aucune vérification qu'une livrée existe pour sa voiture (le
            moteur ne connaît pas la bibliothèque, comme pour les voitures) ;
          - pas de vivier ni de tirage au hasard comme dans l'écran de
            session : l'auteur pose ses adversaires un par un (ou par lot dans
            le sélecteur).
      - **L3, Story Mode côté joueur — découpé en quatre sous-lots (validé
        2026-10-20)** : L3.1 jouer sans course (hub, partie et sauvegarde,
        écran de jeu avec la scène, le téléphone et son fil de messages, les
        dialogues) ; L3.2 les courses (la grille devient une session, AC se
        lance, le résultat revient, tentatives et messages d'échec) ; la suite
        a été redécoupée le 2026-10-21 (voir plus bas) : L3.3 la couche
        dialogue, L3.4 les histoires sans cadre, L3.5 les autres applications
        du téléphone.
        - **L3.1 — fait (2026-10-20).** Le hub liste l'histoire d'exemple et
          celles de Create Your Own, avec la partie en cours ; « Nouvelle
          partie » et « Continuer » ouvrent l'écran de jeu : la scène (fond
          uni), le téléphone et ses fils de messages, la couche de dialogue,
          et à côté ce qu'attend l'histoire (résumé d'une épreuve, action,
          réponse attendue, fin). Le moteur garde dans l'historique les
          répliques vues et le texte des réponses (`Happening::Said`,
          `Replied.text`), et reconstruit l'écran depuis l'état seul
          (`view.rs`) : une partie reprise montre ce qui avait été vu.
          L'histoire d'exemple est embarquée (`sample.rs`, testé contre le
          dossier). Les parties sont dans `app_config_dir/story/saves/`.
          **Décidé avec l'utilisateur** : le découpage ; une partie par
          histoire, écrite après chaque geste (les emplacements et le
          rechargement viendront plus tard) ; l'histoire d'exemple en plus
          de celles de l'auteur.
          **Choisi sans demander** : une histoire avec des erreurs de
          vérification ne se lance pas ; chaque commande relit l'histoire et
          la sauvegarde (rien en mémoire entre deux gestes), donc une histoire
          modifiée dans le studio se joue telle quelle ou refuse la partie
          avec la raison ; écriture par fichier temporaire renommé ; la partie
          d'une histoire de l'auteur est `saves/<id>-<empreinte du dossier>.json`
          (deux copies d'une histoire ne partagent pas leur partie : la
          déplacer fait donc perdre la partie), celle de l'exemple
          `saves/builtin/sample.json` ; une partie qui ne peut plus reprendre
          (histoire modifiée, fichier illisible) reste listée avec sa raison,
          et « Nouvelle partie » demande quand même avant de la remplacer ;
          l'histoire est vérifiée au début d'une partie et à sa reprise, pas à
          chaque geste ; un même geste demandé deux fois de suite (double
          clic) n'est joué qu'une fois ; livrer à nouveau un contenu le rend
          « non vu » (le moteur oubliait déjà ses réponses) ; une ancienne
          livraison d'un contenu compte comme lue ; le format de sauvegarde
          reste le 1 (l'historique gagne une variante et un champ, une partie
          écrite avant se relit) ; un fil par expéditeur, le plus récent en
          haut, marqué non lu tant qu'un message n'est pas ouvert ou qu'une
          réponse est attendue (pastille bleue, le rouge a son barème) ;
          ouvrir un fil marque ses messages vus ; le dialogue en cours est le
          dernier livré, montré tant qu'il attend une réponse ou n'a pas été
          lu jusqu'au bout ; une action du joueur est une carte avec un
          bouton « Fait » ; l'exemple n'a pas ses portraits (initiales) ;
          « Quitter » ne demande rien, tout est déjà enregistré.
          **Limites connues** : pas encore de lancement de course (L3.2) ;
          les autres types de contenu ne sont que nommés dans le téléphone
          (L3.3) ; une histoire sans cadre n'a rien pour ses messages (L3.4) ;
          la date d'une partie n'est pas montrée dans le hub ; le jeu n'a pas
          été lancé à l'écran.
        - **L3.2 — fait (2026-10-20).** « Aller à l'épreuve » paie la tentative
          puis lance la session par le lancement de l'écran de session
          (`startSession`, porte Steam, pilote et livrée du joueur). La
          session part des réglages du joueur pour ce type (les presets de
          l'écran de session) et prend ceux de l'événement par-dessus
          (`story/race.ts`, pur et testé ; `raceLaunch.ts` lit les fichiers).
          Au retour du jeu, le résultat (`race_out.json`, L0) devient celui du
          moteur ; une session restée en cours à la fermeture de Pit Box est
          relue à l'ouverture. Refactoring en passant : `defaultSetup()` sorti
          de `Launch.svelte` (commit à part).
          **Choisi sans demander** : une course part sur la grille, sans essais
          ni qualification (l'auteur règle le départ) ; contre-la-montre =
          hotlap, essai = practice ; météo de l'événement, sinon celle du
          joueur pour ce type, sinon ciel clair ; le niveau de l'événement est
          celui de tous les adversaires anonymes (écart 0) ; sans `grid` ni
          `opponents`, le nombre habituel du joueur (preset, sinon 7) ; un cran
          d'aide = 2 points, jamais sous 70 ; la session décisive est la
          course d'une course, la dernière sinon ; une course est finie sans
          abandon, une autre session avec un tour ; un pilote nommé est
          retrouvé par le nom que la grille lui a donné, pas par sa place (le
          lancement écarte une voiture qui ne peut pas rouler, ce qui décale
          les suivantes), et compte battu s'il finit derrière ou abandonne,
          jamais si le joueur abandonne ; un résultat dont le circuit ou la
          voiture du joueur n'est pas celui de l'épreuve n'est pas pris (une
          session lancée ailleurs entre-temps) ; l'heure du lancement est
          gardée à côté de la sauvegarde (`.launched`), et un relancement
          avant résultat garde la première ; sans résultat, quelle qu'en soit
          la raison (jeu fermé tôt, jamais démarré, lancement échoué) :
          « Relancer » (sans payer) ou « Abandonner » (non fini) ; le bouton
          est le rouge de l'écran, comme le lancement de session.
          **Limites connues** : rien n'a été lancé en vrai (pas de jeu ici) :
          la session construite, le retour du résultat et les noms de pilotes
          dans la grille sont à vérifier en jeu - en particulier que
          `race_out.json` rend bien le nom posé pour chaque adversaire, dont
          dépend « finir devant » ; deux pilotes du roster au même nom
          affiché se confondraient ; les épreuves annexes ne
          se lancent pas encore (aucun écran ne les montre) ; le dernier
          résultat n'est pas affiché sur la carte (seules les réactions du
          téléphone le disent).
        - **Retours sur l'éditeur (2026-10-21)**, demandés par l'utilisateur
          après un premier essai : la fiche d'un événement en deux colonnes,
          une note d'aide à droite de chaque bloc (`HelpNote`, l'encadré du
          thème ; `EffectsHelp` pour la syntaxe des effets et les noms de
          l'histoire) et ses vérifications à droite de son nom ; « Copier
          l'exemple » dans Create Your Own.
          **Choisi sans demander** : les notes sont toujours visibles (pas de
          bouton pour les masquer) ; le tableau des adversaires prend les deux
          colonnes et n'a pas de note ; sous 820 px de fiche, une seule
          colonne ; les vérifications de la section restent en bas quand
          aucun événement n'est affiché ; la copie de l'exemple garde son id
          (`sample`) et va dans un dossier vide, comme une histoire neuve.
          Écart assumé avec la règle « un libellé n'explique jamais son
          fonctionnement » : ces notes sont la documentation d'un langage
          d'auteur (la syntaxe des effets), demandée explicitement.
          **Reste** : les autres écrans du studio (Fil, Monde, Pilotes,
          Contenus) gardent leurs vérifications en bas et n'ont pas de notes ;
          les effets d'un nœud Beat (Fil) mériteraient la même aide.
        - **Suite de L3 — redécoupée et décidée avec l'utilisateur
          (2026-10-21)**, à partir de ses notes « couche dialogue et
          histoires sans cadre ». L'ordre est libre : le plus simple pour le
          développement. **À faire** :
          - **L3.3, la couche dialogue — fait (2026-10-21).** Au début d'un
            dialogue, le cadre et la carte d'attente glissent sous l'écran, le
            flou de la couche de fond passe de 6 à 1 px, puis la scène
            (`DialogueStage` nu, sans fond ni bordure) apparaît en fondu ; à
            la fin, le cadre remonte. Le « Journal » de la barre relit les
            dialogues passés sur la scène, réponses choisies comprises.
            **Choisi sans demander** : le dialogue garde le 16:9 de l'aperçu
            (même composition pour l'auteur et le joueur), aussi grand que la
            scène le permet ; il prend le clavier dès qu'il s'ouvre ; la
            flèche gauche revient d'une réplique (en jeu comme au journal) ;
            au journal, une réponse choisie est une page à elle, au nom de
            « Vous », personne ne parlant sur scène ; le journal liste du plus
            récent au plus ancien, avec la date telle que l'histoire l'affiche
            et l'heure, et les personnages présents ; un dialogue sans
            réplique n'y figure pas ; son bouton est grisé pendant un
            dialogue ; Échap revient à la liste puis ferme (le B de la manette
            aussi) ; aller au bout d'un dialogue relu ramène à la liste ;
            fondu de 250 ms après 200 ms, glissement de 350 ms ; pas
            d'éclaircissement de la scène en plus du flou.
            **Limites connues** : sur le fond uni d'aujourd'hui, le flou ne
            se voit pas (il attend les scènes) ; pas vu à l'écran.
          - **L3.4, les histoires sans cadre — fait (2026-10-21).** Un
            contenu qu'aucun cadre ne peut montrer attend dans l'état
            (`State::held`, sauvegardé, absent des parties écrites avant) et
            sort au Beat qui pose un cadre capable de le montrer ; la carte
            d'épreuve est en tête des messages du téléphone (« Prochaine
            épreuve »), sur la scène sans cadre ; un nœud Action atteint sans
            cadre est signalé. L'exemple a son chapitre 3 sans cadre.
            **Décidé avec l'utilisateur** : en attente, un contenu n'est pas
            livré (`content.<id>.delivered` reste faux) ; plusieurs sortent
            dans l'ordre d'arrivée, datés de leur affichage ; la carte
            d'épreuve dans le téléphone avec un cadre, sur la scène sans ; une
            action sans cadre signalée, « Fait » en secours ; le chapitre 3
            de l'exemple et ses tests.
            **Choisi sans demander** : un nœud Message dont le contenu attend
            ne retient pas le fil (rien ne pourrait y répondre ; les
            vérifications l'annonçaient déjà) ; un contenu réactif en attente
            compte comme parti, il n'est pas remis en attente à chaque
            vérification ; à sa sortie, un contenu réactif n'est pas revérifié
            (il était arrivé, il attendait seulement de quoi s'afficher) ; il
            sort avant les effets et l'avance du temps du Beat qui pose le
            cadre (le cadre est posé avant les effets) ; l'aperçu de l'éditeur
            joue un contenu même sans cadre pour le montrer ; une partie qui
            attend un contenu que l'histoire n'a plus est refusée, avec sa
            raison ; le message de l'exemple est réactif (à l'arrivée du
            dialogue) plutôt qu'envoyé par un Beat, pour que l'exemple reste
            sans avertissement ; l'avertissement d'une action sans cadre dit
            « sur certains chemins » comme celui d'un contenu sans cadre ; une
            épreuve qui se met à attendre ramène le téléphone d'un fil ouvert
            à la liste des messages, qu'elle coiffe ; la flèche gauche est
            sans effet le temps qu'une réponse revienne.
            **Limites connues** : pas vu à l'écran ; aucune indication au
            joueur qu'un message attend son téléphone (c'est voulu : il
            n'existe pas encore pour lui) ; un contenu réactif `each_time`
            redevenu vrai pendant qu'une première arrivée attend ne repart
            pas une seconde fois (comme pendant un délai) ; rien ne signale
            un contenu qui attendrait encore à la fin de l'histoire ; un
            message du fil mis en attente perd ses sorties par réponse (le
            nœud est déjà quitté), ce que l'avertissement « attendra un
            cadre » annonce sans le détailler.
          - **L3.5, les autres applications du téléphone — fait
            (2026-10-21).** Un écran d'accueil d'applications : Messages,
            Mail (boîte de réception), Actualités et Réseaux (fils), Banque
            (relevé, sous le solde). Les modèles (`MailView`, `FeedView`,
            `StatementView`) servent aussi à l'aperçu de l'éditeur.
            **Choisi sans demander** : une application apparaît avec son
            premier contenu, Messages toujours ; le téléphone s'ouvre sur
            Messages (la prochaine épreuve y est) et y revient quand une
            épreuve se met à attendre ; chaque application porte le nombre de
            contenus non lus ou en attente de réponse, et son retour à
            l'accueil une pastille quand une autre en a ; un mail est lu à
            l'ouverture, un fil ou un relevé en entier dès qu'il est ouvert ;
            les articles vont dans Actualités, les posts dans Réseaux, deux
            applications du même modèle ; une vidéo, un appel, une annonce et
            une entrée d'agenda restent nommés dans Messages (pas
            d'application encore : la vidéo attend les cinématiques, l'annonce
            les catalogues) ; un mail n'a pas d'objet (le format n'en a pas),
            sa première ligne en tient lieu dans la liste ; une ligne de relevé
            est le texte du contenu, le montant dans les mots de l'auteur ;
            les icônes sont les initiales du nom de l'application, dans le
            thème par défaut.
            **Limites connues** : pas vu à l'écran ; l'exemple n'a ni article
            ni relevé (Actualités et Banque n'y apparaissent pas).
      - **L3b, le roman visuel — découpé en cinq sous-lots (2026-10-23)**,
        à la demande de l'utilisateur avant L4 : L3b.1 la lecture d'un
        dialogue (historique, avance rapide, avance automatique) ; L3b.2 la
        reprise en plein dialogue ; L3b.3 les illustrations ; L3b.4 les
        transitions ; L3b.5 la vue en graphe du fil.
        - **L3b.1 — fait (2026-10-23).** Une barre au coin de la scène :
          Historique, Avance rapide, Auto et sa vitesse.
          **Choisi sans demander** : l'historique couvre tous les dialogues
          joués, pas seulement celui en cours, et s'ouvre aussi à la molette
          vers le haut ; il suspend l'avance automatique ; l'avance rapide
          se fait au bouton ou Ctrl maintenu (l'usage des romans visuels), à
          une réplique toutes les 70 ms, et s'éteint d'elle-même sur une
          réplique nouvelle, un choix ou la fin ; une réplique est « lue »
          quand le joueur l'a dépassée dans cette conversation ou qu'une
          livraison précédente du même contenu l'a dite (même personnage,
          même texte) ; l'avance automatique attend 1 s plus 45 ms par
          caractère (×1,6 en lente, ×0,55 en rapide), ne ferme jamais un
          dialogue et reste allumée d'un dialogue à l'autre ; la vitesse
          est un réglage du joueur dans `ui_prefs.json`
          (`pitbox.story.autoSpeed`), « normale » par défaut ; un clic sur
          la barre ne prend pas le clavier à la scène.
          **Limites connues** : pas vu à l'écran ; l'aperçu de l'éditeur et
          le journal n'ont pas la barre.
        - **L3b.2 — fait (2026-10-23).** La réplique lue est gardée à côté
          de la partie (`saves/…reading`, comme `.launched`) et une partie
          rouverte en plein dialogue y reprend.
          **Choisi sans demander** : un fichier à côté de la sauvegarde
          plutôt que dans l'état du moteur (la réplique lue est l'affaire de
          l'écran, le moteur joue d'un choix à l'autre ; le simulateur n'en a
          que faire) ; écrit 0,4 s après la dernière réplique, tout de suite
          en quittant la partie ; un échec d'écriture est journalisé et
          coûte au pire un dialogue repris à sa première réplique ; la
          position ne vaut que pour le même contenu livré au même rang (une
          nouvelle livraison repart du début) ; les répliques d'avant la
          position comptent comme lues pour l'avance rapide.
          **Limites connues** : le réglage `saves` de l'histoire
          (sauvegarde par jour ou emplacements, rechargement) n'est toujours
          pas appliqué : quand il le sera, chaque emplacement devra porter
          sa position ; pas vu à l'écran.
        - **L3b.3 — fait (2026-10-23).** Type de contenu `illustration`
          (moteur, éditeur, aperçu, jeu) : une image de `media/` en plein
          cadre de scène, son texte dans une boîte en bas, fermée d'un clic.
          L'exemple en a une à l'arrivée de la course 03.
          **Choisi sans demander** : la clé `picture` (chemin dans `media/`,
          vérifié comme un portrait), obligatoire et réservée à
          l'illustration ; une illustration ne propose pas de réponse
          (erreur) et ne retient donc jamais le fil ; elle n'a pas besoin de
          cadre ; la scène joue dialogues et illustrations **dans l'ordre
          d'arrivée**, le premier non fermé d'abord — avant, seul le dernier
          dialogue jouait, et un dialogue plus ancien encore non lu ne se
          montrait jamais (l'attendu du test correspondant a changé, voulu) ;
          l'image couvre la scène (recadrée plutôt que bordée) ; tout le
          texte s'affiche d'un coup ; le journal et l'historique ne
          reprennent pas les illustrations ; l'exemple joué depuis Pit Box
          n'a pas ses médias, son illustration n'a donc que son texte (comme
          ses portraits, des initiales).
          **Limites connues** : pas vu à l'écran.
        - **L3b.4 — fait (2026-10-23).** Clé `transition` (`fade`, le
          défaut, non écrit ; `cut`) sur un dialogue ou une illustration, et
          étiquette Ink `#leave` (le personnage quitte la scène après sa
          réplique) ; éditeur (choix Fondu / Coupe, case « Sort » par
          réplique), aperçu et jeu. L'exemple a les deux.
          **Choisi sans demander** : une seule option par contenu plutôt
          qu'une par geste (entrée, sortie, changement d'expression) : la
          même pour l'apparition du dialogue, ses portraits et ses visages,
          ce qui garde un format court tant qu'aucune histoire ne demande
          plus fin ; fondu de 200 ms pour un portrait ou un visage, celui de
          la couche (250 ms) inchangé ; « coupe » = sans délai ni fondu ;
          l'entrée d'un personnage n'a pas d'étiquette (il entre en parlant,
          ou présent dès le début) ; un personnage sorti revient s'il parle
          à nouveau ; « entre les scènes » attend les scènes.
          **Limites connues** : pas vu à l'écran ; aucune transition dans
          le journal (relecture) ni entre deux contenus du téléphone ; un
          personnage revenu après `#leave` reprend la première place libre
          et un visage neutre (pas ceux qu'il avait), sauf étiquettes sur sa
          réplique ; un `#leave` sur la dernière réplique ne change rien (le
          dialogue se ferme).
        - **L3b.5 — fait (2026-10-24).** « Colonne / Graphe » dans l'en-tête
          du fil : le chapitre en carte, en lecture seule (`flowGraph.ts`,
          `FlowGraph.svelte`).
          **Choisi sans demander** : la mise en page est faite maison (environ
          150 lignes, testées) plutôt qu'une bibliothèque de
          graphes : un chapitre a quelques dizaines de nœuds, et une
          dépendance de plus pour ça ne se justifie pas ; un nœud est sur la
          rangée de son plus long chemin d'entrée ; une flèche qui saute des
          rangées passe par un point de passage qui prend sa place dans
          chaque rangée, si bien que les branches restent côte à côte ; une
          boucle est une flèche de retour en pointillés à droite ; la sortie
          vers un autre chapitre est une boîte en pointillés ; la flèche
          d'échec est jaune, les autres grises ; les conditions d'un cas
          s'affichent sur sa flèche, coupées à 28 caractères ; le choix de
          vue n'est pas retenu (on revient en colonne en rouvrant) ; le
          panneau du nœud ne s'ouvre pas en vue graphe.
          **Limites connues** : pas vu à l'écran ; un graphe très large
          défile en largeur sans réduction ; les flèches peuvent encore se
          croiser (un seul passage de réordonnancement par sens).
        - **L3b terminé.**
      - **L'histoire d'exemple, avant L3c (2026-10-24).** La spec ajoute
        « The sample shows how everything is made » : chaque lot qui donne
        quelque chose à l'auteur en met un exemple dans l'histoire d'exemple,
        contenu et médias, et Story Mode la joue avec ses images.
        - **Fait** : les répliques sans la marque `[line]` ; des voitures et
          des circuits du jeu de base (Magione, Imola, Mugello, Vallelunga
          `club_circuit` ; Abarth 500, Lotus Elise SC, BMW M3 E30, BMW 1M),
          vérifiés par un test dans `docs/kunos_content_dates.json` — les
          liveries et le layout, absents de la table, ont été vérifiés sur
          une install ; la liste des fichiers embarqués dressée par
          `story-engine/build.rs` ; les médias de l'exemple servis par le
          protocole `storymedia` (`story/media.rs`), depuis les octets
          embarqués, sans rien écrire sur disque ; `sample/media/CREDITS.md`.
        - **Choisi sans demander** : CC0 pour les fichiers faits pour Pit Box
          (un auteur qui copie l'exemple les réutilise sous la licence de son
          choix) ; `CREDITS.md` vit dans `media/`, donc une copie de l'exemple
          l'emporte avec les images ; `storymedia` ne sert pas les plages
          (`Range`) : à ajouter avec la première vidéo de scène.
        - **Reste** : les trois images de l'exemple font 1 × 1 pixel. Qui fait
          quoi, avec noms et tailles : la liste ci-dessous.
      - **Écarts entre la spec (habillages, L3c, L3d) et le code, relevés le
        2026-10-24.** Rien de contradictoire : tout ce qui manque est attribué
        à L3c ou L3d.
        - Le type `document` n'existe pas (moteur, contrôles, éditeur) ;
          `on_scene()` ne couvre que le dialogue et l'illustration.
        - Modèles présents : fil, boîte de réception, fil d'actu (`list`),
          relevé, et la scène. Absents : agenda, catalogue (L6), les
          layouts `page` et `letter`, l'écran d'appel.
        - Le cadre `phone` déclare montrer tous les types (`frame.rs`), mais
          l'appel, la vidéo, l'annonce et l'entrée d'agenda n'y ont pas d'app :
          ils sont livrés et nommés dans Messages.
        - `frames/` et `scenes/` sont ignorés à la lecture (`load.rs`, `READ`) ;
          un Beat n'a pas de `scene` ; la `scene` d'un dialogue est un nom
          libre, non vérifié.
        - Rien des habillages : ni `frames.yaml`, ni emplacements, ni 9-slice,
          ni polices ; le téléphone est dessiné en CSS au thème de l'app.
        - L'exemple n'a ni scène, ni habillage, ni second téléphone, ni journal
          en page, ni appel, ni agenda.
      - **Les médias de l'exemple : qui les fait (proposé le 2026-10-24).**
        Tailles de référence de la spec : écran du téléphone large de 1080 px,
        scène en 1920 × 1080.
        - **Faits par Claude** (dessinés par programme, CC0), avec L3d :
          les corps des deux téléphones et leurs fonds d'écran abstraits, les
          icônes d'app (192 × 192), les bulles, l'en-tête et le bouton de
          réponse en 9-slice, la manchette d'un journal inventé, les planches
          du kit de départ (PNG et SVG), et les sons d'interface simples
          (bips synthétisés). La police : une police OFL existante (Inter pour
          le téléphone, Playfair Display pour le journal), à télécharger avec
          l'accord de l'utilisateur.
        - **À trouver par l'utilisateur** (photos, visages, lieux réalistes ;
          CC0, ou une licence qui permet la redistribution ; sans marque, logo
          ni personne réelle), dans `sample/media/` :
          `portraits/friend_neutral.png` et `portraits/friend_happy.png`
          (800 × 1200, fond transparent, coupés à la taille, en bas de
          l'image) ; `illustrations/finish_line.png` (1920 × 1080) ;
          `scenes/bedroom.webp`, `scenes/garage_day.webp`,
          `scenes/garage_night.webp` (1920 × 1080) ; `scenes/garage.ogg`
          (boucle d'ambiance sans couture, 20 à 60 s) ;
          `documents/race_photo.webp` (1200 × 800, une course sans livrée
          reconnaissable).
      - **L3c, les scènes — découpage proposé (2026-10-24), en attente de
        l'accord de l'utilisateur.**
        - **L3c.1, le moteur.** `scenes/scenes.yaml` : des lieux, chacun avec
          ses variantes (saison, heure, météo, humeur), un fichier dans
          `media/scenes/`, un flou et une ambiance. Un Beat pose la scène
          courante et peut imposer une variante ; la `scene` d'un dialogue
          devient un lieu vérifié. Le moteur choisit la variante d'après le
          temps de l'histoire et l'horloge ; la scène courante va dans la
          sauvegarde (champ ajouté, valeur par défaut). Contrôles : lieu
          inconnu, fichier absent, lieu sans variante par défaut. Exemple :
          une chambre, et un garage de jour et de nuit, posés par les Beats
          des chapitres 1 et 3.
        - **L3c.2, le joueur.** Le fond de `PlayScreen` devient la scène :
          image ou boucle vidéo, flou réglable (plus net quand la scène
          occupe l'écran), fondu ou coupe au changement, l'ambiance en
          boucle avec un bouton muet gardé dans `ui_prefs.json`. `storymedia`
          gagne les plages pour la vidéo.
        - **L3c.3, l'éditeur.** L'écran Scènes : la liste des lieux, leurs
          variantes et leurs fichiers, et un aperçu côté joueur où l'on règle
          saison, heure, météo et humeur pour voir la variante choisie, le
          téléphone par-dessus. Le panneau d'un Beat et la fiche d'un
          dialogue choisissent le lieu dans une liste.
        - **Hors de L3c** : le garage qui montre la voiture du joueur attend
          le garage de l'histoire (L6).
        - **Questions** : d'où viennent la météo et l'humeur (l'histoire n'en
          a pas encore : un champ de Beat qui reste posé ?) ; « quand chaque
          scène est disponible » est-il une condition sur le lieu, ou sur la
          variante ?
      - **L4 à L8** : dans l'ordre du plan de livraison de la spec, après L3c
        et L3d.
      **Écarts assumés par rapport au document de conception** (la spec
      publique les reprend déjà) :
      - Le drapeau qu'il plaçait dans la base SQLite est dans
        `ui_prefs.json` (règle d'or n°6), et il allume toute l'entrée
        Story, pas seulement l'éditeur. La séparation Story Mode / Create
        Your Own ne se fera qu'à l'ouverture de l'éditeur à tous.
      - Une histoire déclare ses propres langues (`story.yaml`) ;
        l'interface du studio, elle, suit la langue de l'app comme tout
        écran de Pit Box : `t()`, clés `story.*` en `fr` et `en`.
      - L'app ne se rouvre pas sur l'écran Story (`KNOWN_SECTIONS`) : un
        drapeau retiré entre deux démarrages laisserait sinon un écran vide.

- [ ] **Page Online — la spec est faite, v1 et v2 (2026-10-04).** Lister,
      rejoindre, noms et vignettes, favoris, récents et amis, niveaux de
      préparation (toutes les voitures du serveur comptent), couches
      écartées le temps d'une session (une couche de son seul n'empêche
      rien), contenu manquant récupéré en une notification, recherche web
      pour ce qui n'a pas de source, avertissement No Hesi, détail du
      panneau, ping, lien collé et copié, tokens de filtre, drapeaux ;
      puis le tableau, les puces oui/non, le regroupement par circuit, la
      carte Online de la fiche circuit, les notifications et « Me
      prévenir », le mot de passe vérifié, la liste de secours. **Ce qui
      reste** : des vérifications à l'écran et des limites connues, listées
      plus bas (« Pas vu à l'écran »).
      - **Vu à l'écran** : le tableau Online, colonnes, tri et regroupement
        (2026-10-03) ; la carte Online de la fiche circuit, « Préparer et
        rejoindre » qui télécharge et importe pour de vrai, la recherche
        web, le layout d'origine de Spa retrouvé (2026-10-04) ; un serveur en
        réservation (frise, « Aussi nécessaires », « Réserver dans Content
        Manager », la couche « spa » annoncée écartée), un faux mot de passe
        refusé avant tout lancement, « Me prévenir » qui s'arrête et pose sa
        notification dans la pile quand la place se libère (2026-10-05).
      - **« Me prévenir » ne survit pas au redémarrage, et c'est voulu
        (2026-10-06)** : c'est une attente du moment, pas un réglage. Une
        version persistée dans `online.json` a été faite puis annulée (revert
        de `9fdae29`) — ne pas la refaire.
      - **Niveaux fins dans la liste : écarté (2026-10-06)**, voir « Écarté » de
        `online-join-research.md`. Ne pas interroger les serveurs un par un pour la liste.
      - **Défaut corrigé (2026-10-05), vu à l'écran le 2026-10-06** : rouvrir le
        serveur déjà ouvert (« Ouvrir » sur une notification, lien collé, clic
        sur sa ligne) ou « Actualiser » ne relisait pas le panneau — « 0
        libre » sur une place libre, erreur Steam restée affichée. La page
        incrémente désormais un compteur (`reread`) que le panneau suit ; il
        se relit en gardant voiture et mot de passe, ou repart de zéro s'il
        était en erreur.
      - **Pour retrouver un serveur précis sans toucher aux filtres** : le
        lobby se lit directement (`http://93.57.10.21/lobby.ashx/list?guid=`
        + SteamID64, agent `Assetto Corsa Launcher`), `pickup: false` pour la
        réservation, `pass: true` pour le mot de passe, puis le lien CM collé
        dans la page. Les places d'une voiture : `/JSON|<SteamID64>` du
        serveur.
      - **Pas vu à l'écran** :
        la notification unique de « Préparer et rejoindre », refaite après
        l'essai ; une couche écartée puis rendue en jeu ; le niveau « Bloqué »
        (jaune dans le panneau aussi depuis le 2026-10-04 ; impossible à
        provoquer sur la machine de dev, qui a tous les DLC et le CSP le plus
        récent du lobby) ; un mot de passe juste accepté ; la notification
        d'ami ; les notifications Windows ; le bandeau de la liste de secours
        (il faut que le lobby tombe) — tests et types seulement.
        Restent aussi « Ouvrir » sur la notification d'une place libérée et
        « Actualiser » après le retour de Steam. **Limites connues** : un
        serveur qui renvoie une liste de places vide le temps d'une requête
        affiche toutes ses voitures à « 0 / 0 » jusqu'à la relecture suivante
        (vu sur un serveur No Hesi) ; BSG et LA Canyons refusaient aussi
        l'authentification — peut-être une communauté à lanceur, à vérifier
        dans `Documents/Assetto Corsa/logs/log.txt` (une ligne `ACP_…`).
        Le clic sur une notification **Windows** n'ouvre pas le serveur : le
        plugin n'expose pas de clic sur bureau (sa doc le réserve au mobile,
        pas vérifié sur Windows) ; c'est la notification de la pile, gardée
        au retour, qui mène au panneau.
      - **Contenu d'un serveur verrouillé : écarté (2026-10-05).** 0 serveur
        sur 65 qui déclarent leur contenu protégé n'héberge un fichier
        lui-même ; la mesure et ce qu'il faudrait pour le reprendre sont dans
        « Écarté » de `online-join-research.md`.
      - **Tokens (lot 8)** : la barre est le `FilterBar` de la bibliothèque,
        alimenté par `online/tokens.ts` ; le test d'une valeur
        (`valTest`) est partagé avec la bibliothèque, pour qu'un token veuille
        dire la même chose sur les deux écrans. Le token de ping est le seul
        qui coûte du réseau : il fait mesurer **tous** les serveurs que les
        autres filtres gardent (des milliers si on le pose seul, 1,5 s par
        serveur muet, 32 à la fois). Mesuré le 2026-10-03 sur le vrai
        lobby : 5 300 serveurs en un peu plus d'une minute, la liste se
        remplissant par le haut. Si c'était trop long un jour, la borner aux
        N premiers serveurs serait le remède, au prix de serveurs écartés en
        silence.
      - **Pas vu de bout en bout dans l'app** : une couche écartée puis
        rendue en jeu (vérifié par un test sur un vrai système de fichiers,
        pas à l'écran ; la couche « spa » de la machine de dev s'y prête).
      - **Délai de garde des couches écartées (2026-10-06)** : deux minutes
        après le join, les couches reviennent si aucun jeu ne tourne. Chiffré
        sur les journaux de CM (`%LOCALAPPDATA%\AcTools Content
        Manager\Logs`, lignes `URI Request` puis `Waiting for exit: …acs.exe`) :
        1,3 à 4,1 s du démarrage de CM au jeu sur trois joins. Un compteur de
        génération fait renoncer la garde d'un join remplacé par un plus
        récent ; l'état du jeu vient du même sondage que la musique.
      - **Catégorie des circuits hors bibliothèque (2026-10-14)** : choix
        dans le panneau, catalogue et mots-clés du serveur. **Reste** à
        remplir la table `track_categories` de `taxonomy-catalog.json`, livrée
        vide : `npm run online:tracks` donne les ids qui pèsent (les 50
        premiers couvrent 75 % des joueurs), le reste est un classement à faire
        à la main. Les choix faits dans l'app se promeuvent ensuite comme les
        autres décisions (`npm run rules:diff` / `rules:promote`) — mais ils
        vivent dans `overlay.sqlite`, pas dans la surcouche du catalogue :
        les promouvoir demande de les recopier, aucun outil ne le fait.
      - **Vignette d'un circuit non installé (2026-10-15)** : Commons par
        l'appariement Wikipédia, sinon pictogramme et drapeau. Vérifié par un
        essai réseau réel (`online::picture::tests::what_lobby_tracks_get`,
        ignoré par défaut : Monza, Salzburgring, Shutoko ont une image ;
        Valencia, Akina, `nohesi_415` non) et à l'écran pour la vignette
        dessinée ; **reste à voir à l'écran** une image Commons et son crédit
        sous la bannière. Le nom d'un circuit non installé vient désormais
        de la table de CSP (`NAME=`), son dossier en repli.
      Ce que fait la page : `SPEC-play-online.md` ; ses relevés et pistes
      écartées : `online-join-research.md`.
      **Pièges déjà payés** (`online-join-research.md`), à relire avant de
      toucher au lancement :
      - `race/online/join` ne prend pas la voiture ; `race/online` si.
      - Sans `[REMOTE] __FEATURES`, CSP n'envoie pas le ticket Steam et un
        AssettoServer refuse (`ACP_AUTH_FAILED`) : c'est `raceini.rs` qui le
        réinjecte après CM, comme le skin hors ligne.
      - Le lobby Kunos exige `User-Agent: Assetto Corsa Launcher` **et** un
        SteamID réel (sinon 302, ou « UNKNOWN FAILURE » en 200).
      - `session` est le **type** de session dans le lobby, mais son
        **index** dans `sessiontypes` pour `/INFO` (même serveur, même
        minute : `1` et `0`) ; le lobby écrit ses nombres en chaînes ;
        `track` porte le build CSP minimal (`csp/3465/../E/../id-layout`).
      - Le skin est imposé par le serveur : ne pas en proposer un choix.
      - Un AssettoServer liste son **trafic IA** parmi ses voitures
        (`traffic_jp_…`), sans slot joueur. Ces voitures sont **nécessaires**
        comme les autres (toutes le sont, constaté en jeu le 2026-10-04) : la
        liste les compte dans son niveau, le panneau les montre sous « Aussi
        nécessaires » sans les proposer au choix, et Rejoindre les pose.
      - Les couches écartées reviennent sur la **fin** du jeu (transition du
        sondage) ou au démarrage suivant. Si CM ne lance finalement pas le jeu
        (erreur de son côté, fenêtre refermée), aucune fin n'arrive : elles
        restent écartées jusqu'à la prochaine partie ou au prochain démarrage
        de Pit Box. Un délai de garde (« pas de jeu après N minutes ») serait
        le remède ; pas fait, faute de savoir combien CM peut prendre à froid
        (24 s mesurées, `raceini.rs`).
      - Les niveaux « 1 clic » et « Bloqué » n'ont pas été vus à l'écran
        (aucun mod désactivé ni DLC manquant sur la machine de dev) : seuls
        les tests les couvrent.
      - Un **serveur à mot de passe** ne sert pas son contenu sans
        l'empreinte du mot de passe ; la recette est trouvée
        (`online-join-research.md`), pas encore câblée.
      - « Me prévenir » vit en mémoire : un redémarrage l'oublie, sans le
        dire.
      **Pas vérifié** : un serveur en booking, faute d'échantillon ; un
      AssettoServer (BSG, LA Canyons) refuse même avec le ticket, cause
      inconnue, et le serveur n'a pas été retrouvé pour réessayer. **Trouvé pour
      No Hesi** (2026-10-04) : `ACP_AUTH_FAILED` dans le log du jeu, et le
      Join de CM échoue pareil — ces serveurs n'acceptent que les joueurs de
      leur lanceur ; le panneau le dit. BSG et LA Canyons relèvent peut-être
      du même cas (une communauté et son propre lanceur) : à vérifier, le
      journal du jeu (`Documents/Assetto Corsa/logs/log.txt`) donne la raison.

- [ ] **Export / import de la bibliothèque (`EXPORT§`) — construit, derrière
      un interrupteur ; reste à l'essayer sur de vraies bibliothèques.**
      Élargi le 2026-10-09 : l'export emporte aussi apps, mannequins, tenues
      et autres mods. Lot A : l'**état** sans fichiers des apps et des autres
      mods (ESPACE§5.6) et le **squelette des couches** (ESPACE§5.4). Lot B :
      l'export et l'import eux-mêmes (`src-tauri/src/transfer/`, `ExportDialog`,
      `ImportDialog`, l'étape de l'assistant), visibles sous `tauri dev`
      seulement (`FEATURE_LIBRARY_TRANSFER`).
      **Reste :**
      - **L'essai réel**, par l'utilisateur : exporter sa bibliothèque,
        l'importer dans une installation vide (un second dossier de jeu et de
        bibliothèque suffit), regarder les cartes, les fiches, les notes, les
        Compléments, puis récupérer quelques mods par leur archive. Ensuite
        seulement, `FEATURE_LIBRARY_TRANSFER = true`.
      - **Pas de suite à l'import, décidé le 2026-10-10** (EXPORT§7.3) : ni
        bibliothèque filtrée pour tout récupérer, ni réactivation automatique.
        Sur une autre machine, la plupart des mods ne se retéléchargent pas ;
        ils restent en vitrine, et on récupère ceux dont on a l'archive. Ne
        pas les refaire sans en reparler.
      - **Mesuré sur la bibliothèque réelle (2026-10-10)** : 186 mods,
        3 couches, 67 apps et compléments, 49,5 Go → un fichier de 16,1 Mo
        en une vingtaine de secondes, dont 8,4 Mo pour la base ; aucun fichier
        lourd dedans. L'estimation annonce 13,7 Mo en moins d'une seconde.
        L'import, lui, n'a été essayé que dans les tests et pour son refus
        (installation non vide) : l'essai réel dans une installation vide
        reste à faire.
      - **Trois défauts trouvés sur ce fichier réel, corrigés** (2026-10-10) :
        un JPEG nommé `preview.png` (`Ph_highway`) n'était pas réduit (28,7 Mo
        au premier essai, `1ff3556`, la vitrine en souffrait aussi) ;
        `ui_prefs.json` emportait 76 chemins absolus et un preset de Content
        Manager le nom d'utilisateur Windows (EXPORT§10.2) ; l'estimation
        préparait tout l'export (6 s) et annonçait 30,7 Mo, tailles non
        compressées. **Le test qui cherchait les chemins de la machine ne
        voyait pas les deux fuites** : il cherche maintenant n'importe quel
        chemin absolu, à toute profondeur d'échappement.
      - **Trois points de relecture traités** (2026-10-10) : l'image figée est
        celle de la livrée ou du layout préféré ; l'estimation ne prépare plus
        l'export ; un mod installé à la main des deux côtés garde sa ligne
        locale (EXPORT§10.1).
      - **Le dossier de la bibliothèque doit ne rien contenir de ce que
        l'export écrit** (`check_free`) ; un dossier `cars/` vide ne gêne pas,
        un mod déjà là refuse l'import — à reprendre si le cas réel
        (bibliothèque copiée à côté) se présente.
      - **L'id d'un autre mod vient du nom de l'archive** (`<archive>__<chemin
        dedans>` pour un reste, `others::other_id`). La récupération ne
        retrouve donc la ligne que si l'archive réimportée porte le même nom ;
        une archive renommée crée une seconde ligne. Même limite pour une
        couche, retrouvée par son archive.
      - **Une couche partie en vitrine avant le 2026-10-09 n'a que son
        manifeste** : les layouts qu'elle apportait restent absents de la
        fiche de son circuit tant qu'elle n'est pas récupérée. Pas de
        rattrapage possible, les fichiers sont partis.
      - **Piège évité** : `import_other` ignorait tout id déjà connu (§7.3) ;
        réimporter l'archive d'un autre mod sans fichiers n'aurait rien
        ramené, en silence.

- [ ] **Dossier du jeu — lot 1 livré, le lot 2 (les tailles) reste.**
      Livré (`SPEC-etat-dossier.md`) : l'onglet Fichiers › Dossier du jeu,
      son scan (`src-tauri/src/gamestate/`), le partage de l'Atelier en
      Classement et Fichiers. Les tailles sont déjà dans l'index, pas encore
      à l'écran (DOSSIER§10).
      **Écarts assumés avec la spec**, à trancher s'ils gênent :
      - *Manette* : haut/bas et A marchent (la navigation géométrique de
        l'app) ; gauche/droite ne plient ni ne déplient, elles quittent
        l'arbre comme partout ailleurs — le clavier, lui, fait tout
        (DOSSIER§8.3 : « l'existant gagne »). Un clic sur un dossier le
        sélectionne **et** le déplie, pour que A suffise.
      - *Mécanisme* : « hardlink ou copie » n'est pas départagé — le nombre
        de liens NTFS n'est pas lisible en Rust stable, et la spec dit qu'il
        n'est pas nécessaire (DOSSIER§10).
      - *Mod non géré* : un dossier de premier niveau de `content/cars` ou
        `content/tracks` ni officiel ni indexé est lui aussi « non géré »
        (la spec demandait l'entité en base) : c'est un mod de
        l'utilisateur, il doit garder sa ligne.
      - *En attente* se lit sur l'arbre des ajouts d'un mod **actif** dont
        la copie n'est réclamée par aucune ligne : quand un fichier étranger
        gagne l'arbitrage, `extras::deploy` n'enregistre pas de
        réclamation — la base seule ne le verrait jamais.
        **Piège payé (2026-10)** : la date seule en faisait trop. Mesuré sur
        l'install de dev, 38 des 39 « en attente » avaient exactement le
        contenu du mod — posés avant Pit Box, ou repackés avec une autre
        date ; un seul était un vrai fichier étranger. Le contenu décide
        désormais (taille et date, puis les octets si seule la date diffère),
        sur ces seuls candidats — à l'écran comme sur la fiche du mod
        (`extras::same_content`, partagé).
      - *Compteur de génération* : `sync_track_skins` le bougeait à tort (il
        n'écrit que dans la base) ; la colonne de session l'appelant à chaque
        démarrage, l'index était périmé avant d'être regardé, et chaque retour
        sur l'écran rescannait. Retiré (2026-10).
      - *Fiche d'une livrée* : « Ouvrir la fiche » ouvre sa fiche dans
        Compléments ; celle d'un pack n'est pas proposée.
      - *Compteur de génération* : bougé par les façades qui écrivent dans le
        jeu (`GameWrite`, au début **et** à la fin), pas au plus bas niveau.
        Une façade oubliée laisserait l'index se croire à jour ; le bouton
        Rescanner et l'heure du scan restent le filet.
      **Piège payé** : `preview_start` lance toujours `tauri dev` depuis le
      dossier principal, même dans une session de worktree — il montrait donc
      le code de `main`. Pour voir une branche, lancer `npm run tauri dev`
      détaché depuis le worktree.

- [ ] **Performances du démarrage et des lectures — première passe faite
      (2026-10-08), à remesurer sur une plus grosse bibliothèque.**
      Mesuré en release, à chaud, 385 mods, **sur une machine haut de gamme**
      (Ryzen 7800X3D, NVMe) — donc un plancher, pas un cas typique : les cartes
      de la bibliothèque arrivaient à 1,15 s du chargement de la page, elles
      arrivent à 0,66 s ; `list_library` seul est passé de 0,37 à 0,19 s. En
      dev, compter environ 1 s de plus (interface servie fichier par fichier,
      code de l'app non optimisé). **À froid (premier démarrage, rien en
      cache), tout est environ six fois plus lent** : 2,4 s pour la liste en
      dev, alors qu'une relance prend 0,49 s.
      Fait : la liste et les logos de marque lisent la base d'abord et leurs
      fichiers ensuite, verrou rendu ; vingt et une lectures qui vont lire des
      fichiers passent hors du fil de la fenêtre (`off_window`,
      `read_off_window`) ; la liste des « autres mods » et l'inventaire rendent
      le verrou avant de parcourir les fichiers ; l'app se rouvre sur le
      dernier écran. La règle qui en sort est dans `CLAUDE.md` (« Une commande
      dont la durée dépend du disque… »).
      **Mesurer : `npm run bench:startup`** (`scripts/bench-startup.mjs`) —
      compile en release, lance l'app cinq fois sur la vraie bibliothèque, et
      compare à la mesure précédente gardée dans `.bench/` (non versionné,
      propre à chaque machine). L'app écrit elle-même ses temps quand
      `PITBOX_TIMING` nomme un fichier (`timing.rs`, `$lib/timing`) ; éteint,
      rien n'est écrit ni envoyé. Le banc refuse de tourner si Pit Box est
      ouvert, et remet les sauvegardes de démarrage exactement comme il les a
      trouvées (chaque lancement en fait une, et seules les 7 dernières sont
      gardées). `--no-build` relance sur la dernière compilation, `--runs 1`
      juste après un redémarrage de Windows donne la mesure à froid.
      Premier banc (2026-10-08, 385 cartes) : cartes à l'écran à 1,06 s du
      lancement du processus (0,66 s du chargement de la page), logos à
      1,18 s. **Le premier lancement après une compilation a pris 3,4 s** —
      sans doute le nouveau `.exe` inspecté par l'antivirus et les caches à
      refaire : c'est ce que vit un utilisateur juste après une mise à jour.
      **Reste** :
      - Remesurer sur l'autre machine, à la bibliothèque plus grosse, à
        chaud **et** à froid (juste après un redémarrage de Windows).
      - Les ressources d'un mod et celles d'un « autre mod » (`list_mod_resources`,
        `list_other_resources`) parcourent un dossier entier, encore sur le
        fil de la fenêtre. Les onze autres lectures synchrones restantes ne
        lisent qu'un fichier ou un petit dossier : laissées telles quelles.
      - **Le saut « initiales → logo »** de l'index des marques au démarrage
        (« FE » puis le logo Ferrari) : les logos arrivent après l'analyse de
        tous les badges (256 ms pour 356 badges, en mémoire seulement, refaite
        à chaque démarrage). Proposé, pas tranché : une case vide tant que les
        logos ne sont pas connus ; en plus, si elle se remarque, l'analyse
        gardée sur disque (un pur cache, indexé par chemin, date et taille).
      - Comprendre les 3,4 s du premier lancement après une compilation (ou
        une mise à jour), et voir s'il y a quelque chose à y faire.
      **Écarté** : un cache de la liste des cartes (décision du 2026-10-08 :
      rien à gagner au démarrage, et une liste qui ne relit plus le disque
      raterait un skin posé par CM) ; alléger la réponse de `list_library`
      (841 Ko, dont 40 % de descriptions dont le filtre a besoin — au mieux
      50 ms à gagner).
      **Pièges payés** :
      - Mesurer **de bout en bout** (cartes reçues par l'interface), jamais
        la seule durée d'une commande : dès qu'une commande ne bloque plus
        les autres, elles se partagent le verrou et sa durée propre gonfle
        (0,37 → 0,71 s) alors que l'écran, lui, va plus vite.
      - Un `.exe` en release n'a pas de console : le chronométrage s'écrit
        dans un fichier (`PITBOX_TIMING`), jamais sur la sortie d'erreur.
      - Le cache disque de Windows ne se vide pas sans droits administrateur :
        le seul vrai « à froid » est un démarrage juste après un redémarrage.
      - Le premier passage de l'analyse des badges se mesure avec le test
        ignoré `logos::tests::real_install_logos`
        (`cargo test --release --lib logos::tests::real_install_logos -- --ignored --nocapture`).

- [ ] **Sessions au format `.cmpreset` — une seule vérification reste.**
      Livré (SESSION§3.6) : une session enregistrée est un preset Quick Drive
      écrit chez Content Manager, portant en plus une clé `PitBox` avec
      l'instantané complet — le skin du joueur, l'intention météo, les skins de
      circuit et les jetons du vivier, dont **aucun n'a de champ dans le schéma
      de CM**. Les presets de CM sont listés avec un badge, convertis au mieux,
      et jamais supprimés depuis Pit Box.

      **Ce qui reste, et c'est mesurable en une minute** : CM ignore-t-il
      vraiment notre clé ? C'est la valeur par défaut de Newtonsoft (membres
      inconnus ignorés), donc c'est attendu — mais ce n'est pas mesuré, et la
      règle de ce projet est de ne pas déduire un format qu'on peut observer.
      Ouvrir Quick Drive dans CM, charger un preset du dossier `Pit Box`,
      vérifier qu'il se charge et se lance. S'il refusait, le repli est écrit
      d'avance : le bloc part dans un fichier jumeau `<nom>.pitbox.json` à côté
      du preset, et rien d'autre ne bouge.

      **Ce qui ne revient pas d'un preset de CM**, et c'est le format qui le
      dit, pas un manque de travail : le skin du joueur (aucun champ — mesuré,
      deux presets sauvegardés avec deux skins différents sont identiques
      octet pour octet) et la température de piste (`crt` dit qu'une valeur a
      été posée, jamais laquelle). Les deux sont annoncés dans le bandeau
      jaune plutôt que devinés.

- [ ] **Texture updates — la couche développée sur toutes les livrées.**
      Recherche faite, **rien d'implémenté** : tout est dans
      `docs/SPEC-texture-update.md`, y compris les mesures, qui sont la partie
      qu'on ne retrouve pas deux fois. En bref : un dossier de textures à
      recopier dans *chaque* livrée d'une voiture n'est ni une livrée ni une
      couche telle qu'on les pose, et finit aujourd'hui en « autre mod »
      `UNRECOGNISED` — ce qui est exact, faute de mécanisme. La détection est
      possible et **nette** (100 % des fichiers du mod sont des textures de son
      `.kn5`, contre 58 % au maximum pour 75 livrées réelles ; 0 livrée sur
      3585 dépourvue des quatre marqueurs de livrée). Restent deux décisions de
      conception, TEXTURE§5 et TEXTURE§6 de la spec. Ne pas commencer sans les avoir lues.
- [ ] **Harmonisation des libellés**. 68 règles de libellé
      produisent 53 signatures visuelles distinctes : 15 tailles de police,
      7 interlettrages, 9 couleurs. La même fonction visuelle change donc
      d'apparence selon l'écran. Cible : quatre niveaux globaux —
      `.lbl-screen` (titre d'écran), `.lbl-sub` (le sous-titre qui l'explique),
      `.lbl` (rubrique), `.lbl-key` (clé de donnée) — et des
      couleurs redevenues sémantiques (rouge = catégorie/session/destructif,
      bleu = info et fichier mod, vert = règle, jaune = alerte). Fait : fiche
      détail ; titres d'écran passés à `.lbl-screen` sur les quatre zones
      restantes (bibliothèque/lancement/réglages/add-ons) — au passage,
      `.lbl-screen` lui-même corrigé à 18px/600 dans `global.css` (4 écrans
      sur 5 avaient déjà convergé là spontanément, sans classe partagée ;
      Réglages à 15px était l'écart, pas la référence) ; quelques `.lbl`/
      `.lbl-key` ponctuels (`BulkEditPanel`, `OpponentsBlock`, `WeatherBlock`,
      `Transversal`). **Explicitement laissé de côté** (décidé avec
      l'utilisateur, à traiter séparément si besoin) : les libellés de champ
      de formulaire (Réglages, Chemins, filtres bibliothèque — rôle différent
      d'une clé de fiche technique en lecture seule, même si visuellement
      proche) et les titres de popup (`OpponentPicker`/`NamedListDialog`,
      13px/majuscules, identiques entre eux mais ne correspondant à aucun des
      quatre niveaux). **Couleurs sémantiques** : le **rouge** a désormais son
      barème, écrit au §7.2ter du SPEC — quatre niveaux, un quota par niveau,
      et la règle « le survol n'introduit jamais de rouge sur un élément qui
      n'y a pas droit au repos ». Appliqué au rail de navigation, à la colonne
      de session, et **aux filtres et à la grille de bibliothèque**
      (2026-09-16). Les neuf emplois hors barème qu'ils portaient se rangeaient
      en trois familles : un **survol qui rougissait un contrôle neutre**
      (bouton « effacer », cœur de favori vide, poignée de largeur de colonne),
      un **focus en rouge** là où il est jaune partout ailleurs — les deux
      champs de recherche des filtres tuaient même l'anneau jaune global par un
      `outline: 0` pour le remplacer par un filet rouge —, et du **rouge sur de
      la structure** (flèche de tri, repère de dépôt d'une colonne). Plus un
      survol de puce qui passait du niveau 3 au niveau 2. Le test de conformité
      du §7.2ter passe maintenant sur les deux zones.
      **Quatre emplois sont des écarts assumés** (2026-09-16, décidés avec
      l'utilisateur après lecture à l'écran) : `.card.sel` en rouge plein quand
      `tbody tr.sel` est en rouge éteint, le cœur de favori, l'épingle d'un
      filtre, et « + Filtre » ouvert. **La réserve du barème ne mord pas ici** :
      il veut que « où je suis » soit jaune, et il l'est — c'est l'anneau de
      `:focus-visible`/`.gp-focus`, qui se distingue nettement du rouge à
      l'usage. Le rouge de `.card.sel` est un second repère, plus grossier, de
      la carte courante ; la carte en session s'en sépare par son filet doublé.
      Le cœur et l'épingle relèvent de l'iconographie (un cœur est rouge) plus
      que du barème. Rien à changer.
      **Reste, et c'est tout ce qui reste : les trois autres couleurs.** Le
      rouge a son barème et il est appliqué partout ; le bleu (info et fichier
      mod), le vert (règle) et le jaune (alerte) n'ont pas le leur — le `--orange`
      ajouté pour « mod inactif » (`StateBadge`) est le premier pas dans cette
      direction (ni le jaune d'alerte, ni le rouge destructif).
- [ ] **Vignettes régénérées de la grille** — **fusionné dans `main`, mais
      éteint** : `FEATURE_GRID_THUMBS` est à `false` dans `src/lib/features.ts`,
      qui porte le mode d'emploi de l'interrupteur. Il ne reste qu'un réglage,
      long et fastidieux, et trois presets livrés avec des valeurs provisoires
      seraient jugés sur elles. Spec dans `docs/SPEC-grille.md`, dont le GRILLE§11 dit ce
      que l'implémentation a changé et pourquoi. La partie A (GRILLE§2 à GRILLE§4) était
      déjà livrée ; la **partie B** (GRILLE§5 à GRILLE§8) l'est — pipeline, tâche de fond,
      écran de réglage, profils à l'installation — et le modèle de **presets**
      est venu après, d'une remarque de l'utilisateur : la preview d'origine
      d'Assetto Corsa est plus *jolie* que notre rendu, le nôtre plus
      *lisible*. Deux objectifs, pas deux qualités d'exécution du même — d'où
      *Catalogue*, *Vitrine* et *Officiel*, embarqués et en lecture seule,
      qu'on duplique pour s'en faire un, et un preset **par densité de
      grille**.
      **Les pièges à ne pas réintroduire**, tous mesurés ou vécus :
      1. **Ne jamais faire passer la génération par `prepare_car_preview`.**
         C'est le GRILLE§5.3, et il a une raison chiffrée : 312 conversions dans un
         cache déjà à son plafond évincent une entrée vivante chacune, donc les
         aperçus que l'utilisateur consulte vraiment. La voie parallèle est
         `preview::prepare_scratch` → brouillon vidé avant chaque conversion,
         jeté après le rendu.
      2. **Le brouillon a un verrou côté frontend** (`withScratch`), en plus du
         verrou de conversion côté Rust. Celui-ci ne protège que la conversion ;
         la fenêtre pendant laquelle three.js va chercher géométrie et textures
         vient après, et la conversion suivante vide le dossier en commençant.
         Symptôme d'un oubli : une voiture blanche, sans texture.
      3. **Un rendu abîmé ne se voit plus une fois le PNG écrit.** Contexte
         WebGL perdu ou textures non arrivées : le fichier existe et son nom est
         valide, donc il est resservi pour toujours. D'où `checkPlausible`
         (réduction à 64×36, refus du vide et du saturé) **avant** l'écriture.
         Le bouton « refaire la vignette » de la fiche a existé puis a été
         retiré, à la demande de l'utilisateur : on parie sur le contrôle, et
         `regenerateGridThumb` reste, un `{#if}` de le ramener si le cas revient.
      4. **Le mat n'entre pas dans l'empreinte d'une vignette** et ne doit
         jamais y entrer : il ne change aucun pixel du PNG. L'y mettre ferait
         régénérer trois cents images pour un changement de couleur de carte.
      5. **Le balayage garde les gabarits de *tous* les presets vivants**, pas
         seulement le dernier appliqué. Sinon chaque changement de densité
         relance cinq minutes de travail, et la coexistence des deux jeux — tout
         l'intérêt du preset par densité — tombe.
      6. **Le mat n'entre dans l'empreinte que quand un preset le cuit**
         (`background > 0`). Toujours le hacher ferait régénérer 312 images pour
         un changement de thème ; ne jamais le hacher laisserait un fond cuit
         qui ne correspond plus à sa carte. Le `renderTemplate` de
         `gridThumbPrefs` est le seul endroit où le gabarit et le mat se
         rejoignent : **tout ce qui rend ou cherche une image passe par lui**,
         jamais par `preset.template` directement.
      7. **`RENDERER_VERSION` s'incrémente dès qu'une correction change les
         pixels produits** (`gridThumbPrefs.svelte.ts`) — même règle et même
         raison que `preview::CONVERTER_VERSION`. Une vignette porte le `.kn5`,
         la livrée, les configs CSP, la version du convertisseur et le
         gabarit… mais rien du code qui dessine : sans cette version, une image
         fausse reste servie pour toujours, parfaitement valide au regard de
         tout ce que son nom sait vérifier. Vécu au premier bug de rendu.
      8. **La génération est un travail de fond, et ça se paie en cœurs.**
         `kn5_gltf::with_background_pool` la borne à la moitié des unités de
         calcul ; sans ça la machine est prise en entier et l'interface saccade
         — d'autant plus visible sur une grosse machine. La file rend aussi la
         main 16 ms entre deux voitures (rendu et écriture du PNG sont sur le
         fil principal), et elle **se suspend au lancement d'une session**,
         jusqu'à la fermeture du jeu. Les deux bouts ne suivent pas le même
         signal : pause **dès le clic** sur « Démarrer », reprise sur la
         disparition du process. C'est la différence avec la musique de Big
         Picture, qui continue pendant tout l'écran de chargement et ne se coupe
         qu'une fois la voiture pilotable — les conversions, elles,
         rallongeraient précisément ce chargement. Le process est déjà surveillé
         par `music/watch.rs` (sondage de `acs.exe`), qui rend maintenant compte
         à une **fermeture** en plus du canal du moteur audio : un seul sondage
         pour deux clients. Les pauses sont un **ensemble de raisons** et non un
         booléen — fermer l'atelier ne doit pas relancer la génération pendant
         que le jeu tourne.
      9. **Un fond cuit rend le contrôle du vide inopérant** : l'image est
         opaque partout, donc « rien n'a été dessiné » ressemble à « tout va
         bien ». D'où le troisième critère de `checkPlausible`, l'écart de
         luminance — un fond seul est un dégradé très doux, une voiture y ajoute
         forcément des clairs et des sombres.
      **Écarts assumés vis-à-vis de la spec** : l'ombre de contact est une vraie
      ombre projetée et non l'ellipse peinte du GRILLE§5.6 ; l'azimut n'a pas été
      « relevé sur les previews Kunos » puisque c'est déjà fait (318°, l'aperçu
      3D) ; la tâche de fond réduite est une barre et non une pastille à anneau
      (la pile bas-droite n'a qu'une forme) ; les six voitures de l'atelier sont
      prises par **catégorie** (plus la voiture de session en tête) et non par
      silhouette, que rien dans les données ne dit ; et le GRILLE§5.7 (versionnage du
      gabarit d'origine) est **supprimé**, rendu inutile par les presets
      embarqués.
      **Trois embarqués**, et ils ne poursuivent pas le même but : *Catalogue*
      (identifier vite, image détourée), *Vitrine* (avoir envie de regarder,
      flaque + reflet cuits dans l'image), *Officiel* (être indiscernable d'une
      `preview.png` du jeu, fond cuit, contenu de base laissé tel quel).
      **Reste — et c'est la seule chose qui reste : finaliser les valeurs par
      défaut des trois presets.** Celles livrées sont une passe de réglage, pas
      un point d'arrivée : l'utilisateur les a posées à l'écran et les reprendra
      plus tard, le travail étant fastidieux. Elles se figent dans l'atelier
      (`Réglages › Vignettes`), comme les défauts de l'aperçu 3D l'ont été, et
      il n'y a **rien à coder pour ça** — seulement à recopier les nombres
      arrêtés dans `BUILTIN_PRESETS`.
      **Pour reprendre :** passer `FEATURE_GRID_THUMBS` à `true`, et vérifier
      les trois points d'entrée qu'il rallume — l'onglet `Réglages › Vignettes`,
      la deuxième page de l'assistant (les trois profils), la tâche de fond.
      Rien n'a été effacé en s'éteignant : les images déjà produites sont dans
      `app_cache_dir/gridthumbs/` et les presets de l'utilisateur dans
      `ui_prefs.json`. Les images, en revanche, se refont dès que les valeurs
      d'un preset embarqué changent — c'est le fonctionnement normal de
      l'empreinte, pas un accident.
      Trois choses à savoir avant d'y toucher :
      - les trois partagent le cadrage arrêté à l'écran (angle 320°, focale
        26°, hauteur 8 %) ; les séparer est possible mais c'était un choix ;
      - la principale de Vitrine à 30 % **n'est pas une coquille** : complément
        et contre-jour s'expriment en pourcentage d'elle, donc la descendre
        éteint tout l'éclairage direct et laisse le showroom seul. La remonter
        remonte aussi les deux autres ;
      - le cadrage d'Officiel a une **mesure** derrière lui (GRILLE§11.4) ;
        s'en écarter est permis, l'ignorer serait dommage.
- [ ] **Aperçu 3D natif des voitures** (fusionné dans `main` ; la branche
      d'origine `feature/3dpreview` ne subsiste que sur `origin`).
      **L'avancement détaillé, les écarts assumés vis-à-vis de la spec et le
      reste à faire sont dans `docs/preview3d-research.md`, et les règles dans `docs/SPEC-preview-3d-kn5.md`** — c'est
      là qu'il faut lire en reprenant, pas ici.
      En bref : **lots 0 à 6 faits et validés à l'écran** — la voiture
      s'affiche dans la fiche, tourne sur son socle, se manipule à la souris,
      porte la couleur de son skin, se règle depuis la fiche et projette son
      ombre sur le sol d'un studio.
      **Ce qu'il faut retenir des neuf écarts de format documentés** (tous dans
      `docs/kn5-format.md`, avec leur méthode de mesure) : **AC renseigne ses
      champs et ses slots standard avec des valeurs que ses shaders n'utilisent
      pas comme on le croirait.** Trois formes rencontrées, dans cet ordre de
      difficulté :
      *un état* (carte de dégâts, saleté de pare-brise) qu'il ne mélange qu'à
      proportion de quelque chose ; *un masque* (la peinture d'un skin, sous
      l'alpha de la diffuse) ; et *un objet entier* (`ksBrokenGlass`, la vitre
      brisée, toujours présente dans le modèle). Un même matériau peut mentir
      sur plusieurs de ses champs à la fois — `ksWindscreen` s'est trompé trois
      fois de suite (texture, opacité, exposant spéculaire). Donc : devant un
      défaut visuel, **ne pas s'arrêter au premier champ coupable**, et
      regarder aussi ce qui est dessiné par-dessus.
      Reste surtout le choix du LOD — PREVIEW§15.
      Le réglage de qualité se réduit au suréchantillonnage : une passe SMAA
      a été essayée, déplacée, puis retirée faute de gain visible pour un
      gigaoctet de mémoire. Il subsiste du crénelage sur les lignes claires
      quasi horizontales ; le prochain essai est **en amont** (normales,
      rugosité), pas un filtre de plus. PREVIEW§15 point 8.
      (R et B de `txMaps` : question close, par la négative ; la métallicité
      vient de `fresnelC` — écarts n°7 et n°10 de `kn5-format.md`.)
      Trois règles à ne pas perdre de vue :
      **`preview::CONVERTER_VERSION` s'incrémente dès qu'on touche au rendu
      produit** (sinon les anciens `.glb` restent servis — la version est dans
      le *nom* des entrées de cache, ce qui permet aussi d'effacer les
      périmées) ; **une conversion ne se valide jamais sur une seule voiture**
      — l'atlas de la MX-5 est symétrique et a masqué une erreur de repère
      pendant deux lots ; et **le `preview.jpg` d'un skin est une référence de
      cadrage, pas de luminosité** — il est plus sombre que le rendu du jeu, ce
      qui m'a fait diagnostiquer un écart inexistant.
- [ ] **Enrichissement Wikipédia de la fiche** (fusionné dans
      `main`). Un extrait de l'article du véhicule ou
      du circuit **réel**, dans un onglet à côté de la description de l'auteur.
      Spec : `docs/SPEC-wikipedia-fiche-detail.md`, et son WIKI§1 commande tout —
      la fonctionnalité est **décorative**, donc l'ambiguïté n'affiche rien et
      l'absence n'est jamais une erreur. Son WIKI§2 est juridique et non
      négociable : le texte reste une **collection** (jamais fusionné à la
      description, jamais reformulé, résumé ni traduit — surtout pas par un
      modèle de langage), sinon le ShareAlike de CC BY-SA remonte sur l'app.
      **Fait : les lots 1 à 3, sans interface** — `src-tauri/src/wiki/` : les
      trois tables dans l'overlay (WIKI§3), la chaîne de repli et la remontée d'un
      cran (WIKI§5), le client Action API (WIKI§6), et l'appariement automatique des
      voitures et des circuits (WIKI§4) avec sa commande de calibration.
      **Le `allow(dead_code)` du module est retiré** (2026-09-17). Il datait du
      temps où rien n'appelait le module ; une fois un appelant venu, il
      masquait 13 warnings au lieu de les justifier. Les sept qui n'étaient pas
      le banc de calibration ont été tranchés un par un, et **trois étaient des
      implémentations supplantées** que le code documentait déjà ailleurs :
      `WikiClient::search` et `parse_search` interrogeaient
      `wbsearchentities`, abandonné après mesure — « BMW M3 E30 » n'y rend
      *rien*, l'item s'appelant `BMW M3` — au profit de `search_pages`, dont le
      doc-comment porte la démonstration ; `COORDINATE_LOCATION` nommait une
      propriété que `list=geosearch` résout côté serveur et que la requête
      n'envoie jamais ; `EntityTitles::langs` était doublé par
      `available_langs`, construit dans `api.rs` et seul à alimenter le
      sélecteur. Supprimés. Les trois survivants disent leur raison sur place :
      `ROUTE_TYPES` **porte une décision** — les routes ne s'apparient pas
      automatiquement, et `matchtrack` dit les y laisser pour la correction
      manuelle —, `Candidate::name` sert le banc, `article_lang` sert son
      propre test. Le banc garde une allowance **à son échelle**, dans
      `calibrate.rs`, avec la raison : il est injoignable par conception.
      **Les identifiants Wikidata sont dans `wiki/ids.rs`**, un par un relevés
      sur l'API vivante (WIKI§4.4 l'exige) — le libellé en commentaire est celui
      que l'API a rendu, et chaque entrée dit sur quel item réel elle a été
      confirmée. Ne pas en ajouter de mémoire.
      **Les seuils sont dans `Prefs`** (`wiki_match_*`, `wiki_track_*`) et la
      liste de nettoyage des noms dans `rules/wiki-matching.json`, semée dans
      le dossier de config et éditable — WIKI§4.3 l'exige, et c'est ce qui permet
      de régler la reconnaissance sans release.
      **Pour calibrer** (rien n'est persisté, le rapport sort en Markdown) :
      ```
      PITBOX_WIKI_LIMIT=20 cargo test --lib wiki -- --ignored --nocapture calibrate_the_library
      ```
      Quatre mesures ont corrigé la spec, et elles ne se retrouvent pas deux
      fois :
      - **La recherche géographique des circuits tourne sur Wikidata, pas sur
        Wikipédia.** L'article anglais « Nürburgring » n'a *aucune* coordonnée
        GeoData (Suzuka non plus) : le `list=geosearch` de la WIKI§4.2 ne peut
        structurellement pas rendre le circuit qui lui sert d'exemple. L'item
        Wikidata porte bien P625, et y chercher rend des Q-ids directement.
      - **Le filtre de type porte toute la stratégie circuit.** À 5 m du
        Nordschleife, les vingt items les plus proches sont dix-neuf éditions
        de Grand Prix, un village, un château et un ruisseau — le circuit n'y
        est pas, une vingtaine d'items partageant la coordonnée exacte. D'où
        `gslimit=50` et l'allowlist de `ids::TRACK_TYPES`.
      - **Les coordonnées viennent de CSP, pas des `geotags`.**
        `sun::track_location` les résout déjà pour 445 circuits ; les `geotags`
        des circuits Kunos sont le littéral `["lat", "lon"]`.
      - **L'année n'existe presque jamais** : aucune voiture mesurée ne porte
        P571, seules les générations portent P580/P582. Elle est donc un bonus
        quand elle existe et jamais une pénalité quand elle manque — son poids
        quitte le dénominateur.
      Trois choses à savoir avant d'y toucher :
      - **Le client HTTP est WinHTTP**, via le crate `windows` déjà présent
        (`wiki/http.rs`) : le projet n'avait aucun client HTTP, et deux GET
        JSON ne justifiaient pas une trentaine de crates plus une pile TLS.
        L'OS fournit TLS, proxy, redirections et délais. Les deux solutions
        écartées (`reqwest` + `native-tls`, `ureq`) sont notées dans le fichier
        avec ce qui les ferait gagner — le jour où l'app cesse d'être
        Windows-only, c'est `reqwest`. Corollaire : tout est **bloquant**, donc
        les futures façades passent par `spawn_blocking`.
      - **Les deux formats de réponse ont été relevés sur l'API réelle**, pas
        déduits : `query.pages` est un *tableau* en `formatversion=2`, le
        parent se lit en `claims.P361[0].mainsnak.datavalue.value.id`, et les
        sitelinks mélangent `commonswiki` aux langues. Un test ignoré
        (`talks_to_wikipedia_for_real`) rejoue le tout contre le vrai service —
        c'est la seule preuve que le FFI WinHTTP fonctionne, la CI ne
        l'exécutant pas (WIKI§11 : aucun test ne dépend de Wikipédia).
      - **Un 404 et un réseau coupé ne sont pas le même non-résultat**
        (`api::Fetched`). Les confondre écrirait « pas d'article » dans le
        cache négatif pour 90 jours à cause d'un tunnel.
      **La calibration a tourné** (335 mods) et les seuils livrés sont les
      siens, plus ceux du WIKI§13. Elle a corrigé quatre choses que le raisonnement
      n'aurait pas trouvées, toutes consignées dans le code :
      - `wbsearchentities` **cherche par préfixe de libellé** : « BMW M3 E30 »
        n'y rend *rien*, aucun item ne s'appelant ainsi. C'était la cause
        dominante des 264 échecs du premier passage. La recherche passe
        désormais par le moteur plein texte de Wikipédia, qui rend « BMW M3 »
        en tête — et « Abarth 500 » pour une variante sans article à elle, ce
        que la WIKI§4.1 veut explicitement.
      - **`gsradius` est plafonné à 10 km par l'API**, qui refuse la requête
        entière au-delà. Un rayon de 25 km a transformé *les 24 circuits* en
        « réseau indisponible » d'un coup — c'est à ça que ressemble une panne
        systématique à côté d'une vraie coupure.
      - **Les routes ne s'apparient plus automatiquement** (écart assumé avec
        la WIKI§4.2) : une rue est à portée de n'importe quelle coordonnée, et le
        nom ne peut pas arbitrer puisque la spec a choisi les coordonnées
        *parce que* « Shutoko » ne ressemble pas à « Metropolitan Expressway ».
        Quatre articles faux pour une poignée de justes. Shutoko et les touge
        relèvent désormais de la correction manuelle (WIKI§7.6).
      - **Un item sans libellé anglais revenait sans nom** et marquait 0 contre
        tout — d'où `borrow_labels`, qui reprend le titre trouvé par la
        recherche.
      Résultat : 15 circuits retenus, tous justes (Monza retrouvé par le repli
      sur le nom, ses coordonnées CSP étant celles de Milan), contre 0 avant.
      **Les six lots du WIKI§12 sont faits.** L'onglet vit dans la fiche
      (voitures et circuits), la correction manuelle y est, et
      `Réglages › Wikipédia` porte l'interrupteur, la langue, la purge du cache
      et l'export des corrections.
      **Quatre écarts assumés avec la spec, tous décidés avec l'utilisateur
      après l'avoir vu à l'écran** — ils sont écrits dans le SPEC de la
      fonctionnalité, pas seulement ici :
      - **L'onglet est permanent** (contre la WIKI§7.1). Un onglet absent ne se
        distingue ni d'une recherche en cours, ni d'une fonctionnalité qui
        n'existe pas — constaté en vrai, sur un circuit qui s'appariait
        pendant qu'on regardait la fiche. Il porte donc six états, dont aucun
        n'est une erreur, et la correction manuelle avec eux : la WIKI§7.6
        l'accrochait à un onglet qui n'existait pas dans le seul cas où elle
        sert.
      - **L'article entier et rendu** (contre la WIKI§7.3, qui n'en voulait que
        l'introduction en texte brut) : sections, sommaire, tableaux, infobox.
        Le HTML n'est jamais injecté tel quel — `wikiHtml.ts` **reconstruit**
        un arbre depuis une liste blanche, la webview ayant accès à `invoke`.
        Aucune dépendance ajoutée pour ça.
      - **Les images sont affichées** (contre la WIKI§9). Ses trois objections
        étaient exactes et sont traitées, pas contournées : **Commons
        uniquement** (`imagerepository == "shared"`), ce qui écarte
        structurellement l'usage loyal puisque Commons n'accepte que du libre ;
        auteur et licence sous chaque image, non masquables.
      - **Le mot « extrait » quitte l'attribution** (WIKI§7.4) : il était exigé
        parce que ne montrer qu'un fragment est une modification. Montrer le
        texte entier est le régime **plus simple**, pas plus risqué.
      **Reste une seule chose : régler les seuils sur les corrections
      manuelles.** Tout le code est livré ; ce qui manque est une **mesure**,
      et elle demande que l'utilisateur ait corrigé un paquet d'articles.
      **Pourquoi ça attend, et pourquoi ça vaut le coup d'attendre.** Le
      rapport de calibration dit aujourd'hui « score 0,689, marge 0,122 », il
      ne dit jamais *juste ou faux* : sans vérité terrain, un seuil s'arbitre
      au jugé — c'est ainsi que le plancher a été posé à 0,70, en relisant onze
      lignes à la main. Chaque correction faite dans l'onglet est au contraire
      un **exemple étiqueté** (`wiki_link` en `source = 'manual'`). À partir
      d'une cinquantaine, la calibration peut comparer son propre verdict aux
      réponses de l'utilisateur, sortir un vrai taux de justesse, et surtout
      essayer des dizaines de combinaisons de seuils **hors ligne** contre ces
      étiquettes.
      **Pour reprendre à froid :**
      1. Vérifier la matière : `SELECT COUNT(*) FROM wiki_link WHERE
         source='manual'` dans `%APPDATA%\com.pitbox.app\overlay.sqlite`. Sous
         une cinquantaine, il n'y a pas encore de quoi mesurer — demander à
         l'utilisateur de corriger au fil de sa navigation.
      2. Faire tourner la calibration, qui ne persiste rien :
         ```
         cd src-tauri; cargo test --lib wiki::calibrate::tests::calibrate_the_library -- --ignored --nocapture
         ```
         (`PITBOX_WIKI_LIMIT` raccourcit un premier passage, `PITBOX_WIKI_REPORT`
         choisit où le Markdown atterrit ; le rapport par défaut va dans le
         dossier de config.)
      3. Ajouter au rapport la comparaison aux étiquettes : pour chaque mod
         corrigé à la main, le verdict du moteur est **juste**, **faux** ou
         **absent**. C'est ce qui transforme le rapport d'un décompte en mesure.
      4. Balayer les couples (`wiki_match_min_score`, `wiki_match_min_margin`)
         sur ces étiquettes et retenir celui qui maximise les justes sans
         laisser passer de faux — la WIKI§1 échange volontiers du rappel contre de
         la précision.
      **Deux pièges à ne pas réintroduire :**
      - **La calibration doit comparer le verdict brut du moteur aux
        étiquettes, sans jamais lire `wiki_link`.** Sinon elle se note sur ses
        propres copies : les appariements qu'elle a elle-même écrits, et les
        entrées livrées, lui renverraient ses réponses comme si c'était la
        vérité.
      - **Un mod de `no_counterpart` n'est pas un manque.** La séparation
        existe déjà dans le rapport (`rules/wiki-links.json`) et c'est elle qui
        empêche de régler les seuils contre du bruit — 129 « sans candidat »
        dont une bonne part sont des succès ne veut rien dire.
      Diagnostic d'un mod isolé, quand un article n'apparaît pas :
      ```
      $env:PITBOX_WIKI_MOD = "ks_ferrari_sf15t"; cargo test --lib wiki::calibrate::tests::what_the_fiche_gets -- --ignored --nocapture
      ```
      Il dit où la résolution s'arrête — appariement, langue, ou réseau —, trois
      causes que rien ne distingue à l'écran.
      **Les images s'ouvrent en grand**, et c'est `Lightbox` qui le fait —
      la visionneuse partagée des captures et des backgrounds, à qui on a
      ajouté deux choses : le **crédit avec son lien Commons** (WIKI§9 : sans lui
      l'image ne peut pas être affichée du tout) et une **source de repli**,
      parce que MediaWiki refuse d'agrandir au-delà de l'original et que rien
      dans l'URL ne dit où il est — on demande 1600 px, on retombe sur la
      vignette au premier échec.
      **L'erreur à ne pas refaire, elle, vaut d'être écrite** : une deuxième
      visionneuse a d'abord été écrite sans chercher si l'app en avait une.
      Elle fonctionnait à la souris et au clavier, et elle était **cassée à la
      manette** — `nav.inputCapture` est le drapeau par lequel une visionneuse
      dit à `gamepadNav` de se taire, si bien que sans lui le B fermait la
      fiche derrière l'image et gauche/droite changeaient de mod. Le
      commentaire de `gamepadNav.ts` décrivait le piège mot pour mot. Une
      brique recopiée ne coûte pas seulement du style dupliqué : elle **perd
      les comportements que l'originale avait appris**.
      **Idée notée, pas un chantier** (« pas très important », dixit
      l'utilisateur) : **ouvrir les articles liés dans l'app**. Tout le pipeline
      existe déjà — un `/wiki/Titre` se résout, se récupère et se rend comme
      l'article principal. Ce qui retient n'est pas technique : l'onglet
      deviendrait un navigateur, il lui faudrait une pile de retour, et surtout
      il **dériverait du mod auquel il appartient** — on lirait « Grand Prix
      d'Allemagne 1976 » dans la fiche d'une Ferrari. Le cache gonflerait sans
      borne claire et l'attribution devrait suivre chaque page visitée. À
      trancher comme une question d'UX, pas à glisser dans un lot.
      TTL : 30 jours en positif, 90 en négatif.
- [ ] **Signature Authenticode** : le workflow est prêt, il attend un
      certificat. Définir la variable de dépôt `SIGN_COMMAND` suffit à
      l'activer — voir `docs/windows-code-signing.md` (lire **avant** d'acheter,
      deux pièges y sont documentés). Repo passé public, `v0.1.0` publiée,
      README doté de la « Code signing policy » exigée : candidature déposée
      auprès de la SignPath Foundation (certificat gratuit, projet open
      source), réponse attendue par email. Si refus ou trop long, plan B
      documenté : Certum Open Source Code Signing (~49€/an, cloud SimplySign,
      pas de jeton USB).
- [ ] **Refonte de la navigation et des fiches** — **livrée et entièrement dans
      `main`**. Rail à deux rangs, inventaire unique des
      compléments, une seule anatomie de fiche, notes sur toutes les entités.
      **Spec, maquette et plan de livraison dans `docs/`** —
      `PLAN-refonte-navigation.md` porte l'ordre des lots et, surtout, les
      **mesures faites sur la bibliothèque réelle avant de commencer** : elles
      ont supprimé un lot entier (la détection CSP des voitures existe déjà,
      167 sur 311) et démenti le fourre-tout redouté (19 des 28 mods « autres »
      sont des mannequins). **Les neuf lots sont faits** (L1 à L9) **et
      fusionnés** : il ne reste aucune branche et le palier du plan est passé.
      Ce qui suit n'est donc plus du code à livrer, mais des questions ouvertes.
      Le détail lot par lot, avec ses écarts assumés, est dans le plan — ne pas
      le recopier ici.
      **Quatre points de la spec sont tombés à la mesure** plutôt qu'en
      implémentation, et c'est le genre d'information qu'on ne retrouve pas
      deux fois : la détection CSP des voitures existait déjà (167 sur 311),
      l'export des notes n'a nulle part où aller (il n'existe aucun export des
      métadonnées de l'overlay), la valeur d'origine des unités est déjà la
      seule affichée (rien n'est converti), et le retrait des mannequins de
      l'inventaire — pourtant demandé par le REFONTE§5 — s'est révélé **nuisible** :
      la galerie de l'écran Pilote est un sélecteur, elle ne gère rien, et les
      en retirer supprimait le seul endroit d'où on pouvait les désactiver ou
      les supprimer. Les livrées tranchent par l'exemple : **choisir et gérer
      sont deux gestes, ils peuvent avoir deux écrans.**
      **Reste trois questions, aucune ligne de code en attente** : « aussi
      dans … » (REFONTE§4.5, un mod rattaché à plusieurs entités), les points
      ouverts de REFONTE§14 à reposer avec l'inventaire réel sous les yeux, et le
      markdown dans les notes (demandé à l'usage le 2026-09-11, voir le 4bis du
      plan).
- [ ] **Taxonomies (`SPEC-taxonomies.md`) — familles et onglet Catégories
      livrés.** L'index de bibliothèque (`SPEC-index-bibliotheque.md`) est
      fait : `car.category_families` dans les règles, huit familles livrées,
      un filtre `Famille`, et l'onglet Catégories de l'Atelier qui les édite.
      Découpage convenu avec l'utilisateur, un lot à la fois avec une pause
      pour regarder : **Catégories (fait)**, **Pays (fait)**, **Marques
      (fusions) (fait)**, **Marques (logos) (fait, à valider)**. **Reste** :
      **la validation du seuil de fond cuit sur le gros corpus** (l'autre PC,
      via l'outil de relevé — le banc `logos::tests::real_install_logos` en
      est l'amorce). Les emblèmes de TAXO§10 sont posés pour les marques
      (suggestions et jetons de l'éditeur de filtre, puce à valeur unique) ;
      les icônes de famille n'y sont pas encore — le filtre Famille montre
      ses noms seuls. Le raccourci de la fiche vers `Atelier › Marques` est
      une entrée du menu ⋮ (« Éditer la marque … ») plutôt qu'une action sur
      le nom dans le sous-titre : même chemin, sans un second geste caché
      dans une ligne de texte.
      **Marques (logos).** Écarts assumés : (1) la résolution est plafonnée à
      128 px dans l'élection (mesuré : sans ça, un Nissan unique de 4096 px
      battait le logo de sept voitures) ; (2) la pastille reprend `--txt`, le
      ton clair de l'app, plutôt que le `#ececed` de la spec — pas un gris de
      plus pour une surface ; (3) les choix de logo sont indexés par le nom de
      marque : fusionner ou renommer une marque ne les emporte pas (à
      reprendre si ça gêne à l'usage) ; (4) le « fanion de curation » de la
      ligne ne tient compte que des fusions, pas du choix de logo.
      **Marques (fusions).** La marque se décide dans `harmonize::compute`
      (règle, sinon fichier, puis fusions, puis casse/accents) — c'est donc
      `Harmonized::brand` qui porte la marque rangée, pas seulement la
      correction d'une règle ; `ENGINE_VERSION` 6 pour que la bibliothèque
      existante soit rangée au démarrage. La fusion automatique vise
      l'orthographe **majoritaire de la bibliothèque**, faute d'une liste de
      référence comme celle des pays du jeu. **Les règles sur le nom se créent
      aussi depuis l'onglet Marques** (« quand leur nom contient »), sans
      changer de maison : décidé avec l'utilisateur après mesure — la seule
      correction livrée qui agit chez lui range deux VRC Auriel 4 sous Audi,
      et leur champ marque dit `VRC` : une fusion aurait emmené les quatorze
      VRC. **Piège de la première version, relevé à l'usage** : le geste
      partait de la marque de DÉPART (« envoyer ces voitures ailleurs ») et ne
      cherchait que dans ses voitures ; l'utilisateur, dans Lamborghini, a
      tapé `lanzo` et ne trouvait rien — la Lanzo était chez RSS. On pense
      depuis la marque d'arrivée : le mot cherche dans toute la bibliothèque.
      Même relecture pour les libellés : « orthographes fusionnées » et
      « rangées ici par leur nom » ne disaient pas en quoi elles différaient ;
      deux lignes parallèles (« quand leur fichier écrit la marque » / « quand
      leur nom contient ») le disent. **TAXO§8 ne s'applique pas ici** :
      les `brand_fix` sont des règles « le nom contient X », une heuristique,
      pas une correspondance exacte — elles restent dans Règles (REGLES§11 les
      y range aussi). Écarts assumés : pas d'emblème dans la liste (lot
      logos), et la proposition « Nismo → Nissan » de la maquette ne sort pas
      (distance 2 sur cinq lettres, au-delà du seuil) — mieux vaut rater une
      proposition que d'en faire des fausses.
      La tuile de marque de l'index montre désormais le logo élu (TAXO§4) ;
      l'élection provisoire par le plus petit id (`brandBadges`) est retirée.
      **Deux écarts assumés avec les specs, à ne pas « corriger » sans les
      relire.** (1) Les familles sont un filtre **à part** (`family`), pas le
      filtre `category` réinterprété : la puce « même catégorie que ma
      voiture » du bloc Adversaires pose `category` = premier tag `#`, et une
      famille aurait élargi une grille GT3 à toutes les voitures de course.
      La colonne Catégorie du tableau montre donc toujours le premier tag `#`.
      (2) La table des familles n'est **pas** une copie TypeScript : elle vit
      dans les règles Rust, là où l'onglet Catégories l'éditera — la table
      d'alias de pays a eu un doublon TS, et c'est le doublon invisible qui
      gagnait.
      **Mesure qui a fixé le seed** : sur les 359 voitures du corpus (tags des
      `ui_car.json`), seuls 238 portent un tag `#` — des familles construites
      sur les seuls `#` auraient laissé un tiers de la bibliothèque « Non
      classé ». Le seed compare donc tous les tags (sans `#` de tête) : 601
      appartenances, 34 non classées, presque toutes du trafic. `sport` (12
      voitures, surtout DDM) et `jdm` restent hors familles : ambigus, et la
      spec veut que `#jdm` reste un genre.
      **Piège payé sur l'onglet Catégories** : « Rétablir » une famille
      reprenait ses tags d'origine mais laissait tomber ceux qu'elle avait
      gagnés — un `gt3` déplacé dans Classique puis Classique rétablie, et
      `gt3` n'était plus dans aucune famille : Course perdait ses GT3 sans
      qu'on y ait touché. Ils retournent désormais dans la famille qui les
      livre (`restoreFamily`, testé).
      **Pièges payés sur les pays.** (1) La première traduction retrouvait
      le code ISO par le **nom anglais** dans la table de régions du moteur :
      33 des 221 noms du jeu n'y ont pas de correspondance exacte (Tchéquie,
      Russie, Turquie, Hong Kong…), et la correspondance par nom rendait en
      silence des **codes retirés** qui portent encore le même nom — `UK` au
      lieu de `GB`, `FX` (France métropolitaine) au lieu de `FR`, `YU`, `DY`,
      `HV`, `TP`. D'où la table alpha-3 → alpha-2 de `nationalities.rs`, les
      codes retirés exclus et un test qui les nomme. (2) Le drapeau de la puce
      se cherchait **par le libellé** : traduire le libellé l'éteignait. Il se
      cherche désormais par la valeur (`ChipValue`).
      **Écart assumé** : la spec voulait un jeu de drapeaux SVG embarqué
      (TAXO§3.1) ; l'app garde ceux du jeu (`content/gui/NationFlags/`), déjà
      présents, qui couvrent les 221 pays et les quatre nations britanniques
      que l'ISO ne connaît pas. Et le nom rangé reste le nom anglais du jeu, pas
      le code : c'est la clé du drapeau et de la table de CM ; le code n'est
      que dérivé, pour traduire.
      **À surveiller** : chaque modification de l'onglet réharmonise toute la
      bibliothèque (le pays est décidé à l'écriture). Non mesuré sur la
      bibliothèque réelle — si c'est lent, regrouper les écritures.

- [ ] **Catalogue de règles et surcouche (`SPEC-regles.md`) — lots 1 à 5 faits.**
      Ordre convenu avec l'utilisateur : (1) deux couches pour les tables de
      taxonomie **— fait**, avec les marques livré / ajouté / retiré dans les
      onglets et `rules-tool` (`diff`, `promote`) pour promouvoir une
      curation faite dans l'app vers le catalogue ; (2) manifeste des catalogues passés + test « diff
      nul » pour les règles en liste **— fait** ; (3) identifiants stables et surcouche
      pour les règles en liste (marque, classe, fusion de tags, specs) **— fait** ;
      (4) rapport de mise à jour et « Revenir à » **— fait** ; (5) écran
      Règles refait (liste unique, bascules, badges, compteurs d'effet)
      **— fait**. Le lot Marques des
      taxonomies vient **après** le lot 1, pour naître dans le bon format.
      **Mesuré avant de coder**, et ça a décidé de la migration : sur les 9
      versions publiées, les règles embarquées n'ont connu que **2** états
      (v0.1.0→v0.3.1, v0.4.0→v0.7.0) ; les tags de pays sont identiques
      partout ; alias et familles n'ont jamais été publiés. Le `tag-rules.json`
      de la machine de dev, semé le 27 juin, portait encore la liste noire
      `remove` retirée en v0.4 — la preuve que rien ne l'avait jamais atteint.
      **Lot 2, mesuré.** Les règles en liste n'ont **jamais changé** du premier
      commit (63e44ba) à v0.7.0 : seule la liste noire `remove` a disparu (v0.4,
      le moteur ne la lisait plus) et `category_allowlist` est apparue (v0.1).
      Un seul manifeste figé les représente donc toutes
      (`rules/manifests/pre-layer-rules.json`, `rule_manifest.rs`), avec le
      classement de REGLES§13.2 (intacte / supprimée / à l'utilisateur /
      réordonnée), par contenu faute d'identifiants (REGLES§13.3 : une règle
      modifiée se lit comme une suppression plus une règle à lui). Le banc
      « diff nul » (`harmonize::snapshot`) recalcule la classification de toute
      la bibliothèque sans rien écrire ; rejoué sur l'install de dev
      (`cargo test --lib harmonize::tests::real_install_diff_nul -- --ignored
      --nocapture`, sur des copies) : 11 sections intactes, **350 mods, 0
      classé différemment**. C'est lui qui prouvera le lot 3.
      **Lot 3.** 123 identifiants écrits une fois dans le catalogue et dans le
      manifeste (jamais recalculés) ; surcouche par section dans
      `rules-overlay.json` (désactivée / dérivée avec empreinte FNV-1a — pas
      `DefaultHasher`, instable d'une version de Rust à l'autre — / à lui) ;
      `tag-rules.json` migré puis mis de côté en `tag-rules.pre-overlay.json`.
      La crate `pitbox-taxonomy` est devenue `pitbox-catalog` (modules
      `taxonomy` et `rules`). Banc réel : aucune décision, 350 mods, 0 écart.
      **Lot 4.** Découvert en le préparant : **un catalogue amélioré n'atteignait
      jamais la bibliothèque existante** — la classification est stockée, et
      seul un changement de `ENGINE_VERSION` la recalculait au démarrage. Le
      rapport de mise à jour est donc aussi ce qui la recalcule. Écarts avec la
      spec, assumés pour ce lot : pas de compteur **par règle** (« a classé 4
      mods », REGLES§6.3) — le rapport donne le total des mods reclassés, et le
      compteur par règle viendra avec ceux de l'écran Règles (lot 5), qui
      demandent au moteur de noter quelle règle a produit quoi ; pas de bouton
      « Désactiver » sur les lignes du détail ; et le nom d'une version est
      celui de l'app, si bien qu'un build de dev qui change le catalogue sans
      changer de version affiche « Catalogue de règles mis à jour » sans flèche.
      L'interrupteur global « Utiliser le catalogue Pit Box » (REGLES§7) est
      venu avec l'écran Règles (lot 5).
      **Vérifié dans l'app** en simulant une mise à jour (le catalogue « vu la
      dernière fois » privé du tag `tuned` dans Route) : rapport juste, et c'est
      cette vérification qui a trouvé le dernier trou — les **familles ne sont
      pas stockées** (elles se dérivent des tags à l'affichage), donc le banc
      annonçait « 0 mod reclassé » alors que l'index changeait. Le banc les
      calcule désormais : 2 voitures reclassées sur l'install de dev.
      **Mystère non résolu, à surveiller** : sur la machine de dev
      (2026-09-23), un `tag-rules.json` plus ancien est **réapparu deux fois**
      dans le dossier de config, dates d'origine conservées (donc recopié, pas
      réécrit), alors qu'aucun processus de Pit Box ni aucune commande de la
      session ne l'écrivait — une autre session active ou un outil externe
      restent les seules pistes. La mise de côté ne peut plus écraser une copie
      précédente (`tag-rules.pre-overlay-2.json`…), et une réapparition après
      migration laisse désormais un `log::warn!` daté : c'est lui qu'il faudra
      lire à la prochaine occurrence.
      **Piège évité de justesse** : la sauvegarde de démarrage (`backup.rs`)
      copiait `tag-rules.json` mais ignorait les fichiers de décisions — tout
      ce que l'utilisateur fait dans les règles aurait été hors du filet.
      **`promote` pour les règles en liste — fait** (`rules-tool/src/lists.rs`).
      Le fichier est lu en structures typées et non en `serde_json::Value`, qui
      trie ses clés : l'option `preserve_order` qui l'éviterait s'étendrait à
      toute la compilation du workspace, où l'ordre des clés fait l'empreinte
      des dérivations (`content()`). Une promotion à vide rend le fichier à
      l'octet près (testé). Les identifiants retirés vivent dans une liste
      `retired` en tête de fichier, ignorée par l'app : une surcouche qui en
      nomme un garde son entrée sans effet, ce qui était déjà le comportement.
      **Pièges pour la suite.** (a) « La règle utilisateur gagne » (REGLES§3)
      n'a pas le même sens selon le type, **et c'est tranché** : `brand_fix`,
      `class_fix`, `tag_merge` et l'extraction des specs s'arrêtent à la
      première règle qui correspond, `name_to_tag` additionne. Pour les
      premières, la règle de l'utilisateur passe **devant** celles du
      catalogue — c'est déjà là que l'écran Règles l'insère (`unshift`). (b) La couche 4 (corrections manuelles, REGLES§14) existe à
      moitié : tags manuels, nom et description repris, mais rien pour la
      marque, le pays ou la classe d'un mod précis. (c) La liste blanche des
      catégories de circuit est encore une table recopiée : elle rejoint la
      surcouche avec les règles en liste.
      **Lot 5.** L'écran édite la surcouche **geste par geste** au lieu de
      listes entières : chaque bascule, modification ou suppression est écrite
      aussitôt et réappliquée, et la barre « Enregistrer & réappliquer » a
      disparu avec l'aperçu d'impact global — il n'y a plus rien en attente.
      Ce que ça a demandé : (a) le moteur **nomme les règles qui agissent**
      (`Harmonized::fired`), sans rien changer à ce qu'il classe — le champ
      est hors sérialisation, donc hors du banc « diff nul » ; (b) les règles
      de l'utilisateur ont un identifiant (`own-N`), attribué à la
      normalisation et déterministe — c'est à lui que tiennent sa bascule et
      son compteur ; (c) une dérivée faite par l'écran arrive sans empreinte
      d'origine, et c'est Rust qui la remplit — le front ne sait pas calculer
      l'empreinte FNV. Réapplication regroupée en **une transaction** : un
      commit par mod, c'est une synchronisation disque par mod, et chaque
      bascule réapplique désormais. Mesuré sur l'install de dev (350 mods,
      debug) : 0,2 s à l'ouverture, 0,2 s par geste
      (`harmonize::tests::real_install_effect_counters`, ignoré, sur des
      copies). **Écarts assumés** : catalogue éteint, ses lignes disparaissent
      de l'écran plutôt que d'apparaître grisées, et une dérivée y perd son
      `✎` (elle s'applique alors comme une règle à soi, ce qu'elle est) ; les
      règles à soi ne se réordonnent pas entre elles (la plus récente en
      tête) ; le filtre « Mes règles seulement » n'est pas mémorisé.
      **Compteur par règle du rapport et « Désactiver » — faits** : les deux
      classements de la mise à jour (ancien et nouveau catalogue) notent déjà
      qui a agi (`harmonize::snapshot_fired`), le compteur n'a coûté aucune
      passe de plus. Pas de compteur sur les familles et les pays, que le
      moteur ne trace pas. Pas de « Voir » sur une ligne corrigée (la maquette
      de REGLES§6.3 le montre) : la liste des mods reclassés est en bas du
      détail.
      **Export/import — fait** (REGLES§9). Écart assumé avec la spec : elle
      dit qu'une décision visant une règle retirée est « ignorée » à l'import ;
      ici elle est fusionnée et comptée à part, parce que c'est ce que fait
      déjà une mise à jour qui retire une règle (REGLES§4 : éteinte, elle est
      sans effet ; dérivée, elle devient une règle à soi) — deux chemins, un
      seul comportement. L'import **fusionne sans rien retirer** plutôt que
      de remplacer : la question « remplacer ou fusionner ? » n'avait pas de
      bonne réponse avant d'avoir vu le résultat.
      **Reste du chantier** : lancer le relevé sur le gros corpus, ci-dessous.
      **Outil de relevé — fait, dans l'app plutôt que dans `rules-tool`**
      (`survey.rs`, Atelier › Maintenance › « Créer un relevé… »). Changement
      de maison assumé : un contributeur a l'app, pas une chaîne Rust, et la
      moitié du relevé (classement, logos, pays connus du jeu) vit déjà dans
      le crate de l'app. Pour Claude Code sur une autre machine, le même relevé
      sort sans l'app : `cargo test --lib survey::tests::real_install_survey
      -- --ignored --nocapture` (sur des copies, `PITBOX_SURVEY_OUT` pour le
      fichier). Anonyme par construction : identifiants de mods et ce que les
      mods publient, jamais un chemin, un auteur, une note ni un tag saisi —
      un test le vérifie. Il contient aussi les **décisions** de l'utilisateur
      (le contenu d'un export) : c'est exactement ce que `rules-tool promote`
      sait intégrer au catalogue. **Piège payé en le mesurant** : le moteur
      écarte volontairement `street`/`race` (portés par la classe) et les tags
      de pays (qui ne parlent que sans pays déclaré) ; comptés comme
      « inconnus », ils écrasaient la liste (`street` 167, `race` 107, `japan`
      52) — ils sont exclus du décompte. Sur l'install de dev : 324 voitures,
      26 circuits, 1 s, 260 Ko ; 28 voitures sans famille ; en tête des tags
      sans règle, `original` (22), `ddm` (17), `lightweight` (16),
      `#vintage supercars` (12). **Reste** : le lancer sur le gros corpus
      (l'autre PC) — valider le seuil de fond cuit des logos, puis curer et
      promouvoir ce qu'il révèle.
      **Premier relevé réel (2026-09-25)** : 397 voitures, 138 circuits. Il a
      trouvé un défaut du relevé (les tags qu'une famille prend comptés
      « inconnus », corrigé) et un du catalogue (`lmdh` inconnu : les LMDh
      tombaient en `#lmp1`), et donné les propositions de marques RSS/VRC —
      relues par l'utilisateur puis **promues** (2026-09-25) : 46 règles sur
      le nom, 26 packs « pas une marque », 13 fusions de marques, 8 pays, la
      famille Trafic, `lmdh`, `#wsc60`, `#gt1` pour les RSS GT, IMSA GTO. La
      détection des fonds cuits s'est confirmée sur les deux corpus (seul le
      Porsche Kunos, aucune fausse alerte). **Relevé d'un dossier** ajouté
      pour le PC sous Mod Organizer, dont le montage virtuel n'est visible que
      des programmes qu'il lance.
      **Relevé Mod Organizer (678 voitures, 318 circuits)** : deux changements
      décidés avec l'utilisateur. (1) Les familles lisent aussi la **classe**
      (43 non classées → 2). (2) **« Ce n'est pas une marque »** pour les packs
      et séries, la marque lue dans le nom — préféré à une quarantaine de
      règles « le nom contient ferrari → Ferrari », dont la recherche par
      sous-chaîne se trompait (« Formula Ford », « Jordan-Ford », « 2-seater »).
      Limite connue : la recherche ne connaît que les marques de la
      bibliothèque et des fusions ; sur une petite bibliothèque, un
      « Benetton B191 » reste sous son pack faute de Benetton connu. Une liste
      livrée de marques connues lèverait la limite — à faire si ça gêne.

- [ ] **Son d'une voiture Kunos — solution provisoire (B), à reconsidérer
      (2026-10-14).** Pour une voiture Kunos, activer un son écrit dans le
      `sfx/` **du jeu**, hors de `gamebackup.rs`, que la règle d'or n°5 dit
      pourtant de toujours emprunter. L'original est à l'abri dans
      `<lib>/sounds/<voiture>/__original__` (copie unique, avant la première
      bascule) ; le seul défaut concret était la fenêtre où une app tuée entre
      l'effacement du `sfx/` et la copie du mod laissait la voiture sans son,
      rien ne la réparant hors « son d'origine » à la main.
      - **Fait (B)** : bascule d'un seul geste (préparé dans
        `sfx.pitbox-new`, deux renommages, base écrite avant le nettoyage de
        `sfx.pitbox-old`) et filet au démarrage
        (`submods::recover_interrupted_swaps`, étape `setup.sound_swaps`), qui
        donne à la voiture le son que dit la base. Tout dans `sounds.rs`.
      - **Écarté pour l'instant (A)** : tout faire passer par `gamebackup.rs`,
        fichier par fichier. Ce qu'il faudrait : (1) son balayage de démarrage
        (`restore_orphans`) remet l'original de **toute** sauvegarde que plus
        rien ne réclame — aujourd'hui seuls les ajouts au jeu et les « autres
        mods » réclament —, donc un son actif devrait devenir un troisième
        réclamant, sinon le balayage annulerait le son à chaque démarrage ;
        (2) l'écoute « Origine » (`enginesound::sound_dir`) lit `__original__`
        et lirait la sauvegarde de `game_backup/` ; (3) l'écran Dossier du jeu
        (`gamestate/classify.rs`, `sounds`) classe le `sfx/` par la même
        copie ; (4) reprendre l'existant : les voitures Kunos qui ont déjà un
        son actif avec l'ancienne copie `__original__`. Une session, zone des
        règles d'or, tests d'abord.
      - **Ce qui ferait changer d'avis** : un second mécanisme hors
        `gamebackup.rs` qui écrirait dans le jeu (deux font un motif), un
        bug où `__original__` ne suffit pas (bibliothèque supprimée alors
        qu'un son Kunos est actif : `clean-ac-footprint.ps1` ne le voit pas,
        pas plus qu'il ne verrait `game_backup/`), ou l'envie d'un seul
        registre de ce que Pit Box a changé dans le jeu.

- [ ] **Corrections par mod — reste la classe.** On corrige d'un mod son nom,
      sa description, ses tags (couche 4 de REGLES§14), depuis la fiche
      technique (FICHE§8) son **pays** et sa **fiche technique**, depuis le
      crayon de l'en-tête (2026-10-20, §5bis.3) sa **marque** et son
      **année**, et depuis la puce de catégorie (2026-10-21, §5, REGLES§3)
      sa **catégorie** — voiture forcée, circuit ajouté ou retiré —, qui
      survit à tout recalcul. **Pas encore sa classe.** La couche est celle
      de REGLES§3 : par mod, la plus prioritaire de la cascade, **jamais
      exportée avec les règles ni remontée par le relevé** (elle appartient à
      une bibliothèque, pas à une façon de classer) — elle voyage avec
      l'export de la bibliothèque (`category_user` et `brand_user`, EXPORT§4).
      La marque suit le modèle du nom (une colonne `brand_user` à côté de
      `brand`, que l'harmonisation réécrit, lue devant elle dans
      `overlay::MOD_SELECT`) ; la catégorie a sa table `category_user`, sans
      clé étrangère comme `tech_user`, posée sur le résultat par
      `harmonize::store` et par `categories::apply`, avec le calculé gardé à
      côté. Reste ouvert : (1) la fusion de marques de l'onglet Marques
      s'applique aux marques que l'harmonisation calcule, pas à une marque
      saisie à la main ; (2) une marque ou une catégorie décidée n'est
      « modifiée par vous » que sur la fiche, pas sur la liste ; (3)
      l'historique d'un circuit dit « fiche technique modifiée » quand on
      corrige son année (même décision `tech_user`) ; (4) le champ de tag
      manuel propose les catégories de la bibliothèque, pas les noms traduits
      d'une catégorie de circuit. Demandé par l'utilisateur (2026-09-24) en
      discutant des fausses marques : pour deux ou trois voitures, une
      correction directe est plus simple qu'une règle.

- [ ] **Catégorie calculée : choisir par rang d'exécution des règles
      (REGLES§3), pas par ordre alphabétique.** Aujourd'hui la catégorie
      d'une voiture est le premier tag `#` **par ordre alphabétique** des tags
      que les règles produisent (`rules::pick_category`, un `BTreeSet`) :
      l'ordre d'exécution de REGLES§3 — par couche, puis par rang dans la
      couche — n'y joue aucun rôle, si bien qu'une règle de l'utilisateur
      passée devant celles du catalogue ne change pas la catégorie choisie.
      À faire : garder, pour chaque tag `#` produit, le rang de la règle qui
      l'a produit, et prendre le premier dans l'ordre d'exécution. Change la
      catégorie de voitures existantes : un lot à part, avec un diff attendu
      et expliqué (REGLES§13.5 et REGLES§13.6). Un forçage reste aujourd'hui le seul
      moyen de choisir une autre catégorie sans règle.

- [ ] **Unifier les retours à l'original sur `ManualMark`.** Le nom, la marque
      et l'année reprennent leur valeur d'origine par un lien « Back to
      original » dans le formulaire du crayon de l'en-tête
      (`IdentityEdit.svelte`) ; la catégorie, elle, porte le fanion et le `↺`
      de `ui/ManualMark.svelte` (la valeur calculée en infobulle). Deux
      gestes pour la même idée, selon l'endroit. À faire : le fanion et le `↺`
      de `ManualMark` sur ce que le formulaire a modifié (puce du nom, de la
      marque, de l'année, dans l'en-tête), le lien du formulaire restant un
      raccourci ou disparaissant ; et la même chose pour la description et
      la fiche technique (`↺ revenir`, `TechSheetEdit.svelte`).

- [ ] **Import en masse : l'analyse n'a ni progression ni annulation.** Depuis que
      l'analyse compare les fichiers d'un mod qui ressemble à un doublon
      (SPEC §4.1), réimporter en une fois un dossier déjà entièrement présent la
      fait lire les deux copies de chaque mod : de l'ordre du double de la taille
      du dossier — **estimé, jamais mesuré** (300 Go : une vingtaine de minutes
      sur SSD, plus d'une heure sur disque dur), accepté tel quel par
      l'utilisateur (2026-10-21). Elle ne retient plus la base pendant ce temps
      (`importer::plan_bulk`, puis `BulkPlan::verify`), mais l'écran n'affiche
      rien et rien ne l'interrompt. À faire : une progression dans la pile de
      notifications et un bouton Arrêter, comme les lots qui touchent au disque
      (`bulk:progress`) ; et une mesure sur une vraie bibliothèque avant de
      décider si un palier de plus (la date de modification) vaut son
      imprécision. À l'**exécution** d'un import (pas de l'analyse), la
      comparaison d'un doublon présumé se fait sous le verrou de la base, le
      temps d'un seul mod.

- [ ] **Deux jeux de règles de tags ont divergé — à trancher.**
      `docs/default-tag-rules-enriched.json` **n'est pas ce que l'app charge** :
      elle sème `src-tauri/rules/default-tag-rules.json` dans le dossier de
      config au premier démarrage, et c'est celui-là qui est édité par l'écran
      Règles. Les deux se sont séparés — la copie de `docs/` porte un groupe de
      règles de plus sous `car` et sous `track`, et six jours d'avance
      (2026-08-29 contre 2026-08-23).
      **Pourquoi ça attend.** Reporter ces règles changerait le tagging de
      **toute** la bibliothèque, donc les catégories, donc les filtres et la
      composition de plateau qui s'appuient dessus. Ce n'est pas un ménage,
      c'est une décision sur le contenu — mise de côté avec l'utilisateur
      pendant la passe documentation.
      **Pour reprendre :** comparer les deux fichiers groupe par groupe,
      décider si l'enrichissement de `docs/` est voulu, et s'il l'est, le
      reporter dans `src-tauri/rules/` — puis faire disparaître l'un des deux,
      parce que deux fichiers qui décrivent la même ontologie finiront toujours
      par se séparer à nouveau. L'avertissement est écrit dans
      `docs/README.md` et au §14 de `SPEC.md` en attendant.

- [ ] **Focus rouge sur les tuiles de l'index — à trancher.** Les tuiles et le
      lien de section de l'index (`BrowseIndex.svelte`) dessinent leur focus en
      rouge, comme le voulait INDEX§9, alors que le focus est jaune partout
      ailleurs (`:focus-visible` de `global.css`, §7.2ter de `SPEC.md`). Relevé
      en reprenant les specs (2026-10-18) ; les deux specs disent ce que fait
      l'app, l'incohérence est dans l'app.
- [ ] **Points à vérifier en conditions réelles** (repris de `SPEC.md` §15,
      où ils vivaient sans qu'aucun chantier ne les porte).
      - **Détection de la stack météo** (Pure / Sol / CSP / rien) et la
        correspondance preset → backend.
      - **Table Kunos** (`kunos_content_dates.json`) : noms de dossiers et
        années à valider contre une installation réelle, ligne par ligne.
      - **Musique (§16)** : testée sur ses parties pures, jamais validée à
        l'oreille. Le premier scan d'un dossier (décodage complet pour le RMS)
        bloque le fil du moteur quelques secondes : à confirmer que ce silence
        à la première entrée en Big Picture ne gêne pas, avant d'investir dans
        un scan progressif.
      - **Détection `AC_LIVE` (§16.2) et plein écran (§16.6)** : offsets repris
        d'une implémentation tierce, jamais essayés avec une vraie session. La
        musique GRID doit tenir pendant le chargement et se couper quand la
        voiture devient pilotable ; le plein écran doit couvrir l'écran entier.
- [ ] **Refactorings repérés, reportés (2026-09-25, revus le 2026-10-04, le
      2026-10-10 et le 2026-10-20).**
      Classés par bénéfice sur risque ; aucun ne change le comportement,
      aucun ne touche un format persisté. Les actions de la fiche
      (`$lib/detail/actions.svelte.ts`) et la scène de l'aperçu 3D
      (`$lib/preview3d/scene.ts` et voisins) sont sorties le 2026-10-04.
      1. **`importer.rs` : la suite du découpage.** Première passe faite le
         2026-10-04 : `process_found` (618 lignes) est devenue
         `importer/filing.rs`, une étape par question, sous deux tests de
         caractérisation ajoutés d'abord. Restent les plus longues :
         `sweep_leftovers` (353 lignes) et `import_leftover` (225) — **le
         cœur de la règle d'or n°3**, ce qui est à côté du dossier du mod et
         jamais dedans —, puis `file_extracted`, `import_one_folder` et
         `exec_one` (140 à 160 chacune ; la dernière est passée dans
         `importer/bulk.rs` le 2026-10-20). Même méthode : tests
         d'abord, puis un module par étape. Une session par fonction ;
         risque élevé pour les deux premières.
      2. **Deux longues fonctions rendues visibles par le découpage de
         `submods`** : `submods/skins.rs::import_skin_pack` (116 lignes) et
         `submods/sounds.rs::import_sound` (93). Elles étaient déjà là, noyées
         dans un fichier de 3 000 lignes. Une petite session chacune, risque
         faible à moyen (l'import), couvertes par leurs tests (repéré le
         2026-10-10).

      **Faits le 2026-10-10**, sans changement de comportement, un commit
      chacun : le curseur de régime mis en commun (`EngineRevControls`,
      `f653c39`) ; `inventory::rows_from_disk` découpée par source
      (`30da7d7`) ; `submods.rs` découpé en `submods/` — livrées, habillages
      de circuit, sons (`9b0925d`) ; les sons et les habillages de circuit
      sortis de `DetailPage` (`SheetSounds`, `SheetTrackSkins`, `275bde7`) ;
      `others::place` découpée en `Placement` après deux tests de
      caractérisation (`9ffd715`, `79d6e1c`) ; le démarrage sorti de `lib.rs`
      vers `startup.rs` (`f01ad6e`) — avec un seul déplacement d'étape, les
      appariements Wikipédia passés en tête des rattrapages.

      **Revue du 2026-10-10** : démarrage sans régression (cartes à l'écran
      en 999 ms, 1 064 ms le 2026-10-08, 385 cartes) ; `transfer/files.rs`
      découpé en un module par sujet (`0164f5c`).

      **Revue du 2026-10-20** (77 commits), ses quatre points faits le jour
      même, sans changement de comportement, un commit chacun : l'import en
      masse sorti d'`importer.rs` vers `importer/bulk.rs` (6 073 → 5 444
      lignes, `20d35e3`) ; une seule file d'écritures pour les onglets de
      Classement, recopiée cinq fois (`workshop/writeQueue.ts`, `d2bb7fb`) ;
      le repli des accents (six copies) et la distance d'édition (deux) mis
      en commun dans `src/lib/text.ts` (`a143298`) ; l'assemblage des sources
      de la vignette Online descendu de la façade dans `online/picture.rs`
      (`b3319e6`). **Démarrage non mesuré** : la revue a tourné dans une
      session distante sous Linux, sans le jeu ni la bibliothèque —
      `npm run bench:startup` reste à lancer sur la machine de référence.
