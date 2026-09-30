# Rejoindre un serveur online via Content Manager — essai préalable

> Mené le 2026-09-30, avant d'implémenter `SPEC-play-online.md`. Deux sources :
> le code de CM (`gro-ove/actools`, branche master : `ArgumentsHandler.Race.cs`,
> `ServerEntry.Join.cs`, `ServerEntry.cs`, `Game.Properties.cs`), puis six
> lancements réels sur des serveurs publics, lus dans `race.ini`, le `log.txt`
> d'Assetto Corsa et le journal de CSP.

## Verdict

**Oui, on peut rejoindre un serveur avec la voiture choisie dans Pit Box**, par
`acmanager://race/online`, à une condition sur les serveurs AssettoServer (la
majorité du lobby) : réinjecter `[REMOTE] __FEATURES` dans `race.ini` après
l'écriture de CM, exactement comme on réinjecte déjà le skin en Quick Drive
(`raceini.rs`). Le skin, lui, n'est pas un choix : c'est le serveur qui
l'impose.

## Deux routes, une seule prend la voiture

| Route | Paramètres | Ce qu'elle fait |
| --- | --- | --- |
| `race/online/join` | `ip`, `httpPort`, `password`/`plainPassword` | Ouvre la fiche serveur de CM. **Le choix de la voiture se refait dans CM**, puis clic sur Join. |
| `race/online` | `ip`, `port` (TCP), `httpPort`, **`car`** (requis) ; `skin`, `track`, `name`, `nationality`, `password`/`plainPassword`, `allowWithoutSteamId` | Lance le jeu directement par `GameWrapper.StartAsync`, sans fiche ni clic. `REQUESTED_CAR` = la voiture passée. |

`race/online` passe par le même pipeline que Quick Drive (`BasicProperties`
peuplé) : le chargement auto CSP se déclenche bien (« Auto-loading stuff for
`la_canyons/freeroam` », puis pour la voiture), vérifié dans le journal de CM.

Mot de passe : `plainPassword` en clair, ou `password` chiffré (format des liens
de partage CM, déchiffré par `OnlineServer.DecryptSharedPassword`).

## Ce que `race/online` ne fait pas et que le Join natif fait

Comparaison de `ProcessRaceOnline` avec `ServerEntry.Join` :

| Le Join natif… | `race/online` | Conséquence |
| --- | --- | --- |
| écrit `[REMOTE] __FEATURES` (fonctions déclarées par le serveur dans `/JSON`) | ne l'écrit pas | **Refus sur AssettoServer**, voir plus bas |
| écrit `__CM_EXTENDED=1` et la météo quand le serveur a des détails étendus | `0`, pas de météo | Non mesuré ; la météo est de toute façon synchronisée par le serveur |
| applique les assistances imposées par le serveur (`/api/details`) | garde celles de l'UI de CM | Le serveur impose de toute façon ABS/TC autorisés |
| gère le mode booking (`TryToBookAsync`) | rien | Serveurs en booking : passer par `race/online/join` |
| écrit `SERVER_NAME` | non | Cosmétique |
| rattrape la casse de l'identifiant voiture (`GetCorrectId`) | non | Pit Box doit envoyer l'identifiant tel que le serveur l'écrit |
| enregistre le serveur dans les récents de CM | non | Les récents de Pit Box sont les siens (`SPEC-play-online.md`) |

## Les essais

| # | Serveur | Envoi | Résultat |
| --- | --- | --- | --- |
| 1 | AssettoServer A (LA Canyons) | `race/online`, Miata, skin demandé | `ACP_AUTH_FAILED` |
| 2 | même | + `__FEATURES` réinjecté | `ACP_AUTH_FAILED`, ticket Steam pourtant envoyé |
| 3 | Kunos classique (Nordschleife, 18 modèles) | `race/online`, Celica ST185 (dernier modèle de la liste), skin `02_racing_2` | **Connecté, dans la Celica** ; skin chargé `00_racing_3` |
| 4 | AssettoServer B (LA Canyons) | `race/online`, M3 E30 Drift | `ACP_AUTH_FAILED`, **aucun** ticket envoyé |
| 5 | même | + `__FEATURES` réinjecté | **Connecté, dans la M3 E30 Drift** |
| 6 | AssettoServer A | sans skin, + `__FEATURES`, + `name` | `ACP_AUTH_FAILED` |

**`__FEATURES` est nécessaire sur AssettoServer.** Les serveurs AssettoServer
déclarent `STEAM_TICKET` dans leurs fonctions ; CSP n'attache le ticket Steam à
la poignée de main que si `race.ini` le lui annonce. Sans lui, refus immédiat
(essai 4) ; avec lui, connexion (essai 5). Réinjection mesurée : CM écrit
`race.ini` ~0,5 s après l'URI, le patch passe 50 ms plus tard, avant que
`acs.exe` ne le lise.

**Le serveur A reste inexpliqué.** Ticket envoyé, CSP au-dessus du minimum
annoncé (build 4157 pour 3465 exigés), pas de skin, rien dans sa description
qui annonce une restriction. Le Join natif n'a pas pu être essayé en référence
(voir plus bas). Hypothèse la plus probable : une restriction propre à ce
serveur (liste blanche, plugin), à confirmer par un Join natif.

## Ce qu'on a appris au passage

- **Le skin est imposé par le serveur.** Le `SKIN=` écrit par CM est ignoré :
  le jeu charge celui du slot libre que le serveur attribue (essai 3). CM
  lui-même passe `AvailableSkin`, le skin du premier slot libre du modèle.
  L'UI doit donc *afficher* le skin du slot, pas le proposer.
- **Le paramètre `name` n'a pas d'effet** : `race.ini` garde le pseudo des
  réglages de CM. C'est aussi ce que ferait le Join natif — rien à faire.
- **Le champ `track` d'un serveur CSP est encodé** :
  `csp/<build CSP minimal>/../[<drapeaux>/../]<circuit>-<layout>`
  (`csp/3465/../E/../la_canyons-freeroam`). CM découpe sur `/../`
  (`ServerEntry.cs`) : le dernier morceau est le circuit, le premier la version
  CSP exigée, le morceau du milieu les drapeaux (physique étendue voitures,
  circuit, glace). À passer **nettoyé** dans `track=`, sinon CM répond « Track
  is missing ». Sur 9 000 serveurs du lobby, 5 417 portent ce préfixe.
- **Le séparateur circuit/layout est un tiret**, avec l'ambiguïté que CM
  signale lui-même (`trento-bondone` est un circuit) : `GetLayoutByKunosId`
  essaie l'identifiant entier, puis chaque tiret en partant de la droite.
- **Le lobby Kunos exige `User-Agent: Assetto Corsa Launcher`** — sans lui,
  une redirection 302 vers la racine. `lobby.ashx/list?guid=<SteamID>` rend
  un tableau JSON (9 000 serveurs, ~7,8 Mo), un objet par serveur au format
  `/INFO`.
- **CM a refusé son propre Join sur le serveur B** (« Required content is
  missing », forçable par Ctrl+Alt+G) alors que ses 75 voitures et le circuit
  sont installés — et le serveur a accepté la connexion par `race/online`
  (essai 5). Cause non identifiée côté CM (version de contenu déclarée par le
  serveur ?). À retenir : **le verdict « contenu manquant » de CM n'est pas
  celui du serveur**, et Pit Box ne doit pas le recopier sans comprendre ce
  qu'il vérifie.
- **`find_cm` ne trouve pas une install de CM hors des deux emplacements
  connus** (dossier du jeu, `%LOCALAPPDATA%`). Le protocole `acmanager://` est
  pourtant enregistré dans le registre (`HKCR\acmanager\shell\open\command`),
  qui donne le chemin réel. Hors périmètre de cet essai, signalé.

## Ce qui n'a pas été vérifié

- Le Join natif sur le serveur A (référence qui trancherait son refus).
- Un serveur à mot de passe, un serveur en mode booking.
- L'effet de `__CM_EXTENDED` et de la météo absents sur un serveur à détails
  étendus : la connexion passe (essai 5), l'effet en jeu n'a pas été regardé.
