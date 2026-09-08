import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

/**
 * Skeleton's presets are Tailwind `@utility` rules, so a name that does not
 * exist produces no CSS and no error — the control simply renders unstyled, and
 * only a human looking at the page notices. Two of these shipped before anyone
 * did.
 *
 * This walks the source for `preset-*` classes and checks each against the
 * names Skeleton actually defines.
 */

const PRESETS_CSS = 'node_modules/@skeletonlabs/skeleton/src/utilities/presets.css';

function sourceFiles(dir: string, acc: string[] = []): string[] {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) sourceFiles(path, acc);
    else if (/\.(svelte|ts)$/.test(entry) && !entry.endsWith('.test.ts')) acc.push(path);
  }
  return acc;
}

describe('Skeleton class names', () => {
  it('every preset- class used in the app exists in Skeleton', () => {
    const defined = new Set(
      [...readFileSync(PRESETS_CSS, 'utf8').matchAll(/@utility (preset-[a-z0-9-]+)/g)].map(
        (m) => m[1]!,
      ),
    );
    expect(defined.size).toBeGreaterThan(10);

    const used = new Set<string>();
    for (const file of sourceFiles('src')) {
      for (const m of readFileSync(file, 'utf8').matchAll(/\bpreset-[a-z0-9-]+/g)) {
        // Skip fragments left by interpolation, which are caught separately.
        if (!m[0].endsWith('-')) used.add(m[0]);
      }
    }
    expect(used.size).toBeGreaterThan(0);

    const missing = [...used].filter((c) => !defined.has(c)).sort();
    expect(missing, 'these classes produce no CSS and render unstyled').toEqual([]);
  });

  it('no preset class is assembled by interpolation', () => {
    // Tailwind scans source text, so `preset-tonal-{tone}` is never generated.
    for (const file of sourceFiles('src')) {
      const text = readFileSync(file, 'utf8');
      expect(text, `${file} builds a preset class at runtime`).not.toMatch(
        /preset-[a-z0-9-]*[-]?\{/,
      );
    }
  });
});
