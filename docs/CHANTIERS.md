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
      reste à faire sont dans `docs/SPEC-preview-3d-kn5.md` PREVIEW§13 à PREVIEW§15** — c'est
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

- [ ] **Corrections par mod — à faire, chantier à part.** Aujourd'hui on
      corrige d'un mod son nom, sa description et ses tags (couche 4 de
      REGLES§14), mais **ni sa marque, ni son pays, ni sa classe, ni sa fiche
      technique**. Demandé par l'utilisateur (2026-09-24) en discutant des
      fausses marques : pour deux ou trois voitures, une correction directe
      est plus simple qu'une règle. À concevoir comme la couche la plus
      prioritaire de la cascade (REGLES§3), par mod, jamais exportée ni
      remontée par le relevé (elle appartient à une bibliothèque, pas à une
      façon de classer), et visible sur la fiche comme « modifié par vous »
      avec retour à la valeur calculée.

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
