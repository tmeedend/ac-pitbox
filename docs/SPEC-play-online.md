# PitBox — Page Online (serveurs multijoueur)

Ce que fait la page Online, et les règles qu'il faut connaître pour la
modifier sans la casser. Le lancement par Content Manager a été vérifié en jeu
avant de construire : `online-join-research.md`, qui porte aussi les relevés
du lobby et les pistes écartées. Ce qui reste à voir à l'écran est dans
`CHANTIERS.md`.

## Parti pris

La page répond à une seule question : **rejoindre vite le bon serveur, avec
tout ce qu'il faut.** Ce qui décide (circuit, remplissage, session, voitures)
est gros et visuel ; ce que Pit Box sait et que CM ne met pas en avant — le
contenu installé — juge chaque serveur ; les filtres sont des tokens, dans la
grammaire de la bibliothèque.

## Cas d'usage

| # | Cas | Ce que la page optimise |
| --- | --- | --- |
| 1 | **Rejoindre un serveur connu** : un ami, une communauté, celui d'hier | Favoris, Récents, Amis, coller un lien |
| 2 | **Pouvoir entrer** : contenu manquant, mauvaise version, CSP, DLC | un niveau de préparation lisible, et un bouton qui prépare tout |
| 3 | **Choisir une ambiance** : freeroam, drift, touge, course | filtre par catégorie de circuit |
| 4 | **Trouver du monde maintenant** | tri par joueurs, session en cours |
| 5 | **Rouler sur ses combos habituels** | tokens circuit et voiture |

La page s'organise autour des cas 1 et 2 ; les cas 3 à 5 sont des filtres.

## Ce qu'un serveur AC expose

| Niveau | Source | Disponible sur | Ce que Pit Box en tire |
| --- | --- | --- | --- |
| 1 · Lobby / INFO | lobby Kunos, `/INFO` du serveur | tous | nom, adresse, pays, joueurs, circuit et layout, voitures, mot de passe, sessions et temps restant |
| 2 · Entry list | `/JSON` | tous | un slot par voiture : modèle, skin imposé, pilote, connecté ou libre |
| 3 · Détails étendus | `/api/details` (wrapper de CM, AssettoServer) | serveurs qui l'activent | description, météo, températures, grip, vent, règles, **liens du contenu**, empreinte du mot de passe, CSP requis, versions |

La liste tient avec le niveau 1 seul ; le panneau exploite les niveaux 2 et 3
quand ils existent. **Le ping n'est pas fourni par le serveur** : Pit Box le
mesure (§ Liste des serveurs).

## Source de la liste

**Le lobby Kunos en direct.** Côté Rust (`online/lobby_cache.rs`), la liste
brute est téléchargée une fois et gardée trois minutes pour tous ses lecteurs ;
deux demandes simultanées n'en font qu'une, « Actualiser » passe outre. La page
Online la juge contre la bibliothèque et garde ce jugement trois minutes —
moins si la bibliothèque change (import, activation, contenu récupéré) : elle
juge alors à nouveau, **sans retélécharger**.

**Le repli est la dernière liste reçue**, écrite telle quelle dans le cache à
chaque réponse : quand le lobby ne répond pas, elle revient sous un bandeau
qui la date, et redemande au bout d'une minute. Les serveurs répondant à leur
`/INFO` et leur `/JSON` quoi que fasse le lobby, le panneau reste juste. La
fiche d'un circuit ne compte rien sur cette liste de secours. **Sans Steam,
pas de repli** : la raison s'affiche.

## Liste des serveurs

**Quatre onglets : `Tous` · `Favoris` · `Récents` · `Amis`**, avec leur
décompte ; l'onglet ouvert est mémorisé. **Coller un lien de connexion**
n'importe où sur la page (lien de partage CM `acstuff.club/s/q:race/online/join?…`,
forme `acmanager://`) ouvre son serveur, même absent du lobby ; un mot de passe
en clair dans le lien remplit le champ (celui, chiffré, de CM ne se lit pas).

**La liste est un tableau**, celui de la bibliothèque : colonnes choisies,
déplacées, redimensionnées, mémorisées (`library_columns.json`, clé `online`).
Colonnes : **Circuit** (fixe), Serveur, Session, Joueurs, Ping, Voitures, CSP,
Pays, État ; IP:port et Mot de passe en option. Le tableau est virtuel.

- **Circuit** : la vignette du layout (§ Vignette d'un circuit non installé), le
  nom du circuit tel que la bibliothèque le connaît, sous lui le nom lisible du
  layout (la répétition du nom du circuit ôtée), l'identifiant en infobulle.
- **Nom du serveur** : tronqué proprement, sans écraser le reste ; le lien
  Discord qu'il contient est rendu cliquable dans le panneau.
- **Voitures** : leurs noms de bibliothèque, **celles qu'on possède en
  premier**, `+N` toujours visible, la liste entière au survol. Les voitures
  sans aucun slot joueur (le trafic IA d'AssettoServer) ne sont pas listées.
- **Joueurs** : `14 / 24`, et la pastille `★ Léo` (ou `★ 3`) quand un ami est
  connecté.
- **Session** : le temps restant, « Ouvert » pour des essais de plus de 3 h,
  « Réservation » écrit une seule fois.
- **Ping** : une connexion TCP au port HTTP du serveur, mesurée sur les lignes
  visibles une fois le défilement arrêté, gardée cinq minutes ; vert sous
  60 ms, orange sous 120, gris au-delà — **jamais rouge**, réservé à la session
  (§7.2ter).
- **Pays** : le drapeau seul, le nom en infobulle (suivi du nom dans le
  panneau) ; un code inconnu du jeu reste écrit.
- **État** : discret quand tout est prêt ; `1 clic` en bleu, `À télécharger` en
  orange, `Bloqué` en jaune ; la cause au survol.

**La recherche** porte sur le nom du serveur et sur les noms de circuit et de
voitures tels que la bibliothèque les connaît.

**Tri par défaut** : serveurs avec amis, puis joignables (`Prêt`, puis
`1 clic`), puis joueurs décroissants. Un serveur vide mais prêt reste listé. Un
clic d'en-tête trie croissant, un deuxième décroissant, un troisième rend
l'ordre par défaut ; ce qui n'a pas de valeur va toujours en dernier.

**Regrouper par circuit** (menu d'affichage, sur `Tous`, mémorisé, éteint
d'usine) : une ligne par layout, avec le nombre de serveurs, les amis et le
total des joueurs ; les groupes viennent par joueurs. Trier sur une colonne à
agrégat (Circuit, Serveur, Joueurs) ordonne les groupes, sur une autre les
serveurs dans chaque groupe. Un layout tenu par un seul serveur garde sa ligne
ordinaire.

## Détail d'un serveur

**Un panneau à droite de la liste, toujours là**, pour que la liste ne change
pas de largeur. Il s'ouvre sur le dernier serveur rejoint, sinon sur le premier
de la liste ; venu d'une fiche circuit, sur le premier serveur de ce circuit ;
sans rien à montrer, il dit « Sélectionnez un serveur ». Dans l'ordre où l'on
décide :

1. **L'en-tête circuit** : bannière avec le tracé, nom et layout, drapeau,
   ping, joueurs ; l'étoile des favoris ; `⧉`, qui copie le **lien de partage
   de CM**, sans mot de passe.
2. **La frise des sessions**, lues comme CM les lit (secondes, sauf une course
   aux tours ; « +1 tour » pour une course au temps qui en ajoute un ; heures
   au-delà de deux), la session en cours encadrée avec le temps restant,
   `Arrêt obligatoire` / `Grille inversée` en pastilles.
3. **Le choix de la voiture**, le cœur de l'écran : une carte par modèle, avec
   la photo **du skin que le serveur imposera**, ses slots libres et son niveau
   de préparation. Une voiture complète est grisée mais se choisit (pour
   attendre une place, § v2). Les voitures sans place joueur s'affichent sous
   « Aussi nécessaires », avec leur niveau. Rouvrir un serveur des Récents
   présélectionne la voiture utilisée si elle est encore prenable.
4. **Les pilotes connectés**. Un clic sur un nom le marque **ami** (☆ → ★, en
   vert), un second le retire ; les noms se comparent sans casse ni espaces
   autour, comme CM.
5. **Conditions** (détails étendus) : météo telle que CSP la nomme, air, piste,
   grip, vent.
6. **Règles** (détails étendus), **en écarts** à ce qu'un serveur AC fait par
   défaut : ABS ou antipatinage interdit ou imposé, ESC, embrayage auto,
   couvertures, rétro virtuel, dégâts, consommation et usure s'ils ne sont pas
   à 100 %, roues hors piste tolérées.
7. **La description**, repliée, sans son BBCode ; les liens du nom et de la
   description (images exclues, `discord.gg/…` sans `https` compris) s'ouvrent
   dans le navigateur.

**Le bouton Rejoindre**, collé en bas, suit le niveau de la voiture choisie :
`Rejoindre`, `Préparer et rejoindre`, ou la raison qui empêche (« Circuit
manquant », « DLC requis : Red Pack », « CSP 3465 ou plus récent requis »,
« Voiture complète »). Un serveur en réservation ouvre la fiche de CM, qui
seule sait réserver.

- **Le mot de passe est vérifié avant le lancement** sur un serveur qui publie
  son empreinte (`passwordChecksum` des détails étendus) : faux, le panneau le
  dit au lieu de laisser le jeu charger pour se faire refuser. Le mot de passe
  joueur ou admin ouvre ; **un mot de passe vide n'est jamais accepté**. Sans
  empreinte, c'est le jeu qui vérifie.
- **Les serveurs qui exigent leur lanceur** : un serveur officiel No Hesi refuse
  qui n'est pas passé par le lanceur No Hesi. Le panneau le dit au-dessus du
  bouton, avec un lien, sans bloquer. Ces serveurs se reconnaissent au lien
  vers `nohesi.gg` dans leur description — ni à leur nom, ni à leurs voitures.
- **Une session à la fois** : le bouton affiche `Lancement…` puis `Session en
  cours` tant qu'Assetto Corsa tourne (SESSION§2.4).

**Le lancement** passe par CM : `acmanager://race/online?ip=…&port=…&httpPort=…&car=…&track=…`,
qui part directement avec la voiture choisie, plus la réinjection de `[REMOTE]
__FEATURES` dans `race.ini` sur AssettoServer. `race/online/join` ne sert
qu'à la réservation. La livrée n'est pas transmise : le serveur impose celle
du slot. Un Rejoindre réussi ajoute le serveur aux **Récents**, avec la voiture
utilisée.

## Contenu manquant : prêt à rejoindre

Pour chaque voiture et pour le circuit, Pit Box sait si l'utilisateur l'a, sous
quelle forme, et comment l'obtenir.

| Ce que Pit Box trouve | Ce qu'il fait | Niveau |
| --- | --- | --- |
| Installé dans le jeu | rien | `Prêt` |
| En bibliothèque, désactivé | Rejoindre l'active d'abord | `1 clic` |
| En vitrine, ou absent, **avec une source** | Préparer et rejoindre le récupère | `1 clic` |
| En vitrine ou absent, sans source directe | le panneau dit où chercher | `À télécharger` |
| Contenu Kunos d'un DLC non possédé | nomme le DLC | `Bloqué` |
| CSP absent ou plus ancien que celui qu'exige le serveur | le dit | `Bloqué` |

**Voitures nécessaires : toutes**, celles de la liste d'engagés — le jeu les
charge toutes, une seule manquante ferme la porte, quelle que soit celle qu'on
conduit, trafic IA compris. **Le niveau d'un serveur est le pire** de son
circuit, de toutes ses voitures et du CSP. La bascule « Joignable » garde
`Prêt` et `1 clic`.

**Les sources**, dans l'ordre : l'archive gardée à l'import d'un mod en
vitrine ; le lien du serveur — le bloc `content` des détails étendus, une `url`
par voiture et pour le circuit, ou sans `url` le fichier servi par le serveur
lui-même (`/content/car/<id>`, `/content/track`) ; une `url` vide ou
`direct: false` ne vaut rien, `cup: true` renvoie au registre — ; enfin le
**registre CUP**, quand il liste l'id sans le réserver (`limited`). Le contenu
servi par un serveur à mot de passe n'est pas proposé.

**Préparer et rejoindre** récupère tout ce qui manque — la voiture choisie
d'abord — par les chemins existants : l'archive gardée comme la récupération
d'un mod en vitrine, un lien ou le registre comme une mise à jour de mod, puis
**l'import ordinaire** : le contenu arrive géré, **jamais posé tel quel dans
`content/`** (c'est ce que ferait le bouton « Install missing content » de CM,
contourné exprès). Toute la préparation se suit dans **une seule
notification**, le mod en cours et le décompte du tout, « Arrêter » coupant le
mod en cours et les suivants ; les notifications d'un seul mod s'effacent pendant
ce temps et leurs comptes rendus reviennent à la fin. Rejoindre pose ensuite
dans le jeu **toutes** les voitures nécessaires, pas seulement celle qu'on
conduit. Une page au lieu d'une archive s'ouvre dans le navigateur ; l'archive
glissée ensuite sur Pit Box fait relire le serveur.

**La liste ne montre que ce qu'elle sait sans détails étendus** : les sources
et les versions du serveur ne se lisent que dans le panneau, à l'ouverture.
Les interroger pour chaque ligne serait abusif envers les serveurs.

**Sans aucune source**, le panneau liste ce qui manque sous « Aucune source de
téléchargement connue », chacun avec « Chercher sur le web » (une recherche
DuckDuckGo sur « {id} assetto corsa », dans le navigateur). Jamais de
téléchargement depuis un résultat.

### Couches et versions

Le serveur contrôle `data.acd` (ou `data/`) des voitures et les `surfaces.ini`
du circuit ; le son seul (`sfx/`) passe.

| Cas | Ce que fait Pit Box | Ce que voit l'utilisateur |
| --- | --- | --- |
| **Échec certain** : une couche active de la voiture choisie ou du circuit remplace l'un de ces fichiers | la désactive pour la session, la réactive à la fermeture du jeu | une ligne au-dessus du bouton, « Garder ces couches » pour refuser ; le bouton devient `Préparer et rejoindre` |
| **Échec possible** : une couche active touche autre chose (hors `sfx/`) | rien par défaut | un encadré jaune qui la nomme, avec « Rejoindre sans ces couches » |
| Aucune couche | rien | rien |

**La réactivation ne dépend pas d'une fermeture propre.** La liste des couches
écartées est écrite (`online_layers.json`) **avant** la moindre désactivation.
La fin du jeu les réactive et le dit dans une notification ; si Pit Box a été
fermé entre-temps, c'est au démarrage suivant ; si CM ne lance aucun jeu, un
délai de garde les rend deux minutes après le join (un join plus récent
l'annule). Seules les couches de la voiture et du circuit rejoints sont
touchées, par l'interrupteur ordinaire.

**Versions à côté du choix.** Sous la voiture choisie et sous le circuit : la
version installée et l'archive d'origine ; quand le serveur déclare sa version,
elle s'affiche à côté, **en orange quand elles diffèrent**. Une version
installée plus ancienne que celle du serveur passe en `1 clic` quand une source
permet la mise à jour.

## Filtres de base

La barre de la bibliothèque (`FilterBar`), sur une ligne : la recherche,
`+ Filtre` et les puces.

- **Quatre puces oui/non épinglées**, à trois états : « Pas plein » (posée
  d'usine), « Sans mot de passe », « Pas vide », « Joignable ». Elles ne
  s'appliquent qu'à `Tous` : un serveur à soi reste listé plein, verrouillé ou
  vide. Le négatif sert (« Avec mot de passe » retrouve le serveur de sa ligue).
- **Sept tokens** : circuit (le layout, avec le nombre de serveurs qui le font
  tourner), voiture (`ET` / `OU`), catégorie de circuit, contenu (`Kunos` :
  circuit et toutes les voitures officiels, DLC compris ; `Mods` : au moins un
  mod), ping (moins de 60, 120 ou 200 ms), pays, session en cours. Circuit,
  Voiture et Catégorie sont épinglés en fantômes. Chaque valeur s'inclut ou
  s'exclut ; les tokens valent pour tous les onglets.
- **Un serveur dont le ping n'est pas mesuré n'est pas « proche »** : poser un
  token de ping fait mesurer les serveurs qui passent les autres filtres, le
  haut de la liste d'abord, par lots de 64, et ils apparaissent à mesure ; un
  serveur qui ne répond pas reste écarté.
- Les tokens, les puces, les épingles et le tri sont mémorisés ; la recherche
  ne l'est pas. Un état enregistré par les anciennes cases à cocher se relit
  comme les puces équivalentes.

### Catégorie de circuit

La catégorie d'un serveur est celle de son circuit, résolue dans cet ordre, la
première source qui répond l'emportant :

1. **le choix de l'utilisateur**, dans le panneau (« Catégorie : inconnue ·
   Choisir », puis « Modifier ») : la liste fermée des catégories de circuit,
   chacune cochée ou décochée et enregistrée aussitôt, rangée dans
   `overlay.sqlite` par id de circuit. Il survit à l'installation du circuit et
   passe devant ce qu'elle apporte ; tout décocher l'oublie ; il voyage avec
   l'export de la bibliothèque ;
2. **la bibliothèque**, quand elle a le circuit avec au moins une catégorie ;
3. **le catalogue Pit Box** (`track_categories` de `taxonomy-catalog.json`, avec
   sa surcouche, REGLES§2).

**Puis les mots du serveur**, marqués « ≈ », qui **s'ajoutent** : un mot-clé du
nom du serveur, de sa description (dans le panneau seulement) ou de l'id du
circuit (`server_keywords` du même catalogue : `drift`, `touge`, `traffic`,
`cruise`, `freeroam`…). Mots entiers, sans casse ni accents ; un mot-clé
précédé de « no » est ignoré. L'infobulle dit la catégorie, la source et le mot.

**Une catégorie inconnue n'est ni incluse ni exclue par défaut** : une catégorie
incluse écarte un serveur inconnu, une catégorie exclue le garde. Ce qu'une
inclusion écarte ainsi se compte sous la liste (« + N serveurs sur des circuits
de catégorie inconnue · Afficher ») — une bascule d'affichage, pas un filtre ;
sous un token de ping, ces serveurs sont mesurés aussi.

## Vignette d'un circuit non installé

L'image d'un circuit, dans la liste et dans la bannière, se résout dans cet
ordre :

1. **la bibliothèque** : circuit installé ou en vitrine ;
2. **Wikimedia Commons**, par l'appariement Wikipédia des circuits (le lien
   livré de l'id, sinon les coordonnées et le nom que la table de CSP donne à
   cet id) ; l'image n'est gardée que si son auteur et sa licence sont connus,
   et son crédit est en infobulle sur la ligne et en clair sous la bannière. Le
   réglage `wiki_online` décide : coupé, seul le cache sert ;
3. **une vignette dessinée** : le pictogramme de la catégorie du circuit et le
   drapeau de son pays (Wikidata, quand l'élément est connu — jamais le pays du
   serveur). **Jamais l'image d'un autre circuit.**

Les images sont demandées une à une (le serveur ouvert d'abord, puis les lignes
visibles) et gardées dans le cache des miniatures ; un circuit sans image est
redemandé après trente jours. **Interdits** : embarquer des `preview.png` de
mods, chercher des images sur les sites de mods, l'affiche qu'un serveur publie
(elle est celle de sa communauté, pas du circuit).

Le **nom** d'un circuit non installé vient de la table de CSP (ou de sa copie
dans CM), son dossier en repli ; le nom de la bibliothèque passe toujours
avant.

## Au-delà de Content Manager

- **Favoris et Récents** : l'étoile met un serveur en favori ; les Récents en
  gardent vingt, un par serveur, le plus récent en tête. **Les deux listes
  gardent une copie du serveur** : elles s'affichent sans attendre le lobby et
  survivent à sa panne ; l'entrée fraîche du lobby remplace la copie dès
  qu'elle existe. Stockage : `online.json`, écrit côté Rust, compris dans la
  sauvegarde de démarrage.
- **Amis** : après chaque chargement de la liste, et seulement si un ami est
  marqué, Pit Box demande leurs pilotes aux serveurs qui ont des joueurs
  (32 requêtes à la fois). Un serveur où roule un ami porte sa pastille, passe
  en tête de `Tous` et forme l'onglet `Amis`.
- **« Online : N serveurs » sur la fiche d'un circuit** : une carte en bas de sa
  colonne de droite, comptée sur la liste brute sans requête aux serveurs, sur
  les seuls serveurs occupés, tous layouts confondus ; « Voir les serveurs »
  ouvre Online sur `Tous` avec le token Circuit posé. Lobby injoignable ou
  personne en piste : rien ne s'affiche.
- **Écarté** : les filtres de bibliothèque (tags, marque, famille) — aucun cas
  d'usage ne les demande.

## v2 — Prévenir : ami connecté, place libérée

Une surveillance démarrée avec la coquille, quel que soit l'écran ouvert. **Elle
ne regarde que ce que l'utilisateur a désigné**, jamais tout le lobby :

- **amis** : le `/JSON` des seuls serveurs en favoris et en récents, toutes les
  2 minutes. Le premier tour, 15 s après le démarrage, ne fait que relever qui
  est là. Un serveur qui ne répond pas garde ce qu'il disait : rien n'est
  annoncé sur lui tant qu'il n'a pas répondu deux fois. Un ami sur un serveur
  inconnu n'est pas vu ;
- **place libre** : seulement sur le serveur où l'utilisateur a cliqué « Me
  prévenir » (proposé quand le serveur est plein ou la voiture choisie
  complète), toutes les 30 s, par le seul `/JSON`. La place attendue est celle
  de la voiture choisie quand c'est elle qui manque de place, sinon n'importe
  laquelle. Elle s'arrête quand la place se libère, au bout d'une heure, ou au
  lancement du jeu ; un serveur à la fois ; fermer Pit Box l'arrête.

Elle se met en pause pendant une session et repart de zéro après. Une alerte
entre dans la pile de notifications, avec « Ouvrir », qui mène au panneau du
serveur ; quand la fenêtre n'a pas le focus, **Windows la montre aussi** — le clic sur la notification Windows n'ouvre pas
le serveur, c'est celle de la pile qui y mène.
Réglages › Général coupe toute la surveillance, « Me prévenir » compris.
