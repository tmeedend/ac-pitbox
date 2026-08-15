# Choix du périphérique de contrôle — conception

Remplace le mode automatique de `docs/SPEC.md` §7.4. Document de travail :
une fois implémenté, §7.4 est réécrit à partir d'ici.

## 1. Principe

**Bug réel constaté** : des éléments d'interface se déplaçaient seuls, volant
branché ; désactiver le volant a fait disparaître le problème.

`mapping === "standard"` est **déclaré par le périphérique**, pas vérifié. Un
volant en « mode Xbox » ou derrière un adaptateur XInput s'annonce standard :
`gamepadNav` applique alors le layout Xbox, où l'axe 1 est « haut/bas » — sur un
volant, c'est une pédale. Effleurer le frein fait défiler le focus, et rien à
l'écran ne l'explique.

> **Un périphérique ne pilote l'interface que si l'utilisateur l'a désigné, une
> fois, explicitement. Sans réponse, il ne pilote rien.**

Deux corollaires :

- **Défaut fermé.** Un périphérique muet se diagnostique (Réglages le dit) ; un
  focus qui dérive n'a aucun recours évident. Dans le doute, on n'active pas.
- **Une seule question par périphérique, jamais deux.** Une sollicitation qui
  revient à chaque rebranchement sera cliquée au hasard — pire que le mode auto.

⚠ **Le consentement ne suffit pas à corriger le bug ci-dessus.** Si le
périphérique fautif est la base Fanatec — déjà présente dans
`DEVICE_OVERRIDES` — l'utilisateur l'aurait de toute façon désignée. La cause
est alors un **axe hors repos**, pas un mauvais choix de périphérique : c'est la
règle « repos mesuré + retour au neutre » (§4) qui la corrige. Les deux
chantiers sont complémentaires, ne pas croire que le premier règle le second.

## 2. Un seul déclencheur

Démarrage, branchement à chaud et première installation sont **le même
événement** : *un périphérique visible sans décision enregistrée*. Un seul
chemin de code.

Pas d'étape dédiée dans le `SetupWizard` : à la première installation personne
n'a encore touché son volant depuis le lancement, la liste serait donc **vide**
(§4) et l'étape ressemblerait à un écran cassé pour qui n'a pas de volant. Le
bandeau fera le travail plus tard, au moment où l'utilisateur touche réellement
son matériel.

La décision est **par périphérique**, pas globale. « Aucun volant déjà
sélectionné » comme condition de déclenchement serait faux : un utilisateur qui
a choisi sa manette et branche ensuite un volant doit être interrogé sur le
volant.

## 3. Identité et persistance

`Gamepad.index` est un **slot réattribué au débranchement** : jamais persisté,
jamais utilisé comme clé.

```ts
// "0eb7:0e04" si VID/PID présents, sinon l'id brut normalisé.
function deviceKey(id: string): string {
  const m = /Vendor:\s*([0-9a-f]{4})\s+Product:\s*([0-9a-f]{4})/i.exec(id);
  return m ? `${m[1]}:${m[2]}`.toLowerCase() : id.trim().toLowerCase();
}
```

Deux manettes XInput identiques partagent le même `id`, donc la même clé :
indiscernables, et sans conséquence — la décision porte sur le modèle. Ne pas
chercher à les séparer.

Un volant se présente souvent sur **plusieurs entrées `Gamepad`** (base +
boîtier de boutons, parfois des PID différents). Adopter l'une marque ses sœurs
— même préfixe constructeur/modèle — répondues et adoptées, sans redemander.
Même logique de sous-chaîne que `DEVICE_OVERRIDES`.

```ts
type Direction = "up" | "down" | "left" | "right";

/** Une liaison = un bouton, ou une position d'axe. Hat, stick et boutons
 *  se réduisent au même modèle — pas d'enum de famille. */
type Binding =
  | { kind: "button"; index: number }
  | { kind: "axis"; hint: number; mode: "equals" | "beyond"; value: number };

type NavProfile = {
  dirs: Partial<Record<Direction, Binding>>;
  confirm?: Binding;
  back?: Binding;
  rest: { axes: number[]; buttons: boolean[] };  // mesuré, jamais supposé — §4
};

type DeviceRecord = {
  key: string;
  label: string;        // Gamepad.id brut, pour réafficher un nom lisible
  use: boolean;         // absence d'entrée = jamais demandé
  profile?: NavProfile;
  answeredAt: string;
};
```

**`ui_prefs.json` via `uiPrefs.svelte.ts`**, clé `pitbox.gamepad.devices`
(règle d'or n°6). Lecture dans la boucle `requestAnimationFrame` par
`peekUiPref`, jamais l'API asynchrone. Le coupe-circuit global reste séparé
(`pitbox.gamepad.enabled`) : le couper ne doit pas effacer les décisions.

Résolution du profil, dans l'ordre : `record.profile` (calibré ici, gagne
toujours) → `DEVICE_OVERRIDES` (livré) → layout standard si
`mapping === "standard"` → rien, périphérique inerte.

**Migration** de `pitbox.gamepadNav.mode`, relu une dernière fois puis retiré :
`off` → coupe-circuit à `false` ; `forced:<id>` → ce périphérique adopté sans
rien demander ; `auto` → aucune décision, le bandeau apparaîtra. Ce dernier cas
coûte un clic aux utilisateurs actuels de manette : à dire dans les notes de
version.

## 4. Pièges du Gamepad API sous WebView2

Chacun de ces points, ignoré, produit un bug qu'on met une soirée à comprendre.

- **Un périphérique n'existe pas tant qu'on ne l'a pas touché.** Chromium ne
  l'expose qu'après une première entrée dessus (anti-fingerprinting). Donc :
  `getGamepads()` peut être vide au démarrage volant branché et allumé ;
  `gamepadconnected` se déclenche à la première pression, pas au branchement ;
  et **toute liste vide affiche « appuyez sur un bouton pour qu'il apparaisse
  ici »**, jamais « aucun périphérique détecté » — sans quoi l'écran paraît
  cassé. La liste ne se fige jamais, y compris panneau ouvert.
- **Le repos n'est pas zéro.** Un hat DirectInput normalisé par Chromium repose
  *hors* de [-1, 1] (~3,2 observé), les pédales à -1, un volant là où on l'a
  laissé. Le repos se **mesure** (2 s au début de la calibration) et tout est
  exprimé en écart au repos.
- **Retour au neutre exigé** avant le premier événement, à l'adoption comme à
  chaque reconnexion : sinon une pédale enfoncée au branchement produit un
  « bas » permanent dès la première image. **C'est le correctif du bug de §1**,
  pas le consentement.
- **L'index d'un axe n'est pas stable** (déjà géré par `DEVICE_OVERRIDES`).
  `Binding.hint` est un indice de départ, la reconnaissance se fait par valeur
  sur tous les axes.
- `getGamepads()` renvoie un **instantané** avec des trous : le relire à chaque
  image, ne jamais garder une référence entre deux frames.
- `Gamepad.timestamp` ne bouge qu'au changement d'état → « dernière activité il
  y a X s », qui distingue seul « endormi » de « absent ».
- Hors focus fenêtre, `requestAnimationFrame` est suspendu : la navigation gèle.
  Attendu, mais à savoir avant de chasser un fantôme.

## 5. Bandeau, puis panneau

**Rien ne s'ouvre tout seul.** Un modal ne se justifie que si l'app ne peut pas
continuer sans réponse — ici elle le peut (défaut fermé, rien ne bouge). Et on
branche un volant *juste avant de lancer une session* : une popup arriverait
systématiquement au pire moment.

**Le bandeau** (dans `AppShell`, discret, en tête de zone de contenu) :

> 🎮 **2 nouveaux périphériques détectés.** [ Configurer ]  [ × ]

- **Persistant, pas un toast qui s'évanouit.** S'il disparaît tout seul,
  l'utilisateur qui n'a pas eu le temps de lire n'a plus aucun chemin évident.
- `[ × ]` = « plus tard » : ne répond rien, le bandeau reviendra au prochain
  démarrage. Il ne vaut jamais refus.
- Regroupement : après le premier `gamepadconnected`, attendre ~1 s et rouvrir
  la fenêtre à chaque apparition, pour que le décompte soit juste. Un rig
  complet (base, pédales, frein à main, boîte, boîtier) énumère six entrées en
  quelques centaines de ms.
- Masqué si le coupe-circuit global est à `false`.
- Aucune logique de report (AC qui tourne, import en cours, lancement de
  session) : un bandeau n'interrompt rien, c'est tout l'intérêt.

**Le panneau** — `ControllerSetup.svelte`, ouvert **au clic** sur `[Configurer]`
ou depuis Réglages. À ce stade l'utilisateur est disponible, un dialogue est
légitime.

- **Suspend la navigation manette tant qu'il est ouvert** — même mécanisme que
  `nav.lightboxOpen`, à généraliser plutôt qu'ajouter un drapeau parallèle. La
  zone de test consomme les entrées du périphérique calibré ; sans ça, « haut »
  validerait un bouton derrière le panneau.
- **Intégralement opérable à la souris et au clavier** : c'est un panneau au
  sujet d'un périphérique qui ne marche peut-être pas.

**Contenu** — une ligne par périphérique : nom, `VID:PID · n axes · n boutons`
en `.mono` discret, et un badge parmi trois : ✔ Reconnu (profil livré) /
🎮 Manette standard (présenté comme un *indice*, pas une garantie) / ⚠ Non
reconnu (jaune) + bouton `[Tester / calibrer]`.

- **Sélection unique (radio)**, pas des cases : la question est « lequel
  utiliser ». Les lignes non retenues sont marquées répondues et ne reviendront
  plus — le rig complet se règle en un geste. Le modèle autorise plusieurs
  `use: true` ; c'est Réglages qui permet d'en ajouter.
- **Aucune sélection par défaut**, même pour un périphérique reconnu : personne
  ne valide par réflexe un panneau qui contient déjà une réponse.
- **« Fermer » ≠ « Aucun pour l'instant ».** Le premier ne répond rien (le
  bandeau revient), le second clôt le sujet. Les confondre donne soit une
  sollicitation qui harcèle, soit une décision prise par accident.

**Texte du ⚠ non reconnu** — c'est de l'aide sur *le matériel*, pas une légende
qui explique la mécanique de l'UI : la règle « un libellé n'explique jamais son
propre fonctionnement » ne s'y applique pas.

> **Ce modèle n'est pas encore connu de Pit Box.** Les volants n'ont aucune
> norme pour leur croix directionnelle : sans repères, Pit Box ne peut pas
> deviner quel bouton fait quoi. La calibration prend deux minutes et rend le
> volant utilisable immédiatement. Vous pourrez ensuite envoyer le profil
> obtenu à l'auteur, en un clic.

Ne **pas** écrire « envoyez-moi les codes des boutons » : on ne demande jamais à
un humain de transcrire des index — il se trompe, ou il ne le fait pas.

## 6. Calibration guidée

Étapes, une par écran, « Passer » et « Recommencer » toujours visibles :
`repos (2 s) → haut → bas → gauche → droite → valider → retour`.

Capture d'une étape :

- Comparer au `rest` de l'étape 0. Retenir le changement le plus marqué :
  bouton passé à `pressed`, ou axe écarté de plus de 0,3 de son repos.
- **150 ms de stabilité exigées** — sinon un rebond de contact ou une valeur
  intermédiaire d'axe analogique est enregistré à la place du geste.
- **Retour au repos exigé** avant l'étape suivante, sinon le même maintien est
  capté deux fois.
- **Doublon refusé** : « ce bouton est déjà utilisé pour *Haut* », on refait
  l'étape. Deux directions sur la même liaison est pire qu'un profil incomplet.
- **Timeout ~10 s** → réessayer / passer. Beaucoup de volants n'ont pas de
  croix : « Passer » est un chemin normal, pas un échec.

**Hat ou stick ?** Les deux sont un axe ; la différence se lit *pendant* la
capture — un stick traverse des valeurs intermédiaires, un hat saute d'une
valeur discrète à une autre. Intermédiaires observées **et** extrême atteint
(|v| ≥ 0,9) → `mode: "beyond"` (seuil, deadzone 0,5 contre les diagonales).
Saut direct → `mode: "equals"` (±0,1). Ce n'est pas cosmétique : un seuil
appliqué à un hat dont « haut » vaut -0,71 ne déclenche jamais rien.

**Écran final** : récapitulatif **plus une zone d'essai** — quatre cases où le
focus bouge réellement avec le profil construit. Lire « Haut → axe 9 = -1,00 »
ne prouve rien à un utilisateur.

**Envoi** : `[Copier le profil]` (JSON : id brut, VID/PID, nombre d'axes et de
boutons, `mapping`, les sept liaisons, le repos, la version de Pit Box) et
`[Ouvrir un ticket pré-rempli]` (`?title=…&body=…` encodé, l'utilisateur clique
Submit). Une ligne sur ce que contient le profil — modèle et index, rien
d'identifiant. Le bouton Copier suffit seul si GitHub est inaccessible.

Côté auteur : **`DEVICE_OVERRIDES` doit adopter le même format que
`NavProfile.dirs`**, sinon chaque contribution reçue demande une traduction
manuelle, donc une occasion de se tromper.

## 7. Réglages

Le tableau de diagnostic quitte Réglages (il devient un repli replié sous
« Détails techniques » dans le panneau, pour le cas où la calibration échoue).
Réglages garde ce qui est rattrapable :

- Coupe-circuit global.
- **Liste des périphériques connus**, y compris débranchés (label mémorisé,
  grisé) : utilisé ✔/✖, source du profil (calibré / livré / standard / aucun),
  et deux actions — basculer, et `[Calibrer]` qui rouvre le panneau.
- `[Oublier ce périphérique]` → repasse en « jamais demandé ». Le bouton « je me
  suis trompé », sans lequel une réponse erronée est définitive.

## 8. Fichiers

Tout est frontend : le Gamepad API est une API web, la persistance passe par
`uiPrefs`. **Aucun module Rust, aucune commande Tauri, aucune ligne dans
`invoke_handler!`.**

| Fichier | Rôle |
| --- | --- |
| `src/lib/gamepadDevices.svelte.ts` | Détection, décisions, persistance |
| `src/lib/gamepadProfile.ts` | **Logique pure** : `deviceKey`, écart au repos, capture, appariement |
| `src/lib/gamepadNav.ts` | Existant — résolution du profil (§3), retour au neutre (§4) |
| `src/lib/components/ControllerSetup.svelte` | Panneau (bandeau + Réglages) |

Clés i18n dans les **deux** locales, namespace structuré d'emblée
(`controller.banner.*`, `controller.panel.*`, `controller.calib.*`,
`controller.settings.*`, `controller.badge.*`). Titre de panneau aligné sur
`OpponentPicker` / `SavedSessionsDialog` (13 px/majuscules), pas un quatrième
niveau de libellé.

`gamepadProfile.ts` en fonctions pures sans dépendance Svelte : c'est le cas de
figure listé dans « Chantiers en cours » comme déclencheur éventuel d'un runner
de tests frontend. Ne pas l'introduire maintenant, juste ne pas fermer la porte.

## 9. Vérification

L'aperçu navigateur ne prouve rien (`invoke` absent, et le Gamepad API demande
du matériel réel). À repasser dans la vraie app :

1. Volant branché **avant** le lancement → n'apparaît qu'après la première
   pression, le bandeau arrive à ce moment-là.
2. Rig complet → **un seul** bandeau, décompte juste.
3. Débrancher/rebrancher un périphérique adopté → aucun bandeau.
4. **Pédale enfoncée / volant tourné à l'adoption → aucun déplacement de focus
   avant relâchement.** Reproduction du bug de §1.
5. **Calibrer la base Fanatec V2.5 à l'aveugle → doit retomber sur l'entrée
   `DEVICE_OVERRIDES` existante.** Le test de non-régression le plus parlant
   qu'on ait.
6. Manette standard → un clic, navigation identique à aujourd'hui.
7. `ui_prefs.json` avec les trois valeurs de `pitbox.gamepadNav.mode` → les
   trois états d'arrivée.

## 10. Découpage possible

Le gros du travail est §6. Si on veut livrer plus tôt : §1 à §5 en v1 — un
volant inconnu est simplement ignoré, proprement, en le disant — et §6 ensuite.
Le type `Binding` est conçu pour que la calibration s'y branche sans réécriture.

**Écarté** : un détecteur d'emballement (suspendre le périphérique après ~8 s de
direction maintenue). Seuil arbitraire, et le retour au neutre (§4) couvre déjà
la cause connue. À reconsidérer seulement si un dérapage est constaté *malgré*
le retour au neutre.
