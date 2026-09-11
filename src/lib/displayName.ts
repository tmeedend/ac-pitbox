// Le nom d'un mod tel qu'une carte de grille l'affiche (SPEC §7.4).
//
// **Purement cosmétique, et il faut que ça le reste.** Trois interdits, chacun
// protégeant un comportement que l'utilisateur tient pour acquis :
//
//  1. **le tri porte toujours sur le nom complet, marque comprise** — c'est
//     lui qui garde les Nissan groupées sous N et les Porsche sous P. Trier
//     sur le nom amputé disperserait la Skyline en S et la 350Z en 3, et
//     ferait perdre le regroupement par marque qu'on obtient aujourd'hui
//     gratuitement : le tri par nom complet EST le tri par marque ;
//  2. **l'index de recherche contient le nom complet** — taper « nissan
//     skyline » doit fonctionner même quand la carte affiche « Skyline GT-R
//     R34 » ;
//  3. **le nom stocké n'est jamais modifié** — le retrait se fait au rendu, à
//     partir du nom et de la marque. Aucune écriture en base, aucune
//     migration, réversible instantanément.
//
// D'où une fonction pure appelée à l'affichage, et rien d'autre.

/**
 * Autres façons d'écrire une marque, pour reconnaître le préfixe quand le nom
 * du mod ne l'orthographie pas comme le champ « marque ».
 *
 * La comparaison est déjà insensible à la casse et aux tirets (voir
 * [`normalise`]), donc `Mercedes-Benz` reconnaît « Mercedes Benz » sans qu'on
 * ait à l'écrire : ne sont listées ici que les formes qui ne s'en déduisent
 * pas, c'est-à-dire les abréviations et les noms courts.
 *
 * **La table reste courte exprès.** Un alias trop gourmand ampute une identité
 * au lieu d'une redondance : `AMG` seul transformerait « AMG GT » en « GT », et
 * un `BMW M` transformerait « BMW M3 E30 » en « 3 E30 ». Dans le doute, ne pas
 * ajouter — le défaut du réglage est de toute façon « garder la marque ».
 */
const ALIASES: Record<string, string[]> = {
  "alfa romeo": ["alfa"],
  "mercedes-benz": ["mercedes amg", "mercedes"],
  chevrolet: ["chevy"],
  volkswagen: ["vw"],
  "aston martin": ["aston"],
  "land rover": ["land"],
};

/**
 * La même table, indexée par marque **normalisée**.
 *
 * Les clés ci-dessus s'écrivent comme la marque s'écrit (`mercedes-benz`),
 * alors que la recherche se fait sur une marque passée par [`normalise`], qui
 * ne laisse jamais de tiret. L'entrée la plus utile de la table était donc
 * inatteignable : toutes les Mercedes gardaient leur marque sur la carte, et
 * « Mercedes AMG GT » ne tombait jamais sur son alias. Trouvé par le premier
 * test de `displayName.test.ts` — à l'œil, la table paraît juste.
 */
const ALIASES_BY_BRAND = new Map(Object.entries(ALIASES).map(([brand, forms]) => [normalise(brand), forms]));

/** Minuscules, tirets et underscores ramenés à l'espace, espaces resserrés. */
function normalise(text: string): string {
  return text
    .toLowerCase()
    .replace(/[-_]+/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

/**
 * Le nom privé de son préfixe de marque, ou le nom entier.
 *
 * Rend le nom tel quel dès que le retrait ne laisserait rien : « Ferrari »
 * tout court reste « Ferrari » — une carte sans libellé ne serait pas un gain
 * de densité mais une perte d'identité.
 */
export function withoutBrand(name: string, brand: string | null): string {
  if (!brand) return name;
  const wanted = normalise(brand);
  if (!wanted) return name;
  // Le plus long d'abord : « mercedes amg » avant « mercedes », sinon le second
  // gagne et laisse un « Mercedes AMG GT » amputé en « AMG GT ».
  const candidates = [wanted, ...(ALIASES_BY_BRAND.get(wanted) ?? [])].sort((a, b) => b.length - a.length);
  for (const candidate of candidates) {
    const rest = afterPrefix(name, candidate);
    if (rest) return rest;
  }
  return name;
}

/**
 * Ce qui suit `candidate` dans `name`, ou `null`.
 *
 * **La coupe ne tombe que sur une espace**, jamais sur un tiret, alors que la
 * *comparaison*, elle, ignore les tirets. Les deux règles sont nécessaires
 * ensemble : sans la première, la marque « Mercedes » couperait
 * « Mercedes-Benz SLS » en « Benz SLS » ; sans la seconde, la marque
 * « Mercedes-Benz » ne reconnaîtrait pas un mod nommé « Mercedes Benz SLS ».
 *
 * On essaie donc chaque espace du nom comme point de coupe, et on garde celui
 * dont la tête vaut le candidat une fois normalisée — ce qui compte les mots
 * sans jamais compter les caractères, dont le nombre diffère précisément quand
 * la ponctuation diverge.
 */
function afterPrefix(name: string, candidate: string): string | null {
  for (let i = 0; i < name.length; i++) {
    if (!/\s/.test(name[i])) continue;
    if (normalise(name.slice(0, i)) !== candidate) continue;
    const rest = name.slice(i + 1).trim();
    return rest || null;
  }
  return null;
}
