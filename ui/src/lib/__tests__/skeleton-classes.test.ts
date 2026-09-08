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

/**
 * The contents of every `class` attribute in a file.
 *
 * Scoped deliberately: `preset-` also prefixes test ids and appears in prose,
 * and matching those made this check fire on things that were never CSS.
 */
function classValues(text: string): string[] {
  return [...text.matchAll(/\bclass=(?:"([^"]*)"|\{([^}]*)\})/g)].map(
    (m) => m[1] ?? m[2] ?? '',
  );
}

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
      for (const value of classValues(readFileSync(file, 'utf8'))) {
        for (const m of value.matchAll(/\bpreset-[a-z0-9-]+/g)) {
          // Skip fragments left by interpolation, caught by the next test.
          if (!m[0].endsWith('-')) used.add(m[0]);
        }
      }
    }
    expect(used.size).toBeGreaterThan(0);

    const missing = [...used].filter((c) => !defined.has(c)).sort();
    expect(missing, 'these classes produce no CSS and render unstyled').toEqual([]);
  });

  it('no preset class is assembled by interpolation', () => {
    // Tailwind scans source text, so a preset class whose colour is spliced in
    // at runtime is never generated and the element renders unstyled.
    for (const file of sourceFiles('src')) {
      for (const value of classValues(readFileSync(file, 'utf8'))) {
        expect(value, `${file} builds a preset class at runtime`).not.toMatch(
          /preset-[a-z0-9-]*-\s*\{/,
        );
      }
    }
  });

  // A guard is only worth having if it still fires. These pin the two
  // detectors against synthetic input, so narrowing the scan to `class`
  // attributes cannot quietly turn the checks into no-ops.
  it('the scan sees class attributes and ignores test ids', () => {
    const svelte = `
      <div data-testid="preset-error" class="badge preset-tonal-success"></div>
      <p class={TONAL[tone]}>hi</p>
    `;
    const found = classValues(svelte).flatMap((v) => [
      ...v.matchAll(/\bpreset-[a-z0-9-]+/g),
    ].map((m) => m[0]));
    expect(found).toEqual(['preset-tonal-success']);
  });

  it('the interpolation detector still fires on a spliced class', () => {
    const bad = '<p class="preset-tonal-{tone} p-2"></p>';
    const values = classValues(bad);
    expect(values.some((v) => /preset-[a-z0-9-]*-\s*\{/.test(v))).toBe(true);
  });
});
