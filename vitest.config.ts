import { defineConfig } from "vitest/config";
import { fileURLToPath } from "node:url";

// Runner de tests frontend — **logique pure uniquement**.
//
// Il était délibérément absent tant que tout vivait dans des composants : le
// risque réel est côté Rust, et tester du rendu Svelte aurait coûté trois
// dépendances pour vérifier ce qu'une relecture visuelle attrape mieux. Ce qui
// a changé, c'est la matière : `displayName.ts` et ses règles de coupe
// contradictoires, les conversions d'unités, le nom de couche dérivé — des
// fonctions pures dont les cas limites ne se vérifient qu'en les exécutant.
//
// D'où une config **séparée de `vite.config.js`**, sans le plugin SvelteKit :
// rien ici ne monte de composant, donc rien n'a besoin de le compiler. Le jour
// où un test de composant serait justifié, il demanderait jsdom et
// `@testing-library/svelte` — c'est une autre décision, à reprendre à ce
// moment-là et pas par inadvertance.
export default defineConfig({
  resolve: {
    alias: { $lib: fileURLToPath(new URL("./src/lib", import.meta.url)) },
  },
  test: {
    include: ["src/**/*.test.ts"],
    environment: "node",
  },
});
