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
Les binaires produits aujourd'hui ne sont **pas signés** — le workflow est prêt
à les signer, il attend une variable de dépôt.

## Activer la signature

Une seule chose à faire : définir la **variable de dépôt** `SIGN_COMMAND`
(*Settings → Secrets and variables → Actions → Variables*). Tant qu'elle est
vide, le workflow construit normalement, sans signer et sans échouer.

Sa valeur est la ligne de commande qui signe **un** fichier, où Tauri remplace
`%1` par le chemin du binaire. Par exemple, avec `signtool` :

```
signtool sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /sha1 <empreinte> %1
```

Avec Azure Trusted Signing ou SignPath, c'est la CLI du fournisseur qui prend
la place de `signtool`. Les identifiants (jeton, secret client) restent des
**secrets** GitHub classiques, référencés par la commande.

### Pourquoi `signCommand` plutôt qu'une étape de signature

C'est le point non évident, et il vaut d'être compris avant de bricoler le
workflow :

- Tauri appelle la commande **pendant** l'empaquetage. L'exécutable *à
  l'intérieur* de l'installateur est donc signé lui aussi. Signer seulement le
  `.msi`/`.exe` final laisserait le binaire réellement installé non signé —
  SmartScreen s'en apercevrait au premier lancement, après l'installation.
- `tauri-action` construit **et** publie la release en une seule étape : il
  n'existe aucune fenêtre entre « bundles produits » et « bundles téléversés »
  où une étape de signature pourrait s'intercaler.

C'est pourquoi `release.yml` injecte `bundle.windows.signCommand` dans
`tauri.conf.json` avant le build, plutôt que de signer après coup.
`tauri.conf.json` étant du JSON strict, la clé ne peut pas y être laissée en
place désactivée : elle est écrite par le workflow.

## Alternative sans certificat

Publier les sommes **SHA-256** des installateurs à côté de la release et
expliquer dans le README comment passer l'avertissement. Ça ne supprime pas
SmartScreen, mais ça donne aux utilisateurs de quoi vérifier ce qu'ils
téléchargent — c'est mieux que rien tant que le certificat n'est pas en place.
