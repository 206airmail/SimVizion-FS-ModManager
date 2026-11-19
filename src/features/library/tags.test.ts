import { describe, expect, it } from 'vitest';
import {
  addTags,
  hasTag,
  MAX_TAG_LENGTH,
  MAX_TAGS,
  parseTagInput,
  removeTag,
  suggestTags,
  tagCounts,
  tagProblem,
} from './tags';

const mod = (...tags: string[]) => ({ tags });

describe('tagCounts', () => {
  it('counts each tag once per mod, case-insensitively, and sorts them', () => {
    const counts = tagCounts([mod('Zulu', 'alpha'), mod('ALPHA'), mod('Mike', 'mike')]);
    expect(counts).toEqual([
      { tag: 'alpha', count: 2 },
      { tag: 'Mike', count: 1 },
      { tag: 'Zulu', count: 1 },
    ]);
  });

  it('is empty when no mod has a tag', () => {
    expect(tagCounts([mod(), mod()])).toEqual([]);
    expect(tagCounts([])).toEqual([]);
  });
});

describe('parseTagInput', () => {
  it('splits at commas and line breaks, trims and drops empty parts', () => {
    expect(parseTagInput(' one, two ,,three\nfour ')).toEqual(['one', 'two', 'three', 'four']);
    expect(parseTagInput('   ')).toEqual([]);
    expect(parseTagInput('single')).toEqual(['single']);
  });
});

describe('addTags and removeTag', () => {
  it('adds a tag once, keeps an existing spelling and sorts', () => {
    expect(addTags(['beta'], ['Alpha'])).toEqual(['Alpha', 'beta']);
    expect(addTags(['Beta'], ['beta', 'BETA'])).toEqual(['Beta']);
  });

  it('removes a tag without regard to case, and leaves the rest', () => {
    expect(removeTag(['Alpha', 'beta'], 'ALPHA')).toEqual(['beta']);
    expect(removeTag(['Alpha'], 'gamma')).toEqual(['Alpha']);
  });

  it('finds a tag without regard to case', () => {
    expect(hasTag(['Alpha'], 'aLPHA')).toBe(true);
    expect(hasTag(['Alpha'], 'beta')).toBe(false);
  });
});

describe('tagProblem', () => {
  it('refuses a tag over the length limit, counting characters', () => {
    expect(tagProblem([mod()], 'x'.repeat(MAX_TAG_LENGTH))).toBeNull();
    expect(tagProblem([mod()], 'x'.repeat(MAX_TAG_LENGTH + 1))).toBe('tooLong');
    expect(tagProblem([mod()], '😀'.repeat(MAX_TAG_LENGTH))).toBeNull();
  });

  it('refuses a new tag for a mod that already has the most, but not one it has', () => {
    const full = mod(...Array.from({ length: MAX_TAGS }, (_, i) => `tag${i}`));
    expect(tagProblem([full], 'another')).toBe('tooMany');
    expect(tagProblem([full], 'TAG3')).toBeNull();
    expect(tagProblem([mod('a'), full], 'another')).toBe('tooMany');
    expect(tagProblem([mod('a')], 'another')).toBeNull();
  });
});

describe('suggestTags', () => {
  const known = ['Favourite', 'Keep', 'Multiplayer', 'Realistic', 'Testing'];

  it('offers every known tag that is not on every mod, narrowed by the text', () => {
    const applied = [{ tag: 'keep', count: 1 }];
    expect(suggestTags(known, applied, 1, '')).toEqual([
      'Favourite',
      'Multiplayer',
      'Realistic',
      'Testing',
    ]);
    expect(suggestTags(known, applied, 1, 'te')).toEqual(['Testing', 'Favourite']);
  });

  it('puts tags that start with the text first', () => {
    expect(suggestTags(['Beach', 'Ache', 'Chase'], [], 1, 'ch')).toEqual([
      'Chase',
      'Beach',
      'Ache',
    ]);
  });

  it('still offers a tag some of the selected mods have, but not one all have', () => {
    const applied = [
      { tag: 'Keep', count: 2 },
      { tag: 'Testing', count: 3 },
    ];
    const offered = suggestTags(known, applied, 3, '');
    expect(offered).toContain('Keep');
    expect(offered).not.toContain('Testing');
  });

  it('limits the list', () => {
    const many = Array.from({ length: 20 }, (_, i) => `tag${i}`);
    expect(suggestTags(many, [], 1, '')).toHaveLength(6);
    expect(suggestTags(many, [], 1, '', 3)).toHaveLength(3);
  });
});
