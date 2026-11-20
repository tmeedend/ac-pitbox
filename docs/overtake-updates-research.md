# Mises à jour des mods Overtake — recherche (lot 0)

Relevé du 2026-11-20, avant toute ligne de code. La question : Pit Box
peut-il savoir qu'un mod publié sur overtake.gg a une nouvelle version,
**sans jamais rien télécharger d'Overtake** et sans contourner quoi que ce
soit ? Il préviendrait, puis ouvrirait la page du mod dans le navigateur.

Cas réel qui a lancé la question : *Legends Car Coupe 2026 Italy*
(ressource `88197`), importé le 2026-11-16, version 1.5 publiée sur Overtake
le 2026-11-19 — rien signalé, le mod n'est pas au registre CUP (§4.7).

## Méthode

Toutes les requêtes passent par **WinHTTP** — l'objet COM
`WinHttp.WinHttpRequest.5.1`, même pile que `http.rs` (TLS du système,
proxy de l'utilisateur, redirections) — avec le User-Agent qu'aurait Pit Box,
`PitBox/<version> (+https://github.com/tmeedend/ac-pitbox)`, sans cookie,
espacées de 3 s. Une vingtaine de requêtes en tout. Le navigateur intégré n'a
servi qu'à **lire** ce que WinHTTP ne pouvait pas voir (les conditions
d'utilisation, et la page du mod pour recouper les dates) : jamais pour
fournir quoi que ce soit à l'app.

## 1. Une page de mod, sans connexion : inaccessible

`GET /downloads/legends-car-coupe-2026-italy.88197/` répond **200**, mais le
corps (4,4 Ko) est une page « Checking your browser » : `meta refresh` vers
`/.stile/challenge/verify?nojs=1&nonce=…`, un cookie `__stile_chal_nojs`, et
le script de défi Cloudflare (`/cdn-cgi/challenge-platform/…`). Sans
suivre les redirections, la même adresse répond **307** vers
`/.stile/challenge?rung=nojs&band=watch&…`.

C'est un **défi anti-robot**. Le passer demanderait de suivre le lien
`verify` en gardant le cookie, c'est-à-dire de se faire passer pour un
navigateur : exclu par la consigne, et par principe.

Ce qu'une page contient, lu dans le navigateur pour recouper :

| Où | Valeur pour 88197 |
| --- | --- |
| Titre, `.p-title-value .u-muted` | libellé de version `1.5` |
| JSON-LD `SoftwareApplication` | `datePublished` 2026-10-31T10:54:31+01:00, `dateModified` 2026-11-19T20:14:31+01:00 |
| `<time datetime=… data-time=…>` | dates de création et de chaque mise à jour |
| Onglet History | 1.0 à 1.5, la 1.5 « Yesterday at 20:14 » |

`dateModified` est exactement la date de sortie de la dernière version.
Mais rien de tout ça n'est lisible par WinHTTP.

## 2. Les flux : tous derrière le même défi

| Adresse | Résultat |
| --- | --- |
| `/downloads/index.rss` | défi |
| `/downloads/categories/assetto-corsa.7/index.rss` | défi |
| `/downloads/<slug>.88197/updates.rss`, `…/index.rss` | défi |
| `/forums/-/index.rss` | défi |
| `/`, `/downloads/` | défi |
| `/api/resources/88197/` | **400** `no_api_key_in_request` : l'API REST XenForo existe, elle demande une clé |
| **`/sitemap.xml`** | **200, XML, pas de défi** |

**Le sitemap est la seule porte ouverte**, et c'est aussi celle que le site
désigne lui-même aux robots (dernière ligne de `robots.txt`). Il se compose
d'un index (7 fichiers, régénéré chaque nuit, `lastmod` 05:39 UTC) et de
sept fichiers de 2 à 7 Mo. Les ressources (`/downloads/<slug>.<id>/`) sont
dans **les deux premiers**, par numéro croissant — 45 040 dans
`sitemap-1.xml` (jusqu'à `51925`), 32 835 dans `sitemap-2.xml` — **toutes
avec un `<lastmod>`** :

```xml
<url><loc>https://www.overtake.gg/downloads/legends-car-coupe-2026-italy.88197/</loc><lastmod>2026-11-19T19:14:31+00:00</lastmod></url>
```

`2026-11-19T19:14:31Z` = `dateModified` de la page = sortie de la 1.5. Les
voisines jamais mises à jour (`88199`, `88200`, `88201`) ont un `lastmod` du
jour de leur création : **le `lastmod` du sitemap est la date de dernière
mise à jour de la ressource**, pas une date de recrawl. Vérifié sur un
échantillon — en XenForo Resource Manager c'est le champ `last_update`, qui
bouge aussi sur un message de mise à jour publié sans fichier : faux positif
possible, à couvrir par « Ignorer ».

Ce que le sitemap **ne donne pas** : le libellé de version, l'auteur, la
catégorie. Seulement l'adresse et la date.

Coût, mesuré : index (1 Ko) + `sitemap-1.xml` (7,2 Mo) + `sitemap-2.xml`
(6,7 Mo), **1,3 Mo par fichier une fois compressé en gzip**, ce que WinHTTP
négocie déjà (`WINHTTP_OPTION_DECOMPRESSION` dans `http.rs`). Trois
requêtes par jour pour toute la bibliothèque, quelle que soit sa taille. Le
serveur répond `Cache-Control: private, no-cache` et Cloudflare ne le cache
pas (`cf-cache-status: DYNAMIC`) : chaque lecture va jusqu'à l'origine, qui
sert un fichier déjà construit. Pas de `ETag` ni de `Last-Modified` relevés
sur les fichiers : pas de requête conditionnelle possible.

## 3. Cloudflare

Tout ce qui est HTML ou RSS est défié pour une requête non navigateur
(`band=watch`, `rung=nojs`) ; `robots.txt`, `sitemap*.xml` et `/api/` ne le
sont pas. Le défi n'a pas été essayé avec un User-Agent de navigateur : ce
serait mentir, et Pit Box ne le fera pas, donc la réponse ne servirait à
rien.

## 4. robots.txt et conditions d'utilisation

`robots.txt` (`User-agent: *`) interdit `/account/`, `/attachments/`,
`/goto/`, `/posts/`, `/login/`, `/search/`, `/whats-new/`, `/misc/…`,
`/admin.php` ; **autorise tout le reste, `/downloads/` compris**, et annonce
`Sitemap: https://www.overtake.gg/sitemap.xml`. Sept robots d'indexation
nommés sont exclus en entier.

Les conditions (« Terms and rules », mises à jour le 2024-11-28) ne parlent
**ni de lecture automatisée, ni de robots, ni du sitemap**. Une phrase les
touche de près, dans la section 3 (Intellectual Property and Copyright) :
elle interdit le *deep-linking* vers « our resources and modding files »
sans autorisation explicite d'Overtake.

Elle vise vraisemblablement les liens directs vers les fichiers, qui
privent le site de la visite. Mais **lue à la lettre, « our resources »
couvre la page d'une ressource** — c'est-à-dire exactement le geste « Open
page » du lot 3, et le lien affiché sur la fiche au lot 1. Le sitemap, lui,
n'est concerné par aucune phrase.

## 5. Les anciennes adresses RaceDepartment

`racedepartment.com/downloads/<slug>.<id>/` redirige vers
`www.overtake.gg/downloads/<slug>.<id>/`, **même numéro** (vu dans le
navigateur : WinHTTP, lui, prend le défi dès `racedepartment.com`). Le
numéro de ressource est donc bien l'identité qui survit : au renommage du
slug comme au changement de domaine.

Dans la bibliothèque de test (347 voitures et circuits gérés), **21** ont
dans leur `ui_*.json` une adresse de ressource reconnaissable
(`racedepartment.com` 17, `overtake.gg` 6, dont deux adresses de profil),
124 n'en ont aucune, et le mod du cas réel n'a **pas** de champ `url`. La
saisie à la main (lot 1) n'est pas un confort : c'est le cas courant.

## Conclusion

- **Lire les pages ou les flux : impossible sans contourner un défi.** Écarté.
- **Le sitemap : autorisé par `robots.txt`, désigné aux robots, non défié,
  et il porte la bonne date.** Trois requêtes par jour (~2,6 Mo compressés)
  pour toute la bibliothèque, aucune page de mod lue, rien téléchargé que du
  XML. Comparaison par date contre `imported_at` de la version active : pour
  88197, 2026-11-19 > 2026-11-16, la 1.5 aurait été annoncée, et la 1.4 du
  13 novembre ne l'aurait pas été.
- **Ce qu'on perd** : le libellé de version Overtake (absent du sitemap).
  L'annonce dirait « mis à jour le 19 novembre », pas « 1.5 ».

**La condition d'arrêt n'est pas atteinte à la lettre** : il existe un
chemin que ni `robots.txt` ni les conditions ne refusent, et qui ne
contourne rien. **Mais deux points relèvent d'Overtake, pas de Pit Box**, et
justifient de leur poser la question avant de coder :

1. Le sitemap est fait pour les moteurs de recherche. Une app installée chez
   des milliers d'utilisateurs qui le lisent chaque jour, c'est un usage que
   le site n'a pas prévu, servi par l'origine sans cache.
2. La phrase « deep-linking to our resources » des conditions peut viser le
   simple lien vers la page d'un mod.

## Si le feu est vert — esquisse, pas encore une spec

- Une vérification = l'index, puis les fichiers dans l'ordre, arrêtée dès
  que tous les numéros suivis sont trouvés ou qu'un fichier ne contient plus
  aucune ressource. Lecture en flux (`http::get_url`, plafond relevé à
  16 Mo), extraction `numéro → lastmod` par une fonction pure testée sur un
  extrait réel du sitemap enregistré en fixture.
- Pas de suivi par mod, pas d'espacement à gérer : la même lecture vaut
  pour toute la bibliothèque.
- Un mod suivi absent du sitemap (ressource retirée, rendue privée) n'est
  pas une erreur : il n'est simplement pas annoncé.
- Une clé d'API XenForo, si Overtake en accorde une, remplacerait le sitemap
  par une requête ciblée et rendrait le libellé de version ; le reste du
  chantier (page d'un mod, annonce, « Open page ») ne changerait pas.
