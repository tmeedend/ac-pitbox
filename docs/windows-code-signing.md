# Signature Authenticode de l'installateur Windows

Objectif : supprimer l'écran **SmartScreen « Éditeur inconnu »** au lancement de
l'installateur de Pit Box.

## À savoir avant d'acheter un certificat

Trois points qui changent la façon de s'y prendre, et qu'on découvre en général
trop tard.

**1. On ne peut plus mettre un `.pfx` dans les secrets de la CI.** Depuis le
1er juin 2023, le CA/Browser Forum impose que la clé privée d'un certificat de
signature de code publiquement reconnu vive dans du matériel certifié
(FIPS 140-2 niveau 2 ou équivalent) : jeton USB, ou HSM en ligne. Un fichier
`.pfx` téléchargeable n'existe plus pour ces certificats. Signer depuis un
runner GitHub hébergé impose donc un service de signature, ou un runner
auto-hébergé sur lequel le jeton USB est physiquement branché.

**2. Un certificat OV ne fait pas disparaître SmartScreen immédiatement.** La
réputation SmartScreen se construit avec le volume de téléchargements et le
temps. Un certificat **OV** (validation d'organisation, le moins cher) signe
correctement — l'éditeur devient identifié — mais l'avertissement peut persister
tant que la réputation n'est pas établie. Les certificats **EV** bénéficient
historiquement d'une réputation immédiate. C'est la vraie différence de prix.

**3. Pour un particulier, la validation est le point dur**, pas la technique.
L'autorité de certification vérifie une identité légale (entreprise
enregistrée, ou personne physique avec justificatifs). C'est ce qui prend le
plus de temps.

> Prix, paliers et critères d'éligibilité évoluent : les vérifier chez le
> fournisseur au moment de l'achat plutôt que de se fier à ce document.

## Les trois voies possibles

| Voie | Pour qui | En CI |
| --- | --- | --- |
| **Azure Trusted Signing** | Recommandée. Clé chez Azure, signature via API, abonnement mensuel modeste | Oui, nativement |
| **SignPath.io** | Projets open source (offre gratuite sous conditions) | Oui |
| **Certificat OV/EV + jeton USB** | Si on veut posséder le certificat | Runner auto-hébergé uniquement |

Pit Box étant open source, **SignPath** et **Azure Trusted Signing** sont les
deux pistes à instruire en premier.

## Ce qui est déjà en place

[`.github/workflows/release.yml`](../.github/workflows/release.yml) construit
les installateurs MSI et NSIS sur un tag `v*` et crée une release **brouillon**.
L'étape de signature Azure y est écrite mais **commentée** : les binaires
produits aujourd'hui ne sont pas signés.

## Activer la signature (Azure Trusted Signing)

1. Créer la ressource Trusted Signing dans Azure, faire valider l'identité,
   créer un profil de certificat.
2. Créer un enregistrement d'application (service principal) et lui donner le
   rôle de signature.
3. Renseigner les secrets du dépôt GitHub : `AZURE_TENANT_ID`,
   `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_ENDPOINT`,
   `AZURE_CODE_SIGNING_NAME`, `AZURE_CERT_PROFILE_NAME`.
4. Décommenter l'étape « Signature » dans `release.yml`.

⚠️ L'ordre compte : l'étape de signature doit s'exécuter **après** que
`tauri-action` a produit les bundles, sur les fichiers de
`src-tauri/target/release/bundle`. En l'état, l'étape commentée est placée
avant — la déplacer après le build, ou passer par `bundle.windows.signCommand`
dans `tauri.conf.json` pour que Tauri signe chaque binaire au moment de
l'empaquetage (préférable : l'exécutable *dans* l'installateur est signé lui
aussi, pas seulement l'installateur).

## Alternative sans certificat

Publier les sommes **SHA-256** des installateurs à côté de la release et
expliquer dans le README comment passer l'avertissement. Ça ne supprime pas
SmartScreen, mais ça donne aux utilisateurs de quoi vérifier ce qu'ils
téléchargent — c'est mieux que rien tant que le certificat n'est pas en place.
