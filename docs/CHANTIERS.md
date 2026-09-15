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
      conception, §5 et §6 de la spec. Ne pas commencer sans les avoir lues.
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
      proche) et les titres de popup (`OpponentPicker`/`SavedSessionsDialog`,
      13px/majuscules, identiques entre eux mais ne correspondant à aucun des
      quatre niveaux). **Couleurs sémantiques** : le **rouge** a désormais son
      barème, écrit au §7.2ter du SPEC — quatre niveaux, un quota par niveau,
      et la règle « le survol n'introduit jamais de rouge sur un élément qui
      n'y a pas droit au repos ». Appliqué au rail de navigation et à la
      colonne de session ; **les filtres et la grille de bibliothèque restent
      à faire** (specs séparées). Le reste n'est pas attaqué — le `--orange`
      ajouté pour « mod inactif » (`StateBadge`) est le premier pas dans cette
      direction (ni le jaune d'alerte, ni le rouge destructif).
- [ ] **Vignettes régénérées de la grille** — **fusionné dans `main`, mais
      éteint** : `FEATURE_GRID_THUMBS` est à `false` dans `src/lib/features.ts`,
      qui porte le mode d'emploi de l'interrupteur. Il ne reste qu'un réglage,
      long et fastidieux, et trois presets livrés avec des valeurs provisoires
      seraient jugés sur elles. Spec dans `docs/SPEC-grille.md`, dont le §11 dit ce
      que l'implémentation a changé et pourquoi. La partie A (§2 à §4) était
      déjà livrée ; la **partie B** (§5 à §8) l'est — pipeline, tâche de fond,
      écran de réglage, profils à l'installation — et le modèle de **presets**
      est venu après, d'une remarque de l'utilisateur : la preview d'origine
      d'Assetto Corsa est plus *jolie* que notre rendu, le nôtre plus
      *lisible*. Deux objectifs, pas deux qualités d'exécution du même — d'où
      *Catalogue*, *Vitrine* et *Officiel*, embarqués et en lecture seule,
      qu'on duplique pour s'en faire un, et un preset **par densité de
      grille**.
      **Les pièges à ne pas réintroduire**, tous mesurés ou vécus :
      1. **Ne jamais faire passer la génération par `prepare_car_preview`.**
         C'est le §5.3, et il a une raison chiffrée : 312 conversions dans un
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
      ombre projetée et non l'ellipse peinte du §5.6 ; l'azimut n'a pas été
      « relevé sur les previews Kunos » puisque c'est déjà fait (318°, l'aperçu
      3D) ; la tâche de fond réduite est une barre et non une pastille à anneau
      (la pile bas-droite n'a qu'une forme) ; les six voitures de l'atelier sont
      prises par **catégorie** (plus la voiture de session en tête) et non par
      silhouette, que rien dans les données ne dit ; et le §5.7 (versionnage du
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
      - le cadrage d'Officiel a une **mesure** derrière lui (§11.4 de la spec) ;
        s'en écarter est permis, l'ignorer serait dommage.
- [ ] **Écran Pilote** (fusionné dans `main`). Spec et maquette dans
      `docs/SPEC-ecran-pilote.md` + `maquettes/pitbox-ecran-pilote.html`, résumé au
      SESSION§5 du SPEC. **À lire avant de reprendre** — l'asymétrie qui structure
      tout l'écran (le corps commande, la tenue en découle) y est expliquée une
      fois pour toutes.
      Fait : le backend liste les corps installés et écarte ceux sans squelette
      (`driver::bodies`, 45 sur 52 à la référence) ; l'époque d'un corps se lit
      sur sa texture de casque ; substituer le corps fait tomber la garde-robe
      de la livrée. Côté écran : section `driver`, ligne « Mon pilote » avec
      badge dans la colonne de session (elle remplace la bascule et les trois
      menus déroulants), panneau d'essayage + galerie, survol = essai / clic =
      adoption, favoris, récents, recherche, regroupement, bannière
      d'invalidation, états vides.
      Le plateau est en 3D : le mannequin seul (`kn5_gltf::standalone_driver`,
      pas `graft` avec un hôte vide — trois de ses quatre tâches parlent d'une
      voiture qui n'est pas là) et un cadrage par piste déduit du rig. La
      galerie des corps porte des vignettes rendues à la demande
      (`driverThumbs.svelte.ts`), une à la fois, au défilement, et **le PNG est
      conservé hors du plafond du cache d'aperçus**. Ce dernier point est un
      bug corrigé, pas une précaution : les 45 conversions écrivent ~180 Mo
      dans un pool déjà à ses 2 Gio, donc chacune évinçait une entrée plus
      ancienne — y compris les mannequins des vignettes précédentes. Le cache
      se mangeait lui-même et tout se recalculait à chaque visite.
      **Le volant générique du §D5 est abandonné**, décidé avec l'utilisateur
      après deux essais : sur les poignets (cerceau de 55 cm, personne ne le
      touche), puis sur les phalanges des majeurs — 13 cm plus en avant,
      mesuré, ce qui le met pourtant au bon endroit. Il ne convainc toujours
      pas à l'écran et n'a pas assez d'intérêt pour continuer. `DriverRig`
      garde `grip` : c'est le point que vise le cadrage « Gants ».
      **La mesure qui a débloqué le lot, et la correction qui a suivi** : les
      41 mannequins sur 44 qui partagent une pose de repos au millimètre
      (mains à ±0,277, 1,027, 0,530) *ressemblent* à une prise de volant et
      n'en sont pas — 55 cm d'écart, un volant de car, que les doigts ne
      touchent pas. Rendu à l'écran, c'était le principal défaut du premier
      essai. Appliquer la hiérarchie d'assise **et** l'animation de braquage
      de la voiture ramène l'écart à 35–43 cm selon la voiture, mesuré sur
      douze : le diamètre de son vrai volant. Donc `standalone` ne retire que
      l'ancrage (`DRIVEREYES`), jamais la pose — et la clé de cache porte la
      pose, puisque c'est elle qui décide du volant dessiné.
      **La deuxième mesure, celle qui rend le survol possible** : le `.jpg`
      qu'AC range à côté de chaque `.dds` de garde-robe est la **même image
      aux mêmes dimensions** (`HELMET_2012.dds` 2048×512 DXT5, son `.jpg`
      2048×512). Le survol échange donc la texture côté three.js — quelques
      millisecondes, comme la spec le suppose — au lieu de redemander une
      conversion. L'adoption, elle, reconvertit. Les noms d'image survivent
      dans le glTF (`2016_Suit_DIFF.dds#paint-babbba`), ce qui permet de
      retrouver le matériau à échanger ; les noms de *texture*, eux, sont
      vides — se fier aux images.
      **Le choix est passé par voiture** (`driverOverride.svelte.ts`), contre
      la spec qui le voulait global : une tenue choisie une fois s'imposait aux
      312 voitures, en silence. Cascade à trois niveaux — la tenue de cette
      voiture, puis la tenue par défaut si l'option est cochée, puis la livrée
      — le niveau 1 gagnant toujours. Rangé une clé par voiture dans
      `ui_prefs.json` sur le patron de `preferred.ts`, **parce que le filtre
      « Pilote choisi » de la bibliothèque lit ce drapeau par carte**, donc de
      façon synchrone (`peekUiPref`).
      **Reste, dans cet ordre :**
      1. **Écart spec/réalité à trancher avec l'utilisateur** : le §6.3 range
         les époques par ce que désigne la *famille*, mais mesuré sur
         l'installation, c'est la *variante* qui porte le sens en 1969
         (amon, clark…) et en 1985 (les couleurs). D'où le repli implémenté :
         un regroupement qui ne produirait qu'un groupe passe en grille plate.
      2. **Casque posé de travers sur un mannequin à pièces statiques.**
         `rh_schuberth_helmet_driver_19` traverse sa propre figure. Il est le
         cas que rien d'autre ne représente : cinq maillages **statiques**
         (casque, visière, HANS, prise d'air, visage) accrochés au nœud
         `DRIVER:RIG_Head`, là où les autres mannequins sont entièrement
         skinnés. Or `pose::apply_locals` ne remplace la transformation **que
         des nœuds `Dummy`** — un maillage nommé par la hiérarchie est ignoré.
         À vérifier avant de corriger : c'est le casque qui bouge, ou la tête ?
      3. **Poser le pilote en jeu — fait.** Le **corps** passe par une section
         `[DRIVER3D_MODEL] NAME=…` dans `<voiture>/extension/ext_config.ini`
         (CSP surcharge une section de `data.acd` en préfixant le nom du
         fichier ; `data.acd` n'est pas touché, donc le checksum tient), la
         **tenue** par le `skin.ini` de la livrée, sous le nom du mannequin —
         il n'existe aucune route CSP pour celle-là, cherchée et non trouvée.
         Tout est dans `docs/csp-driver-research.md`, et le code dans
         `driverapply.rs`, appelé par `launch()` : une seule entrée `sync()`
         qui pose quand la voiture a un choix et **retire** quand elle n'en a
         plus. Conséquence à ne pas rater : remplacer le corps rend la section
         de la livrée inopérante, la tenue doit être réécrite sous le
         **nouveau** nom de mannequin. Le déploiement étant en hardlink,
         l'écriture efface le fichier avant de le réécrire — sinon c'est la
         copie de bibliothèque qu'on modifie. **Ne pas écrire `POSITION`**
         dans `[DRIVER3D_MODEL]`, contrairement à ce que suggèrent les réponses
         trouvées en ligne : c'est le même `[MODEL] POSITION` que
         `seating_offset` a mesuré comme inapplicable.
      **Écarts assumés vis-à-vis de la spec, décidés avec l'utilisateur** :
      le favori se pose sur le cœur de la bibliothèque (`♥`/`♡`) placé sous
      l'image et non sur elle — on garde l'argument du §7.3 (l'échantillon est
      montré entier, un bouton posé dessus en cache un morceau) en prenant le
      glyphe du reste de l'app ; les **tenues enregistrées**
      (`driverOutfits.svelte.ts`, §13 complété) reposent les quatre pièces
      d'un clic — le corps d'abord, sinon `setDriverBody` efface les trois
      autres juste après les avoir posées ; le badge `MODIFIÉ` du §3.2 est
      retiré, la ligne de session disant désormais elle-même « Tenue
      d'origine », le nom de la tenue enregistrée, ou « Tenue personnalisée » ;
      et une pièce gardée qui ne s'applique pas au corps courant est **barrée**
      dans sa piste au lieu d'être affichée comme active — elle est conservée
      (§13) mais ne change rien, ce que rien ne disait.
      Le **verrou « voiture de course »** du §11.2 est retiré (décidé avec
      l'utilisateur) : son texte était devenu faux, le corps se pose là comme
      ailleurs, et un écran intermédiaire qu'un clic franchit ne protège rien.
      **Ce qui n'est pas tranché** : ce qu'AC met dans son checksum en ligne.
      La tenue ne demande qu'un `skin.ini`, fichier de skin — probablement sûr,
      non prouvé ; le corps ne touche qu'`ext_config.ini`, hors `data.acd`.
      Deux mesures à garder en tête : les trois voitures dont la `.knh` est
      vide retombent sur `DRIVEREYES`, et `[MODEL] POSITION` ne doit **pas**
      être appliqué (voir `seating_offset`).
- [ ] **Aperçu 3D natif des voitures** (fusionné dans `main` ; la branche
      d'origine `feature/3dpreview` ne subsiste que sur `origin`).
      **L'avancement détaillé, les écarts assumés vis-à-vis de la spec et le
      reste à faire sont dans `docs/SPEC-preview-3d-kn5.md` §13 à §15** — c'est
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
      Reste surtout le choix du LOD — §15.
      Le réglage de qualité se réduit au suréchantillonnage : une passe SMAA
      a été essayée, déplacée, puis retirée faute de gain visible pour un
      gigaoctet de mémoire. Il subsiste du crénelage sur les lignes claires
      quasi horizontales ; le prochain essai est **en amont** (normales,
      rugosité), pas un filtre de plus. §15 point 8.
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
      Spec : `docs/SPEC-wikipedia-fiche-detail.md`, et son §1 commande tout —
      la fonctionnalité est **décorative**, donc l'ambiguïté n'affiche rien et
      l'absence n'est jamais une erreur. Son §2 est juridique et non
      négociable : le texte reste une **collection** (jamais fusionné à la
      description, jamais reformulé, résumé ni traduit — surtout pas par un
      modèle de langage), sinon le ShareAlike de CC BY-SA remonte sur l'app.
      **Fait : les lots 1 à 3, sans interface** — `src-tauri/src/wiki/` : les
      trois tables dans l'overlay (§3), la chaîne de repli et la remontée d'un
      cran (§5), le client Action API (§6), et l'appariement automatique des
      voitures et des circuits (§4) avec sa commande de calibration.
      Le module porte encore un `allow(dead_code)`, mais **plus pour la raison
      qui l'a fait poser** : il datait du temps où rien n'appelait le module.
      Mesuré en le retirant, il masque aujourd'hui **13 warnings de deux
      natures** — le banc de calibration, mort dans une compilation de lib
      puisque seuls des tests `#[ignore]` l'atteignent (couverture légitime),
      et **sept éléments réellement inutilisés** (`ids::ROUTE_TYPES`,
      `ids::COORDINATE_LOCATION`, `api::parse_search`, `WikiClient::search`,
      `CachedArticle::langs`…). Le restreindre à `calibrate` et trancher ces
      sept-là un par un est un chantier à part : certains sont des restes,
      et au moins un (`ROUTE_TYPES`) **porte une décision** — les routes ne
      s'apparient pas automatiquement — qu'une suppression effacerait.
      **Les identifiants Wikidata sont dans `wiki/ids.rs`**, un par un relevés
      sur l'API vivante (§4.4 l'exige) — le libellé en commentaire est celui
      que l'API a rendu, et chaque entrée dit sur quel item réel elle a été
      confirmée. Ne pas en ajouter de mémoire.
      **Les seuils sont dans `Prefs`** (`wiki_match_*`, `wiki_track_*`) et la
      liste de nettoyage des noms dans `rules/wiki-matching.json`, semée dans
      le dossier de config et éditable — §4.3 l'exige, et c'est ce qui permet
      de régler la reconnaissance sans release.
      **Pour calibrer** (rien n'est persisté, le rapport sort en Markdown) :
      ```
      PITBOX_WIKI_LIMIT=20 cargo test --lib wiki -- --ignored --nocapture calibrate_the_library
      ```
      Quatre mesures ont corrigé la spec, et elles ne se retrouvent pas deux
      fois :
      - **La recherche géographique des circuits tourne sur Wikidata, pas sur
        Wikipédia.** L'article anglais « Nürburgring » n'a *aucune* coordonnée
        GeoData (Suzuka non plus) : le `list=geosearch` de la §4.2 ne peut
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
        l'exécutant pas (§11 : aucun test ne dépend de Wikipédia).
      - **Un 404 et un réseau coupé ne sont pas le même non-résultat**
        (`api::Fetched`). Les confondre écrirait « pas d'article » dans le
        cache négatif pour 90 jours à cause d'un tunnel.
      **La calibration a tourné** (335 mods) et les seuils livrés sont les
      siens, plus ceux du §13. Elle a corrigé quatre choses que le raisonnement
      n'aurait pas trouvées, toutes consignées dans le code :
      - `wbsearchentities` **cherche par préfixe de libellé** : « BMW M3 E30 »
        n'y rend *rien*, aucun item ne s'appelant ainsi. C'était la cause
        dominante des 264 échecs du premier passage. La recherche passe
        désormais par le moteur plein texte de Wikipédia, qui rend « BMW M3 »
        en tête — et « Abarth 500 » pour une variante sans article à elle, ce
        que la §4.1 veut explicitement.
      - **`gsradius` est plafonné à 10 km par l'API**, qui refuse la requête
        entière au-delà. Un rayon de 25 km a transformé *les 24 circuits* en
        « réseau indisponible » d'un coup — c'est à ça que ressemble une panne
        systématique à côté d'une vraie coupure.
      - **Les routes ne s'apparient plus automatiquement** (écart assumé avec
        la §4.2) : une rue est à portée de n'importe quelle coordonnée, et le
        nom ne peut pas arbitrer puisque la spec a choisi les coordonnées
        *parce que* « Shutoko » ne ressemble pas à « Metropolitan Expressway ».
        Quatre articles faux pour une poignée de justes. Shutoko et les touge
        relèvent désormais de la correction manuelle (§7.6).
      - **Un item sans libellé anglais revenait sans nom** et marquait 0 contre
        tout — d'où `borrow_labels`, qui reprend le titre trouvé par la
        recherche.
      Résultat : 15 circuits retenus, tous justes (Monza retrouvé par le repli
      sur le nom, ses coordonnées CSP étant celles de Milan), contre 0 avant.
      **Les six lots du §12 sont faits.** L'onglet vit dans la fiche
      (voitures et circuits), la correction manuelle y est, et
      `Réglages › Wikipédia` porte l'interrupteur, la langue, la purge du cache
      et l'export des corrections.
      **Quatre écarts assumés avec la spec, tous décidés avec l'utilisateur
      après l'avoir vu à l'écran** — ils sont écrits dans le SPEC de la
      fonctionnalité, pas seulement ici :
      - **L'onglet est permanent** (contre la §7.1). Un onglet absent ne se
        distingue ni d'une recherche en cours, ni d'une fonctionnalité qui
        n'existe pas — constaté en vrai, sur un circuit qui s'appariait
        pendant qu'on regardait la fiche. Il porte donc six états, dont aucun
        n'est une erreur, et la correction manuelle avec eux : la §7.6
        l'accrochait à un onglet qui n'existait pas dans le seul cas où elle
        sert.
      - **L'article entier et rendu** (contre la §7.3, qui n'en voulait que
        l'introduction en texte brut) : sections, sommaire, tableaux, infobox.
        Le HTML n'est jamais injecté tel quel — `wikiHtml.ts` **reconstruit**
        un arbre depuis une liste blanche, la webview ayant accès à `invoke`.
        Aucune dépendance ajoutée pour ça.
      - **Les images sont affichées** (contre la §9). Ses trois objections
        étaient exactes et sont traitées, pas contournées : **Commons
        uniquement** (`imagerepository == "shared"`), ce qui écarte
        structurellement l'usage loyal puisque Commons n'accepte que du libre ;
        auteur et licence sous chaque image, non masquables.
      - **Le mot « extrait » quitte l'attribution** (§7.4) : il était exigé
        parce que ne montrer qu'un fragment est une modification. Montrer le
        texte entier est le régime **plus simple**, pas plus risqué.
      **Reste deux choses** : le ménage du `allow(dead_code)` ci-dessus, et
      surtout — **régler les seuils sur les corrections manuelles.** Tout le code est livré ; ce qui manque est une **mesure**,
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
         laisser passer de faux — la §1 échange volontiers du rappel contre de
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
      ajouté deux choses : le **crédit avec son lien Commons** (§9 : sans lui
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
- [ ] **Refonte de la navigation et des fiches** (premier palier fusionné dans
      `main`). Rail à deux rangs, inventaire unique des
      compléments, une seule anatomie de fiche, notes sur toutes les entités.
      **Spec, maquette et plan de livraison dans `docs/`** —
      `PLAN-refonte-navigation.md` porte l'ordre des lots et, surtout, les
      **mesures faites sur la bibliothèque réelle avant de commencer** : elles
      ont supprimé un lot entier (la détection CSP des voitures existe déjà,
      167 sur 311) et démenti le fourre-tout redouté (19 des 28 mods « autres »
      sont des mannequins). **Les neuf lots sont faits** (L1 à L9), le premier
      palier est fusionné dans `main`, le second reste à fusionner. Le détail
      lot par lot, avec ses écarts assumés, est dans le plan — ne pas le
      recopier ici.
      **Quatre points de la spec sont tombés à la mesure** plutôt qu'en
      implémentation, et c'est le genre d'information qu'on ne retrouve pas
      deux fois : la détection CSP des voitures existait déjà (167 sur 311),
      l'export des notes n'a nulle part où aller (il n'existe aucun export des
      métadonnées de l'overlay), la valeur d'origine des unités est déjà la
      seule affichée (rien n'est converti), et le retrait des mannequins de
      l'inventaire — pourtant demandé par le §5 — s'est révélé **nuisible** :
      la galerie de l'écran Pilote est un sélecteur, elle ne gère rien, et les
      en retirer supprimait le seul endroit d'où on pouvait les désactiver ou
      les supprimer. Les livrées tranchent par l'exemple : **choisir et gérer
      sont deux gestes, ils peuvent avoir deux écrans.**
      **Reste** : « aussi dans … » (§4.5, un mod rattaché à plusieurs entités),
      les points ouverts §14 à reposer avec l'inventaire réel sous les yeux, et
      le markdown dans les notes (demandé à l'usage, voir le §4bis du plan).
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
